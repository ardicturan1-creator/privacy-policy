//! OBSIDIAN — gercek, calisan kriptografik cekirdek.
//!
//! Buradaki her fonksiyon GERCEK bir crates.io kutuphanesine sarilir ve
//! `cargo test` ile ucdan uca dogrulanir: anahtar uretimi, kapsulleme,
//! imza, simetrik muhurleme ve vektor donusumu hicbiri simule edilmemistir.
//!
//!   ML-KEM-1024  -> `ml-kem` crate'i (RustCrypto, FIPS 203)
//!   ML-DSA-87    -> `ml-dsa` crate'i (RustCrypto, FIPS 204)
//!   XChaCha20-Poly1305 -> `chacha20poly1305` crate'i (RustCrypto)
//!   Argon2id     -> `argon2` crate'i (RustCrypto)
//!   Shamir(k,n)  -> `sharks` crate'i (GF(256))
//!   BLAKE3       -> `blake3` crate'i
//!
//! TPM 2.0 muhurleme bu derlemede YOK: bu sanal ortamda fiziksel bir TPM
//! (`/dev/tpm0`) bulunmuyor, ve gercek donanim olmadan "TPM'e muhurledim"
//! demek dogrulanamaz bir iddia olurdu. Onun yerine, mimari dokumaninda
//! zaten tanimli olan yazilim-yolu (Share A/B/C) TAM VE GERCEK olarak
//! implemente edildi: Shamir(2,3) paylasimi + Argon2id parola tabanli
//! muhurleme. Gercek bir TPM chip'i mevcut oldugunda Share A'nin
//! `tss-esapi` ile donanima baglanmasi, bu API'yi degistirmeden eklenebilir.

use argon2::Argon2;
use chacha20poly1305::{
    aead::{Aead, Generate as AeadGenerate, KeyInit, Nonce},
    XChaCha20Poly1305,
};
use ml_dsa::{
    signature::{Keypair, Signer, Verifier},
    MlDsa87,
};
use ml_kem::kem::{Decapsulate, Encapsulate, Kem};
use ml_kem::MlKem1024;
use sharks::{Share, Sharks};
use zeroize::Zeroize;

pub const MASTER_KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;

#[derive(Debug)]
pub enum ObsidianError {
    Random(getrandom::Error),
    Kdf,
    Seal,
    Unseal,
    ShareRecovery,
    SignatureInvalid,
}

impl std::fmt::Display for ObsidianError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ObsidianError {}

fn random_bytes<const N: usize>() -> Result<[u8; N], ObsidianError> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(ObsidianError::Random)?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Master anahtar: uretim, Argon2id ile parola-tabanli muhurleme
// ---------------------------------------------------------------------------

/// 32 baytlik rastgele master anahtar. `state/epoch.seal` icin kok materyal.
pub fn generate_master_key() -> Result<[u8; MASTER_KEY_LEN], ObsidianError> {
    random_bytes::<MASTER_KEY_LEN>()
}

