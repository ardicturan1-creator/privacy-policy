//! El sıkışma sonrası oturum: her mesaj XChaCha20-Poly1305 ile ayrı,
//! rastgele bir nonce ile mühürlenir (192-bit genişletilmiş nonce, rastgele
//! üretimde çakışma riski ihmal edilebilir düzeydedir) ve 4 baytlık
//! uzunluk ön ekiyle çerçevelenir.
//!
//! **Replay/yeniden-sıralama koruması** (standart AEAD kullanımı — özel bir
//! kriptografik ilkel DEĞİL): her yönün kendi monoton artan 64-bit sıra
//! sayacı vardır ve bu sayaç, mesajın AEAD "associated data" (AAD) alanına
//! konur. AAD, şifrelenmez ama kimlik doğrulama etiketine dahildir — bir
//! saldırgan sırayı değiştirip eski bir çerçeveyi yeniden oynatırsa (replay)
//! ya da çerçeveleri yer değiştirirse, AAD uyuşmazlığı AEAD doğrulamasını
//! GERÇEKTEN başarısız kılar. Alıcı ayrıca gelen sıra numarasının TAM
//! OLARAK beklenenle eşleşmesini ister — "büyükse kabul et" değil, "tam
//! sıradaki mi" kontrolü, ekleme (injection) ve atlama saldırılarını da
//! yakalar. `PROTOCOL_VERSION` de aynı AAD'ye eklenir: sürüm uyumsuzluğu
//! da sessizce yanlış ayrıştırma yerine AEAD hatasına döner.
//!
//! `SecureChannel<S>` herhangi bir `Read + Write` üzerinde çalışır — bu
//! sayede gerçek bir Unix soketi/named pipe üzerinde de, testte bir
//! `TcpStream` çifti üzerinde de AYNI koddur.

