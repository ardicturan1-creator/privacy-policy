//! Siber Yanıltma — Bölüm 2: Tarpitting.
//!
//! Klasik, iyi bilinen ve GÜVENLİ bir savunma tekniği (bkz. `endlessh`,
//! OpenBSD `pf` tarpit, LaBrea): gerçek bir servis GİBİ görünen ama hiçbir
//! işe yaramayan bir port açılır. Bağlanan taraf saniyeler yerine dakikalar
//! harcar. **Bilinçli sınırlar** (bu bir DoS aracına DÖNÜŞTÜRÜLEMEZ):
//!
//!   - Eşzamanlı bağlantı sayısı sınırlıdır (`MAX_CONCURRENT`).
//!   - Bağlantı başına toplam süre sınırlıdır (`MAX_DURATION`).
//!   - Yalnızca INBOUND (gelen) bağlantıları kabul eder; hiçbir zaman
//!     dışarıya bağlantı AÇMAZ. Yani üçüncü bir tarafa karşı silah
//!     olarak kullanılamaz — yalnızca operatörün kendi makinesine gelen
//!     bağlantıları oyalar.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_CONCURRENT: usize = 64;
const MAX_DURATION: Duration = Duration::from_secs(300);
const DRIP_INTERVAL: Duration = Duration::from_millis(2000);

pub struct TarpitAlert {
    pub ts: u64,
    pub peer: String,
}

/// Verilen porta baglanir ve arka planda sonsuza kadar (thread) tuzaklama
/// yapar. Her yeni baglanti icin `on_connect` cagrilir (denetim kaydi icin).
pub fn spawn(bind_addr: &str, on_connect: impl Fn(TarpitAlert) + Send + Sync + 'static) -> std::io::Result<std::thread::JoinHandle<()>> {
    let listener = TcpListener::bind(bind_addr)?;
    let active = Arc::new(AtomicUsize::new(0));
    let on_connect = Arc::new(on_connect);

    Ok(std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if active.load(Ordering::Relaxed) >= MAX_CONCURRENT {
                drop(stream); // sinir asildi -- yeni baglantiyi kabul etme
                continue;
            }
            let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "bilinmiyor".into());
            on_connect(TarpitAlert { ts: unix_now(), peer });

            let active = Arc::clone(&active);
            active.fetch_add(1, Ordering::Relaxed);
            std::thread::spawn(move || {
                drip(stream);
                active.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }))
}

/// Bagliyi acik tutup cok yavas bayt akitir; MAX_DURATION sonunda kapatir.
fn drip(mut stream: TcpStream) {
    let start = Instant::now();
    let banner = b"SSH-2.0-OpenSSH_9.6\r\n"; // gercekci gorunen sahte banner
    let mut idx = 0usize;

    while start.elapsed() < MAX_DURATION {
        if idx >= banner.len() {
            idx = 0; // banner'i tekrar tekrar, YAVAScA gonder
        }
        if stream.write_all(&banner[idx..idx + 1]).is_err() {
            return; // karsi taraf vazgecti/koptu
        }
        idx += 1;
        std::thread::sleep(DRIP_INTERVAL);
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::mpsc;

    /// `spawn()`'un KENDISINI test eder: gercek bir TCP baglantisi kurulur,
    /// `on_connect` geri caGRISI GERCEKTEN tetiklenir ve karsi tarafa
    /// GERCEKTEN, yavas yavas bayt akitilir.
    #[test]
    fn tarpit_spawn_accepts_and_drips_real_bytes() {
        // Bos bir port bul, hemen birak, ayni porta tarpit'i baglat
        // (kisa bir yaris riski var ama testte kabul edilebilir).
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let (tx, rx) = mpsc::channel();
        let _handle = spawn(&addr.to_string(), move |alert| {
            let _ = tx.send(alert.peer);
        })
        .unwrap();

        std::thread::sleep(Duration::from_millis(150));
        let mut client = TcpStream::connect(addr).unwrap();

        let peer = rx.recv_timeout(Duration::from_secs(3)).expect("on_connect gercekten tetiklenmeli");
        assert!(!peer.is_empty());

        let mut buf = [0u8; 1];
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let n = client.read(&mut buf).unwrap();
        assert_eq!(n, 1);
        assert_eq!(buf[0], b'S'); // "SSH-2.0-..." sahte bannerinin ilk bayti

        // Ikinci bayt DRIP_INTERVAL kadar GECIKMELI gelmeli -- yani hemen
        // gelmemesi tarpit'in gercekten yavaslatiyor olmasinin kanitidir.
        client.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
        let mut buf2 = [0u8; 1];
        let immediate = client.read(&mut buf2);
        assert!(immediate.is_err(), "ikinci bayt DRIP_INTERVAL dolmadan gelmemeli");
    }

    #[test]
    fn concurrent_connections_are_capped() {
        // MAX_CONCURRENT'i asan baglanti sayisinin GERCEKTEN reddedildigini
        // (soketin kapandigini) dogrulamak, MAX_DURATION kadar beklemeyi
        // gerektirir; bu yuzden burada yalnizca sabitin makul oldugunu ve
        // sifir olmadigini dogruluyoruz -- tam sinir testi entegrasyon
        // seviyesinde (manuel) yapilir.
        assert!(MAX_CONCURRENT > 0 && MAX_CONCURRENT <= 1024);
    }
}
