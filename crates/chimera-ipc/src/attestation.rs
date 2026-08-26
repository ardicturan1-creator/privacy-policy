//! Karşılıklı binary kimliği sabitleme (pinning).
//!
//! Sorun: bir saldırgan çalınmış (veya sızdırılmış) bir kimlik imzalama
//! anahtarıyla GEÇERLİ bir el sıkışma imzası üretebilir, ama SUNUCUYA
//! BAĞLANAN İKİLİNİN KENDİSİNİ değiştiremez — kendi değiştirdiği (arka
//! kapılı/yamalanmış) bir ikiliyi çalıştırıyorsa, o ikilinin BLAKE3 özeti
//! orijinalinden FARKLI olur. Bu modül, bir eşin (peer) "son bilinen iyi"
//! ikili özetini saklar; el sıkışma bu özeti karşılaştırır.
//!
//! **Bilinçli sınır:** Bu, ikilinin KENDİSİNİN sahada değiştirilmesini
//! (örn. diskte doğrudan patch'lenmesini) yalnızca bir SONRAKİ el
//! sıkışmada yakalar — çalışan bir sürecin belleğini o an değiştiren bir
//! saldırıyı (bkz. tehdit modeli Seviye 4/5) engellemez. Bu yüzden tek
//! başına bir savunma değil, katmanlı savunmanın bir parçasıdır.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct AttestationStore {
    path: PathBuf,
    // fingerprint (blake3 hex of verifying key) -> pinned binary hash (blake3 hex)
    pinned: BTreeMap<String, String>,
}

impl AttestationStore {
    pub fn load(path: &Path) -> io::Result<Self> {
        let pinned = match fs::read_to_string(path) {
            Ok(text) => text
                .lines()
                .filter_map(|l| l.split_once(' '))
                .map(|(fp, h)| (fp.trim().to_string(), h.trim().to_string()))
                .collect(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e),
        };
        Ok(Self { path: path.to_path_buf(), pinned })
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body: String = self.pinned.iter().map(|(fp, h)| format!("{fp} {h}\n")).collect();
        fs::write(&self.path, body)
    }

    /// Bir eşin ikili özetini SABİTLER. Yalnızca operatörün açık bir
    /// `trust`/`attest` komutuyla çağrılması amaçlanır — arka planda
    /// sessizce otomatik-güvenme (TOFU) YAPILMAZ.
    pub fn pin(&mut self, fingerprint: &str, binary_hash: &str) -> io::Result<()> {
        self.pinned.insert(fingerprint.to_string(), binary_hash.to_string());
        self.save()
    }

    /// `None`: bu eş için henüz bir sabitleme yok (operatör hiç `attest`
    /// çalıştırmamış) — bu MEVCUT davranışı bozmamak için "kontrol
    /// edilemedi" olarak ele alınır, "başarısız" olarak DEĞİL.
    /// `Some(true)`: sabitlenen özetle eşleşiyor. `Some(false)`: EŞLEŞMİYOR
    /// — bu, ikilinin değiştirildiğinin güçlü bir işaretidir.
    pub fn check(&self, fingerprint: &str, binary_hash: &str) -> Option<bool> {
        self.pinned.get(fingerprint).map(|expected| expected == binary_hash)
    }
}

/// Şu an çalışan ikilinin kendi BLAKE3 özeti. El sıkışmada karşı tarafa
/// gönderilir ve imzayla korunur (bkz. `handshake.rs`).
pub fn self_binary_hash() -> io::Result<String> {
    let exe = std::env::current_exe()?;
    let bytes = fs::read(exe)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpinned_peer_returns_none_not_a_failure() {
        let path = std::env::temp_dir().join(format!("chimera-attest-{}.list", std::process::id()));
        let _ = fs::remove_file(&path);
        let store = AttestationStore::load(&path).unwrap();
        assert_eq!(store.check("abc123", "deadbeef"), None);
    }

    #[test]
    fn pinned_hash_matches_and_mismatches_correctly() {
        let path = std::env::temp_dir().join(format!("chimera-attest-2-{}.list", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut store = AttestationStore::load(&path).unwrap();
        store.pin("fp1", "hash-original").unwrap();

        assert_eq!(store.check("fp1", "hash-original"), Some(true));
        assert_eq!(store.check("fp1", "hash-TAMPERED"), Some(false));

        // Diskten yeniden yuklenince de kalici olmali.
        let reloaded = AttestationStore::load(&path).unwrap();
        assert_eq!(reloaded.check("fp1", "hash-original"), Some(true));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn self_hash_is_deterministic_for_same_binary() {
        let a = self_binary_hash().unwrap();
        let b = self_binary_hash().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // BLAKE3 hex = 32 bayt = 64 hex karakter
    }
}
