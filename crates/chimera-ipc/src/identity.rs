//! Kalıcı bileşen kimliği: bir ML-DSA-87 anahtar çifti.
//!
//! İmzalama (gizli) anahtarı diskte ASLA açık yazılmaz — yerel bir
//! parolayla (`seal_with_password`, Argon2id + XChaCha20-Poly1305)
//! mühürlenir. Gerçek bir kurumsal dağıtımda bu parola OS anahtar
//! deposundan (Windows DPAPI / macOS Keychain / Linux kernel keyring)
//! ya da bir TPM'den gelir; bu ortamda o API'lere erişim YOK, bu yüzden
//! `CHIMERA_IDENTITY_PASSPHRASE` ortam değişkeninden ya da yoksa dosya
//! izinleriyle (0600) korunan yerel bir "makine anahtarı" dosyasından
//! okunur — bu sınırlama README'de açıkça belgelenir.

use chimera_crypto::obsidian::{self, DsaKeypair};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct Identity {
    pub keypair: DsaKeypair,
}

impl Identity {
    /// `dir/identity.sealed` içinde var olan bir kimlik varsa açar,
    /// yoksa yeni bir ML-DSA-87 anahtar çifti üretip mühürleyerek yazar.
    /// Dönüş: kimlik + bunun açık anahtarının onaltılık parmak izi.
    pub fn load_or_create(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = sealed_path(dir);
        let password = machine_passphrase(dir)?;

        if let Ok(sealed) = fs::read(&path) {
            let plain = obsidian::open_with_password(&password, &sealed).map_err(to_io)?;
            let signing_key = obsidian::dsa_signing_key_from_bytes(&plain).map_err(to_io)?;
            let verifying_key = signing_key_verifying(&signing_key);
            return Ok(Self { keypair: DsaKeypair { signing_key, verifying_key } });
        }

        let kp = obsidian::dsa_generate_keypair();
        let sk_bytes = obsidian::dsa_signing_key_bytes(&kp.signing_key);
        let sealed = obsidian::seal_with_password(&password, &sk_bytes).map_err(to_io)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?;
            use std::io::Write;
            f.write_all(&sealed)?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&path, &sealed)?;
        }

        Ok(Self { keypair: kp })
    }

    pub fn verifying_key_bytes(&self) -> Vec<u8> {
        obsidian::dsa_verifying_key_bytes(&self.keypair.verifying_key)
    }

    pub fn fingerprint(&self) -> String {
        let hash = blake3_hex(&self.verifying_key_bytes());
        hash[..16].to_string()
    }
}

fn sealed_path(dir: &Path) -> PathBuf {
    dir.join("identity.sealed")
}

fn machine_key_path(dir: &Path) -> PathBuf {
    dir.join(".machine_key")
}

/// `CHIMERA_IDENTITY_PASSPHRASE` set edilmişse onu kullanır (üretimde CI/CD
/// veya OS anahtar deposu entegrasyonu için). Yoksa, yalnızca bu dizine
/// özel, rastgele üretilmiş ve 0600 ile korunan bir dosya-anahtarı kullanılır.
fn machine_passphrase(dir: &Path) -> io::Result<Vec<u8>> {
    if let Ok(p) = std::env::var("CHIMERA_IDENTITY_PASSPHRASE") {
        return Ok(p.into_bytes());
    }
    let path = machine_key_path(dir);
    if let Ok(existing) = fs::read(&path) {
        if existing.len() == 32 {
            return Ok(existing);
        }
    }
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(to_io)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?;
        use std::io::Write;
        f.write_all(&key)?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&path, key)?;
    }
    Ok(key.to_vec())
}

fn signing_key_verifying(sk: &ml_dsa::SigningKey<ml_dsa::MlDsa87>) -> ml_dsa::VerifyingKey<ml_dsa::MlDsa87> {
    use ml_dsa::signature::Keypair;
    sk.verifying_key()
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn to_io<E: std::fmt::Debug>(e: E) -> io::Error {
    io::Error::other(format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_persists_across_reload() {
        let dir = std::env::temp_dir().join(format!("chimera-ipc-identity-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        let id1 = Identity::load_or_create(&dir).unwrap();
        let fp1 = id1.fingerprint();

        let id2 = Identity::load_or_create(&dir).unwrap();
        let fp2 = id2.fingerprint();

        assert_eq!(fp1, fp2, "ayni dizinden yeniden yuklenen kimlik AYNI anahtar cifti olmali");
        assert_eq!(id1.verifying_key_bytes(), id2.verifying_key_bytes());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn different_directories_get_different_identities() {
        let base = std::env::temp_dir().join(format!("chimera-ipc-identity-diff-{}", std::process::id()));
        let a = base.join("a");
        let b = base.join("b");
        let _ = fs::remove_dir_all(&base);

        let id_a = Identity::load_or_create(&a).unwrap();
        let id_b = Identity::load_or_create(&b).unwrap();
        assert_ne!(id_a.fingerprint(), id_b.fingerprint());

        let _ = fs::remove_dir_all(&base);
    }
}
