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
const RESPAWN_COOLDOWN: Duration = Duration::from_secs(15);

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

    loop {
        // PID dosyasini periyodik olarak "dokun": non-Linux platformlarda
        // core'un canlilik sinyali icin kullandigi dosya-zamani sezgisi
        // bu sayede taze kalir (bkz. core'daki `sentinel_is_alive`).
        let _ = std::fs::write(&pid_path, std::process::id().to_string());

        match heartbeat_once(root, &identity, &trust, &attest) {
            Ok(()) => {
                consecutive_failures = 0;
            }
            Err(e) => {
                consecutive_failures += 1;
                eprintln!("core'a heartbeat basarisiz ({consecutive_failures}/{FAILURES_BEFORE_RESPAWN}): {e}");
                if consecutive_failures >= FAILURES_BEFORE_RESPAWN {
                    let can_respawn = last_respawn.map(|t| t.elapsed() > RESPAWN_COOLDOWN).unwrap_or(true);
                    if can_respawn {
                        eprintln!("core yanit vermiyor -- YENIDEN BASLATILIYOR");
                        respawn_core(root);
                        last_respawn = Some(Instant::now());
                        consecutive_failures = 0;
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

fn respawn_core(root: &Path) {
    let exe = which_core();
    match std::process::Command::new(&exe).arg("serve").arg("--root").arg(root).spawn() {
        Ok(_) => eprintln!("core yeniden baslatildi: {}", exe.display()),
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
