//! chimera-admin — Sıfır Güven kontrol paneli (konsol istemcisi).
//!
//! Bu, grafik bir masaüstü GUI DEĞİLDİR — bu ortamda bir grafik araç
//! setini (Windows/Linux/macOS'ta gerçek pencere sistemleriyle) inşa edip
//! doğrulamanın imkânı yok, o yüzden "GUI" diye sahte bir mockup sunmak
//! yerine gerçek, çalışan, test edilebilir bir KONSOL kontrol paneli
//! sunuyoruz. Aynı IPC protokolü üzerinde gerçek bir masaüstü GUI (Tauri/
//! egui) bu konsol istemcisinin yaptığı çağrıları birebir yeniden kullanır.
//!
//! Sıfır Güven kuralı: `status` ve `ping` DIŞINDAKİ her komut, Shamir(2,3)
//! paylarından EN AZ İKİSİNİ ister. Paylar eksik veya yanlışsa, Core
//! `Denied` döner — panel "boş bir kasa" kalır, hiçbir hassas veri
//! GÖRÜNTÜLENMEZ.
//!
//!   chimera-admin identity   --root R
//!   chimera-admin trust-core --root R <core-pubkey-hex>
//!   chimera-admin status     --root R
//!   chimera-admin logs       --root R --share HEX --share HEX
//!   chimera-admin degrade    --root R --share HEX --share HEX <on|off>
//!   chimera-admin decoys     --root R --share HEX --share HEX
//!   chimera-admin verify-audit --root R --share HEX --share HEX

use chimera_ipc::{Identity, Request, Response, TrustStore};
use interprocess::local_socket::prelude::*;
use sharks::Share;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("status");
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
            eprintln!("chimera-admin: beklenmeyen bir ic hata olustu; ayrintilar {} icinde.", "logs/client.jsonl");
        }));
    }

    let code = match cmd {
        "identity" => cmd_identity(&root),
        "trust-core" => cmd_trust_core(&root, flag(&args, "--pubkey")),
        "attest-core" => cmd_attest_core(&root, flag(&args, "--pubkey"), flag(&args, "--binary-hash")),
        "status" => cmd_status(&root),
        "logs" => cmd_privileged(&root, &args, |unlock| Request::GetLogs { unlock }, "GUNLUKLER"),
        "decoys" => cmd_privileged(&root, &args, |unlock| Request::ListDecoyAlerts { unlock }, "DECOY UYARILARI"),
        "degrade" => {
            let on = args.iter().any(|a| a == "on");
            cmd_privileged(&root, &args, move |unlock| Request::SetDegraded { on, unlock }, "MOD DEGISTIRME")
        }
        "verify-audit" => cmd_privileged(&root, &args, |unlock| Request::VerifyAuditLog { unlock }, "DENETIM ZINCIRI DOGRULAMA"),
        other => {
            eprintln!("bilinmeyen alt komut: {other}");
            eprintln!("kullanim: chimera-admin <identity|trust-core|status|logs|decoys|degrade|verify-audit> --root DIR");
            2
        }
    };
    std::process::exit(code);
}

fn identity_dir(root: &Path) -> PathBuf { root.join("state/admin_identity") }
fn trust_list(root: &Path) -> PathBuf { root.join("state/admin_trusted.list") }
fn attestation_list(root: &Path) -> PathBuf { root.join("state/admin_attest.list") }

fn cmd_identity(root: &Path) -> i32 {
    let id = match Identity::load_or_create(&identity_dir(root)) {
        Ok(i) => i,
        Err(e) => { eprintln!("kimlik olusturulamadi: {e}"); return 1; }
    };
    println!("chimera-admin kimlik parmak izi: {}", id.fingerprint());
    println!("acik anahtar (hex): {}", hex(&id.verifying_key_bytes()));
    match chimera_ipc::attestation::self_binary_hash() {
        Ok(h) => println!("ikili ozet (BLAKE3): {h}"),
        Err(e) => eprintln!("ikili ozet hesaplanamadi: {e}"),
    }
    println!("\nBu deger 'chimera-core trust <hex>' ile Core'un guven deposuna eklenmelidir.");
    0
}

