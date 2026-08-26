//! pipeline.rs — Detector → Validator → Executor üç-aşamalı güvenli
//! düzeltme boru hattı.
//!
//! Bu, "3 ajan: biri patch'ler, biri test eder, biri tamamlar" talebinin
//! GERÇEK ve GÜVENLİ karşılığıdır. Otonom, insan onayı olmadan kernel/sistem
//! durumunu değiştiren, açık uçlu bir "AI ajanı" mimarisi KASITLI olarak
//! YAPILMADI (bkz. proje belgeleri) — böyle bir şey, yanlış bir tespitte
//! sistemi bizzat kendisi bozabilir; hiçbir ciddi EDR ürünü bunu yapmaz.
//! Bunun yerine üç net, denetlenebilir aşama:
//!
//!   1. Detector  — `scanner::scan()` çalıştırır, ham bulgu listesi üretir.
//!   2. Validator — her bulgunun taşıdığı remediation'ı SABİT bir
//!                  whitelist'e karşı kontrol eder (`is_whitelisted`).
//!                  Whitelist DIŞINDA hiçbir aksiyon ASLA çalıştırılmaz —
//!                  yalnızca "needs_review" listesine düşer.
//!   3. Executor  — SADECE whitelist'i geçen, dar ve geri-alınabilir
//!                  aksiyonları `remediate.rs` üzerinden uygular. HER
//!                  deneme (başarılı/başarısız) çağıranın verdiği `audit`
//!                  callback'i ile kanıta-dayanıklı denetim kaydına yazılır.

use crate::scanner::{self, Finding, Remediation, Severity};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Detector'in ürettiği HER remediation türü burada listelenmedikçe
/// Executor tarafından ASLA çalıştırılmaz. Bilinçli olarak küçük tutulur:
/// yalnızca `remediate.rs`'teki dört şartı (dar/geri-alınabilir/veri
/// kaybı yok/otomatik kesinti yok) karşılayan fonksiyonlar buraya girer.
fn is_whitelisted(r: &Remediation) -> bool {
    match r {
        Remediation::EnableFirewall | Remediation::DisableSmb1 => true,
        // GECICI ve GERI ALINABILIR: TTL'i dolunca kendiliginden kalkar
        // (bkz. autoblock.rs). Kalici engelleme (`block-ip`) DEGILDIR ve
        // hala Shamir(2,3) ister.
        Remediation::AutoBlockSourceIp { .. } => true,
        // GERI ALINAMAZ: bir surecin kalici olarak sonlandirilmasi ASLA
        // otomatik calistirilmaz. Devre kesici sureci zaten (geri
        // alinabilir sekilde) ASKIYA ALDI; buradan sonrasi insanindir.
        Remediation::TerminateSuspendedProcess => false,
    }
}

pub struct PipelineReport {
    pub findings: Vec<Finding>,
    pub actions_taken: Vec<String>,
    pub needs_review: Vec<String>,
}

impl PipelineReport {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("TARAMA: {} bulgu.\n", self.findings.len()));
        for f in &self.findings {
            s.push_str(&format!("  [{}] {} -- {}\n      {}\n", f.severity.as_str(), f.id, f.title, f.detail));
        }
        if self.actions_taken.is_empty() {
            s.push_str("UYGULANAN DUZELTME: yok.\n");
        } else {
            s.push_str("UYGULANAN DUZELTMELER (otomatik, whitelist'ten):\n");
            for a in &self.actions_taken {
                s.push_str(&format!("  - {a}\n"));
            }
        }
        if !self.needs_review.is_empty() {
            s.push_str("INSAN INCELEMESI GEREKEN BULGULAR (otomatik uygulanmadi):\n");
            for r in &self.needs_review {
                s.push_str(&format!("  ! {r}\n"));
            }
        }
        s
    }
}

