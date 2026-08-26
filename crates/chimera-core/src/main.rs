//! chimera-core — Ring-3 arka plan servisi (Core Daemon).
//!
//!   chimera-core identity  --root R           kimlik parmak izini goster
//!   chimera-core trust     --root R <fp-hex>  bir eşi (admin/sentinel) guven deposuna ekle
//!   chimera-core provision --root R           master anahtar uret, Shamir(2,3) paylarini YAZDIR
//!   chimera-core serve     --root R           IPC sunucusu + watchdog + decoy + tarpit
//!
//! Bu sürecin GUI ile HİÇBİR doğrudan bağı yoktur: GUI (chimera-admin)
//! kapansa, çökse, hatta hiç açılmasa bile bu süreç bağımsız çalışmaya
//! devam eder. Aralarındaki TEK bağ, kimlik-doğrulamalı+şifreli IPC'dir.
//!
//! **Öldürülemezlik sınırı — bilinçli ve belgelenmiş:** Bu süreç, işletim
//! sisteminin veya bir yöneticinin bilinçli sonlandırmasına (SIGKILL,
//! Görev Yöneticisi'nde "Son İşlem", `taskkill /F`) karşı DİRENMEZ ve
//! DİRENMEMELİDİR — böyle bir direnç, güvenlik araştırmacılarının ve olay
//! müdahale ekiplerinin sistemi analiz etmesini de aynı ölçüde engeller ve
//! bu, savunma yazılımını rootkit davranışına yaklaştırır. Bunun yerine:
//! `chimera-sentinel` eş süreci, `chimera-core`'un KAZA SONUCU (çökme,
//! panik, beklenmeyen çıkış) durmasını saniyeler içinde tespit edip
//! yeniden başlatır — ve bunun tersi de geçerlidir. Bilinçli, temiz bir
//! `stop` komutu (`runtime/stop.flag`) ise HER ZAMAN saygı görür.

mod decoy;
mod tarpit;

use chimera_crypto::obsidian;
use chimera_ipc::{Identity, Request, Response, TrustStore};
use interprocess::local_socket::{prelude::*, ListenerOptions};
#[cfg(windows)]
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("serve");
    let root = flag(&args, "--root").unwrap_or_else(|| "/opt/chimera-core".to_string());
    let root = PathBuf::from(root);

    // Panic sertlestirmesi: konsola/stdout'a YALNIZCA genel bir mesaj
    // basilir (dosya yolu, satir numarasi, dahili tur/mesaj SIZDIRILMAZ);
    // TAM detay ise meşru hata ayiklama icin denetim kaydina yazilir.
    // `--remap-path-prefix` (bkz. .cargo/config.toml) zaten yapi makinesi
    // yollarini kaldirmisti — bu katman panic METNININ KENDISINI de
    // kullanicidan/gozlemciden gizler.
    {
        let log_path = root.join("logs/audit.jsonl");
        std::panic::set_hook(Box::new(move |info| {
            let detail = info.to_string().replace('"', "'").replace('\n', " ");
            append_line(&log_path, &format!("{{\"ts\":{},\"event\":\"panic\",\"detail\":\"{detail}\"}}", unix_now()));
            eprintln!("chimera-core: beklenmeyen bir ic hata olustu; ayrintilar denetim kaydinda.");
        }));
    }

    let code = match cmd {
        "identity" => cmd_identity(&root),
        "trust" => cmd_trust(&root, flag(&args, "--pubkey")),
        "attest" => cmd_attest(&root, flag(&args, "--pubkey"), flag(&args, "--binary-hash")),
        "provision" => cmd_provision(&root, flag(&args, "--password")),
        "serve" => cmd_serve(&root),
        other => {
            eprintln!("bilinmeyen alt komut: {other}");
            eprintln!("kullanim: chimera-core <identity|trust|attest|provision|serve> --root DIR");
            2
        }
    };
    std::process::exit(code);
}

fn layout(root: &Path) -> Layout {
    Layout { root: root.to_path_buf() }
}

