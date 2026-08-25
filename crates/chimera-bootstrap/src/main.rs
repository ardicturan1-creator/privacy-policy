//! CHIMERA / MONOLITH — uc kisilikli tek binary.
//!
//!   chimera install [--root DIR] [--silent]   `.mono` materyalize + plan
//!   chimera supervise [--root DIR]            preflight + watchdog
//!   chimera worker --plan P --root R          motor (supervisor tarafindan)
//!   chimera probe                             yalnizca donanim raporu
//!
//! Ayni dosyanin installer, supervisor ve worker olmasi bir numara degil,
//! bir kisittir: kullanicinin makinesinde ikinci bir calistirilabilir yoktur,
//! dolayisiyla imzalanacak, guncellenecek ve dogrulanacak tek bir yuzey vardir.

// FFI baglantisi (`TODO(ffi)`) eklenene kadar bazi ogeler cagrilmaz; iskeletin
// tamamini gormek, yarim gostermekten daha faydali oldugu icin bilincli tercih.
#![allow(dead_code)]

mod boot;
mod hw;
mod merkle;
mod obsidian;
mod planner;

use boot::{Layout, Mode, Watchdog};
use planner::{Canary, ModelGeometry, NullCanary, Plan};

const DEFAULT_ROOT: &str = "/opt/chimera";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("probe");
    let root = flag(&args, "--root").unwrap_or_else(|| DEFAULT_ROOT.to_string());
    let silent = args.iter().any(|a| a == "--silent");

    let code = match cmd {
        "probe" => cmd_probe(),
        "install" => cmd_install(&root, silent, flag(&args, "--password")),
        "supervise" => cmd_supervise(&root),
        "worker" => cmd_worker(&root),
        "verify" => cmd_verify(&root, args.iter().any(|a| a == "--repair")),
        "obsidian-demo" => cmd_obsidian_demo(),
        "corrupt-test" => cmd_corrupt_test(&root),
        other => {
            eprintln!("bilinmeyen alt komut: {other}");
            eprintln!("kullanim: chimera <probe|install|supervise|worker|verify|obsidian-demo|corrupt-test>");
            2
        }
    };
    std::process::exit(code);
}

// ---------------------------------------------------------------------------

fn cmd_probe() -> i32 {
    let host = hw::detect();
    println!("CHIMERA donanim raporu");
    println!("  P-core     : {}", host.cpu.perf_cores);
    println!("  Mantiksal  : {}", host.cpu.logical);
    println!("  NUMA       : {}", host.cpu.numa_nodes);
    println!("  RAM (avail): {}", hw::human(host.ram_available));
    if host.gpus.is_empty() {
        println!("  GPU        : bulunamadi (CPU planina dusulecek)");
    }
    for g in &host.gpus {
        println!(
            "  GPU        : {} [{}] butce {} / toplam {}{}",
            g.name,
            g.backend.as_str(),
            hw::human(g.vram_budget),
            hw::human(g.vram_total),
            if g.unified { " (birlesik bellek)" } else { "" }
        );
    }
    0
}

