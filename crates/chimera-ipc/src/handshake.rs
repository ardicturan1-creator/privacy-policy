//! El sıkışma protokolü — mTLS'in sağladığı güvenlik özelliğiyle aynı,
//! özel bir ikili protokol.
//!
//! ```text
//!   İstemci                                          Sunucu
//!   ---------------------------------------------------------------
//!   ephemeral ML-KEM-1024 (ek_c, dk_c) uret
//!   ClientHello = { client_vk, ek_c, nonce_c,
//!                   sig_c = Sign(sk_client, ek_c || nonce_c) }
//!                            -------->
//!                                          client_vk GUVENILIYOR MU?
//!                                            hayirsa: BAGLANTIYI KES
//!                                          sig_c dogrula (client_vk ile)
//!                                          (ct, ss) = Encapsulate(ek_c)
//!                                          ServerHello = { server_vk, ct,
//!                                            nonce_s, sig_s = Sign(sk_server,
//!                                            ct || nonce_c || nonce_s) }
//!                            <--------
//!   server_vk GUVENILIYOR MU?
//!     hayirsa: BAGLANTIYI KES
//!   sig_s dogrula (server_vk ile)
//!   ss = Decapsulate(dk_c, ct)
//!   ---------------------------------------------------------------
//!   session_key = HKDF-SHA256(ss, salt=nonce_c||nonce_s, info="chimera-ipc-v1")
//! ```
//!
//! Her iki taraf da KARŞI TARAFIN İMZASINI KENDİ GÜVEN DEPOSUNA KARŞI
//! doğrulamadan oturum anahtarını asla türetmez — bu, "birbirini
//! kriptografik olarak doğrulamadan asla komut kabul etmesin" kuralının
//! el sıkışma seviyesindeki karşılığıdır.

use crate::trust::TrustStore;
use chimera_crypto::obsidian;
use hkdf::Hkdf;
use sha2::Sha256;
use std::io::{self, Read, Write};