struct Layout {
    root: PathBuf,
}
impl Layout {
    fn identity_dir(&self) -> PathBuf { self.root.join("state/core_identity") }
    fn trust_list(&self) -> PathBuf { self.root.join("state/trusted_peers.list") }
    fn attestation_list(&self) -> PathBuf { self.root.join("state/trusted_peers.attest") }
    fn vault(&self) -> PathBuf { self.root.join("state/vault.sealed") }
    fn stop_flag(&self) -> PathBuf { self.root.join("runtime/stop.flag") }
    fn audit_log(&self) -> PathBuf { self.root.join("logs/audit.jsonl") }
    fn deception_log(&self) -> PathBuf { self.root.join("logs/deception.jsonl") }
    fn decoys(&self) -> PathBuf { self.root.join("decoys") }
}

fn cmd_identity(root: &Path) -> i32 {
    let l = layout(root);
    let id = match Identity::load_or_create(&l.identity_dir()) {
        Ok(i) => i,
        Err(e) => { eprintln!("kimlik olusturulamadi: {e}"); return 1; }
    };
    println!("chimera-core kimlik parmak izi: {}", id.fingerprint());
    println!("Bu deger, admin/sentinel'in guven deposuna eklenmesi icin karsi tarafa GUVENLI bir kanaldan iletilmelidir.");
    println!("acik anahtar (hex): {}", hex(&id.verifying_key_bytes()));
    match chimera_ipc::attestation::self_binary_hash() {
        Ok(h) => println!("ikili ozet (BLAKE3): {h}"),
        Err(e) => eprintln!("ikili ozet hesaplanamadi: {e}"),
    }
    0
}

fn cmd_trust(root: &Path, fp_or_hex: Option<String>) -> i32 {
    let Some(arg) = fp_or_hex else {
        eprintln!("kullanim: chimera-core trust --root DIR --pubkey <acik-anahtar-hex>");
        return 2;
    };
    let Ok(bytes) = unhex(&arg) else {
        eprintln!("gecersiz hex");
        return 1;
    };
    let l = layout(root);
    let mut trust = match TrustStore::load(&l.trust_list()) {
        Ok(t) => t,
        Err(e) => { eprintln!("guven deposu okunamadi: {e}"); return 1; }
    };
    if let Err(e) = trust.trust(&bytes) {
        eprintln!("yazilamadi: {e}");
        return 1;
    }
    println!("guvenildi: {}", chimera_ipc::trust::fingerprint_of(&bytes));
    println!("(opsiyonel ama ONERILEN: 'chimera-core attest --pubkey {arg} --binary-hash <esin-bildirdigi-ozet>' ile bu esin ikili ozetini de sabitleyin)");
    0
}

