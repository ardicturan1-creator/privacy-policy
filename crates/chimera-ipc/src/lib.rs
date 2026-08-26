//! chimera-ipc — Core (daemon) ile Admin (kontrol paneli) ve Sentinel
//! (watchdog eşi) arasındaki yerel haberleşme katmanı.
//!
//! "mTLS" ile aynı GÜVENLİK ÖZELLİĞİNİ (karşılıklı kimlik doğrulama +
//! uçtan uca şifreleme) sağlayan, ama X.509/rustls yığınını taşımayan,
//! özel bir protokoldür — bunu açıkça böyle adlandırıyoruz, X.509 TLS
//! olduğunu iddia etmiyoruz:
//!
//!   1. Her iki taraf da kalıcı bir ML-DSA-87 kimlik anahtar çiftine
//!      sahiptir (SSH host key / mTLS istemci sertifikası ile aynı rol).
//!   2. El sıkışmada taraflar geçici (ephemeral) bir ML-KEM-1024
//!      anahtarıyla bir paylaşılan sır kurar; bu kapsülleme, karşı
//!      tarafın kalıcı kimliğiyle İMZALANIR — yani hem ileri gizlilik
//!      (ephemeral anahtar) hem kimlik doğrulama (kalıcı imza) aynı
//!      anda sağlanır.
//!   3. **Karşı tarafın kalıcı açık anahtarı, önceden "güvenilir" olarak
//!      damgalanmamışsa bağlantı KURULAMAZ.** Kör TOFU (ilk-görüşte-güven)
//!      yoktur — bu, "iki bileşen birbirini kriptografik olarak
//!      doğrulamadan asla komut kabul etmesin" gereksinimidir.
//!   4. Oturum anahtarı HKDF-SHA256 ile türetilir; sonraki her mesaj
//!      XChaCha20-Poly1305 ile ayrı, rastgele nonce'larla mühürlenir.

pub mod attestation;
pub mod channel;
pub mod endpoint;
pub mod handshake;
pub mod identity;
pub mod protocol;
pub mod trust;

pub use attestation::AttestationStore;
pub use channel::SecureChannel;
pub use endpoint::socket_name;
pub use handshake::{run_client_handshake, run_server_handshake, HandshakeError};
pub use identity::Identity;
pub use protocol::{Request, Response};
pub use trust::TrustStore;

/// `SecureChannel` uzerinde tek bir `Request` gonderir ve `Response` bekler.
/// Protokol kodlamasini kanal katmanindan ayri tutmak icin kucuk bir yardimci.
pub fn call<S: std::io::Read + std::io::Write>(channel: &mut SecureChannel<S>, req: &Request) -> Result<Response, IpcError> {
    channel.send(&req.encode())?;
    let raw = channel.recv()?;
    Response::decode(&raw).map_err(IpcError::Io)
}

pub fn serve_one<S: std::io::Read + std::io::Write>(
    channel: &mut SecureChannel<S>,
    handle: impl FnOnce(Request) -> Response,
) -> Result<(), IpcError> {
    let raw = channel.recv()?;
    let req = Request::decode(&raw).map_err(IpcError::Io)?;
    let resp = handle(req);
    channel.send(&resp.encode())?;
    Ok(())
}

#[derive(Debug)]
pub enum IpcError {
    Io(std::io::Error),
    Handshake(HandshakeError),
    Crypto(chimera_crypto::obsidian::ObsidianError),
    Protocol(&'static str),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for IpcError {}

impl From<std::io::Error> for IpcError {
    fn from(e: std::io::Error) -> Self { IpcError::Io(e) }
}
impl From<HandshakeError> for IpcError {
    fn from(e: HandshakeError) -> Self { IpcError::Handshake(e) }
}