#[derive(Debug)]
pub enum HandshakeError {
    Io(io::Error),
    UntrustedPeer(String),
    InvalidSignature,
    Protocol(&'static str),
}
impl std::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for HandshakeError {}
impl From<io::Error> for HandshakeError {
    fn from(e: io::Error) -> Self { HandshakeError::Io(e) }
}

const MAGIC_CLIENT_HELLO: &[u8; 4] = b"CIH1";
const MAGIC_SERVER_HELLO: &[u8; 4] = b"SIH1";

fn write_msg<S: Write>(s: &mut S, magic: &[u8; 4], fields: &[&[u8]]) -> io::Result<()> {
    s.write_all(magic)?;
    for f in fields {
        s.write_all(&(f.len() as u32).to_le_bytes())?;
        s.write_all(f)?;
    }
    s.flush()
}

fn read_field<S: Read>(s: &mut S) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    s.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 1 << 20 {
        return Err(io::Error::other("el sikisma alani cok buyuk"));
    }
    let mut buf = vec![0u8; len];
    s.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_magic<S: Read>(s: &mut S, expected: &[u8; 4]) -> io::Result<()> {
    let mut magic = [0u8; 4];
    s.read_exact(&mut magic)?;
    if &magic != expected {
        return Err(io::Error::other("beklenmeyen el sikisma mesaji"));
    }
    Ok(())
}

/// İstemci tarafı (ör. `chimera-admin`, `chimera-sentinel`).
pub fn run_client_handshake<S: Read + Write>(
    stream: &mut S,
    identity: &chimera_crypto::obsidian::DsaKeypair,
    trust: &TrustStore,
) -> Result<[u8; 32], HandshakeError> {
    let kem_kp = obsidian::kem_generate_keypair();
    let ek_bytes = obsidian::kem_encapsulation_key_bytes(&kem_kp.encapsulation_key);
    let nonce_c = random32()?;

    let mut to_sign = ek_bytes.clone();
    to_sign.extend_from_slice(&nonce_c);
    let sig_c = obsidian::dsa_sign(&identity.signing_key, &to_sign);
    let sig_c_bytes = obsidian::dsa_signature_bytes(&sig_c);
    let client_vk_bytes = obsidian::dsa_verifying_key_bytes(&identity.verifying_key);

    write_msg(stream, MAGIC_CLIENT_HELLO, &[&client_vk_bytes, &ek_bytes, &nonce_c, &sig_c_bytes])?;

    read_magic(stream, MAGIC_SERVER_HELLO)?;
    let server_vk_bytes = read_field(stream)?;
    let ct = read_field(stream)?;
    let nonce_s = read_field(stream)?;
    let sig_s_bytes = read_field(stream)?;

    if !trust.is_trusted(&server_vk_bytes) {
        return Err(HandshakeError::UntrustedPeer(crate::trust::fingerprint_of(&server_vk_bytes)));
    }
    let server_vk = obsidian::dsa_verifying_key_from_bytes(&server_vk_bytes).map_err(|_| HandshakeError::InvalidSignature)?;
    let sig_s = obsidian::dsa_signature_from_bytes(&sig_s_bytes).map_err(|_| HandshakeError::InvalidSignature)?;

    let mut server_signed = ct.clone();
    server_signed.extend_from_slice(&nonce_c);
    server_signed.extend_from_slice(&nonce_s);
    obsidian::dsa_verify(&server_vk, &server_signed, &sig_s).map_err(|_| HandshakeError::InvalidSignature)?;

    let shared_secret = obsidian::kem_decapsulate(&kem_kp.decapsulation_key, &ct);
    Ok(derive_session_key(&shared_secret, &nonce_c, &nonce_s))
}

/// Sunucu tarafı (ör. `chimera-core`).
pub fn run_server_handshake<S: Read + Write>(
    stream: &mut S,
    identity: &chimera_crypto::obsidian::DsaKeypair,
    trust: &TrustStore,
) -> Result<[u8; 32], HandshakeError> {
    read_magic(stream, MAGIC_CLIENT_HELLO)?;
    let client_vk_bytes = read_field(stream)?;
    let ek_bytes = read_field(stream)?;
    let nonce_c = read_field(stream)?;
    let sig_c_bytes = read_field(stream)?;

    // *** ZORUNLU KAPI ***: guven deposunda olmayan bir istemci buradan
    // ASLA gecemez — imza matematiksel olarak gecerli olsa bile.
    if !trust.is_trusted(&client_vk_bytes) {
        return Err(HandshakeError::UntrustedPeer(crate::trust::fingerprint_of(&client_vk_bytes)));
    }
    let client_vk = obsidian::dsa_verifying_key_from_bytes(&client_vk_bytes).map_err(|_| HandshakeError::InvalidSignature)?;
    let sig_c = obsidian::dsa_signature_from_bytes(&sig_c_bytes).map_err(|_| HandshakeError::InvalidSignature)?;

    let mut client_signed = ek_bytes.clone();
    client_signed.extend_from_slice(&nonce_c);
    obsidian::dsa_verify(&client_vk, &client_signed, &sig_c).map_err(|_| HandshakeError::InvalidSignature)?;

    let ek = obsidian::kem_encapsulation_key_from_bytes(&ek_bytes).map_err(|_| HandshakeError::Protocol("gecersiz ephemeral KEM anahtari"))?;
    let (ct, shared_secret) = obsidian::kem_encapsulate(&ek);
    let nonce_s = random32()?;

    let mut to_sign = ct.clone();
    to_sign.extend_from_slice(&nonce_c);
    to_sign.extend_from_slice(&nonce_s);
    let sig_s = obsidian::dsa_sign(&identity.signing_key, &to_sign);
    let sig_s_bytes = obsidian::dsa_signature_bytes(&sig_s);
    let server_vk_bytes = obsidian::dsa_verifying_key_bytes(&identity.verifying_key);

    write_msg(stream, MAGIC_SERVER_HELLO, &[&server_vk_bytes, &ct, &nonce_s, &sig_s_bytes])?;

    Ok(derive_session_key(&shared_secret, &nonce_c, &nonce_s))
}

fn derive_session_key(shared_secret: &[u8], nonce_c: &[u8], nonce_s: &[u8]) -> [u8; 32] {
    let mut salt = Vec::with_capacity(nonce_c.len() + nonce_s.len());
    salt.extend_from_slice(nonce_c);
    salt.extend_from_slice(nonce_s);
    let hk = Hkdf::<Sha256>::new(Some(&salt), shared_secret);
    let mut okm = [0u8; 32];
    hk.expand(b"chimera-ipc-session-v1", &mut okm).expect("32 bayt HKDF cikisi gecerli");
    okm
}

fn random32() -> io::Result<Vec<u8>> {
    let mut b = vec![0u8; 32];
    getrandom::fill(&mut b).map_err(|e| io::Error::other(format!("{e:?}")))?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    fn loopback_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        (client, server)
    }