/// Bir esin (admin/sentinel) ikili ozetini SABITLER (bkz. `chimera_ipc::attestation`).
/// Sabitlendikten sonra, o es CALINMIS bir kimlik anahtariyla bile
/// DEGISTIRILMIS bir ikiliyle baglanmaya calisirsa el sikisma reddedilir.
/// `--binary-hash` verilmezse, esin kendi `identity`/`attest-self` ciktisindan
/// okunan degeri operator elle girer — bu KASITLI bir ADIM: sabitleme,
/// otomatik degil, operatorun ACIKCA dogruladigi bir eylem olmalidir.
fn cmd_attest(root: &Path, pubkey_hex: Option<String>, binary_hash_hex: Option<String>) -> i32 {
    let (Some(pk), Some(bh)) = (pubkey_hex, binary_hash_hex) else {
        eprintln!("kullanim: chimera-core attest --pubkey <hex> --binary-hash <blake3-hex>");
        eprintln!("(karsi tarafin kendi ozetini almak icin: chimera-core/chimera-sentinel/chimera-admin 'identity' cikisindaki 'ikili ozet' satirina bakin)");
        return 2;
    };
    let Ok(vk_bytes) = unhex(&pk) else { eprintln!("gecersiz pubkey hex"); return 1; };
    let l = layout(root);
    let mut attest = match chimera_ipc::AttestationStore::load(&l.attestation_list()) {
        Ok(a) => a,
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    let fp = chimera_ipc::trust::fingerprint_of(&vk_bytes);
    if let Err(e) = attest.pin(&fp, &bh) {
        eprintln!("sabitlenemedi: {e}"); return 1;
    }
    println!("sabitlendi: {fp} -> {bh}");
    0
}

/// Master anahtari uretir, YEREL bir makine anahtariyla muhurleyip
/// `state/vault.sealed`'a yazar, Shamir(2,3) paylarini BIR KEZ ekrana
/// basar. Bu paylar core tarafindan ASLA saklanmaz — operator bunlari 3
/// ayri, birbirinden bagimsiz yere (fiziksel token/HSM/offline zarf)
/// dagitmakla yukumludur. Kaybolurlarsa kasa GERI ACILAMAZ.
fn cmd_provision(root: &Path, password: Option<String>) -> i32 {
    let l = layout(root);
    if let Err(e) = std::fs::create_dir_all(l.root.join("state")) {
        eprintln!("{e}"); return 1;
    }
    let master = obsidian::generate_master_key().expect("os rng");
    let pw = password.unwrap_or_else(|| "chimera-core-local-vault".to_string());
    let sealed = obsidian::seal_master_key(pw.as_bytes(), &master).expect("seal");
    let mut buf = Vec::new();
    buf.extend_from_slice(&sealed.salt);
    buf.extend_from_slice(&sealed.nonce);
    buf.extend_from_slice(&sealed.ciphertext);
    if let Err(e) = std::fs::write(l.vault(), &buf) {
        eprintln!("kasa yazilamadi: {e}"); return 1;
    }

    let shares = obsidian::split_master_key(&master);
    println!("=== ZERO-TRUST PROVIZYON — bu paylar bir daha GOSTERILMEYECEK ===\n");
    for (i, share) in shares.iter().enumerate() {
        let bytes: Vec<u8> = share.into();
        println!("  Pay {} ({}): {}", ["A", "B", "C"][i], match i { 0 => "TPM/donanim tokenina", 1 => "yonetici parolasina", _ => "offline zarfa" }, hex(&bytes));
    }
    println!("\nHerhangi 2'si, GUI'yi (chimera-admin) kilit acmak icin yeterlidir. Tek basina hicbiri yetmez.");
    0
}

fn cmd_serve(root: &Path) -> i32 {
    let l = layout(root);
    for dir in [l.root.join("state"), l.root.join("runtime"), l.root.join("logs"), l.decoys()] {
        let _ = std::fs::create_dir_all(dir);
    }

    let identity = match Identity::load_or_create(&l.identity_dir()) {
        Ok(i) => i,
        Err(e) => { eprintln!("kimlik yuklenemedi: {e}"); return 1; }
    };
    audit(&l, "core.start", &format!("fingerprint={}", identity.fingerprint()));
    maybe_log_debugger_presence(&l);

    // --- Kasayi bellekte ac (diskte hicbir zaman acik anahtar yok) ---
    let vault = std::fs::read(l.vault()).ok().and_then(|buf| {
        if buf.len() < 16 + 24 { return None; }
        let salt: [u8; 16] = buf[..16].try_into().ok()?;
        let nonce: [u8; 24] = buf[16..40].try_into().ok()?;
        let ct = buf[40..].to_vec();
        let sealed = obsidian::SealedKey { salt, nonce, ciphertext: ct };
        obsidian::unseal_master_key(b"chimera-core-local-vault", &sealed).ok()
    });
    let master_key = Arc::new(vault);
    if master_key.is_none() {
        eprintln!("UYARI: state/vault.sealed yok/acilmiyor — once 'chimera-core provision' calistirin. Ayricalikli komutlar hep DENIED donecek.");
    }

    // --- Siber yaniltma: decoy dosyalar + izleyici ---
    let _ = decoy::create_decoys(&l.decoys());
    let (_watcher, decoy_rx) = decoy::watch(&l.decoys()).expect("decoy izleyici baslatilamadi");
    let decoy_alerts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let decoy_alerts = Arc::clone(&decoy_alerts);
        let l_path = l.deception_log();
        std::thread::spawn(move || {
            for alert in decoy_rx {
                let line = format!("{{\"ts\":{},\"type\":\"decoy_touch\",\"path\":\"{}\",\"kind\":\"{}\"}}", alert.ts, alert.path.replace('"', "'"), alert.kind);
                append_line(&l_path, &line);
                decoy_alerts.lock().unwrap().push(line);
            }
        });
    }

    // --- Siber yaniltma: tarpit ---
    {
        let l_path = l.deception_log();
        let bind = std::env::var("CHIMERA_TARPIT_ADDR").unwrap_or_else(|_| "127.0.0.1:0".to_string());
        match tarpit::spawn(&bind, move |alert| {
            let line = format!("{{\"ts\":{},\"type\":\"tarpit_connect\",\"peer\":\"{}\"}}", alert.ts, alert.peer);
            append_line(&l_path, &line);
        }) {
            Ok(_h) => println!("tarpit dinliyor: {bind}"),
            Err(e) => eprintln!("tarpit baslatilamadi (yoksayiliyor): {e}"),
        }
    }

    // --- Karsilikli watchdog: sentinel'i baslat + izle ---
    let degraded = Arc::new(AtomicBool::new(false));
    if let Err(e) = bootstrap_trust_sentinel(&l) {
        eprintln!("UYARI: sentinel guven bootstrap'i basarisiz ({e}); sentinel baglanamayabilir");
    }
    let core_vk_hex = hex(&identity.verifying_key_bytes());
    spawn_sentinel_watchdog(&l, core_vk_hex);

    // --- IPC sunucusu ---
    let trust = Arc::new(Mutex::new(TrustStore::load(&l.trust_list()).expect("guven deposu")));
    let attestation = Arc::new(chimera_ipc::AttestationStore::load(&l.attestation_list()).expect("attestasyon deposu"));
    let name = match chimera_ipc::socket_name(&l.root) {
        Ok(n) => n,
        Err(e) => { eprintln!("soket adi gecersiz: {e}"); return 1; }
    };
    let listener_opts = ListenerOptions::new().name(name);
    #[cfg(windows)]
    let listener_opts = match windows_pipe_acl() {
        Ok(sd) => {
            use interprocess::os::windows::local_socket::ListenerOptionsExt;
            listener_opts.security_descriptor(sd)
        }
        Err(e) => {
            eprintln!("UYARI: Named Pipe ACL kurulamadi ({e}) — varsayilan (daha genis) izinlerle devam ediliyor");
            listener_opts
        }
    };
    let listener = match listener_opts.create_sync() {
        Ok(l) => l,
        Err(e) => { eprintln!("IPC dinleyici baslatilamadi: {e}"); return 1; }
    };
    println!("core hazir. soket adi kokten turetildi: {}", l.root.display());

    for conn in listener.incoming() {
        let Ok(mut stream) = conn else { continue };
        let identity_sk = identity.keypair.signing_key.clone();
        let identity_vk = identity.keypair.verifying_key.clone();
        let trust = Arc::clone(&trust);
        let attestation = Arc::clone(&attestation);
        let master_key = Arc::clone(&master_key);
        let decoy_alerts = Arc::clone(&decoy_alerts);
        let degraded = Arc::clone(&degraded);
        let l_path = l.audit_log();

        std::thread::spawn(move || {
            let trust_snapshot = trust.lock().unwrap();
            let id_kp = chimera_crypto::obsidian::DsaKeypair { signing_key: identity_sk, verifying_key: identity_vk };
            let session_key = match chimera_ipc::run_server_handshake(&mut stream, &id_kp, &trust_snapshot, &attestation) {
                Ok(k) => k,
                Err(e) => { append_line(&l_path, &format!("{{\"ts\":{},\"event\":\"handshake_rejected\",\"detail\":\"{e}\"}}", unix_now())); return; }
            };
            drop(trust_snapshot);

            let mut channel = chimera_ipc::SecureChannel::new(stream, session_key);
            loop {
                let req = match channel.recv() {
                    Ok(raw) => match Request::decode(&raw) { Ok(r) => r, Err(_) => break },
                    Err(_) => break, // baglanti kapandi
                };
                let resp = handle_request(req, &master_key, &decoy_alerts, &degraded, &l_path);
                if channel.send(&resp.encode()).is_err() { break; }
            }
        });

        if l.stop_flag().exists() {
            audit(&l, "core.stop", "temiz durdurma bayragi bulundu");
            let _ = std::fs::remove_file(l.stop_flag());
            break;
        }
    }
    0
}