fn cmd_install(root: &str, silent: bool, password: Option<String>) -> i32 {
    let layout = Layout::new(root);
    let host = hw::detect();

    for dir in [
        layout.runtime(),
        layout.active_slot(),
        layout.root.join("restore"),
        layout.state(),
        layout.root.join("logs"),
        layout.quarantine(),
    ] {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("dizin olusturulamadi ({}): {e}", dir.display());
            return 1;
        }
    }

    let canary = build_canary(&host);
    let mut plans: Vec<Plan> = Vec::new();

    for geom in swarm_models() {
        let p = planner::plan(&host, &geom, canary.as_ref());
        if !silent {
            println!("\n[{}]", geom.name);
            print!("{}", planner::render_summary(&host, &p));
        }
        plans.push(p);
    }

    // TODO(ffi): ASIL NUMARA — yerinde, delik-acmali yeniden kuantizasyon.
    //
    //   for tensor in gguf_tensors(kaynak):
    //       hedef.write(requantize(tensor, plan.quant))
    //       fallocate(kaynak, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE,
    //                 tensor.offset, tensor.len)
    //
    // Blok bazinda alan iadesi sayesinde 60 GB -> 18 GB donusumu 78 GB degil
    // ~61 GB bos alanla yapilir. Her 256 MiB'da bir `fdatasync` ile checkpoint
    // alinir; elektrik kesilirse watchdog kalinan tensor indeksinden DEVAM
    // eder, bastan baslamaz. Destek: ext4 / XFS / Btrfs / ZFS / NTFS.
    //
    // Bu ortamda gercek, cok-GB'lik bir GGUF dosyasi YOK; bu yuzden bu adim
    // burada calistirilamaz/dogrulanamaz. Ama butunluk-koruma zinciri
    // (Merkle + ML-DSA-87 imza + parcali onarim) asagida GERCEK dosyalar
    // uzerinde uygulanir — mekanizmanin kendisi taklit degildir.

    let plan_path = layout.runtime().join("plan.json");
    let doc: String = plans.iter().map(planner::plan_to_json).collect::<Vec<_>>().join(",\n");
    let plan_bytes = format!("[\n{doc}\n]\n").into_bytes();
    if let Err(e) = std::fs::write(&plan_path, &plan_bytes) {
        eprintln!("plan yazilamadi: {e}");
        return 1;
    }

    // --- GERCEK butunluk zinciri: engine -> golden -> Merkle -> ML-DSA-87 imza ---
    if let Err(e) = std::fs::write(layout.active_engine(), &plan_bytes) {
        eprintln!("engine.mono yazilamadi: {e}");
        return 1;
    }
    if let Err(e) = std::fs::write(layout.golden(), &plan_bytes) {
        eprintln!("golden.mono yazilamadi: {e}");
        return 1;
    }
    let leaf_size = 4096u64;
    let tree = match merkle::build_tree_from_file(&layout.golden(), leaf_size as usize) {
        Ok(t) => t,
        Err(e) => { eprintln!("merkle agaci kurulamadi: {e}"); return 1; }
    };
    let dsa_kp = obsidian::dsa_generate_keypair();
    let sig = obsidian::dsa_sign(&dsa_kp.signing_key, &tree.root);
    if let Err(e) = boot::write_manifest(
        &layout.manifest_sig(),
        &obsidian::dsa_verifying_key_bytes(&dsa_kp.verifying_key),
        &tree.root,
        leaf_size,
        &obsidian::dsa_signature_bytes(&sig),
    ) {
        eprintln!("MANIFEST.sig yazilamadi: {e}");
        return 1;
    }

    // --- GERCEK Obsidian master anahtar: uret, parola ile muhurle, Shamir(2,3) boler ---
    let master = match obsidian::generate_master_key() {
        Ok(k) => k,
        Err(e) => { eprintln!("master anahtar uretilemedi: {e}"); return 1; }
    };
    let (pw, generated) = match password {
        Some(p) => (p, false),
        None => (random_passphrase(), true),
    };
    let sealed = match obsidian::seal_master_key(pw.as_bytes(), &master) {
        Ok(s) => s,
        Err(e) => { eprintln!("master anahtar muhurlenemedi: {e}"); return 1; }
    };
    let mut seal_bytes = Vec::with_capacity(16 + 24 + sealed.ciphertext.len());
    seal_bytes.extend_from_slice(&sealed.salt);
    seal_bytes.extend_from_slice(&sealed.nonce);
    seal_bytes.extend_from_slice(&sealed.ciphertext);
    if let Err(e) = std::fs::write(layout.epoch_seal(), &seal_bytes) {
        eprintln!("epoch.seal yazilamadi: {e}");
        return 1;
    }

    // Shamir(2,3): mimari dokumandaki Share A/B/C. Ucu de burada tek dizine
    // yazilmasi YALNIZCA bu demo/kurulum komutu icindir — GERCEK bir
    // dagitimda Share A TPM'e, Share C offline bir zarfa gider ve ASLA
    // ayni diskte durmaz. Bu, mimari belgesinde zaten boyle tanimliydi;
    // burada sadece dogru sekilde vurgulanmis oluyor.
    let shares = obsidian::split_master_key(&master);
    for (i, share) in shares.iter().enumerate() {
        let bytes: Vec<u8> = share.into();
        let _ = std::fs::write(layout.state().join(format!("shamir_share_{i}_DEMO_ONLY.bin")), bytes);
    }

    if !silent {
        println!("\n--- OBSIDIAN ---");
        println!("  ML-DSA-87 dogrulama anahtari : {} bayt", obsidian::dsa_verifying_key_bytes(&dsa_kp.verifying_key).len());
        println!("  Merkle koku (golden)         : {}", hex(&tree.root));
        println!("  MANIFEST.sig                 -> {}", layout.manifest_sig().display());
        println!("  epoch.seal (master anahtar)  -> {}", layout.epoch_seal().display());
        if generated {
            println!("  KAYIT EDIN — kurtarma parolasi (bir daha gosterilmeyecek): {pw}");
        }
        println!("\nplan  -> {}", plan_path.display());
        println!("sonraki adim: chimera verify --root {root}   (butunluk zincirini dogrula)");
        println!("           veya: chimera supervise --root {root}");
    }
    0
}

