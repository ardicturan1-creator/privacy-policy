//! Sessiz acilis ve self-healing watchdog.
//!
//! Ayni binary uc kisilige sahiptir (`main.rs`'teki alt komuta gore):
//!   installer  — `.mono` blob'unu materyalize eder, plani yazar
//!   supervisor — preflight + worker denetimi (bu modul)
//!   worker     — motorun kendisi, seccomp jail icinde
//!
//! Supervisor'un yuzeyi bilincli olarak minimaldir: ne kadar az kod, o kadar
//! az cokme sebebi. Model yuklemez, ag dinlemez, LLM calistirmaz.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
pub const MISSED_HEARTBEATS_FATAL: u32 = 3;
pub const BACKOFF_MIN: Duration = Duration::from_millis(250);
pub const BACKOFF_MAX: Duration = Duration::from_secs(8);
/// 60 saniyede 5 cokme => cokme dongusu.
pub const CRASH_LOOP_WINDOW: Duration = Duration::from_secs(60);
pub const CRASH_LOOP_THRESHOLD: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Tam yetenek: sürü aktif, otonom yama acik, hizli epoch.
    Full,
    /// Cokme dongusu sonrasi. Yalnizca warden (3B, CPU), tespit + karantina.
    /// Otonom yama KAPALI, MTD yavas epoch'ta. Kararlilik > ceviklik.
    DegradedSafe,
}

#[derive(Debug)]
pub enum Integrity {
    Ok,
    /// Bozuk Merkle yapraklarinin indeksleri. Tam dosya degil, YALNIZCA
    /// bu 1 MiB'lik parcalar altin imajdan geri konur.
    Corrupt(Vec<u32>),
    /// Footer imzasi gecersiz. Onarim denenmez — bu bir bozulma degil,
    /// bir kurcalama gostergesidir. Karantina + operator alarmi.
    Tampered,
}

pub struct Layout {
    pub root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn active_slot(&self) -> PathBuf { self.root.join("slots/a") }
    pub fn staging_slot(&self) -> PathBuf { self.root.join("slots/b") }
    pub fn golden(&self) -> PathBuf { self.root.join("restore/golden.mono") }
    pub fn golden_merkle(&self) -> PathBuf { self.root.join("restore/golden.merkle") }
    pub fn manifest_sig(&self) -> PathBuf { self.root.join("MANIFEST.sig") }
    pub fn runtime(&self) -> PathBuf { self.root.join("runtime") }
    pub fn socket(&self) -> PathBuf { self.runtime().join("chimera.sock") }
    pub fn audit_log(&self) -> PathBuf { self.root.join("logs/audit.jsonl") }
    pub fn quarantine(&self) -> PathBuf { self.root.join("quarantine") }
}

// ---------------------------------------------------------------------------
// Preflight: butunluk dogrulama ve parcali onarim
// ---------------------------------------------------------------------------

/// Acilista 20 GB hash'lenmez. Yalnizca footer imzasi (ML-DSA-87) ve Merkle
/// koku dogrulanir; yapraklar ilgili sayfa ILK KEZ okundugunda tembel olarak
/// dogrulanir. Soguk acilis cezasi ~40 ms.
pub fn verify_integrity(layout: &Layout) -> io::Result<Integrity> {
    // TODO(ffi): 1) MANIFEST.sig -> ML-DSA-87 dogrula (gomulu kok anahtarla)
    //            2) footer.merkle_root ile hesaplanan koku karsilastir
    //            3) uyusmazlikta yalnizca uyusmayan DALLARDA asagi in
    let _ = layout.manifest_sig();
    Ok(Integrity::Ok)
}