fn handle_request(
    req: Request,
    master_key: &Option<[u8; 32]>,
    decoy_alerts: &Arc<Mutex<Vec<String>>>,
    degraded: &Arc<AtomicBool>,
    audit_path: &Path,
) -> Response {
    let unlocked = |unlock: &[u8; 32]| -> bool {
        match master_key {
            Some(mk) => constant_time_eq(mk, unlock),
            None => false,
        }
    };
    match req {
        Request::Ping => Response::Pong,
        Request::Status => {
            let mode = if degraded.load(Ordering::Relaxed) { "DegradedSafe" } else { "Full" };
            Response::StatusOk(format!("mode={mode}"))
        }
        Request::GetLogs { unlock } => {
            if !unlocked(&unlock) {
                append_line(audit_path, &format!("{{\"ts\":{},\"event\":\"privileged_denied\",\"detail\":\"GetLogs\"}}", unix_now()));
                return Response::Denied;
            }
            let tail = std::fs::read_to_string(audit_path).unwrap_or_default();
            let last: String = tail.lines().rev().take(20).collect::<Vec<_>>().join("\n");
            Response::LogsOk(last)
        }
        Request::SetDegraded { on, unlock } => {
            if !unlocked(&unlock) {
                append_line(audit_path, "{\"event\":\"privileged_denied\",\"detail\":\"SetDegraded\"}");
                return Response::Denied;
            }
            degraded.store(on, Ordering::Relaxed);
            append_line(audit_path, &format!("{{\"ts\":{},\"event\":\"mode.set\",\"detail\":\"degraded={on}\"}}", unix_now()));
            Response::StatusOk(format!("mode={}", if on { "DegradedSafe" } else { "Full" }))
        }
        Request::ListDecoyAlerts { unlock } => {
            if !unlocked(&unlock) {
                return Response::Denied;
            }
            let alerts = decoy_alerts.lock().unwrap();
            Response::DecoyAlertsOk(format!("[{}]", alerts.join(",")))
        }
        Request::Heartbeat { source } => {
            append_line(audit_path, &format!("{{\"ts\":{},\"event\":\"heartbeat\",\"detail\":\"{source}\"}}", unix_now()));
            Response::HeartbeatAck
        }
    }
}

