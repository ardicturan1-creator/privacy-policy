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
        "install" => cmd_install(&root, silent),
        "supervise" => cmd_supervise(&root),
        "worker" => cmd_worker(&root),
        other => {
            eprintln!("bilinmeyen alt komut: {other}");
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

fn cmd_install(root: &str, silent: bool) -> i32 {
    let layout = Layout::new(root);
    let host = hw::detect();

    if let Err(e) = std::fs::create_dir_all(layout.runtime()) {
        eprintln!("runtime dizini olusturulamadi: {e}");
        return 1;
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
    // eder, bastan baslamaz.
    // Destek: ext4 / XFS / Btrfs / ZFS / NTFS(FSCTL_SET_ZERO_DATA).

    let plan_path = layout.runtime().join("plan.json");
    let doc: String = plans.iter().map(planner::plan_to_json).collect::<Vec<_>>().join(",\n");
    if let Err(e) = std::fs::write(&plan_path, format!("[\n{doc}\n]\n")) {
        eprintln!("plan yazilamadi: {e}");
        return 1;
    }
    if !silent {
        println!("\nplan  -> {}", plan_path.display());
        println!("sonraki adim: chimera supervise --root {root}");
    }
    0
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