fn cmd_trust_core(root: &Path, hex_arg: Option<String>) -> i32 {
    let Some(hex_str) = hex_arg else {
        eprintln!("kullanim: chimera-admin trust-core --root DIR --pubkey <core-acik-anahtar-hex>");
        return 2;
    };
    let Ok(bytes) = unhex(&hex_str) else { eprintln!("gecersiz hex"); return 1; };
    let mut trust = match TrustStore::load(&trust_list(root)) {
        Ok(t) => t,
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    if let Err(e) = trust.trust(&bytes) {
        eprintln!("{e}"); return 1;
    }
    println!("core guvenildi: {}", chimera_ipc::trust::fingerprint_of(&bytes));
    0
}

fn cmd_attest_core(root: &Path, pubkey_hex: Option<String>, binary_hash_hex: Option<String>) -> i32 {
    let (Some(pk), Some(bh)) = (pubkey_hex, binary_hash_hex) else {
        eprintln!("kullanim: chimera-admin attest-core --pubkey <hex> --binary-hash <blake3-hex>");
        return 2;
    };
    let Ok(vk_bytes) = unhex(&pk) else { eprintln!("gecersiz hex"); return 1; };
    let mut attest = match chimera_ipc::AttestationStore::load(&attestation_list(root)) {
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

fn connect(root: &Path) -> Result<chimera_ipc::SecureChannel<interprocess::local_socket::Stream>, String> {
    let identity = Identity::load_or_create(&identity_dir(root)).map_err(|e| e.to_string())?;
    let trust = TrustStore::load(&trust_list(root)).map_err(|e| e.to_string())?;
    let attestation = chimera_ipc::AttestationStore::load(&attestation_list(root)).map_err(|e| e.to_string())?;
    let name = chimera_ipc::socket_name(root).map_err(|e| e.to_string())?;
    let mut stream = interprocess::local_socket::Stream::connect(name).map_err(|e| format!("core'a baglanilamadi: {e} (calisiyor mu? 'chimera-core serve' baslatilmis mi?)"))?;
    let session_key = chimera_ipc::run_client_handshake(&mut stream, &identity.keypair, &trust, &attestation)
        .map_err(|e| format!("el sikisma basarisiz: {e} (once 'chimera-core trust <admin-hex>' ile guvenilir kilindiniz mi?)"))?;
    Ok(chimera_ipc::SecureChannel::new(stream, session_key))
}

fn cmd_status(root: &Path) -> i32 {
    let mut channel = match connect(root) {
        Ok(c) => c,
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    match chimera_ipc::call(&mut channel, &Request::Status) {
        Ok(Response::StatusOk(s)) => { println!("{s}"); 0 }
        Ok(other) => { eprintln!("beklenmeyen yanit: {other:?}"); 1 }
        Err(e) => { eprintln!("{e}"); 1 }
    }
}

/// Shamir(2,3) paylarını toplar, GERÇEKTEN yeniden birleştirir ve
/// ayrıcalıklı isteği bu değerle Core'a gönderir. Paylar eksik/yanlışsa
/// Core zaten `Denied` döner — burada ek bir "doğru mu" kontrolü YOKTUR,
/// çünkü doğru cevap yalnızca Core'un kendi kasasında bilinir (Zero Trust:
/// istemci kendi kendine "doğru" karar veremez).
fn cmd_privileged(root: &Path, args: &[String], make_req: impl FnOnce([u8; 32]) -> Request, label: &str) -> i32 {
    let share_args: Vec<&String> = args.iter().enumerate().filter(|(i, a)| **a == "--share" && args.get(i + 1).is_some()).map(|(i, _)| &args[i + 1]).collect();

    if share_args.len() < 2 {
        eprintln!("KASA KAPALI: en az 2 Shamir payi gerekli (--share HEX --share HEX). {label} icin yetkiniz dogrulanamadi.");
        return 1;
    }

    let mut shares = Vec::new();
    for s in &share_args {
        let Ok(bytes) = unhex(s) else { eprintln!("gecersiz pay formati"); return 1; };
        match Share::try_from(bytes.as_slice()) {
            Ok(share) => shares.push(share),
            Err(_) => { eprintln!("pay ayristirilamadi"); return 1; }
        }
    }

    let master = match chimera_crypto::obsidian::recover_master_key(&shares) {
        Ok(m) => m,
        Err(_) => { eprintln!("KASA KAPALI: paylardan gecerli bir anahtar kurulamadi."); return 1; }
    };

    let mut channel = match connect(root) {
        Ok(c) => c,
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    match chimera_ipc::call(&mut channel, &make_req(master)) {
        Ok(Response::Denied) => { eprintln!("KASA KAPALI: Core paylari reddetti (yanlis/eksik pay kombinasyonu)."); 1 }
        Ok(Response::LogsOk(s)) => { println!("{s}"); 0 }
        Ok(Response::DecoyAlertsOk(s)) => { println!("{s}"); 0 }
        Ok(Response::StatusOk(s)) => { println!("{s}"); 0 }
        Ok(Response::AuditVerifyOk(s)) => { let ok = s.starts_with("SAGLAM") || s.starts_with("BOS"); println!("{s}"); if ok { 0 } else { 1 } }
        Ok(other) => { eprintln!("beklenmeyen yanit: {other:?}"); 1 }
        Err(e) => { eprintln!("{e}"); 1 }
    }
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