fn cmd_verify(root: &str, repair: bool) -> i32 {
    let layout = Layout::new(root);
    match boot::verify_integrity(&layout) {
        Ok(boot::Integrity::Ok) => {
            println!("butunluk: OK — golden ve aktif dosya, imzali Merkle kokuyle eslesiyor");
            0
        }
        Ok(boot::Integrity::Corrupt(leaves)) => {
            println!("butunluk: BOZUK — {} yaprak golden ile uyusmuyor: {leaves:?}", leaves.len());
            if repair {
                match boot::repair_leaves(&layout, &leaves) {
                    Ok(n) => {
                        println!("onarim: {n} yaprak golden'dan geri yazildi");
                        match boot::verify_integrity(&layout) {
                            Ok(boot::Integrity::Ok) => { println!("onarim sonrasi: OK"); 0 }
                            other => { println!("onarim sonrasi hala sorunlu: {other:?}"); 1 }
                        }
                    }
                    Err(e) => { eprintln!("onarim basarisiz: {e}"); 1 }
                }
            } else {
                println!("(onarmak icin --repair ekleyin)");
                1
            }
        }
        Ok(boot::Integrity::Tampered) => {
            println!("butunluk: KURCALANMIS — imza gecersiz veya golden bozuk. Onarim DENENMEDI.");
            1
        }
        Err(e) => { eprintln!("dogrulama basarisiz: {e}"); 1 }
    }
}

/// Bilerek aktif dosyada bir bayt bozar — `chimera verify` ile tespit ve
/// (`--repair` ile) onarimin gercekten calistigini canli gostermek icin.
fn cmd_corrupt_test(root: &str) -> i32 {
    let layout = Layout::new(root);
    let path = layout.active_engine();
    let mut data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => { eprintln!("{} okunamadi: {e} (once 'chimera install' calistirin)", path.display()); return 1; }
    };
    if data.is_empty() {
        eprintln!("dosya bos, bozulacak bayt yok");
        return 1;
    }
    let idx = data.len() / 3;
    data[idx] ^= 0xFF;
    if let Err(e) = std::fs::write(&path, &data) {
        eprintln!("yazilamadi: {e}");
        return 1;
    }
    println!("kasten bozuldu: {} (offset {idx})", path.display());
    println!("simdi calistirin: chimera verify --root {root} --repair");
    0
}

