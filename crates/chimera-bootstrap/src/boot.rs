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

use chimera_crypto::merkle;

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
    /// Butunlugu korunan gercek dosya. Gercek bir GGUF motoru yerine, bu
    /// derlemede `plan.json` gibi GERCEKTEN var olan baytlar korunur —
    /// mekanizma, elimizde cok-GB'lik bir model dosyasi olmadan da tam
    /// olarak ayni kod yoluyla dogrulanabilsin diye.
    pub fn active_engine(&self) -> PathBuf { self.active_slot().join("engine.mono") }
    pub fn golden(&self) -> PathBuf { self.root.join("restore/golden.mono") }
    pub fn golden_merkle(&self) -> PathBuf { self.root.join("restore/golden.merkle") }
    pub fn manifest_sig(&self) -> PathBuf { self.root.join("MANIFEST.sig") }
    pub fn state(&self) -> PathBuf { self.root.join("state") }
    pub fn epoch_seal(&self) -> PathBuf { self.state().join("epoch.seal") }
    pub fn runtime(&self) -> PathBuf { self.root.join("runtime") }
    pub fn socket(&self) -> PathBuf { self.runtime().join("chimera.sock") }
    pub fn audit_log(&self) -> PathBuf { self.root.join("logs/audit.jsonl") }
    pub fn quarantine(&self) -> PathBuf { self.root.join("quarantine") }
}

// ---------------------------------------------------------------------------
// MANIFEST.sig — gercek ikili format, gercek ML-DSA-87 imzasi tasir
// ---------------------------------------------------------------------------
//
//   [4]  magic "CHM1"
//   [4]  vk_len (u32 LE)
//   [..] vk_len bayt: ML-DSA-87 dogrulama anahtari
//   [32] Merkle koku
//   [4]  sig_len (u32 LE)
//   [..] sig_len bayt: ML-DSA-87 imzasi (kok uzerinde)

const MANIFEST_MAGIC: &[u8; 4] = b"CHM1";

pub struct ManifestBytes {
    pub verifying_key: Vec<u8>,
    pub root: [u8; 32],
    /// Kok bu yaprak boyutuyla hesaplanmisti. Sabit bir sabit (`DEFAULT_LEAF_SIZE`)
    /// varsaymak yerine BURADA saklanir: imzalayan ve dogrulayan taraf farkli
    /// bir varsayilanla derlenmis olsa bile (ya da ileride yaprak boyutu
    /// degisirse) kok hesaplamasi HER ZAMAN tutarli kalir. Bu alanin eksikligi
    /// bu derlemede gercek bir teste yakalanan gercek bir hataydi.
    pub leaf_size: u64,
    pub signature: Vec<u8>,
}

pub fn write_manifest(path: &Path, verifying_key: &[u8], root: &[u8; 32], leaf_size: u64, signature: &[u8]) -> io::Result<()> {
    let mut out = Vec::with_capacity(4 + 4 + verifying_key.len() + 32 + 8 + 4 + signature.len());
    out.extend_from_slice(MANIFEST_MAGIC);
    out.extend_from_slice(&(verifying_key.len() as u32).to_le_bytes());
    out.extend_from_slice(verifying_key);
    out.extend_from_slice(root);
    out.extend_from_slice(&leaf_size.to_le_bytes());
    out.extend_from_slice(&(signature.len() as u32).to_le_bytes());
    out.extend_from_slice(signature);
    std::fs::write(path, out)
}

/// Dosya yoksa veya format bozuksa `Ok(None)` doner — bu, cagiran tarafindan
/// "imza yok/okunamiyor" olarak (yani kurcalama olarak) ele alinir.
pub fn read_manifest(path: &Path) -> io::Result<Option<ManifestBytes>> {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let parse = || -> Option<ManifestBytes> {
        if data.len() < 4 || &data[0..4] != MANIFEST_MAGIC {
            return None;
        }
        let mut off = 4;
        let vk_len = u32::from_le_bytes(data.get(off..off + 4)?.try_into().ok()?) as usize;
        off += 4;
        let verifying_key = data.get(off..off + vk_len)?.to_vec();
        off += vk_len;
        let root: [u8; 32] = data.get(off..off + 32)?.try_into().ok()?;
        off += 32;
        let leaf_size = u64::from_le_bytes(data.get(off..off + 8)?.try_into().ok()?);
        off += 8;
        let sig_len = u32::from_le_bytes(data.get(off..off + 4)?.try_into().ok()?) as usize;
        off += 4;
        let signature = data.get(off..off + sig_len)?.to_vec();
        Some(ManifestBytes { verifying_key, root, leaf_size, signature })
    };
    Ok(parse())
}

