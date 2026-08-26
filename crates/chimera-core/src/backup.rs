//! backup.rs — Kanıta-dayanıklı, İMZALI, periyodik anlık görüntüler.
//!
//! Fidye yazılımına karşı gerçek son savunma hattı yedektir. Ama
//! **doğrulanmamış bir yedek, yedek değildir**: saldırgan yedeği sessizce
//! bozmuşsa, geri yükleme anına kadar bunu kimse fark etmez. Bu yüzden bu
//! modül, projede zaten var olan iki gerçek mekanizmayı yeniden kullanır
//! (yenisini icat etmez):
//!
//!   - **`chimera_crypto::merkle`** — her dosya için gerçek BLAKE3 Merkle
//!     ağacı. Bozulmanın hangi 1 MiB'lik parçada olduğu tespit edilebilir.
//!   - **`chimera_crypto::merkle::sign_root`** — manifestonun kökü,
//!     core'un ML-DSA-87 (post-kuantum) kimlik anahtarıyla İMZALANIR.
//!     Yani saldırgan hem dosyayı hem manifestoyu değiştirse bile, core'un
//!     özel anahtarı olmadan geçerli bir imza ÜRETEMEZ.
//!
//! ## "Immutable" ve "offsite" sözcüklerinin DÜRÜST karşılığı
//!
//! Bu iki sözcük pazarlama metinlerinde çok kolay kullanılır; burada tam
//! olarak ne sağlanıp ne sağlanmadığı yazılıdır:
//!
//! | İddia | Bu modülde GERÇEKTE olan |
//! |---|---|
//! | Değiştirilemez (immutable) | Anlık görüntü dizini **zaman damgalıdır ve asla üzerine yazılmaz**; dosyalar salt-okunur işaretlenir; ve her şey **imzalıdır**, yani sessiz değişiklik TESPİT EDİLİR. |
//! | | **Sağlanmayan:** yerel yönetici haklarına sahip bir saldırgan salt-okunur bayrağını kaldırıp dosyayı silebilir. GERÇEK değiştirilemezlik, WORM/object-lock destekli bir depolama (S3 Object Lock, kaset, donanım WORM) gerektirir — bu ortamda YOKTUR. Bu modülün verdiği garanti **"silinemez" değil, "sessizce bozulamaz"dır.** |
//! | Offsite (uzak konum) | Hedef dizin operatör tarafından `CHIMERA_BACKUP_DIR` ile verilir; bir ağ paylaşımı veya çıkarılabilir disk bağlama noktası olabilir. |
//! | | **Sağlanmayan:** bu modül bir ağ protokolü KONUŞMAZ — S3/SFTP/rsync istemcisi YAZILMAMIŞTIR. "Offsite" ancak operatörün verdiği yol gerçekten başka bir makinedeyse gerçektir. Kod bunu doğrulayamaz ve doğruladığını İDDİA ETMEZ. |
//!
//! ## Kapsam
//!
//! Varsayılan olarak `state/` yedeklenir (kimlik, güven listeleri, mühürlü
//! kasa, devre kesici kuyruğu). Operatör `CHIMERA_BACKUP_INCLUDE` ile
//! kullanıcı verisi dizinlerini de (`;` ile ayırarak) ekleyebilir.

use chimera_crypto::merkle;
use std::path::{Path, PathBuf};

/// Anlık görüntüler arası varsayılan süre.
pub const DEFAULT_INTERVAL_SECS: u64 = 6 * 3600;
/// Diskte tutulacak en fazla anlık görüntü sayısı (eskiler budanır).
pub const DEFAULT_KEEP: usize = 8;
/// Manifesto satır formatı sürümü — ileride format değişirse eski
/// yedeklerin YANLIŞ okunmasını önler.
const MANIFEST_VERSION: u32 = 1;

/// Tek bir yedeklenmiş dosyanın kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Kaynak köke göreli yol (dizin ayracı her zaman '/').
    pub rel_path: String,
    pub size: u64,
    /// Dosyanın Merkle kökü (BLAKE3).
    pub root_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub created_at: u64,
    pub entries: Vec<FileEntry>,
    /// TÜM dosya köklerinden türetilen tek bir üst kök — imzalanan değer.
    pub manifest_root_hex: String,
}