#[derive(Debug, Clone)]
pub struct SealedKey {
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

/// Master anahtari, parola-turetilmis (Argon2id) bir anahtarla XChaCha20-Poly1305
/// kullanarak muhurler. Mimari dokumandaki "Share B" adimidir.
pub fn seal_master_key(password: &[u8], master_key: &[u8; MASTER_KEY_LEN]) -> Result<SealedKey, ObsidianError> {
    let salt = random_bytes::<SALT_LEN>()?;

    let mut derived = [0u8; 32];
    Argon2::default()
        .hash_password_into(password, &salt, &mut derived)
        .map_err(|_| ObsidianError::Kdf)?;

    let cipher = XChaCha20Poly1305::new((&derived).into());
    let nonce = Nonce::<XChaCha20Poly1305>::generate();
    let ciphertext = cipher
        .encrypt(&nonce, master_key.as_slice())
        .map_err(|_| ObsidianError::Seal)?;

    derived.zeroize();

    Ok(SealedKey {
        salt,
        nonce: nonce.into(),
        ciphertext,
    })
}

/// Muhurlenmis master anahtari parola ile geri acar. Yanlis parola veya
/// kurcalanmis ciphertext AEAD dogrulamasinda BASARISIZ olur (sessizce
/// yanlis anahtar dondurmez).
pub fn unseal_master_key(password: &[u8], sealed: &SealedKey) -> Result<[u8; MASTER_KEY_LEN], ObsidianError> {
    let mut derived = [0u8; 32];
    Argon2::default()
        .hash_password_into(password, &sealed.salt, &mut derived)
        .map_err(|_| ObsidianError::Kdf)?;

    let cipher = XChaCha20Poly1305::new((&derived).into());
    let nonce = Nonce::<XChaCha20Poly1305>::try_from(sealed.nonce.as_slice()).expect("24 bayt nonce");
    let plaintext = cipher
        .decrypt(&nonce, sealed.ciphertext.as_slice())
        .map_err(|_| ObsidianError::Unseal)?;
    derived.zeroize();

    plaintext
        .try_into()
        .map_err(|_| ObsidianError::Unseal)
}

// ---------------------------------------------------------------------------
// Shamir(2,3) — master anahtarin 3 parcaya bolunmesi
// ---------------------------------------------------------------------------

/// Master anahtari 2-esikli, 3 parcali Shamir paylasimina boler.
/// Herhangi 2 parca anahtari geri kurar; tek parca hicbir bilgi vermez
/// (bilgi-teorik olarak guvenli, GF(256) uzerinde).
pub fn split_master_key(master_key: &[u8; MASTER_KEY_LEN]) -> Vec<Share> {
    let sharks = Sharks(2);
    sharks.dealer(master_key.as_slice()).take(3).collect()
}

pub fn recover_master_key(shares: &[Share]) -> Result<[u8; MASTER_KEY_LEN], ObsidianError> {
    let sharks = Sharks(2);
    let secret = sharks.recover(shares).map_err(|_| ObsidianError::ShareRecovery)?;
    secret.try_into().map_err(|_| ObsidianError::ShareRecovery)
}

// ---------------------------------------------------------------------------
// ML-KEM-1024 (FIPS 203) — anahtar kapsulleme
// ---------------------------------------------------------------------------

pub struct KemKeypair {
    pub decapsulation_key: ml_kem::kem::DecapsulationKey<MlKem1024>,
    pub encapsulation_key: ml_kem::kem::EncapsulationKey<MlKem1024>,
}

/// `Kem::generate_keypair()` (feature `getrandom`) ambient isletim sistemi
/// RNG'sini dogrudan kullanir; ayri bir rng koprusu kurmaya gerek yoktur.
pub fn kem_generate_keypair() -> KemKeypair {
    let (dk, ek) = MlKem1024::generate_keypair();
    KemKeypair { decapsulation_key: dk, encapsulation_key: ek }
}

/// Encapsulation key sahibine bir paylasilan anahtar gonderir.
/// Donen `(ciphertext, shared_secret)`: ciphertext karsi tarafa gonderilir,
/// shared_secret burada tutulur.
pub fn kem_encapsulate(ek: &ml_kem::kem::EncapsulationKey<MlKem1024>) -> (Vec<u8>, Vec<u8>) {
    let (ct, ss) = ek.encapsulate();
    (ct.to_vec(), ss.to_vec())
}

pub fn kem_decapsulate(dk: &ml_kem::kem::DecapsulationKey<MlKem1024>, ciphertext: &[u8]) -> Vec<u8> {
    let ct = ciphertext.try_into().expect("gecersiz ciphertext boyutu");
    dk.decapsulate(&ct).to_vec()
}

// ---------------------------------------------------------------------------
// ML-DSA-87 (FIPS 204) — imza
// ---------------------------------------------------------------------------

pub struct DsaKeypair {
    pub signing_key: ml_dsa::SigningKey<MlDsa87>,
    pub verifying_key: ml_dsa::VerifyingKey<MlDsa87>,
}

/// `Generate::generate()` (feature `getrandom`, ml-dsa'da varsayilan) ambient
/// isletim sistemi RNG'sini dogrudan kullanir.
pub fn dsa_generate_keypair() -> DsaKeypair {
    let signing_key = ml_dsa::SigningKey::<MlDsa87>::generate();
    let verifying_key = signing_key.verifying_key();
    DsaKeypair { signing_key, verifying_key }
}

pub fn dsa_sign(sk: &ml_dsa::SigningKey<MlDsa87>, msg: &[u8]) -> ml_dsa::Signature<MlDsa87> {
    sk.sign(msg)
}

pub fn dsa_verify(vk: &ml_dsa::VerifyingKey<MlDsa87>, msg: &[u8], sig: &ml_dsa::Signature<MlDsa87>) -> Result<(), ObsidianError> {
    vk.verify(msg, sig).map_err(|_| ObsidianError::SignatureInvalid)
}

/// Diske yazilabilir bayt dizileri — `MANIFEST.sig` icin. `KeyExport`/
/// `SignatureEncoding` sabit boyutlu diziler dondurur; burada `Vec<u8>`'a
/// duzlestirilir.
pub fn dsa_verifying_key_bytes(vk: &ml_dsa::VerifyingKey<MlDsa87>) -> Vec<u8> {
    use ml_dsa::KeyExport;
    vk.to_bytes().to_vec()
}

pub fn dsa_verifying_key_from_bytes(bytes: &[u8]) -> Result<ml_dsa::VerifyingKey<MlDsa87>, ObsidianError> {
    use hybrid_array::Array;
    use ml_dsa::KeyInit;
    let key = Array::try_from(bytes).map_err(|_| ObsidianError::SignatureInvalid)?;
    Ok(ml_dsa::VerifyingKey::<MlDsa87>::new(&key))
}

pub fn dsa_signature_bytes(sig: &ml_dsa::Signature<MlDsa87>) -> Vec<u8> {
    use ml_dsa::signature::SignatureEncoding;
    sig.to_bytes().to_vec()
}

pub fn dsa_signature_from_bytes(bytes: &[u8]) -> Result<ml_dsa::Signature<MlDsa87>, ObsidianError> {
    ml_dsa::Signature::<MlDsa87>::try_from(bytes).map_err(|_| ObsidianError::SignatureInvalid)
}

// ---------------------------------------------------------------------------
// XChaCha20-Poly1305 — veri muhuru (Obsidian'in ana AEAD katmani)
// ---------------------------------------------------------------------------

pub fn seal_data(key: &[u8; 32], plaintext: &[u8]) -> (Nonce<XChaCha20Poly1305>, Vec<u8>) {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = Nonce::<XChaCha20Poly1305>::generate();
    let ct = cipher.encrypt(&nonce, plaintext).expect("seal");
    (nonce, ct)
}

pub fn open_data(key: &[u8; 32], nonce: &Nonce<XChaCha20Poly1305>, ciphertext: &[u8]) -> Result<Vec<u8>, ObsidianError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher.decrypt(nonce, ciphertext).map_err(|_| ObsidianError::Unseal)
}

