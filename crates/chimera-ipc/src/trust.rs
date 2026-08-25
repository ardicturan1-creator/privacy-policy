//! Güven deposu: yalnızca burada listelenen açık anahtarlar bağlantı
//! kurabilir. **Kör TOFU (ilk-görüşte-güven) yoktur** — bir eş, operatör
//! onu açıkça `trust()` ile eklemeden önce el sıkışmayı asla tamamlayamaz.
//! Bu, "iki bileşen birbirini doğrulamadan asla komut kabul etmesin"
//! gereksiniminin doğrudan karşılığıdır.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct TrustStore {
    path: PathBuf,
    fingerprints: BTreeSet<String>,
}

impl TrustStore {
    pub fn load(path: &Path) -> io::Result<Self> {
        let fingerprints = match fs::read_to_string(path) {
            Ok(text) => text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeSet::new(),
            Err(e) => return Err(e),
        };
        Ok(Self { path: path.to_path_buf(), fingerprints })
    }

    pub fn is_trusted(&self, verifying_key_bytes: &[u8]) -> bool {
        self.fingerprints.contains(&fingerprint_of(verifying_key_bytes))
    }

    pub fn trust(&mut self, verifying_key_bytes: &[u8]) -> io::Result<()> {
        self.fingerprints.insert(fingerprint_of(verifying_key_bytes));
        self.save()
    }

    pub fn revoke(&mut self, verifying_key_bytes: &[u8]) -> io::Result<()> {
        self.fingerprints.remove(&fingerprint_of(verifying_key_bytes));
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body: String = self.fingerprints.iter().map(|f| format!("{f}\n")).collect();
        fs::write(&self.path, body)
    }
}

pub fn fingerprint_of(verifying_key_bytes: &[u8]) -> String {
    blake3::hash(verifying_key_bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_key_is_rejected_trusted_key_is_accepted() {
        let path = std::env::temp_dir().join(format!("chimera-ipc-trust-{}.list", std::process::id()));
        let _ = fs::remove_file(&path);

        let mut store = TrustStore::load(&path).unwrap();
        let key_a = b"fake-verifying-key-a";
        let key_b = b"fake-verifying-key-b";

        assert!(!store.is_trusted(key_a));
        store.trust(key_a).unwrap();
        assert!(store.is_trusted(key_a));
        assert!(!store.is_trusted(key_b));

        // Diskten yeniden yuklenince de kalici olmali.
        let reloaded = TrustStore::load(&path).unwrap();
        assert!(reloaded.is_trusted(key_a));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn revoke_removes_trust() {
        let path = std::env::temp_dir().join(format!("chimera-ipc-trust-revoke-{}.list", std::process::id()));
        let _ = fs::remove_file(&path);

        let mut store = TrustStore::load(&path).unwrap();
        let key = b"fake-key";
        store.trust(key).unwrap();
        assert!(store.is_trusted(key));
        store.revoke(key).unwrap();
        assert!(!store.is_trusted(key));

        let _ = fs::remove_file(&path);
    }
}