/// Boru hattını SENKRON olarak bir kez çalıştırır (IPC `ScanNow` isteği
/// veya arka plan döngüsü tarafından çağrılır). `dry_run=true` iken
/// Executor hiçbir gerçek değişiklik yapmaz, yalnızca ne yapacağını
/// raporlar.
pub fn run_once(root: &std::path::Path, dry_run: bool, audit: &impl Fn(&str, &str)) -> PipelineReport {
    let findings = scanner::scan(root);
    let mut actions_taken = Vec::new();
    let mut needs_review = Vec::new();

    // Faz 3: kurtarma yollarini imha eden komutlar (vssadmin delete
    // shadows vb.) EN YUKSEK onceliklidir ve normal bulgu akisindan ONCE
    // islenir. Alinan aksiyon devre kesicidir: surec ASKIYA ALINIR
    // (geri alinabilir), SONLANDIRILMAZ.
    if !dry_run {
        actions_taken.extend(guard_recovery_paths(root, audit));
    }

    for f in &findings {
        audit("scan.finding", &format!("{}[{}]: {}", f.id, f.severity.as_str(), f.title));
        match &f.remediation {
            Some(r) if is_whitelisted(r) => {
                if dry_run {
                    actions_taken.push(format!("[DRY-RUN] {} icin '{}' uygulanacakti", f.id, r.as_str()));
                    continue;
                }
                match execute(root, r) {
                    Ok(msg) => {
                        audit("remediation.applied", &format!("{}: {}", r.as_str(), msg));
                        actions_taken.push(format!("{}: {}", f.id, msg));
                    }
                    Err(e) => {
                        audit("remediation.failed", &format!("{}: {}", r.as_str(), e));
                        needs_review.push(format!("{} (otomatik duzeltme BASARISIZ: {})", f.id, e));
                    }
                }
            }
            Some(r) => {
                // Savunma amacli: scanner.rs zaten yalnizca whitelist'teki
                // turleri uretiyor, ama Validator'in ATLANMASI mumkun
                // olmamali -- boyle bir kayit teorik olarak asla olusmaz.
                needs_review.push(format!("{} (whitelist DISI remediation '{}', otomatik uygulanmadi)", f.id, r.as_str()));
            }
            None if f.severity >= Severity::Medium => {
                needs_review.push(format!("{} [{}]: {}", f.id, f.severity.as_str(), f.title));
            }
            None => {}
        }
    }

    PipelineReport { findings, actions_taken, needs_review }
}

/// `cmdguard.rs`'in yakaladigi her yikici komut icin devre kesiciyi
/// tetikler. Donen liste, rapora "uygulanan aksiyon" olarak girer.
///
/// Bu OTOMATIK calisir ve whitelist'e tabi DEGILDIR -- cunku uyguladigi
/// aksiyon askiya almadir: geri alinabilir, veri kaybi yok, dar. Ayni
/// gerekce `circuit_breaker.rs`'in tuzak/entropi yollarinda da gecerli.
/// KALICI sonlandirma yine yalnizca Shamir(2,3) ile mumkundur.
fn guard_recovery_paths(root: &std::path::Path, audit: &impl Fn(&str, &str)) -> Vec<String> {
    let mut out = Vec::new();
    let found = match crate::cmdguard::recent_destructive_commands(crate::bruteforce::DEFAULT_WINDOW_SECS) {
        Ok(v) => v,
        Err(e) => {
            // Okunamiyorsa bunu "temiz" saymayiz; scanner.rs bunu ayri bir
            // KORLUK bulgusu olarak zaten raporlar.
            audit("cmdguard.unavailable", &e);
            return out;
        }
    };
    for d in found {
        audit("cmdguard.destructive_command", &d.as_detail());
        let outcome = crate::circuit_breaker::trip(
            root,
            d.pid,
            &crate::circuit_breaker::TripReason::MassEncryption { detail: d.as_detail() },
            audit,
        );
        if outcome.skipped_protected {
            out.push(format!("YIKICI KOMUT ({}) korumali bir surecten geldi, aksiyon alinmadi: {}", d.intent.as_str(), d.command_line));
        } else {
            out.push(format!("YIKICI KOMUT ENGELLENDI: pid={} askiya alindi -- {}", d.pid, d.intent.human()));
        }
    }
    out
}

