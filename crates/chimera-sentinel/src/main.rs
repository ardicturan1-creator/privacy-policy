//! chimera-sentinel — karşılıklı watchdog eşi.
//!
//! `chimera-core`, kendi başına da (chimera-core'un içindeki thread ile)
//! sentinel'i izler; bu binary de TERSİNİ yapar: core'un IPC soketine
//! periyodik olarak GERÇEK bir el sıkışmalı bağlantı kurup Heartbeat
//! gönderir. Bağlantı art arda başarısız olursa (core çökmüş/durmuş
//! demektir) core'u yeniden başlatır.
//!
//!   chimera-sentinel identity --root R
//!   chimera-sentinel watch    --root R --core-pubkey-hex HEX

use chimera_ipc::{Identity, Request, Response, TrustStore};
use interprocess::local_socket::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(4);
const FAILURES_BEFORE_RESPAWN: u32 = 3;
const RESPAWN_COOLDOWN_BASE: Duration = Duration::from_secs(15);
/// *** GERCEK BIR HATANIN DUZELTMESI ***: sabit 15sn'lik bir bekleme,
/// core KALICI olarak baslatilamiyorsa (or. bozuk/uyumsuz bir guven
/// deposu, disk dolu, ikili bozuk) sentinel'i SONSUZA KADAR ~15sn'de bir
/// yeniden baslatma denemesine sokar. Canli testte bu GERCEKTEN yasandi:
/// bir test sirasinda unutulan, guveni bozuk bir sentinel/core cifti
/// ~50 dakika boyunca kesintisiz denemeye devam etti ve YUZLERCE zombie
/// surec biriktirdi (bkz. `respawn_core`'daki reap duzeltmesi de). Tavan
/// degerli bir ustel geri-cekilme (core'un KENDI sentinel-izleme
/// dongusundeki -- `chimera-core::spawn_sentinel_watchdog` -- ayni
/// desenle TUTARLI), boyle bir kalici-arizada deneme sikligini giderek
/// azaltir; basarili bir heartbeat'te taban degere sifirlanir.
const RESPAWN_COOLDOWN_MAX: Duration = Duration::from_secs(300);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("watch");
    let root = PathBuf::from(flag(&args, "--root").unwrap_or_else(|| "/opt/chimera-core".to_string()));

    {
        let log_path = root.join("logs/client.jsonl");
        std::panic::set_hook(Box::new(move |info| {
            let detail = info.to_string().replace('"', "'").replace('\n', " ");
            let _ = std::fs::create_dir_all(log_path.parent().unwrap());
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
                use std::io::Write as _;
                let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let _ = writeln!(f, "{{\"ts\":{ts},\"event\":\"panic\",\"detail\":\"{detail}\"}}");
            }
            eprintln!("chimera-sentinel: beklenmeyen bir ic hata olustu; ayrintilar {} icinde.", "logs/client.jsonl");
        }));
    }

    let code = match cmd {
        "identity" => cmd_identity(&root),
        "watch" => cmd_watch(&root, flag(&args, "--core-pubkey-hex"), flag(&args, "--core-binary-hash")),
        other => {
            eprintln!("bilinmeyen alt komut: {other}");
            2
        }
    };
    std::process::exit(code);
}

fn identity_dir(root: &Path) -> PathBuf { root.join("state/sentinel_identity") }
fn trust_list(root: &Path) -> PathBuf { root.join("state/sentinel_trusted.list") }

fn cmd_identity(root: &Path) -> i32 {
    let id = match Identity::load_or_create(&identity_dir(root)) {
        Ok(i) => i,
        Err(e) => { eprintln!("kimlik olusturulamadi: {e}"); return 1; }
    };
    println!("chimera-sentinel kimlik parmak izi: {}", id.fingerprint());
    println!("acik anahtar (hex): {}", hex(&id.verifying_key_bytes()));
    match chimera_ipc::attestation::self_binary_hash() {
        Ok(h) => println!("ikili ozet (BLAKE3): {h}"),
        Err(e) => eprintln!("ikili ozet hesaplanamadi: {e}"),
    }
    0
}

fn attestation_list(root: &Path) -> PathBuf { root.join("state/sentinel_attest.list") }