impl Manifest {
    pub fn to_text(&self) -> String {
        let mut s = format!("version={}\ncreated_at={}\n", self.version, self.created_at);
        for e in &self.entries {
            s.push_str(&format!("file\t{}\t{}\t{}\n", e.rel_path, e.size, e.root_hex));
        }
        s.push_str(&format!("manifest_root={}\n", self.manifest_root_hex));
        s
    }

    pub fn from_text(text: &str) -> Option<Manifest> {
        let mut version = 0u32;
        let mut created_at = 0u64;
        let mut entries = Vec::new();
        let mut manifest_root_hex = String::new();
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("version=") {
                version = v.trim().parse().ok()?;
            } else if let Some(v) = line.strip_prefix("created_at=") {
                created_at = v.trim().parse().ok()?;
            } else if let Some(v) = line.strip_prefix("manifest_root=") {
                manifest_root_hex = v.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("file\t") {
                let mut parts = rest.split('\t');
                let rel_path = parts.next()?.to_string();
                let size = parts.next()?.parse().ok()?;
                let root_hex = parts.next()?.to_string();
                entries.push(FileEntry { rel_path, size, root_hex });
            }
        }
        if manifest_root_hex.is_empty() {
            return None;
        }
        Some(Manifest { version, created_at, entries, manifest_root_hex })
    }
}