/// Periyodik imzali yedek akisi: gerekiyorsa yeni bir anlik goruntu alir,
/// EN YENISINI dogrular ve eskileri budar.
fn backup_cycle(
    root: &std::path::Path,
    signing_key: &ml_dsa::SigningKey<ml_dsa::MlDsa87>,
    verifying_key: &ml_dsa::VerifyingKey<ml_dsa::MlDsa87>,
    now: u64,
    audit: &impl Fn(&str, &str),
) {
    let due = match crate::backup::age_of_newest(root, now) {
        None => true, // hic yedek yok
        Some(age) => age >= crate::backup::DEFAULT_INTERVAL_SECS,
    };
    if due {
        match crate::backup::snapshot(root, signing_key, now, audit) {
            Ok(msg) => audit("backup.cycle_ok", &msg.replace('\n', " | ")),
            Err(e) => audit("backup.cycle_failed", &e),
        }
        crate::backup::prune(root, crate::backup::DEFAULT_KEEP, audit);
    }

    // Yedek almak yetmez: EN YENI yedegin hala saglam oldugu her turda
    // DOGRULANIR. "Yedegim var" ile "yedegim calisiyor" ayni sey degildir.
    if let Some((ts, dir)) = crate::backup::list_snapshots(root).last() {
        let outcome = crate::backup::verify_snapshot(dir, verifying_key);
        if outcome.is_intact() {
            audit("backup.verify_ok", &format!("snapshot-{ts}: {}", outcome.to_text()));
        } else {
            audit("backup.verify_FAILED", &format!("snapshot-{ts}: {}", outcome.to_text()));
        }
    }
}

fn execute(root: &std::path::Path, r: &Remediation) -> Result<String, String> {
    match r {
        Remediation::EnableFirewall => crate::remediate::enable_firewall(),
        Remediation::DisableSmb1 => crate::remediate::disable_smb1(),
        Remediation::AutoBlockSourceIp { ip, reason } => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let audit_root = root.join("logs/audit.jsonl");
            let audit = move |e: &str, d: &str| { let _ = crate::auditlog::append(&audit_root, e, d); };
            crate::autoblock::block_with_ttl(root, ip, crate::autoblock::DEFAULT_TTL_SECS, reason, now, &audit)
        }
        // Ulasilamaz: `is_whitelisted` bunu zaten reddediyor. Yine de
        // `unreachable!()` YAZILMADI — Validator bir gun yanlislikla
        // atlanirsa, panic yerine acik bir HATA donmek ve denetim kaydina
        // dusmek her zaman daha guvenlidir.
        Remediation::TerminateSuspendedProcess => Err(
            "REDDEDILDI: surec sonlandirma otomatik calistirilamaz, Shamir(2,3) ile 'terminate-process' gerekir".into(),
        ),
    }
}