use chacha20poly1305::aead::{Aead, Generate, KeyInit, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use std::io::{self, Read, Write};

const MAX_FRAME: u32 = 16 * 1024 * 1024; // 16 MiB — bellek tuketimi sinirlamasi
pub const PROTOCOL_VERSION: u8 = 1;

pub struct SecureChannel<S> {
    stream: S,
    key: [u8; 32],
    send_seq: u64,
    recv_seq: u64,
}

fn aad_for(seq: u64) -> [u8; 9] {
    let mut aad = [0u8; 9];
    aad[0] = PROTOCOL_VERSION;
    aad[1..9].copy_from_slice(&seq.to_le_bytes());
    aad
}

impl<S: Read + Write> SecureChannel<S> {
    pub fn new(stream: S, session_key: [u8; 32]) -> Self {
        Self { stream, key: session_key, send_seq: 0, recv_seq: 0 }
    }

    pub fn send(&mut self, plaintext: &[u8]) -> io::Result<()> {
        let nonce = chacha20poly1305::aead::Nonce::<XChaCha20Poly1305>::generate();
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let aad = aad_for(self.send_seq);
        let ct = cipher
            .encrypt(&nonce, Payload { msg: plaintext, aad: &aad })
            .map_err(|_| io::Error::other("muhurleme basarisiz"))?;

        let mut frame = Vec::with_capacity(24 + ct.len());
        frame.extend_from_slice(nonce.as_slice());
        frame.extend_from_slice(&ct);

        let len = frame.len() as u32;
        if len > MAX_FRAME {
            return Err(io::Error::other("cerceve MAX_FRAME'i asiyor"));
        }
        self.stream.write_all(&len.to_le_bytes())?;
        self.stream.write_all(&frame)?;
        self.stream.flush()?;

        // `checked_add`: 64-bit sayac 2^64 mesajda taser (pratikte
        // imkansiz), ama tasma UB/panik yerine acik bir hataya donusur.
        self.send_seq = self.send_seq.checked_add(1).ok_or_else(|| io::Error::other("oturum sirasi tukendi"))?;
        Ok(())
    }

    pub fn recv(&mut self) -> io::Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf);
        if len > MAX_FRAME || len < 24 {
            return Err(io::Error::other("gecersiz cerceve boyutu"));
        }
        let mut frame = vec![0u8; len as usize];
        self.stream.read_exact(&mut frame)?;

        let nonce_bytes = &frame[..24];
        let ct = &frame[24..];
        let nonce = chacha20poly1305::aead::Nonce::<XChaCha20Poly1305>::try_from(nonce_bytes)
            .map_err(|_| io::Error::other("gecersiz nonce"))?;

        let aad = aad_for(self.recv_seq);
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let plaintext = cipher
            .decrypt(&nonce, Payload { msg: ct, aad: &aad })
            .map_err(|_| io::Error::other("AEAD dogrulama basarisiz — cerceve kurcalanmis, yeniden oynatilmis veya sira disi olabilir"))?;

        self.recv_seq = self.recv_seq.checked_add(1).ok_or_else(|| io::Error::other("oturum sirasi tukendi"))?;
        Ok(plaintext)
    }

    pub fn into_inner(self) -> S {
        self.stream
    }
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

    #[test]
    fn encrypted_round_trip_over_real_socket() {
        let (client_stream, server_stream) = loopback_pair();
        let key = [42u8; 32];

        let mut client = SecureChannel::new(client_stream, key);
        let mut server = SecureChannel::new(server_stream, key);

        client.send(b"merhaba core").unwrap();
        let received = server.recv().unwrap();
        assert_eq!(received, b"merhaba core");

        server.send(b"merhaba admin").unwrap();
        let received2 = client.recv().unwrap();
        assert_eq!(received2, b"merhaba admin");
    }

    #[test]
    fn tampered_frame_is_rejected() {
        let (client_stream, mut server_stream) = loopback_pair();
        let key = [7u8; 32];
        let mut client = SecureChannel::new(client_stream, key);

        client.send(b"gizli komut").unwrap();

        // Sunucu tarafinda hamimini kendi elimizle okuyup bir bayti bozup
        // "yeniden gonderiyoruz" — gercek bir saldirganin araya girip
        // paketi degistirmesini simule eder.
        let mut len_buf = [0u8; 4];
        server_stream.read_exact(&mut len_buf).unwrap();
        let len = u32::from_le_bytes(len_buf);
        let mut frame = vec![0u8; len as usize];
        server_stream.read_exact(&mut frame).unwrap();
        frame[30] ^= 0xFF; // ciphertext bolgesinde bir bayt boz

        let (relay_client, relay_server) = loopback_pair();
        let mut writer = relay_client;
        writer.write_all(&len_buf).unwrap();
        writer.write_all(&frame).unwrap();
        drop(writer);

        let mut victim = SecureChannel::new(relay_server, key);
        let result = victim.recv();
        assert!(result.is_err(), "kurcalanmis cerceve KABUL EDILMEMELI");
    }

    /// GERÇEK bir replay saldırısı simülasyonu: aynı geçerli çerçeve iki kez
    /// gönderilir. AAD'deki sıra numarası ikinci teslimatta beklenenle
    /// uyuşmadığı için AEAD doğrulaması BAŞARISIZ olmalı — çerçevenin
    /// kendisi kriptografik olarak tamamen geçerli olsa bile.
    #[test]
    fn replayed_valid_frame_is_rejected() {
        let (client_stream, mut server_stream) = loopback_pair();
        let key = [3u8; 32];
        let mut client = SecureChannel::new(client_stream, key);
        client.send(b"para transferi: 1 kez").unwrap();

        let mut len_buf = [0u8; 4];
        server_stream.read_exact(&mut len_buf).unwrap();
        let len = u32::from_le_bytes(len_buf);
        let mut frame = vec![0u8; len as usize];
        server_stream.read_exact(&mut frame).unwrap();

        // Ayni GERCEK (bozulmamis) cerceveyi taze bir alicidan IKI KEZ okut.
        let (relay_client, relay_server) = loopback_pair();
        let mut writer = relay_client;
        writer.write_all(&len_buf).unwrap();
        writer.write_all(&frame).unwrap();
        writer.write_all(&len_buf).unwrap();
        writer.write_all(&frame).unwrap();
        drop(writer);

        let mut victim = SecureChannel::new(relay_server, key);
        let first = victim.recv();
        assert!(first.is_ok(), "ilk (gercek, ilk kez gorulen) teslimat kabul edilmeli");
        let second = victim.recv();
        assert!(second.is_err(), "AYNI cerceve IKINCI KEZ geldiginde REDDEDILMELI (replay)");
    }

    /// Sira numarasi manuel olarak "ileri sarilirsa" (ornegin bir saldirgan
    /// aradaki bir cerceveyi dusurup sonrakini erken sunmaya calisirsa) bu
    /// da reddedilir — yalnizca TAM OLARAK beklenen sira kabul edilir.
    #[test]
    fn out_of_order_frame_is_rejected() {
        let (client_stream, server_stream) = loopback_pair();
        let key = [9u8; 32];
        let mut client = SecureChannel::new(client_stream, key);
        let mut server = SecureChannel::new(server_stream, key);

        client.send(b"birinci").unwrap();
        client.send(b"ikinci").unwrap();

        // Alicinin sira sayacini manuel olarak ileri kaydirip "birinci"yi
        // atlamis gibi yapiyoruz -- bu, kayip/yeniden-siralanmis bir
        // cercevenin GERCEK etkisini simule eder.
        server.recv_seq = 1;
        let result = server.recv(); // burada aslinda "birinci" (seq=0) bekleniyor
        assert!(result.is_err(), "beklenen sira ile eslesmeyen cerceve reddedilmeli");
    }
}