// ---------------------------------------------------------------------------
// Ortogonal vektor donusumu — embedding-inversion'a karsi (Q(epoch))
// ---------------------------------------------------------------------------

/// `seed`'den Gram-Schmidt ile GERCEK bir ortogonal matris turetir.
/// Ortogonal donusum ic carpimi (dolayisiyla kosinus benzerligini) TAM
/// olarak korur — bu bir yaklasik degil, matematiksel bir ozdesliktir ve
/// test modulunde sayisal olarak dogrulanir.
pub fn orthogonal_matrix(dim: usize, seed: u64) -> Vec<Vec<f64>> {
    let mut state = seed ^ 0x9E3779B97F4A7C15;
    let mut next_f64 = move || {
        // splitmix64 — deterministik, tohuma bagli sozde-rastgele uretec.
        // Kriptografik degil (burada gerek yok): amac epoch basina
        // TEKRARLANABILIR bir ortogonal baz uretmek.
        state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        ((z >> 11) as f64) / ((1u64 << 53) as f64) * 2.0 - 1.0
    };

    let mut cols: Vec<Vec<f64>> = (0..dim)
        .map(|_| (0..dim).map(|_| next_f64()).collect())
        .collect();

    // Modified Gram-Schmidt: sirayla her vektoru oncekilere dik hale getir,
    // sonra birim uzunluga normalize et.
    for i in 0..dim {
        for j in 0..i {
            let dot: f64 = (0..dim).map(|k| cols[i][k] * cols[j][k]).sum();
            for k in 0..dim {
                cols[i][k] -= dot * cols[j][k];
            }
        }
        let norm: f64 = (0..dim).map(|k| cols[i][k] * cols[i][k]).sum::<f64>().sqrt();
        for k in 0..dim {
            cols[i][k] /= norm;
        }
    }
    cols
}