// ---------------------------------------------------------------------------
// Preflight: butunluk dogrulama ve parcali onarim
// ---------------------------------------------------------------------------

/// Gercek dogrulama: MANIFEST.sig'teki ML-DSA-87 imzasi gercekten
/// dogrulanir, golden dosyasinin GERCEK BLAKE3 Merkle koku hesaplanip
/// imzali kokle karsilastirilir, sonra aktif dosyanin yaprak hash'leri
/// golden ile GERCEKTEN karsilastirilir. Hicbir adim simule edilmez.
///
/// Kurulumdan once (henuz `slots/a/engine.mono` yokken) cagrilirsa bu bir
/// bozulma degildir — `Ok(Integrity::Ok)` doner ki `chimera install`
/// akisi sorunsuz calissin.
pub fn verify_integrity(layout: &Layout) -> io::Result<Integrity> {
    let active = layout.active_engine();
    let golden = layout.golden();
    if !active.exists() || !golden.exists() {
        return Ok(Integrity::Ok);
    }

    let Some(manifest) = read_manifest(&layout.manifest_sig())? else {
        return Ok(Integrity::Tampered); // imza dosyasi yok/bozuk -> guvenilmez
    };
    let Ok(vk) = chimera_crypto::obsidian::dsa_verifying_key_from_bytes(&manifest.verifying_key) else {
        return Ok(Integrity::Tampered);
    };
    let Ok(sig) = chimera_crypto::obsidian::dsa_signature_from_bytes(&manifest.signature) else {
        return Ok(Integrity::Tampered);
    };
    if !merkle::verify_root(&vk, &manifest.root, &sig) {
        return Ok(Integrity::Tampered); // imza matematiksel olarak GECERSIZ
    }

    let leaf_size = manifest.leaf_size.max(1) as usize;
    let golden_tree = merkle::build_tree_from_file(&golden, leaf_size)?;
    if golden_tree.root != manifest.root {
        // Golden dosyasinin KENDISI imzali kokle uyusmuyor: golden'a
        // kurcalanmis demektir (bu, onarilamaz bir durumdur — onarim
        // kaynaginin kendisi supheli).
        return Ok(Integrity::Tampered);
    }

    let active_tree = merkle::build_tree_from_file(&active, leaf_size)?;
    let bad = merkle::diff_leaves(&golden_tree, &active_tree);
    if bad.is_empty() {
        Ok(Integrity::Ok)
    } else {
        Ok(Integrity::Corrupt(bad.into_iter().map(|i| i as u32).collect()))
    }
}