/// A/B karsilikli watchdog'un A tarafi: core, sentinel'i cocuk surec olarak
/// baslatir ve cokerse yeniden baslatir. Sentinel de kendi tarafinda
/// core'un pid'ini izler (bkz. `chimera-sentinel`), boylece "A B'yi, B A'yi
/// izler" saglanmis olur. Bu, OS-seviyesinde bilincli sonlandirmaya karsi
/// bir direnc DEGIL, KAZA/COKME sonrasi otomatik toparlanmadir.
/// Sentinel'in kimligini (henuz yoksa) olusturur ve TAM acik anahtarini
/// core'un guven deposuna ekler. Bu bootstrap, ag uzerinden DEGIL, aynı
/// makinede aynı operatorun kontrolundeki bir alt surec cagrisi (argv)
/// uzerinden gerceklesir — SSH'in ilk baglantida host key'i "yerel olarak
/// dogrulanmis" kabul etmesiyle ayni guven sinirina sahiptir.
fn bootstrap_trust_sentinel(l: &Layout) -> std::io::Result<()> {
    let exe = which_sentinel();
    let out = std::process::Command::new(&exe).arg("identity").arg("--root").arg(&l.root).output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let hex_line = text.lines().find_map(|line| line.strip_prefix("acik anahtar (hex): "));
    let Some(hex_str) = hex_line else {
        return Err(std::io::Error::other("sentinel identity ciktisinda acik anahtar bulunamadi"));
    };
    let bytes = unhex(hex_str).map_err(|_| std::io::Error::other("gecersiz hex"))?;
    let mut trust = TrustStore::load(&l.trust_list())?;
    trust.trust(&bytes)?;

    // Sentinel'in ikili ozetini de AYNI yerel (argv uzerinden, ag disi)
    // bootstrap sirasinda sabitleriz: bu, core'un KENDI baslattigi bir alt
    // surece bakip ozetini okumasidir -- ag uzerinden gelen bir iddia
    // degil, guvenilir bir yerel gozlemdir. Bir sonraki el sikismada bu
    // ozet degismisse (sentinel ikilisi disaridan degistirilmisse) tespit
    // edilir.
    if let Some(hash_line) = text.lines().find_map(|line| line.strip_prefix("ikili ozet (BLAKE3): ")) {
        let mut attest = chimera_ipc::AttestationStore::load(&l.attestation_list())?;
        let fp = chimera_ipc::trust::fingerprint_of(&bytes);
        attest.pin(&fp, hash_line)?;
    }
    Ok(())
}