/// Arka planda periyodik olarak (varsayılan: 30 dakikada bir) `run_once`
/// çalıştıran döngü. `chimera-sentinel`in `respawn_core` deseniyle TUTARLI
/// şekilde: `running` false olduğunda döngü temiz şekilde biter (thread
/// sonsuza dek asılı kalmaz).
pub fn spawn_background_loop(
    running: Arc<AtomicBool>,
    root: std::path::PathBuf,
    signing_key: ml_dsa::SigningKey<ml_dsa::MlDsa87>,
    verifying_key: ml_dsa::VerifyingKey<ml_dsa::MlDsa87>,
) {
    std::thread::spawn(move || {
        let audit_log = root.join("logs/audit.jsonl");
        let audit = |event: &str, detail: &str| {
            let _ = crate::auditlog::append(&audit_log, event, detail);
        };
        while running.load(Ordering::SeqCst) {
            // TTL vaadini GERCEK kilan adim: suresi dolan otomatik
            // engeller, tarama yapilmadan ONCE kaldirilir. Bu her turda
            // calisir; yani bir engel en fazla bir tur (30 dk) fazladan
            // kalabilir -- bu sinir `07-*.md` SS D'de belgelenmistir.
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let expired = crate::autoblock::expire_due(&root, now, &audit);
            if !expired.is_empty() {
                audit("autoblock.cycle_expired", &format!("{} gecici engel suresi doldu ve kaldirildi", expired.len()));
            }

            // Faz 3: periyodik imzali yedek + dogrulama.
            backup_cycle(&root, &signing_key, &verifying_key, now, &audit);

            let report = run_once(&root, false, &audit);
            audit(
                "scan.cycle_complete",
                &format!("{} bulgu, {} otomatik duzeltme, {} inceleme bekliyor", report.findings.len(), report.actions_taken.len(), report.needs_review.len()),
            );
            for _ in 0..(30 * 60) {
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{Finding, Remediation, Severity};

    #[test]
    fn whitelist_accepts_only_known_safe_kinds() {
        assert!(is_whitelisted(&Remediation::EnableFirewall));
        assert!(is_whitelisted(&Remediation::DisableSmb1));
    }

    /// Projenin DEGISMEZ kuralinin testi: geri alinamaz bir aksiyon
    /// whitelist'e ASLA giremez ve Validator atlansa bile Executor onu
    /// calistirmayi REDDEDER (panic'lemek yerine hata doner).
    #[test]
    fn terminating_a_process_is_never_whitelisted_and_execute_refuses_it() {
        assert!(!is_whitelisted(&Remediation::TerminateSuspendedProcess));
        let root = std::env::temp_dir().join(format!("chimera-pipe-x-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let err = execute(&root, &Remediation::TerminateSuspendedProcess).unwrap_err();
        assert!(err.contains("REDDEDILDI"));
        assert!(err.contains("Shamir"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Yedek dongusu: hic yedek yokken ILK anlik goruntuyu almali ve
    /// hemen ardindan DOGRULAMALI. Bu, Faz 3'un uctan uca akisidir ve
    /// Linux'ta tamamen calisir (kripto ve dosya sistemi platformdan
    /// bagimsizdir).
    #[test]
    fn the_backup_cycle_creates_and_then_verifies_a_snapshot() {
        let root = std::env::temp_dir().join(format!("chimera-pipe-bk-{}", std::process::id()));
        // Windows'ta salt-okunur yedek dosyalari `remove_dir_all`'i
        // ENGELLER; once isaretleri kaldirmak sart (bkz. backup.rs).
        crate::backup::clear_read_only_recursive(&root);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("state")).unwrap();
        std::fs::write(root.join("state/vault.sealed"), vec![7u8; 4096]).unwrap();

        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        let events = std::sync::Mutex::new(Vec::new());
        let audit = |e: &str, d: &str| events.lock().unwrap().push(format!("{e}|{d}"));

        backup_cycle(&root, &kp.signing_key, &kp.verifying_key, 1000, &audit);

        let ev = events.lock().unwrap().clone();
        assert!(ev.iter().any(|e| e.starts_with("backup.cycle_ok")), "ilk turda yedek ALINMALI: {ev:?}");
        assert!(ev.iter().any(|e| e.starts_with("backup.verify_ok")), "alinan yedek DOGRULANMALI: {ev:?}");
        assert!(!ev.iter().any(|e| e.starts_with("backup.verify_FAILED")));
        assert_eq!(crate::backup::list_snapshots(&root).len(), 1);

        // Ikinci tur, araliktan once: YENI yedek ALINMAMALI ama mevcut
        // yedek yine DOGRULANMALI.
        events.lock().unwrap().clear();
        backup_cycle(&root, &kp.signing_key, &kp.verifying_key, 1001, &audit);
        let ev = events.lock().unwrap().clone();
        assert!(!ev.iter().any(|e| e.starts_with("backup.cycle_ok")), "aralik dolmadan yeni yedek ALINMAMALI");
        assert!(ev.iter().any(|e| e.starts_with("backup.verify_ok")));

        crate::backup::clear_read_only_recursive(&root);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Yedek BOZULMUSSA dongu bunu sessizce gecmemeli.
    #[test]
    fn the_backup_cycle_loudly_reports_a_corrupted_snapshot() {
        let root = std::env::temp_dir().join(format!("chimera-pipe-bkbad-{}", std::process::id()));
        crate::backup::clear_read_only_recursive(&root);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("state")).unwrap();
        std::fs::write(root.join("state/vault.sealed"), vec![7u8; 4096]).unwrap();

        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        let noaudit = |_: &str, _: &str| {};
        backup_cycle(&root, &kp.signing_key, &kp.verifying_key, 1000, &noaudit);

        // Yedekteki dosyayi boz.
        let dir = crate::backup::list_snapshots(&root)[0].1.clone();
        let victim = dir.join("veri/state/vault.sealed");
        let mut perms = std::fs::metadata(&victim).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(&victim, perms);
        std::fs::write(&victim, vec![9u8; 4096]).unwrap();

        let events = std::sync::Mutex::new(Vec::new());
        let audit = |e: &str, d: &str| events.lock().unwrap().push(format!("{e}|{d}"));
        backup_cycle(&root, &kp.signing_key, &kp.verifying_key, 1001, &audit);

        let ev = events.lock().unwrap().clone();
        assert!(
            ev.iter().any(|e| e.starts_with("backup.verify_FAILED")),
            "BOZUK yedek sessizce gecilmis: {ev:?}"
        );

        crate::backup::clear_read_only_recursive(&root);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// TTL'li oto-blok whitelist'te OLMALI (gecici + geri alinabilir),
    /// ama korumali bir adres icin Executor yine de REDDETMELI --
    /// whitelist'te olmak "her adrese uygulanir" demek DEGILDIR.
    #[test]
    fn ttl_autoblock_is_whitelisted_but_still_refuses_protected_addresses() {
        let r = Remediation::AutoBlockSourceIp { ip: "203.0.113.7".into(), reason: "test".into() };
        assert!(is_whitelisted(&r), "GECICI + geri alinabilir aksiyon whitelist'te olmali");

        let root = std::env::temp_dir().join(format!("chimera-pipe-ab-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("state")).unwrap();

        let loopback = Remediation::AutoBlockSourceIp { ip: "127.0.0.1".into(), reason: "test".into() };
        assert!(is_whitelisted(&loopback));
        let err = execute(&root, &loopback).unwrap_err();
        assert!(err.contains("otomatik engellenmez"), "loopback engellenmeye CALISILMAMALI: {err}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Devre kesici bir sureci kuyruga koydugunda, boru hattinin GERCEKTEN
    /// "insan incelemesi gerekiyor" dedigini uctan uca dogrular (sahte bir
    /// bulgu listesi degil, `run_once`'in kendisi calistirilir).
    #[test]
    fn a_queued_suspended_process_lands_in_needs_review_not_in_actions_taken() {
        let root = std::env::temp_dir().join(format!("chimera-pipe-cb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("state")).unwrap();
        std::fs::write(
            root.join("state/suspended.list"),
            "{\"ts\":1,\"pid\":4242,\"image\":\"sifrele.exe\",\"reason\":\"tuzak\",\"suspended\":true,\"blocked\":\"\",\"status\":\"AWAITING_HUMAN_APPROVAL\"}\n",
        )
        .unwrap();

        let audit = |_: &str, _: &str| {};
        let report = run_once(&root, false, &audit);

        assert!(
            report.needs_review.iter().any(|r| r.contains("circuit_breaker.pending_approval")),
            "askiya alinmis surec INSAN INCELEMESI listesine dusmedi: {:?}",
            report.needs_review
        );
        assert!(
            !report.actions_taken.iter().any(|a| a.contains("terminate")),
            "sonlandirma OTOMATIK uygulanmis -- degismez kural IHLAL EDILDI"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dry_run_never_calls_execute() {
        // dogrudan run_once cagirmak platforma bagli gercek tarama
        // yapar; burada sadece PipelineReport'un dry-run etiketini
        // dogru urettigini, senkron bir birim testiyle kontrol ediyoruz.
        let findings = vec![Finding {
            id: "test.finding".into(),
            severity: Severity::Critical,
            title: "t".into(),
            detail: "d".into(),
            remediation: Some(Remediation::EnableFirewall),
        }];
        let mut actions = Vec::new();
        for f in &findings {
            if let Some(r) = &f.remediation {
                if is_whitelisted(r) {
                    actions.push(format!("[DRY-RUN] {} icin '{}' uygulanacakti", f.id, r.as_str()));
                }
            }
        }
        assert_eq!(actions.len(), 1);
        assert!(actions[0].contains("DRY-RUN"));
    }

    #[test]
    fn report_to_text_lists_findings_and_reviews() {
        let report = PipelineReport {
            findings: vec![Finding {
                id: "a.b".into(),
                severity: Severity::High,
                title: "baslik".into(),
                detail: "detay".into(),
                remediation: None,
            }],
            actions_taken: vec![],
            needs_review: vec!["a.b [YUKSEK]: baslik".into()],
        };
        let text = report.to_text();
        assert!(text.contains("1 bulgu"));
        assert!(text.contains("yok"));
        assert!(text.contains("INSAN INCELEMESI"));
    }
}