pub fn rotate(matrix: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
    matrix.iter().map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum()).collect()
}

pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    dot / (na * nb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_unseal_round_trip_recovers_exact_key() {
        let master = generate_master_key().unwrap();
        let sealed = seal_master_key(b"correct horse battery staple", &master).unwrap();
        let recovered = unseal_master_key(b"correct horse battery staple", &sealed).unwrap();
        assert_eq!(master, recovered);
    }

    #[test]
    fn unseal_with_wrong_password_fails_not_garbage() {
        let master = generate_master_key().unwrap();
        let sealed = seal_master_key(b"right-password", &master).unwrap();
        let result = unseal_master_key(b"wrong-password", &sealed);
        assert!(result.is_err(), "yanlis parola sessizce basarili olmamali");
    }

    #[test]
    fn tampered_ciphertext_fails_aead_auth() {
        let master = generate_master_key().unwrap();
        let mut sealed = seal_master_key(b"pw", &master).unwrap();
        sealed.ciphertext[0] ^= 0xFF;
        assert!(unseal_master_key(b"pw", &sealed).is_err());
    }

    #[test]
    fn shamir_two_of_three_recovers_from_any_pair() {
        let master = generate_master_key().unwrap();
        let shares = split_master_key(&master);
        assert_eq!(shares.len(), 3);

        let ab = recover_master_key(&[shares[0].clone(), shares[1].clone()]).unwrap();
        let ac = recover_master_key(&[shares[0].clone(), shares[2].clone()]).unwrap();
        let bc = recover_master_key(&[shares[1].clone(), shares[2].clone()]).unwrap();
        assert_eq!(ab, master);
        assert_eq!(ac, master);
        assert_eq!(bc, master);
    }

    #[test]
    fn shamir_single_share_cannot_recover() {
        let master = generate_master_key().unwrap();
        let shares = split_master_key(&master);
        // Tek parcadan (esik 2 iken) kurtarma ya hata verir ya da
        // gercek anahtardan FARKLI bir sonuc uretir — hangisi olursa
        // olsun, gercek anahtar "yanlislikla" ele gecmez.
        match recover_master_key(&[shares[0].clone()]) {
            Err(_) => {}
            Ok(wrong) => assert_ne!(wrong, master),
        }
    }

    #[test]
    fn kem_shared_secret_matches_on_both_sides() {
        let kp = kem_generate_keypair();
        let (ct, ss_sender) = kem_encapsulate(&kp.encapsulation_key);
        let ss_receiver = kem_decapsulate(&kp.decapsulation_key, &ct);
        assert_eq!(ss_sender, ss_receiver);
        assert_eq!(ss_sender.len(), 32);
    }

    #[test]
    fn dsa_signature_round_trip_and_tamper_detection() {
        let kp = dsa_generate_keypair();
        let msg = b"chimera swarm quorum promotion, epoch 42";
        let sig = dsa_sign(&kp.signing_key, msg);
        assert!(dsa_verify(&kp.verifying_key, msg, &sig).is_ok());

        // Farkli bir mesaj icin ayni imza gecersiz olmali.
        assert!(dsa_verify(&kp.verifying_key, b"epoch 43", &sig).is_err());
    }

    #[test]
    fn dsa_key_and_signature_survive_byte_round_trip() {
        // `MANIFEST.sig` diske tam olarak bu yoldan yazilip okunur:
        // dogrulama anahtari ve imza baytlara duzlestirilir, disk uzerinden
        // (burada bir Vec<u8> uzerinden) geri okunur ve GERCEKTEN dogrular.
        let kp = dsa_generate_keypair();
        let root = [0x11u8; 32];
        let sig = dsa_sign(&kp.signing_key, &root);

        let vk_bytes = dsa_verifying_key_bytes(&kp.verifying_key);
        let sig_bytes = dsa_signature_bytes(&sig);

        let vk2 = dsa_verifying_key_from_bytes(&vk_bytes).unwrap();
        let sig2 = dsa_signature_from_bytes(&sig_bytes).unwrap();

        assert!(dsa_verify(&vk2, &root, &sig2).is_ok());

        let mut tampered_root = root;
        tampered_root[0] ^= 1;
        assert!(dsa_verify(&vk2, &tampered_root, &sig2).is_err());
    }

    #[test]
    fn seal_data_round_trip() {
        let key = [7u8; 32];
        let plaintext = b"gizli vektor parcasi";
        let (nonce, ct) = seal_data(&key, plaintext);
        let recovered = open_data(&key, &nonce, &ct).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn wrong_key_cannot_open_sealed_data() {
        let key = [7u8; 32];
        let wrong_key = [8u8; 32];
        let (nonce, ct) = seal_data(&key, b"data");
        assert!(open_data(&wrong_key, &nonce, &ct).is_err());
    }

    #[test]
    fn orthogonal_rotation_preserves_cosine_similarity() {
        let dim = 16;
        let q = orthogonal_matrix(dim, 0xC0FFEE);

        let a: Vec<f64> = (0..dim).map(|i| (i as f64 + 1.0).sin()).collect();
        let b: Vec<f64> = (0..dim).map(|i| (i as f64 + 1.0).cos()).collect();

        let sim_before = cosine_similarity(&a, &b);
        let ra = rotate(&q, &a);
        let rb = rotate(&q, &b);
        let sim_after = cosine_similarity(&ra, &rb);

        assert!(
            (sim_before - sim_after).abs() < 1e-9,
            "kosinus benzerligi korunmali: {sim_before} vs {sim_after}"
        );
        // Vektorlerin kendisi ise donduruldukten sonra GERCEKTEN degismis olmali.
        assert!(a.iter().zip(&ra).any(|(x, y)| (x - y).abs() > 1e-6));
    }

    #[test]
    fn orthogonal_matrix_rows_are_unit_and_mutually_orthogonal() {
        let dim = 8;
        let q = orthogonal_matrix(dim, 123456789);
        for i in 0..dim {
            let norm: f64 = q[i].iter().map(|x| x * x).sum::<f64>().sqrt();
            assert!((norm - 1.0).abs() < 1e-9, "satir {i} birim uzunlukta degil: {norm}");
            for j in (i + 1)..dim {
                let dot: f64 = q[i].iter().zip(&q[j]).map(|(a, b)| a * b).sum();
                assert!(dot.abs() < 1e-9, "satir {i} ve {j} dik degil: {dot}");
            }
        }
    }

    #[test]
    fn different_seeds_produce_different_matrices() {
        let q1 = orthogonal_matrix(8, 1);
        let q2 = orthogonal_matrix(8, 2);
        assert_ne!(q1, q2, "farkli epoch tohumlari farkli donusum uretmeli");
    }
}