/// OBSIDIAN'in tum kriptografik hattini, diske DOKUNMADAN, saf bellek
/// icinde ve GERCEK sayilarla gosterir: ML-KEM-1024 kapsulleme, ML-DSA-87
/// imza, XChaCha20-Poly1305 muhur, Shamir(2,3) ve ortogonal vektor donusumu.
fn cmd_obsidian_demo() -> i32 {
    println!("=== OBSIDIAN canli demo (hepsi gercek, hicbiri simule degil) ===\n");

    let master = obsidian::generate_master_key().expect("os rng");
    println!("[1] Master anahtar uretildi: {} bayt", master.len());

    let pw = b"demo-parola-ornek";
    let sealed = obsidian::seal_master_key(pw, &master).expect("seal");
    let recovered = obsidian::unseal_master_key(pw, &sealed).expect("unseal");
    assert_eq!(master, recovered);
    println!("[2] Argon2id + XChaCha20-Poly1305 muhur/ac dogrulandi ({} bayt ciphertext)", sealed.ciphertext.len());

    let shares = obsidian::split_master_key(&master);
    let recovered2 = obsidian::recover_master_key(&[shares[0].clone(), shares[2].clone()]).expect("recover");
    assert_eq!(master, recovered2);
    println!("[3] Shamir(2,3): {} parca uretildi, (A,C) ciftinden GERCEKTEN kurtarildi", shares.len());

    let kem = obsidian::kem_generate_keypair();
    let (ct, ss_a) = obsidian::kem_encapsulate(&kem.encapsulation_key);
    let ss_b = obsidian::kem_decapsulate(&kem.decapsulation_key, &ct);
    assert_eq!(ss_a, ss_b);
    println!("[4] ML-KEM-1024: ciphertext {} bayt, paylasilan sir {} bayt, iki taraf eslesti", ct.len(), ss_a.len());

    let dsa = obsidian::dsa_generate_keypair();
    let msg = b"chimera epoch root, canli demo";
    let sig = obsidian::dsa_sign(&dsa.signing_key, msg);
    assert!(obsidian::dsa_verify(&dsa.verifying_key, msg, &sig).is_ok());
    println!(
        "[5] ML-DSA-87: dogrulama anahtari {} bayt, imza {} bayt, dogrulama GECTI",
        obsidian::dsa_verifying_key_bytes(&dsa.verifying_key).len(),
        obsidian::dsa_signature_bytes(&sig).len()
    );

    let dim = 16;
    let q = obsidian::orthogonal_matrix(dim, 0xC0FFEE);
    let a: Vec<f64> = (0..dim).map(|i| (i as f64 + 1.0).sin()).collect();
    let b: Vec<f64> = (0..dim).map(|i| (i as f64 + 1.0).cos()).collect();
    let sim_before = obsidian::cosine_similarity(&a, &b);
    let ra = obsidian::rotate(&q, &a);
    let rb = obsidian::rotate(&q, &b);
    let sim_after = obsidian::cosine_similarity(&ra, &rb);
    println!(
        "[6] Ortogonal vektor donusumu: kosinus benzerligi once {sim_before:.10}, sonra {sim_after:.10} (fark {:.2e})",
        (sim_before - sim_after).abs()
    );

    println!("\nTum adimlar gercekten calisti ve dogrulandi.");
    0
}