fn cmd_watch(root: &Path, core_pubkey_hex: Option<String>, core_binary_hash: Option<String>) -> i32 {
    // Kendi PID'imizi yaz: core'un bizi tekrar tekrar (her kendi
    // yeniden baslamasinda) COGALTMASINI onler. `main`'in sonunda,
    // process cikarken bu dosya silinir (bkz. asagida ctrlc benzeri
    // temizlik yerine basit bir "olu PID -> yeniden yazilabilir" kurali).
    let pid_path = root.join("runtime/sentinel.pid");
    let _ = std::fs::create_dir_all(root.join("runtime"));
    let _ = std::fs::write(&pid_path, std::process::id().to_string());

    let identity = match Identity::load_or_create(&identity_dir(root)) {
        Ok(i) => i,
        Err(e) => { eprintln!("kimlik olusturulamadi: {e}"); return 1; }
    };

    let mut trust = match TrustStore::load(&trust_list(root)) {
        Ok(t) => t,
        Err(e) => { eprintln!("guven deposu okunamadi: {e}"); return 1; }
    };

    // Bootstrap: core'un TAM acik anahtari, bizi baslatan SUREC tarafindan
    // (argv uzerinden, ayni makinede) bildirildi -- ag uzerinden degil.
    let mut attest = match chimera_ipc::AttestationStore::load(&attestation_list(root)) {
        Ok(a) => a,
        Err(e) => { eprintln!("attestasyon deposu okunamadi: {e}"); return 1; }
    };
    if let Some(hex_str) = core_pubkey_hex {
        if let Ok(bytes) = unhex(&hex_str) {
            let _ = trust.trust(&bytes);
            if let Some(bh) = &core_binary_hash {
                let fp = chimera_ipc::trust::fingerprint_of(&bytes);
                let _ = attest.pin(&fp, bh);
            }
        }
    }

    let mut consecutive_failures: u32 = 0;
    let mut last_respawn: Option<Instant> = None;
    let mut respawn_backoff = RESPAWN_COOLDOWN_BASE;
    let stop_flag = root.join("runtime/stop.flag");

    loop {
        // *** GERCEK BIR HATANIN DUZELTMESI ***: bu kontrol eklenmeden
        // once, bir operator `chimera-core`'u BILINCLI olarak (SIGTERM/
        // Ctrl-C ile) durdurdugunda, sentinel bunu "core coktu" sanip
        // birkac saniye icinde GERI BASLATIYORDU — "temiz bir stop komutu
        // HER ZAMAN saygi gorur" dokumantasyon vaadi GERCEKTE yalan
        // cikiyordu. Simdi: stop.flag varsa sentinel core'u ASLA
        // yeniden baslatmaz VE kendisi de temiz sekilde cikar (core
        // tekrar `serve` ile baslatildiginda taze bir sentinel dogar).
        if stop_flag.exists() {
            eprintln!("temiz durdurma bayragi bulundu -- sentinel core'u yeniden baslatmadan cikiyor");
            let _ = std::fs::remove_file(&pid_path);
            return 0;
        }

        // PID dosyasini periyodik olarak "dokun": non-Linux platformlarda
        // core'un canlilik sinyali icin kullandigi dosya-zamani sezgisi
        // bu sayede taze kalir (bkz. core'daki `sentinel_is_alive`).
        let _ = std::fs::write(&pid_path, std::process::id().to_string());

        match heartbeat_once(root, &identity, &trust, &attest) {
            Ok(()) => {
                consecutive_failures = 0;
                // Basarili bir heartbeat, core'un GERCEKTEN saglikli
                // calistigini kanitlar -- bir sonraki (farkli, gelecekteki)
                // ariza icin geri-cekilmeyi taban degere sifirlariz.
                respawn_backoff = RESPAWN_COOLDOWN_BASE;
            }
            Err(e) => {
                consecutive_failures += 1;
                eprintln!("core'a heartbeat basarisiz ({consecutive_failures}/{FAILURES_BEFORE_RESPAWN}): {e}");
                if consecutive_failures >= FAILURES_BEFORE_RESPAWN {
                    // Coklu kontrol: heartbeat basarisiz oldu VE bu arada
                    // bir stop.flag OLUSMUS olabilir (SIGTERM aninda tam
                    // bu pencerede gelmis olabilir) -- yeniden baslatmadan
                    // hemen once SON BIR KEZ daha kontrol edilir.
                    if stop_flag.exists() {
                        eprintln!("temiz durdurma bayragi bulundu -- yeniden baslatma IPTAL edildi");
                        let _ = std::fs::remove_file(&pid_path);
                        return 0;
                    }
                    let can_respawn = last_respawn.map(|t| t.elapsed() > respawn_backoff).unwrap_or(true);
                    if can_respawn {
                        eprintln!("core yanit vermiyor -- YENIDEN BASLATILIYOR (bir sonraki deneme icin bekleme: {respawn_backoff:?})");
                        respawn_core(root);
                        last_respawn = Some(Instant::now());
                        consecutive_failures = 0;
                        // KALICI bir ariza (or. bozuk guven deposu) ayni
                        // respawn'i sonsuza kadar tekrarlatirdi -- bkz.
                        // RESPAWN_COOLDOWN_MAX'in ustundeki not. Ustel
                        // geri-cekilme, boyle bir durumda CPU/PID tuketimini
                        // sinirlar; core GERCEKTEN duzelirse bir sonraki
                        // basarili heartbeat bunu hemen taban degere geri
                        // sifirlar.
                        respawn_backoff = (respawn_backoff * 2).min(RESPAWN_COOLDOWN_MAX);
                    }
                }
            }
        }
        std::thread::sleep(HEARTBEAT_INTERVAL);
    }
}