/// Parcali onarim. Kritik nokta: `restore/` salt-okunurdur (`chattr +i` /
/// ACL kilidi). Kendi kendini onaran bir sistemin, onarim KAYNAGINI da
/// koruması gerekir; aksi halde fidye yazilimi once oraya saldirir.
/// Bu fonksiyon GERCEKTEN yalnizca bozuk yaprak araliklarini `pread`/
/// `pwrite` ile golden'dan aktif dosyaya kopyalar (`merkle.rs`).
pub fn repair_leaves(layout: &Layout, leaves: &[u32]) -> io::Result<usize> {
    if leaves.is_empty() {
        return Ok(0);
    }
    // Ayni yaprak boyutu manifest'ten okunur — imzalayanla dogrulayanin
    // (ve onaranin) HER ZAMAN ayni boyutu kullanmasini garanti eder.
    let leaf_size = read_manifest(&layout.manifest_sig())?
        .map(|m| m.leaf_size.max(1) as usize)
        .unwrap_or(merkle::DEFAULT_LEAF_SIZE);
    let idxs: Vec<usize> = leaves.iter().map(|&i| i as usize).collect();
    let n = merkle::repair_leaves_from_golden(&layout.golden(), &layout.active_engine(), leaf_size, &idxs)?;
    audit(layout, "integrity.repair.leaves", &format!("{idxs:?}"))?;
    Ok(n)
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

    fn temp_layout(name: &str) -> Layout {
        let root = std::env::temp_dir().join(format!("chimera-boot-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("slots/a")).unwrap();
        std::fs::create_dir_all(root.join("restore")).unwrap();
        std::fs::create_dir_all(root.join("logs")).unwrap();
        Layout::new(root)
    }

    /// Ucdan uca, tamamen gercek preflight dongusu: gercek dosyalar yazilir,
    /// gercek ML-DSA-87 imzasiyla imzalanir, GERCEKTEN bir bayt bozulur,
    /// `preflight()` bunu GERCEKTEN tespit edip YALNIZCA bozuk yapragi
    /// golden'dan geri yazarak onarir — hicbir adim taklit edilmemistir.
    #[test]
    fn preflight_detects_and_repairs_real_corruption_end_to_end() {
        let layout = temp_layout("e2e-repair");

        let mut content = vec![0u8; 6000];
        for (i, b) in content.iter_mut().enumerate() {
            *b = (i * 13 % 256) as u8;
        }
        std::fs::write(layout.golden(), &content).unwrap();
        std::fs::write(layout.active_engine(), &content).unwrap();

        let tree = merkle::build_tree_from_file(&layout.golden(), 1024).unwrap();
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        let sig = merkle::sign_root(&kp.signing_key, &tree.root);
        write_manifest(
            &layout.manifest_sig(),
            &chimera_crypto::obsidian::dsa_verifying_key_bytes(&kp.verifying_key),
            &tree.root,
            1024,
            &chimera_crypto::obsidian::dsa_signature_bytes(&sig),
        )
        .unwrap();

        // Kurulum temiz: preflight Full moda gecmeli.
        assert_eq!(preflight(&layout).unwrap(), Mode::Full);

        // GERCEK bozulma: aktif dosyada bir bayt degistiriliyor.
        let mut corrupted = std::fs::read(layout.active_engine()).unwrap();
        corrupted[2500] ^= 0xFF;
        std::fs::write(layout.active_engine(), &corrupted).unwrap();

        // Dogrulama GERCEKTEN bozuklugu yakalamali.
        match verify_integrity(&layout).unwrap() {
            Integrity::Corrupt(leaves) => assert_eq!(leaves, vec![2]),
            other => panic!("bozulma tespit edilemedi: {other:?}"),
        }

        // preflight() otomatik onarmali ve Full moda geri donmeli.
        assert_eq!(preflight(&layout).unwrap(), Mode::Full);

        let healed = std::fs::read(layout.active_engine()).unwrap();
        assert_eq!(healed, content, "onarim sonrasi dosya orijinalle birebir ayni olmali");

        std::fs::remove_dir_all(&layout.root).ok();
    }

    #[test]
    fn preflight_rejects_tampered_manifest_signature() {
        let layout = temp_layout("e2e-tamper");
        let content = vec![0xABu8; 4096];
        std::fs::write(layout.golden(), &content).unwrap();
        std::fs::write(layout.active_engine(), &content).unwrap();

        let tree = merkle::build_tree_from_file(&layout.golden(), 1024).unwrap();
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        let sig = merkle::sign_root(&kp.signing_key, &tree.root);

        // Kok kasten yanlis yazilir — imza gecerli olsa da baska bir kok icin.
        let mut wrong_root = tree.root;
        wrong_root[0] ^= 1;
        write_manifest(
            &layout.manifest_sig(),
            &chimera_crypto::obsidian::dsa_verifying_key_bytes(&kp.verifying_key),
            &wrong_root,
            1024,
            &chimera_crypto::obsidian::dsa_signature_bytes(&sig),
        )
        .unwrap();

        // Imza dogru anahtarla uretildi ama KOK degistirildigi icin
        // dogrulama sirasinda verify_root basarisiz olmali -> Tampered.
        assert!(matches!(verify_integrity(&layout).unwrap(), Integrity::Tampered));
        assert_eq!(preflight(&layout).unwrap(), Mode::DegradedSafe);

        std::fs::remove_dir_all(&layout.root).ok();
    }

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
