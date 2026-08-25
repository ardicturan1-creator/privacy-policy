//! El sıkışma sonrası oturum: her mesaj XChaCha20-Poly1305 ile ayrı,
//! rastgele bir nonce ile mühürlenir (192-bit genişletilmiş nonce, rastgele
//! üretimde çakışma riski ihmal edilebilir düzeydedir) ve 4 baytlık
//! uzunluk ön ekiyle çerçevelenir.
//!
//! `SecureChannel<S>` herhangi bir `Read + Write` üzerinde çalışır — bu
//! sayede gerçek bir Unix soketi/named pipe üzerinde de, testte bir
//! `TcpStream` çifti üzerinde de AYNI koddur.

use chimera_crypto::obsidian;
use std::io::{self, Read, Write};

const MAX_FRAME: u32 = 16 * 1024 * 1024; // 16 MiB — bellek tuketimi sinirlamasi

pub struct SecureChannel<S> {
    stream: S,
    key: [u8; 32],
}

impl<S: Read + Write> SecureChannel<S> {
    pub fn new(stream: S, session_key: [u8; 32]) -> Self {
        Self { stream, key: session_key }
    }

    pub fn send(&mut self, plaintext: &[u8]) -> io::Result<()> {
        let (nonce, ct) = obsidian::seal_data(&self.key, plaintext);
        let mut frame = Vec::with_capacity(24 + ct.len());
        frame.extend_from_slice(nonce.as_slice());
        frame.extend_from_slice(&ct);

        let len = frame.len() as u32;
        if len > MAX_FRAME {
            return Err(io::Error::other("cerceve MAX_FRAME'i asiyor"));
        }
        self.stream.write_all(&len.to_le_bytes())?;
        self.stream.write_all(&frame)?;
        self.stream.flush()
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
        let nonce = chacha20poly1305::aead::Nonce::<chacha20poly1305::XChaCha20Poly1305>::try_from(nonce_bytes)
            .map_err(|_| io::Error::other("gecersiz nonce"))?;
        obsidian::open_data(&self.key, &nonce, ct).map_err(|_| io::Error::other("AEAD dogrulama basarisiz — cerceve kurcalanmis olabilir"))
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
}