/// Tüm dosya köklerinden tek bir üst kök türetir. Sıra ÖNEMLİDİR ve
/// `rel_path`'e göre kararlı biçimde sıralanır — aksi halde aynı içerik
/// farklı bir kök üretir ve doğrulama rastgele başarısız olurdu.
fn manifest_root(entries: &[FileEntry]) -> [u8; 32] {
    let mut sorted: Vec<&FileEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    let mut hasher = blake3::Hasher::new();
    for e in sorted {
        hasher.update(e.rel_path.as_bytes());
        hasher.update(&e.size.to_le_bytes());
        hasher.update(e.root_hex.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Bir dizindeki TÜM dosyaları (özyinelemeli) göreli yollarıyla toplar.
/// Sembolik bağlantılar bilinçli olarak İZLENMEZ: bir saldırganın
/// yedekleyiciyi `C:\` gibi bir yere yönlendirip diski doldurmasını
/// (ve döngüye sokmasını) önler.
fn collect_files(base: &Path, prefix: &str, out: &mut Vec<(PathBuf, String)>) {
    let Ok(rd) = std::fs::read_dir(base) else { return };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        if ft.is_dir() {
            collect_files(&entry.path(), &rel, out);
        } else if ft.is_file() {
            out.push((entry.path(), rel));
        }
    }
}

/// Yedeklenecek kaynak dizinleri belirler: her zaman `state/`, artı
/// operatörün `CHIMERA_BACKUP_INCLUDE` ile verdiği yollar.
pub fn sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut out = vec![(root.join("state"), "state".to_string())];
    if let Ok(extra) = std::env::var("CHIMERA_BACKUP_INCLUDE") {
        for (i, p) in extra.split(';').map(str::trim).filter(|p| !p.is_empty()).enumerate() {
            // Etiket, manifestoda cakismasin diye sirali ve sabittir.
            out.push((PathBuf::from(p), format!("kullanici{i}")));
        }
    }
    out
}

/// Yedeklerin yazılacağı kök dizin. Operatör `CHIMERA_BACKUP_DIR` ile
/// başka bir makineye bağlı bir yolu gösterebilir (bkz. modül başlığındaki
/// "offsite" notu).
pub fn backup_root(root: &Path) -> PathBuf {
    std::env::var("CHIMERA_BACKUP_DIR").map(PathBuf::from).unwrap_or_else(|_| root.join("backups"))
}

/// Yeni bir anlık görüntü alır, manifestoyu imzalar ve diske yazar.
pub fn snapshot(
    root: &Path,
    signing_key: &ml_dsa::SigningKey<ml_dsa::MlDsa87>,
    now: u64,
    audit: &impl Fn(&str, &str),
) -> Result<String, String> {
    let dest_root = backup_root(root);
    // Zaman damgali dizin: MEVCUT bir anlik goruntunun uzerine ASLA
    // yazilmaz. Ayni saniyede ikinci bir cagri gelirse hata doner --
    // sessizce ustune yazmaktansa acikca reddetmek dogrudur.
    let dir = dest_root.join(format!("snapshot-{now}"));
    if dir.exists() {
        return Err(format!("bu zaman damgasiyla bir anlik goruntu ZATEN VAR: {} (uzerine YAZILMAZ)", dir.display()));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("yedek dizini olusturulamadi ({}): {e}", dir.display()))?;

    let mut entries = Vec::new();
    let mut copied = 0usize;
    let mut skipped = Vec::new();

    for (src_base, label) in sources(root) {
        if !src_base.is_dir() {
            skipped.push(format!("{} (dizin yok)", src_base.display()));
            continue;
        }
        let mut files = Vec::new();
        collect_files(&src_base, "", &mut files);
        for (abs, rel) in files {
            let rel_path = format!("{label}/{rel}");
            let tree = match merkle::build_tree_from_file(&abs, merkle::DEFAULT_LEAF_SIZE) {
                Ok(t) => t,
                Err(e) => {
                    // Okunamayan bir dosya SESSIZCE atlanmaz: manifestoda
                    // yoksa, geri yuklemede eksik oldugu fark edilmezdi.
                    skipped.push(format!("{rel_path} (okunamadi: {e})"));
                    continue;
                }
            };
            let size = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
            let target = dir.join("veri").join(&rel_path);
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::copy(&abs, &target) {
                skipped.push(format!("{rel_path} (kopyalanamadi: {e})"));
                continue;
            }
            mark_read_only(&target);
            entries.push(FileEntry { rel_path, size, root_hex: hex(&tree.root) });
            copied += 1;
        }
    }

    let mroot = manifest_root(&entries);
    let manifest = Manifest {
        version: MANIFEST_VERSION,
        created_at: now,
        entries,
        manifest_root_hex: hex(&mroot),
    };

    std::fs::write(dir.join("manifest.txt"), manifest.to_text())
        .map_err(|e| format!("manifesto yazilamadi: {e}"))?;

    // ASIL guvence: manifestonun kokunu core'un ML-DSA-87 anahtariyla
    // IMZALA. Bu imza olmadan, manifesto da dosyalar da birlikte
    // degistirilebilirdi.
    let sig = merkle::sign_root(signing_key, &mroot);
    let sig_bytes = chimera_crypto::obsidian::dsa_signature_bytes(&sig);
    std::fs::write(dir.join("manifest.sig"), hex(&sig_bytes))
        .map_err(|e| format!("imza yazilamadi: {e}"))?;

    mark_read_only(&dir.join("manifest.txt"));
    mark_read_only(&dir.join("manifest.sig"));

    audit(
        "backup.snapshot",
        &format!("{} dosya yedeklendi, {} atlandi, kok={} dizin={}", copied, skipped.len(), &manifest.manifest_root_hex[..16], dir.display()),
    );
    for s in &skipped {
        audit("backup.skipped", s);
    }

    let mut msg = format!(
        "Anlik goruntu alindi: {} dosya, imzalandi (kok {}...), dizin: {}",
        copied,
        &manifest.manifest_root_hex[..16],
        dir.display()
    );
    if !skipped.is_empty() {
        msg.push_str(&format!("\nATLANAN {} oge: {}", skipped.len(), skipped.join("; ")));
    }
    Ok(msg)
}

/// Dosyayı salt-okunur işaretler. Bunun **yerel yöneticiyi durdurmadığı**
/// modül başlığında açıkça yazılıdır; amaç kazara değişikliği ve
/// ayrıcalıksız bir sürecin yazmasını engellemektir.
fn mark_read_only(p: &Path) {
    if let Ok(md) = std::fs::metadata(p) {
        let mut perms = md.permissions();
        perms.set_readonly(true);
        let _ = std::fs::set_permissions(p, perms);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// İmza ve tüm dosya kökleri doğrulandı.
    Intact { files: usize },
    /// İmza geçersiz — manifesto değiştirilmiş (veya farklı bir anahtarla
    /// imzalanmış).
    SignatureInvalid,
    /// İmza geçerli ama bazı dosyalar manifestoyla uyuşmuyor.
    FilesCorrupted { bad: Vec<String> },
    /// Yedek okunamadı/eksik.
    Unreadable { reason: String },
}

impl VerifyOutcome {
    pub fn to_text(&self) -> String {
        match self {
            VerifyOutcome::Intact { files } => format!("SAGLAM: imza gecerli, {files} dosyanin tamami manifestoyla uyusuyor"),
            VerifyOutcome::SignatureInvalid => {
                "IMZA GECERSIZ: manifesto DEGISTIRILMIS olabilir. Bu yedek GUVENILMEZ.".to_string()
            }
            VerifyOutcome::FilesCorrupted { bad } => format!(
                "BOZULMA TESPIT EDILDI: imza gecerli ama {} dosya manifestoyla UYUSMUYOR: {}",
                bad.len(),
                bad.join(", ")
            ),
            VerifyOutcome::Unreadable { reason } => format!("DOGRULANAMADI: {reason}"),
        }
    }

    pub fn is_intact(&self) -> bool {
        matches!(self, VerifyOutcome::Intact { .. })
    }
}

/// Bir anlık görüntüyü baştan sona doğrular: önce İMZA, sonra her dosyanın
/// Merkle kökü. Sıra önemlidir — imza geçersizse manifestodaki köklere
/// güvenmenin bir anlamı yoktur.
pub fn verify_snapshot(dir: &Path, verifying_key: &ml_dsa::VerifyingKey<ml_dsa::MlDsa87>) -> VerifyOutcome {
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.txt")) else {
        return VerifyOutcome::Unreadable { reason: format!("manifest.txt okunamadi ({})", dir.display()) };
    };
    let Some(manifest) = Manifest::from_text(&text) else {
        return VerifyOutcome::Unreadable { reason: "manifest.txt ayristirilamadi".into() };
    };
    let Ok(sig_hex) = std::fs::read_to_string(dir.join("manifest.sig")) else {
        return VerifyOutcome::Unreadable { reason: "manifest.sig okunamadi".into() };
    };

    // 1) Manifestonun ICERIGI ile iddia ettigi kok tutarli mi?
    let recomputed = manifest_root(&manifest.entries);
    let Some(claimed) = unhex32(&manifest.manifest_root_hex) else {
        return VerifyOutcome::Unreadable { reason: "manifest_root cozulemedi".into() };
    };
    if recomputed != claimed {
        // Manifesto kendi icinde tutarsiz: satirlar degistirilmis ama kok
        // guncellenmemis. Imzayi kontrol etmeye bile gerek yok.
        return VerifyOutcome::SignatureInvalid;
    }

    // 2) Bu kok GERCEKTEN core tarafindan mi imzalanmis?
    let Ok(sig_bytes) = unhex_vec(sig_hex.trim()) else {
        return VerifyOutcome::Unreadable { reason: "imza hex cozulemedi".into() };
    };
    let Ok(sig) = chimera_crypto::obsidian::dsa_signature_from_bytes(&sig_bytes) else {
        return VerifyOutcome::SignatureInvalid;
    };
    if !merkle::verify_root(verifying_key, &claimed, &sig) {
        return VerifyOutcome::SignatureInvalid;
    }

    // 3) Diskteki dosyalar manifestoyla uyusuyor mu?
    let mut bad = Vec::new();
    for e in &manifest.entries {
        let p = dir.join("veri").join(&e.rel_path);
        match merkle::build_tree_from_file(&p, merkle::DEFAULT_LEAF_SIZE) {
            Ok(t) if hex(&t.root) == e.root_hex => {}
            Ok(_) => bad.push(e.rel_path.clone()),
            Err(_) => bad.push(format!("{} (eksik/okunamadi)", e.rel_path)),
        }
    }
    if bad.is_empty() {
        VerifyOutcome::Intact { files: manifest.entries.len() }
    } else {
        VerifyOutcome::FilesCorrupted { bad }
    }
}

fn unhex_vec(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ())).collect()
}

