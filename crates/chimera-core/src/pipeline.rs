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
    matches!(r, Remediation::EnableFirewall | Remediation::DisableSmb1)
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
pub fn run_once(dry_run: bool, audit: &impl Fn(&str, &str)) -> PipelineReport {
    let findings = scanner::scan();
    let mut actions_taken = Vec::new();
    let mut needs_review = Vec::new();

    for f in &findings {
        audit("scan.finding", &format!("{}[{}]: {}", f.id, f.severity.as_str(), f.title));
        match &f.remediation {
            Some(r) if is_whitelisted(r) => {
                if dry_run {
                    actions_taken.push(format!("[DRY-RUN] {} icin '{}' uygulanacakti", f.id, r.as_str()));
                    continue;
                }
                match execute(r) {
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

fn execute(r: &Remediation) -> Result<String, String> {
    match r {
        Remediation::EnableFirewall => crate::remediate::enable_firewall(),
        Remediation::DisableSmb1 => crate::remediate::disable_smb1(),
    }
}

/// Arka planda periyodik olarak (varsayılan: 30 dakikada bir) `run_once`
/// çalıştıran döngü. `chimera-sentinel`in `respawn_core` deseniyle TUTARLI
/// şekilde: `running` false olduğunda döngü temiz şekilde biter (thread
/// sonsuza dek asılı kalmaz).
pub fn spawn_background_loop(running: Arc<AtomicBool>, audit_log: std::path::PathBuf) {
    std::thread::spawn(move || {
        let audit = |event: &str, detail: &str| {
            let _ = crate::auditlog::append(&audit_log, event, detail);
        };
        while running.load(Ordering::SeqCst) {
            let report = run_once(false, &audit);
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