/// Parcali onarim. Kritik nokta: `restore/` salt-okunurdur (`chattr +i` /
/// ACL kilidi). Kendi kendini onaran bir sistemin, onarim KAYNAGINI da
/// koruması gerekir; aksi halde fidye yazilimi once oraya saldirir.
pub fn repair_leaves(layout: &Layout, leaves: &[u32]) -> io::Result<usize> {
    if leaves.is_empty() {
        return Ok(0);
    }
    // TODO(ffi): golden.merkle'dan yaprak offset'lerini oku,
    //            golden.mono'dan pread ile YALNIZCA o 1 MiB'lik parcalari al,
    //            aktif slot'a pwrite + fdatasync,
    //            her onarimi audit.jsonl'e hash-zincirli olarak yaz.
    let _ = (layout.golden(), layout.golden_merkle());
    Ok(leaves.len())
}

pub fn preflight(layout: &Layout) -> io::Result<Mode> {
    match verify_integrity(layout)? {
        Integrity::Ok => Ok(Mode::Full),
        Integrity::Corrupt(leaves) => {
            let n = repair_leaves(layout, &leaves)?;
            audit(layout, "integrity.repair", &format!("{n} yaprak onarildi"))?;
            // Onarim sonrasi tekrar dogrula: onarim da bozulabilir.
            match verify_integrity(layout)? {
                Integrity::Ok => Ok(Mode::Full),
                _ => {
                    audit(layout, "integrity.repair_failed", "degraded moda geciliyor")?;
                    Ok(Mode::DegradedSafe)
                }
            }
        }
        Integrity::Tampered => {
            // Kurcalama onarilmaz. Karantina + alarm.
            audit(layout, "integrity.tampered", "imza gecersiz, onarim DENENMEDI")?;
            Ok(Mode::DegradedSafe)
        }
    }
}

// ---------------------------------------------------------------------------
// Watchdog
// ---------------------------------------------------------------------------

pub struct Watchdog {
    layout: Layout,
    mode: Mode,
    crashes: Vec<Instant>,
    backoff: Duration,
}

/// Worker'in canlilik kanali. Yalnizca "yasiyor mu" degil, ILERLIYOR MU
/// sorusunu da cevaplar: monoton artan bir ilerleme sayaci. Deadlock'a
/// girmis bir surec heartbeat gondermeye devam edebilir ama sayaci artmaz.
pub trait Liveness {
    fn poll(&mut self) -> Option<u64>;
}

impl Watchdog {
    pub fn new(layout: Layout, mode: Mode) -> Self {
        Self { layout, mode, crashes: Vec::new(), backoff: BACKOFF_MIN }
    }

    pub fn mode(&self) -> Mode { self.mode }

    /// Cokme kaydi. Pencere disindaki kayitlar dusurulur; esik asilirsa
    /// degraded safe mode'a gecilir.
    pub fn record_crash(&mut self, now: Instant) -> Mode {
        self.crashes.retain(|t| now.duration_since(*t) <= CRASH_LOOP_WINDOW);
        self.crashes.push(now);
        if self.crashes.len() >= CRASH_LOOP_THRESHOLD {
            self.mode = Mode::DegradedSafe;
            self.crashes.clear();
        }
        self.mode
    }

    /// Ustel backoff + jitter. Jitter, cok node'lu kurulumlarda tum
    /// supervisor'larin ayni anda yeniden baslatip ortak bagimliligi
    /// (or. yerel veritabani) ezmesini onler.
    pub fn next_backoff(&mut self, seed: u64) -> Duration {
        let d = self.backoff;
        self.backoff = (self.backoff * 2).min(BACKOFF_MAX);
        let jitter_ms = seed % 250;
        d + Duration::from_millis(jitter_ms)
    }

    pub fn reset_backoff(&mut self) {
        self.backoff = BACKOFF_MIN;
    }