fn heartbeat_once(root: &Path, identity: &Identity, trust: &TrustStore, attestation: &chimera_ipc::AttestationStore) -> Result<(), String> {
    let name = chimera_ipc::socket_name(root).map_err(|e| e.to_string())?;
    let mut stream = interprocess::local_socket::Stream::connect(name).map_err(|e| format!("connect: {e}"))?;

    let session_key = chimera_ipc::run_client_handshake(&mut stream, &identity.keypair, trust, attestation).map_err(|e| format!("handshake: {e}"))?;
    let mut channel = chimera_ipc::SecureChannel::new(stream, session_key);

    let resp = chimera_ipc::call(&mut channel, &Request::Heartbeat { source: "sentinel".into() }).map_err(|e| format!("call: {e}"))?;
    match resp {
        Response::HeartbeatAck => Ok(()),
        other => Err(format!("beklenmeyen yanit: {other:?}")),
    }
}

/// *** GERCEK BIR HATANIN DUZELTMESI (canli testte YAKALANDI) ***: burada
/// spawn edilen cocuk surec ASLA `wait()` ile toplanmiyordu (reap
/// edilmiyordu). Isletim sistemi kurallari geregi, bir cocuk surec
/// sonlaninca, ebeveyni onu `wait()` ile "toplayana" kadar PID tablosunda
/// "defunct"/zombie olarak kalir. Core kalici olarak baslatilamiyorsa
/// (bkz. RESPAWN_COOLDOWN_MAX yorumu) sentinel bu fonksiyonu tekrar tekrar
/// cagirir ve HER cagri kalici bir zombie birakirdi -- canli bir testte bu
/// GERCEKTEN yasandi ve kisa surede yuzlerce zombie surec birikti (PID
/// tuketimi -- kendi kendine bir kaynak-tukenmesi DoS'u). Duzeltme: ana
/// heartbeat dongusunu BLOKLAMADAN, ayri kisa-omurlu bir thread'de
/// `child.wait()` cagirarak cocugu reap ediyoruz.
fn respawn_core(root: &Path) {
    let exe = which_core();
    match std::process::Command::new(&exe).arg("serve").arg("--root").arg(root).spawn() {
        Ok(mut child) => {
            eprintln!("core yeniden baslatildi: {}", exe.display());
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => eprintln!("core yeniden baslatilamadi: {e}"),
    }
}

fn which_core() -> PathBuf {
    let mut p = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("chimera-sentinel"));
    p.set_file_name(if cfg!(windows) { "chimera-core.exe" } else { "chimera-core" });
    p
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