    fn temp_trust_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("chimera-ipc-hs-{name}-{}.list", std::process::id()))
    }

    #[test]
    fn mutual_handshake_both_sides_derive_identical_session_key() {
        let (client_stream, server_stream) = loopback_pair();
        let client_id = obsidian::dsa_generate_keypair();
        let server_id = obsidian::dsa_generate_keypair();

        let mut client_trust = TrustStore::load(&temp_trust_path("mutual-client")).unwrap();
        client_trust.trust(&obsidian::dsa_verifying_key_bytes(&server_id.verifying_key)).unwrap();
        let mut server_trust = TrustStore::load(&temp_trust_path("mutual-server")).unwrap();
        server_trust.trust(&obsidian::dsa_verifying_key_bytes(&client_id.verifying_key)).unwrap();

        let server_thread = std::thread::spawn(move || {
            let mut s = server_stream;
            run_server_handshake(&mut s, &server_id, &server_trust)
        });

        let mut c = client_stream;
        let client_key = run_client_handshake(&mut c, &client_id, &client_trust).unwrap();
        let server_key = server_thread.join().unwrap().unwrap();

        assert_eq!(client_key, server_key, "iki taraf da AYNI oturum anahtarina varmali");
    }

    #[test]
    fn untrusted_client_is_rejected_by_server() {
        let (client_stream, server_stream) = loopback_pair();
        let client_id = obsidian::dsa_generate_keypair();
        let server_id = obsidian::dsa_generate_keypair();

        // Sunucunun guven deposu BOS -- istemciyi hic tanimiyor.
        let server_trust = TrustStore::load(&temp_trust_path("reject-server")).unwrap();
        let mut client_trust = TrustStore::load(&temp_trust_path("reject-client")).unwrap();
        client_trust.trust(&obsidian::dsa_verifying_key_bytes(&server_id.verifying_key)).unwrap();

        let server_thread = std::thread::spawn(move || {
            let mut s = server_stream;
            run_server_handshake(&mut s, &server_id, &server_trust)
        });

        let mut c = client_stream;
        // Istemci gonderimini yapar ama sunucu ServerHello DONMEZ (baglantiyi keser).
        let _ = run_client_handshake(&mut c, &client_id, &client_trust);
        let server_result = server_thread.join().unwrap();
        assert!(matches!(server_result, Err(HandshakeError::UntrustedPeer(_))));
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let (client_stream, server_stream) = loopback_pair();
        let client_id = obsidian::dsa_generate_keypair();
        let server_id = obsidian::dsa_generate_keypair();

        let mut server_trust = TrustStore::load(&temp_trust_path("tamper-server")).unwrap();
        server_trust.trust(&obsidian::dsa_verifying_key_bytes(&client_id.verifying_key)).unwrap();
        // Baska bir (yanlis) istemci kimligini guvenilir kabul eden bir
        // istemci taklit edip gercek istemcinin private key'i OLMADAN
        // sunucuya baglanmaya calisirsa imza dogrulanamaz.
        let impostor_id = obsidian::dsa_generate_keypair();

        let server_thread = std::thread::spawn(move || {
            let mut s = server_stream;
            run_server_handshake(&mut s, &server_id, &server_trust)
        });

        // Impostor, GERCEK istemcinin acik anahtarini (client_id.verifying_key)
        // gonderiyormus gibi yapip kendi (impostor) gizli anahtariyla imzalarsa
        // -- imza dogrulamasi client_id.verifying_key ile basarisiz olmali.
        let mut c = client_stream;
        let ek_bytes = obsidian::kem_encapsulation_key_bytes(&obsidian::kem_generate_keypair().encapsulation_key);
        let nonce_c = vec![9u8; 32];
        let mut to_sign = ek_bytes.clone();
        to_sign.extend_from_slice(&nonce_c);
        let forged_sig = obsidian::dsa_sign(&impostor_id.signing_key, &to_sign); // YANLIS anahtarla imzalandi
        let sig_bytes = obsidian::dsa_signature_bytes(&forged_sig);
        let claimed_vk = obsidian::dsa_verifying_key_bytes(&client_id.verifying_key); // ama GERCEK kimlik iddia ediliyor

        write_msg(&mut c, MAGIC_CLIENT_HELLO, &[&claimed_vk, &ek_bytes, &nonce_c, &sig_bytes]).unwrap();

        let server_result = server_thread.join().unwrap();
        assert!(matches!(server_result, Err(HandshakeError::InvalidSignature)));
    }
}