/// Diskteki anlık görüntüleri ESKİDEN YENİYE sıralı olarak döner.
pub fn list_snapshots(root: &Path) -> Vec<(u64, PathBuf)> {
    let base = backup_root(root);
    let Ok(rd) = std::fs::read_dir(&base) else { return Vec::new() };
    let mut out: Vec<(u64, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ts: u64 = name.strip_prefix("snapshot-")?.parse().ok()?;
            Some((ts, e.path()))
        })
        .collect();
    out.sort_by_key(|(ts, _)| *ts);
    out
}

/// En yeni `keep` anlık görüntüyü bırakıp eskileri siler. Silinen her
/// görüntü denetim kaydına yazılır — yedek silme, sessizce yapılmaması
/// gereken bir iştir.
pub fn prune(root: &Path, keep: usize, audit: &impl Fn(&str, &str)) -> usize {
    let snaps = list_snapshots(root);
    if snaps.len() <= keep {
        return 0;
    }
    let mut removed = 0;
    for (ts, path) in &snaps[..snaps.len() - keep] {
        // Salt-okunur isaretledigimiz dosyalar Windows'ta silinmeyi
        // engelleyebilir; once isareti kaldiriyoruz.
        clear_read_only_recursive(path);
        match std::fs::remove_dir_all(path) {
            Ok(()) => {
                audit("backup.pruned", &format!("eski anlik goruntu silindi: snapshot-{ts}"));
                removed += 1;
            }
            Err(e) => audit("backup.prune_failed", &format!("snapshot-{ts}: {e}")),
        }
    }
    removed
}