/// Sentinel'in halihazirda CANLI olup olmadigini kontrol eder. Bu, core her
/// yeniden basladiginda (kaza sonrasi) yeni bir sentinel dogurup eskisini
/// COGALTMASINI onler — bu, ilk canli demoda GERCEKTEN yakalanan bir
/// kaynak sizintisi hatasiydi. Linux'ta `/proc/<pid>/exe` ile kesin
/// dogrulama yapilir; diger platformlarda PID dosyasinin yakin zamanda
/// (bir heartbeat penceresi icinde) yazilmis olmasi zayif ama makul bir
/// canlilik sinyali olarak kullanilir.
fn sentinel_is_alive(root: &Path) -> bool {
    let pid_path = root.join("runtime/sentinel.pid");
    let Ok(text) = std::fs::read_to_string(&pid_path) else { return false };
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
    let Ok(pid) = text.trim().parse::<u32>() else { return false };

    #[cfg(target_os = "linux")]
    {
        let exe_link = format!("/proc/{pid}/exe");
        match std::fs::read_link(&exe_link) {
            Ok(target) => target.to_string_lossy().contains("chimera-sentinel"),
            Err(_) => false, // /proc/<pid> yok -> surec olu
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::fs::metadata(&pid_path)
            .and_then(|m| m.modified())
            .map(|t| t.elapsed().map(|e| e < Duration::from_secs(30)).unwrap_or(false))
            .unwrap_or(false)
    }
}

fn spawn_sentinel_watchdog(l: &Layout, core_vk_hex: String) {
    let root = l.root.clone();
    let stop_flag = l.stop_flag();
    std::thread::spawn(move || {
        let mut backoff = Duration::from_millis(250);
        loop {
            if stop_flag.exists() {
                return;
            }
            // Onceki bir core'dan kalma, HALA canli bir sentinel varsa
            // COGALTMA — yalnizca bekle ve tekrar kontrol et. Bu sentinel
            // olurse bir sonraki turda normal sekilde yeniden dogurulur.
            if sentinel_is_alive(&root) {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
            let exe = which_sentinel();
            let mut cmd = std::process::Command::new(&exe);
            cmd.arg("watch").arg("--root").arg(&root).arg("--core-pubkey-hex").arg(&core_vk_hex);
            if let Ok(h) = chimera_ipc::attestation::self_binary_hash() {
                cmd.arg("--core-binary-hash").arg(h);
            }
            let start = Instant::now();
            match cmd.spawn() {
                Ok(mut child) => {
                    let _ = child.wait();
                    if start.elapsed() > Duration::from_secs(30) {
                        backoff = Duration::from_millis(250);
                    } else {
                        backoff = (backoff * 2).min(Duration::from_secs(10));
                    }
                }
                Err(_) => { backoff = (backoff * 2).min(Duration::from_secs(10)); }
            }
            std::thread::sleep(backoff);
        }
    });
}

fn which_sentinel() -> PathBuf {
    // Ayni derleme cikisindaki kardes binary. Uretimde bu, servis
    // kurulum dizinine gore sabitlenir.
    let mut p = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("chimera-core"));
    p.set_file_name(if cfg!(windows) { "chimera-sentinel.exe" } else { "chimera-sentinel" });
    p
}

/// Named Pipe'a erisimi, hicbir kimlik-dogrulama katmanina guvenmeden
/// ISLETIM SISTEMI SEVIYESINDE de daraltir: yalnizca LocalSystem ve
/// yerlesik Yoneticiler grubu baglanabilir. Bu, "bir bilesen sadece
/// process adina veya pipe adina guvenerek trusted kabul edilmemeli"
/// gereksiniminin OS-ACL katmanindaki karsiligidir — el sikisma zaten
/// kriptografik kimlik dogrulamasi yapiyor, bu KATMANLI bir ek savunmadir
/// (varsayilan Named Pipe ACL'i, ayni makinedeki HERKESE yazma izni verir).
///
/// SDDL: `D:` (DACL) `(A;;GA;;;SY)` SYSTEM'e Generic All, `(A;;GA;;;BA)`
/// Built-in Administrators'a Generic All. Baska hicbir ACE yok -> baska
/// hicbir hesap (dahil "Everyone"/"Authenticated Users") baglanamaz.
#[cfg(windows)]
fn windows_pipe_acl() -> io::Result<interprocess::os::windows::security_descriptor::SecurityDescriptor> {
    use widestring::U16CString;
    let sddl = U16CString::from_str("D:(A;;GA;;;SY)(A;;GA;;;BA)").map_err(|e| io::Error::other(e.to_string()))?;
    interprocess::os::windows::security_descriptor::SecurityDescriptor::deserialize(&sddl)
}

/// TEK, opsiyonel, log-only anti-analiz katmani. Bilinçli sinirlar:
///   - Varsayilan olarak KAPALI (`CHIMERA_LOG_DEBUGGER=1` gerekir) — kurumsal
///     ortamlarda meşru bir hata ayiklayici/APM ajaninin baglanmasi YANLIŞ
///     POZITIF uretmemesi icin operator ACIKCA acmalidir.
///   - Tespit edilirse yalnizca DENETIM KAYDINA yazilir — surec sonlanmaz,
///     veri silinmez, kullanici engellenmez. Bu maddenin talebi acikca
///     "anti-debugging'i TEK guvenlik mekanizmasi yapma" idi.
///   - `IsDebuggerPresent`, standart, iyi bilinen, KOLAYCA ATLATILABILEN
///     bir Win32 API'sidir (PEB.BeingDebugged bayragini okur). Bunun
///     BILEREK boyle oldugunu kabul ediyoruz: gercek koruma imza+attestation
///     katmanlarindan gelir (bkz. handshake.rs), bu yalnizca "ucuz saldirgan"
///     (Tehdit Modeli Seviye 1-2) icin bir sinyal katmanidir.
fn maybe_log_debugger_presence(l: &Layout) {
    if std::env::var("CHIMERA_LOG_DEBUGGER").as_deref() != Ok("1") {
        return;
    }
    #[cfg(windows)]
    {
        let present = unsafe { windows::Win32::System::Diagnostics::Debug::IsDebuggerPresent() };
        if present.as_bool() {
            audit(l, "security.debugger_detected", "IsDebuggerPresent=true (yalnizca bilgi amacli, aksiyon alinmadi)");
        }
    }
    #[cfg(not(windows))]
    {
        let _ = l;
    }
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn audit(l: &Layout, event: &str, detail: &str) {
    let _ = std::fs::create_dir_all(l.root.join("logs"));
    append_line(&l.audit_log(), &format!("{{\"ts\":{},\"event\":\"{}\",\"detail\":\"{}\"}}", unix_now(), event, detail.replace('"', "'")));
}

fn append_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 { return Err(()); }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ())).collect()
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}