fn random_passphrase() -> String {
    let mut buf = [0u8; 20];
    getrandom::fill(&mut buf).expect("os rng");
    hex(&buf)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn cmd_supervise(root: &str) -> i32 {
    let layout = Layout::new(root);
    let mode = match boot::preflight(&layout) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("preflight basarisiz: {e}");
            return 1;
        }
    };
    if mode == Mode::DegradedSafe {
        eprintln!("UYARI: degraded safe mode — otonom yama KAPALI, yalnizca tespit");
    }

    let plan_path = layout.runtime().join("plan.json");
    let mut wd = Watchdog::new(Layout::new(root), mode);

    loop {
        let started = std::time::Instant::now();
        let mut child = match wd.spawn_worker(&plan_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("worker baslatilamadi: {e}");
                return 1;
            }
        };

        let status = child.wait();
        let uptime = started.elapsed();

        // Uzun sure ayakta kaldiysa backoff sifirlanir: tek seferlik bir
        // cokme, kalici bir arizanin kaniti degildir.
        if uptime > std::time::Duration::from_secs(60) {
            wd.reset_backoff();
        }

        match status {
            Ok(s) if s.success() => {
                let _ = boot::audit(&layout, "worker.exit", "temiz cikis");
                return 0;
            }
            Ok(s) => {
                let _ = boot::audit(&layout, "worker.crash", &format!("kod {s:?}"));
            }
            Err(e) => {
                let _ = boot::audit(&layout, "worker.wait_error", &e.to_string());
            }
        }

        let new_mode = wd.record_crash(std::time::Instant::now());
        if new_mode == Mode::DegradedSafe {
            let _ = boot::audit(&layout, "mode.degraded", "cokme dongusu tespit edildi");
        }
        // Sicak yeniden baslatma: agirliklar hala page cache'te, ~400 ms.
        std::thread::sleep(wd.next_backoff(uptime.as_millis() as u64));
    }
}

fn cmd_worker(root: &str) -> i32 {
    // TODO(ffi): plan.json'i oku -> llama.cpp baglamini kur
    //   llama_model_params { n_gpu_layers, use_mmap: true, use_mlock: false }
    //   -> .mono blob bolgesinden DOGRUDAN mmap (RAM'e kopya YOK)
    //   usearch HNSW indeksini ac, redb'yi ac
    //   supervisor'a unix soket uzerinden heartbeat + ilerleme sayaci gonder
    let _ = root;
    eprintln!("worker: motor baglantisi bu taslakta uygulanmadi (TODO(ffi))");
    0
}

// ---------------------------------------------------------------------------

/// Sürünün üç ajani + guard. Geometriler uretimde GGUF metadata'sindan
/// okunur; burada MVP icin sabittir.
fn swarm_models() -> Vec<ModelGeometry> {
    vec![
        ModelGeometry {
            name: "sentinel-8b".into(),
            params: 8_000_000_000,
            n_layer: 32,
            n_kv_head: 8,
            head_dim: 128,
            kv_bytes: 2,
        },
        ModelGeometry {
            name: "artificer-14b".into(),
            params: 14_000_000_000,
            n_layer: 48,
            n_kv_head: 8,
            head_dim: 128,
            kv_bytes: 2,
        },
        ModelGeometry {
            name: "adjudicator-14b".into(),
            params: 14_000_000_000,
            n_layer: 48,
            n_kv_head: 8,
            head_dim: 128,
            kv_bytes: 2,
        },
        ModelGeometry {
            // Guard modeli her zaman yuksek hassasiyette calisir: politika
            // kararinda kuantizasyon gurultusu kabul edilemez.
            name: "warden-3b".into(),
            params: 3_000_000_000,
            n_layer: 26,
            n_kv_head: 4,
            head_dim: 128,
            kv_bytes: 1,
        },
    ]
}

fn build_canary(host: &hw::HostProfile) -> Box<dyn Canary> {
    match host.backend() {
        hw::Backend::Cpu => Box::new(NullCanary),
        // TODO(ffi): CudaCanary  -> cuMemAlloc / cuMemFree
        //            MetalCanary -> newBufferWithLength:options:
        //            VulkanCanary-> vkAllocateMemory / vkFreeMemory
        // Ikinci tahsis (largest_tensor) fragmentasyon testidir:
        // "toplam yer var ama bitisik yer yok" durumunu yakalar.
        _ => Box::new(NullCanary),
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}