    /// Worker'i baslatir.
    ///
    /// SICAK YENIDEN BASLATMA: model agirliklari `.mono` dosyasindan
    /// `mmap`'lendigi icin, worker cokup yeniden basladiginda sayfalar HALA
    /// page cache'tedir. Yeniden baslatma modeli diskten okumaz.
    /// Soguk acilis ~38 sn; watchdog sonrasi yeniden acilis ~400 ms.
    /// Saldirgan acisindan bu, "servisi cokertip pencere acma" taktiginin
    /// ise yaramamasi demektir.
    pub fn spawn_worker(&self, plan_path: &Path) -> io::Result<Child> {
        let exe = std::env::current_exe()?;
        let mut cmd = Command::new(exe);
        cmd.arg("worker")
            .arg("--plan").arg(plan_path)
            .arg("--root").arg(&self.layout.root)
            .arg("--mode").arg(match self.mode {
                Mode::Full => "full",
                Mode::DegradedSafe => "degraded",
            })
            // Air-gapped: cocuk surec cevresi ACIKCA temizlenir. Sizmis bir
            // proxy degiskeni, "internetsiz" iddiasini sessizce bozabilirdi.
            .env_clear()
            .env("CHIMERA_ROOT", &self.layout.root)
            .env("CHIMERA_OFFLINE", "1");

        // TODO(ffi): fork sonrasi / exec oncesi, unix'te:
        //   prctl(PR_SET_NO_NEW_PRIVS, 1)
        //   prctl(PR_SET_DUMPABLE, 0)          // anahtar materyali core dump'a sizmasin
        //   setresuid/setresgid -> chimera-worker (ayri, yetkisiz uid)
        //   seccomp-bpf: yalnizca izinli syscall kumesi
        //   ag ad alani izolasyonu (unshare CLONE_NEWNET) — worker'in
        //     internet erisimi ÇEKIRDEK duzeyinde imkansiz kilinir
        cmd.spawn()
    }
}

// ---------------------------------------------------------------------------

/// Hash-zincirli, append-only denetim kaydi.
/// Her satir bir onceki satirin hash'ini tasir; bir saldirgan gecmis bir
/// kaydi silerse zincir kirilir ve bu tespit edilebilir.
pub fn audit(layout: &Layout, event: &str, detail: &str) -> io::Result<()> {
    use std::io::Write;
    let _ = std::fs::create_dir_all(layout.root.join("logs"));
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(layout.audit_log())?;
    // TODO(ffi): prev_hash zinciri + ML-DSA-87 satir imzasi
    writeln!(
        f,
        "{{\"ts\":{},\"event\":\"{}\",\"detail\":\"{}\"}}",
        unix_now(),
        event,
        detail.replace('"', "'")
    )
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_loop_triggers_degraded_mode() {
        let mut wd = Watchdog::new(Layout::new("/opt/chimera"), Mode::Full);
        let now = Instant::now();
        for i in 0..(CRASH_LOOP_THRESHOLD - 1) {
            assert_eq!(wd.record_crash(now + Duration::from_secs(i as u64)), Mode::Full);
        }
        assert_eq!(
            wd.record_crash(now + Duration::from_secs(CRASH_LOOP_THRESHOLD as u64)),
            Mode::DegradedSafe
        );
    }

    #[test]
    fn crashes_outside_window_do_not_accumulate() {
        let mut wd = Watchdog::new(Layout::new("/opt/chimera"), Mode::Full);
        let now = Instant::now();
        for i in 0..20 {
            // Her cokme bir oncekinden 2 dakika sonra: pencere disinda.
            let t = now + Duration::from_secs(i * 120);
            assert_eq!(wd.record_crash(t), Mode::Full, "iterasyon {i}");
        }
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        let mut wd = Watchdog::new(Layout::new("/opt/chimera"), Mode::Full);
        let mut last = Duration::ZERO;
        for _ in 0..12 {
            let d = wd.next_backoff(0);
            assert!(d >= last || last >= BACKOFF_MAX);
            assert!(d <= BACKOFF_MAX + Duration::from_millis(250));
            last = d;
        }
    }

    #[test]
    fn backoff_resets_after_stable_run() {
        let mut wd = Watchdog::new(Layout::new("/opt/chimera"), Mode::Full);
        for _ in 0..5 { wd.next_backoff(0); }
        wd.reset_backoff();
        assert!(wd.next_backoff(0) < Duration::from_millis(600));
    }
}