/// Bir ağacın salt-okunur işaretlerini özyinelemeli olarak kaldırır.
///
/// `pub(crate)`'tir çünkü **testler de buna ihtiyaç duyar**: Windows'ta
/// `remove_dir_all`, salt-okunur bir dosyada BAŞARISIZ olur (Linux'ta
/// yazılabilir bir dizindeki salt-okunur dosya silinebilir). Bu fark,
/// yedek dizinlerini temizleyen testlerin Linux'ta geçip Wine altında
/// başarısız olmasına yol açtı — gerçek bir platform farkıdır ve
/// testlerin bunu tek tek yeniden uygulaması yerine burada TEK bir
/// doğru kaynaktan çözülür.
pub(crate) fn clear_read_only_recursive(p: &Path) {
    if let Ok(md) = std::fs::metadata(p) {
        let mut perms = md.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(p, perms);
    }
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            clear_read_only_recursive(&e.path());
        }
    }
}

/// En yeni anlık görüntünün yaşı (saniye). Hiç yoksa `None`.
pub fn age_of_newest(root: &Path, now: u64) -> Option<u64> {
    list_snapshots(root).last().map(|(ts, _)| now.saturating_sub(*ts))
}

pub fn list_text(root: &Path, now: u64) -> String {
    let snaps = list_snapshots(root);
    if snaps.is_empty() {
        return format!("HIC ANLIK GORUNTU YOK (aranan dizin: {})", backup_root(root).display());
    }
    let mut s = format!("{} ANLIK GORUNTU (yeniden eskiye):\n", snaps.len());
    for (ts, path) in snaps.iter().rev() {
        let yas = now.saturating_sub(*ts);
        s.push_str(&format!("  snapshot-{ts}  ({} saat once)  {}\n", yas / 3600, path.display()));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("chimera-backup-{name}-{}", std::process::id()));
        let _ = clear_and_remove(&p);
        std::fs::create_dir_all(p.join("state")).unwrap();
        p
    }

    fn clear_and_remove(p: &Path) -> std::io::Result<()> {
        clear_read_only_recursive(p);
        std::fs::remove_dir_all(p)
    }

    fn noaudit() -> impl Fn(&str, &str) {
        |_: &str, _: &str| {}
    }

    fn seed_state(root: &Path) {
        std::fs::write(root.join("state/vault.sealed"), vec![0xABu8; 5000]).unwrap();
        std::fs::write(root.join("state/trusted_peers.list"), "abc\ndef\n").unwrap();
        std::fs::create_dir_all(root.join("state/core_identity")).unwrap();
        std::fs::write(root.join("state/core_identity/key.bin"), vec![0x11u8; 128]).unwrap();
    }

    /// Uçtan uca: anlık görüntü al → imzayla doğrula → SAĞLAM.
    #[test]
    fn a_fresh_snapshot_verifies_as_intact() {
        let root = temp_root("intact");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();

        let msg = snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap();
        assert!(msg.contains("3 dosya"), "3 dosya yedeklenmeliydi: {msg}");

        let snaps = list_snapshots(&root);
        assert_eq!(snaps.len(), 1);
        let out = verify_snapshot(&snaps[0].1, &kp.verifying_key);
        assert_eq!(out, VerifyOutcome::Intact { files: 3 }, "{}", out.to_text());
        assert!(out.is_intact());

        let _ = clear_and_remove(&root);
    }

    /// **Asıl güvence testi:** yedekteki bir dosya sessizce değiştirilirse
    /// doğrulama bunu YAKALAMALI.
    #[test]
    fn silently_corrupting_a_backed_up_file_is_detected() {
        let root = temp_root("corrupt");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap();
        let dir = list_snapshots(&root)[0].1.clone();

        let victim = dir.join("veri/state/vault.sealed");
        clear_read_only_recursive(&victim);
        let mut data = std::fs::read(&victim).unwrap();
        data[42] ^= 0xFF; // TEK BIR BIT degistir
        std::fs::write(&victim, &data).unwrap();

        match verify_snapshot(&dir, &kp.verifying_key) {
            VerifyOutcome::FilesCorrupted { bad } => {
                assert!(bad.iter().any(|b| b.contains("vault.sealed")), "bozulan dosya adiyla raporlanmali: {bad:?}");
            }
            other => panic!("BOZULMA YAKALANAMADI: {}", other.to_text()),
        }
        let _ = clear_and_remove(&root);
    }

    /// Saldırgan hem dosyayı hem manifestoyu değiştirirse: imza tutmaz.
    #[test]
    fn rewriting_the_manifest_to_match_a_tampered_file_is_still_caught() {
        let root = temp_root("manifest");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap();
        let dir = list_snapshots(&root)[0].1.clone();

        // Dosyayi degistir VE manifestoyu yeni gercege gore YENIDEN YAZ.
        let victim = dir.join("veri/state/trusted_peers.list");
        clear_read_only_recursive(&dir);
        std::fs::write(&victim, "saldirganin-anahtari\n").unwrap();

        let tree = merkle::build_tree_from_file(&victim, merkle::DEFAULT_LEAF_SIZE).unwrap();
        let text = std::fs::read_to_string(dir.join("manifest.txt")).unwrap();
        let mut m = Manifest::from_text(&text).unwrap();
        for e in m.entries.iter_mut() {
            if e.rel_path.ends_with("trusted_peers.list") {
                e.root_hex = hex(&tree.root);
                e.size = std::fs::metadata(&victim).unwrap().len();
            }
        }
        // Kok'u de tutarli hale getir -- yani manifesto KENDI ICINDE dogru.
        m.manifest_root_hex = hex(&manifest_root(&m.entries));
        std::fs::write(dir.join("manifest.txt"), m.to_text()).unwrap();

        // IMZA hala ESKI koke ait: saldirganin core'un ozel anahtari YOK.
        assert_eq!(
            verify_snapshot(&dir, &kp.verifying_key),
            VerifyOutcome::SignatureInvalid,
            "manifesto yeniden yazilsa bile IMZA tutmamali"
        );
        let _ = clear_and_remove(&root);
    }

    /// Manifesto satırı değiştirilip kök güncellenmezse: iç tutarsızlık.
    #[test]
    fn an_internally_inconsistent_manifest_is_rejected() {
        let root = temp_root("inconsistent");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap();
        let dir = list_snapshots(&root)[0].1.clone();
        clear_read_only_recursive(&dir);

        let text = std::fs::read_to_string(dir.join("manifest.txt")).unwrap();
        let tampered = text.replace("state/trusted_peers.list", "state/baska_dosya.list");
        std::fs::write(dir.join("manifest.txt"), tampered).unwrap();

        assert_eq!(verify_snapshot(&dir, &kp.verifying_key), VerifyOutcome::SignatureInvalid);
        let _ = clear_and_remove(&root);
    }

    /// BAŞKA bir anahtarla imzalanmış bir yedek kabul EDİLMEMELİ —
    /// saldırgan kendi anahtar çiftini üretip sahte bir yedek koyamaz.
    #[test]
    fn a_snapshot_signed_by_a_different_key_is_rejected() {
        let root = temp_root("wrongkey");
        seed_state(&root);
        let real = chimera_crypto::obsidian::dsa_generate_keypair();
        let attacker = chimera_crypto::obsidian::dsa_generate_keypair();
        snapshot(&root, &attacker.signing_key, 1000, &noaudit()).unwrap();
        let dir = list_snapshots(&root)[0].1.clone();

        assert_eq!(
            verify_snapshot(&dir, &real.verifying_key),
            VerifyOutcome::SignatureInvalid,
            "yabanci anahtarla imzalanmis yedek REDDEDILMELI"
        );
        let _ = clear_and_remove(&root);
    }

    /// Aynı zaman damgasına ikinci kez yazmak, mevcut yedeğin ÜZERİNE
    /// yazmak demektir — bu reddedilmeli ("asla üzerine yazılmaz" vaadi).
    #[test]
    fn an_existing_snapshot_is_never_overwritten() {
        let root = temp_root("nooverwrite");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap();
        let err = snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap_err();
        assert!(err.contains("ZATEN VAR"));
        assert_eq!(list_snapshots(&root).len(), 1);
        let _ = clear_and_remove(&root);
    }

    #[test]
    fn manifest_text_round_trips_exactly() {
        let m = Manifest {
            version: 1,
            created_at: 1700000000,
            entries: vec![
                FileEntry { rel_path: "state/a.bin".into(), size: 10, root_hex: "aa".repeat(32) },
                FileEntry { rel_path: "kullanici0/belge.docx".into(), size: 20, root_hex: "bb".repeat(32) },
            ],
            manifest_root_hex: "cc".repeat(32),
        };
        assert_eq!(Manifest::from_text(&m.to_text()).unwrap(), m);
    }

    /// Manifesto kökü dosya SIRASINDAN bağımsız olmalı — aksi halde
    /// dizin okuma sırası değiştiğinde doğrulama rastgele başarısız olurdu.
    #[test]
    fn the_manifest_root_is_independent_of_entry_order() {
        let a = FileEntry { rel_path: "state/a".into(), size: 1, root_hex: "11".repeat(32) };
        let b = FileEntry { rel_path: "state/b".into(), size: 2, root_hex: "22".repeat(32) };
        assert_eq!(manifest_root(&[a.clone(), b.clone()]), manifest_root(&[b, a]));
    }

    /// Farklı içerik farklı kök üretmeli (aksi halde imza hiçbir şey
    /// korumazdı).
    #[test]
    fn different_content_produces_a_different_manifest_root() {
        let a = FileEntry { rel_path: "state/a".into(), size: 1, root_hex: "11".repeat(32) };
        let mut b = a.clone();
        b.root_hex = "22".repeat(32);
        assert_ne!(manifest_root(&[a.clone()]), manifest_root(&[b]));
        let mut c = a.clone();
        c.size = 999;
        assert_ne!(manifest_root(&[a]), manifest_root(&[c]), "boyut degisikligi de koku degistirmeli");
    }

    #[test]
    fn pruning_keeps_only_the_newest_snapshots() {
        let root = temp_root("prune");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        for ts in [1000u64, 2000, 3000, 4000, 5000] {
            snapshot(&root, &kp.signing_key, ts, &noaudit()).unwrap();
        }
        assert_eq!(list_snapshots(&root).len(), 5);
        let removed = prune(&root, 2, &noaudit());
        assert_eq!(removed, 3);
        let left = list_snapshots(&root);
        assert_eq!(left.len(), 2);
        assert_eq!(left[0].0, 4000, "en YENI ikisi kalmali");
        assert_eq!(left[1].0, 5000);
        let _ = clear_and_remove(&root);
    }

    #[test]
    fn pruning_does_nothing_when_under_the_limit() {
        let root = temp_root("noprune");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap();
        assert_eq!(prune(&root, 8, &noaudit()), 0);
        assert_eq!(list_snapshots(&root).len(), 1);
        let _ = clear_and_remove(&root);
    }

    #[test]
    fn age_and_listing_report_honestly_when_there_are_no_backups() {
        let root = temp_root("none");
        assert_eq!(age_of_newest(&root, 9999), None);
        assert!(list_text(&root, 9999).contains("HIC ANLIK GORUNTU YOK"));
        let _ = clear_and_remove(&root);
    }

    #[test]
    fn age_of_newest_uses_the_most_recent_snapshot() {
        let root = temp_root("age");
        seed_state(&root);
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        snapshot(&root, &kp.signing_key, 1000, &noaudit()).unwrap();
        snapshot(&root, &kp.signing_key, 3000, &noaudit()).unwrap();
        assert_eq!(age_of_newest(&root, 5000), Some(2000));
        let _ = clear_and_remove(&root);
    }

    #[test]
    fn a_missing_snapshot_directory_is_reported_not_silently_ok() {
        let root = temp_root("missing");
        let kp = chimera_crypto::obsidian::dsa_generate_keypair();
        match verify_snapshot(&root.join("yok"), &kp.verifying_key) {
            VerifyOutcome::Unreadable { .. } => {}
            other => panic!("eksik yedek SAGLAM sayilamaz: {}", other.to_text()),
        }
        let _ = clear_and_remove(&root);
    }
}
