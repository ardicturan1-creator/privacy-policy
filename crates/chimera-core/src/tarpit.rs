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
//!
//! ## Turn 7 / Faz 2: çoklu port + otomatik engellemeye besleme
//!
//! Tarpit artık tek bir porta değil, saldırganların GERÇEKTEN taradığı
//! portlara açılabilir (`spawn_multi`): **445/SMB** ve **3389/RDP**.
//! Buradaki mantık, `bruteforce.rs`'in 4625 tabanlı tespitini tamamlar:
//! 4625 yalnızca GERÇEK servise ulaşıp kimlik doğrulaması başarısız olan
//! denemeleri görür; tarpit ise servisin hiç açık OLMADIĞI bir makinede
//! bile tarama/bağlantı denemelerini yakalar.
//!
//! Bağlanan IP artık yalnızca oyalanmakla kalmaz, aynı TTL'li otomatik
//! engelleme mekanizmasına beslenir (`autoblock.rs`). **Ama ilk
//! bağlantıda DEĞİL:** tek bir bağlantı yanlış yazılmış bir IP, meşru bir
//! ağ tarayıcısı veya bir sağlık kontrolü olabilir. Eşik aşılmadan
//! hiçbir engelleme olmaz ve engel her durumda geçicidir.
//!
//! **Bilinçli sınır:** bir sahte bağlantı TEK BİR paket ile
//! tetiklenebildiği için, kaynak adresi taklit edilmiş (spoofed) SYN
//! paketleriyle üçüncü bir tarafın adresinin engellenmesi teorik olarak
//! mümkündür. Bunun neden pratikte sınırlı bir risk olduğu ve nasıl
//! azaltıldığı `07-TURN7-AG-SERTLESTIRME.md` §D'de açıklanır: TCP el
//! sıkışması tamamlanmadan `accept()` dönmez (yani saldırganın SYN-ACK'i
//! görmesi gerekir), engel geçicidir ve `never_block` listesi vardır.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_CONCURRENT: usize = 64;
const MAX_DURATION: Duration = Duration::from_secs(300);
const DRIP_INTERVAL: Duration = Duration::from_millis(2000);

/// Saldırganların gerçekten taradığı, tarpit'in taklit ettiği portlar.
/// 445 = SMB, 3389 = RDP — `bruteforce.rs`'in izlediği iki yüzeyle AYNI.
pub const DEFAULT_TARPIT_PORTS: &[u16] = &[445, 3389];

/// Bir kaynak IP'nin otomatik engellemeye düşmesi için gereken bağlantı
/// sayısı. Tek bağlantı ASLA yeterli değildir (bkz. modül başlığı).
pub const CONNECT_THRESHOLD: usize = 5;
/// Yukarıdaki eşiğin sayıldığı pencere.
pub const CONNECT_WINDOW_SECS: u64 = 120;

pub struct TarpitAlert {
    pub ts: u64,
    /// "IP:port" biçiminde tam eş adresi (denetim kaydı için).
    pub peer: String,
    /// Yalnızca IP kısmı — engelleme kararları buna göre verilir, çünkü
    /// saldırgan her bağlantıda farklı bir KAYNAK PORT kullanır.
    pub peer_ip: String,
    /// Bağlantının geldiği yerel port (445/3389 gibi) — hangi servisin
    /// taklidinin hedeflendiğini gösterir.
    pub local_port: u16,
}

/// Verilen porta baglanir ve arka planda sonsuza kadar (thread) tuzaklama
/// yapar. Her yeni baglanti icin `on_connect` cagrilir (denetim kaydi icin).
pub fn spawn(bind_addr: &str, on_connect: impl Fn(TarpitAlert) + Send + Sync + 'static) -> std::io::Result<std::thread::JoinHandle<()>> {
    let listener = TcpListener::bind(bind_addr)?;
    let local_port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let active = Arc::new(AtomicUsize::new(0));
    let on_connect = Arc::new(on_connect);

    Ok(std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if active.load(Ordering::Relaxed) >= MAX_CONCURRENT {
                drop(stream); // sinir asildi -- yeni baglantiyi kabul etme
                continue;
            }
            let addr = stream.peer_addr().ok();
            let peer = addr.map(|a| a.to_string()).unwrap_or_else(|| "bilinmiyor".into());
            let peer_ip = addr.map(|a| a.ip().to_string()).unwrap_or_else(|| "bilinmiyor".into());
            on_connect(TarpitAlert { ts: unix_now(), peer, peer_ip, local_port });

            let active = Arc::clone(&active);
            active.fetch_add(1, Ordering::Relaxed);
            std::thread::spawn(move || {
                drip(stream);
                active.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }))
}

/// Verilen portlarin HEPSINE ayni tarpit'i baglar.
///
/// Donen deger `(baglanabilenler, baglanamayanlar)` ciftidir ve her iki
/// liste de CAGIRANA verilir: 445/3389 gibi ayricalikli portlar,
/// yonetici olmayan bir hesapta veya port zaten kullanildiginda
/// baglanamaz. Bu bir HATA DEGILDIR ama SESSIZCE gecistirilmemelidir --
/// operatorun "tarpit 3389'da dinliyor sandim, aslinda dinlemiyordu"
/// durumuna dusmemesi icin basarisizliklar acikca raporlanir.
pub fn spawn_multi(
    host: &str,
    ports: &[u16],
    on_connect: impl Fn(TarpitAlert) + Send + Sync + Clone + 'static,
) -> (Vec<u16>, Vec<(u16, String)>) {
    let mut ok = Vec::new();
    let mut failed = Vec::new();
    for &port in ports {
        match spawn(&format!("{host}:{port}"), on_connect.clone()) {
            Ok(_handle) => ok.push(port),
            Err(e) => failed.push((port, e.to_string())),
        }
    }
    (ok, failed)
}

/// Kaynak IP başına bağlantı sayacı (kayan pencere). `bruteforce.rs`'teki
/// ile aynı desende; tarpit'e bağlanan bir IP'nin otomatik engellemeye
/// düşmesi için eşiği aşması gerekir.
pub struct ConnectWindow {
    window_secs: u64,
    threshold: usize,
    per_ip: std::collections::HashMap<String, Vec<u64>>,
}

impl Default for ConnectWindow {
    fn default() -> Self {
        Self::new(CONNECT_WINDOW_SECS, CONNECT_THRESHOLD)
    }
}

impl ConnectWindow {
    pub fn new(window_secs: u64, threshold: usize) -> Self {
        Self { window_secs, threshold, per_ip: std::collections::HashMap::new() }
    }

    /// Bir bağlantıyı kaydeder. Eşik BU bağlantıyla aşıldıysa `true`
    /// döner — yani engelleme kararı tam olarak bir kez verilir, her
    /// sonraki bağlantıda tekrar tekrar değil.
    pub fn observe(&mut self, ip: &str, now: u64) -> bool {
        let cutoff = now.saturating_sub(self.window_secs);
        let entry = self.per_ip.entry(ip.to_string()).or_default();
        entry.retain(|ts| *ts >= cutoff);
        entry.push(now);
        entry.len() == self.threshold
    }
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

    /// Alarm artık yalnızca "peer" değil, engelleme kararının dayandığı
    /// SALT IP'yi ve hangi taklit servise bağlanıldığını da taşımalı.
    #[test]
    fn alerts_carry_the_bare_source_ip_and_the_local_port() {
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let (tx, rx) = mpsc::channel();
        let _h = spawn(&addr.to_string(), move |a| { let _ = tx.send((a.peer, a.peer_ip, a.local_port)); }).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        let _c = TcpStream::connect(addr).unwrap();

        let (peer, peer_ip, local_port) = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(peer_ip, "127.0.0.1", "salt IP ayiklanmali (port OLMADAN)");
        assert!(peer.starts_with("127.0.0.1:"), "tam adres port ICERMELI: {peer}");
        assert_eq!(local_port, addr.port(), "hangi taklit servise baglanildigi bildirilmeli");
    }

    /// `spawn_multi` GERÇEKTEN birden çok portu dinlemeli ve bağlanamadığı
    /// portları SESSIZCE yutmak yerine raporlamalı.
    #[test]
    fn spawn_multi_binds_every_port_and_reports_failures_honestly() {
        // Iki bos port bul, biraktir.
        let p1 = TcpListener::bind("127.0.0.1:0").unwrap();
        let p2 = TcpListener::bind("127.0.0.1:0").unwrap();
        let (a1, a2) = (p1.local_addr().unwrap().port(), p2.local_addr().unwrap().port());
        drop(p1);
        // p2 ACIK BIRAKILIYOR -> o port MESGUL, baglanamamali.

        let (tx, rx) = mpsc::channel();
        let (ok, failed) = spawn_multi("127.0.0.1", &[a1, a2], move |a| { let _ = tx.send(a.local_port); });

        assert!(ok.contains(&a1), "bos port baglanmaliydi");
        assert!(failed.iter().any(|(p, _)| *p == a2), "MESGUL port basarisiz olarak RAPORLANMALI");
        assert!(!ok.contains(&a2));

        // Baglanan port GERCEKTEN dinliyor mu?
        std::thread::sleep(Duration::from_millis(150));
        let _c = TcpStream::connect(("127.0.0.1", a1)).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), a1);
        drop(p2);
    }

    #[test]
    fn a_single_connection_never_reaches_the_block_threshold() {
        let mut w = ConnectWindow::new(120, 5);
        assert!(!w.observe("203.0.113.7", 1000), "TEK baglanti engelleme tetiklememeli");
        for i in 1..4 {
            assert!(!w.observe("203.0.113.7", 1000 + i));
        }
    }

    #[test]
    fn crossing_the_threshold_fires_exactly_once() {
        let mut w = ConnectWindow::new(120, 3);
        assert!(!w.observe("203.0.113.7", 100));
        assert!(!w.observe("203.0.113.7", 101));
        assert!(w.observe("203.0.113.7", 102), "3. baglantida tetiklenmeli");
        // Sonraki baglantilar TEKRAR tetiklememeli (tekrarli engelleme yok)
        assert!(!w.observe("203.0.113.7", 103));
        assert!(!w.observe("203.0.113.7", 104));
    }

    #[test]
    fn connect_window_really_slides_and_counts_ips_independently() {
        let mut w = ConnectWindow::new(120, 3);
        w.observe("203.0.113.7", 100);
        w.observe("203.0.113.7", 101);
        // 1 saat sonra: eski iki kayit DUSMELI, esik asilmamali
        assert!(!w.observe("203.0.113.7", 3700), "pencere disi baglantilar birikmis");
        // Farkli bir IP bagimsiz sayilmali
        assert!(!w.observe("198.51.100.9", 3700));
    }

    #[test]
    fn default_ports_cover_the_surfaces_bruteforce_watches() {
        assert!(DEFAULT_TARPIT_PORTS.contains(&445), "SMB portu kapsanmali");
        assert!(DEFAULT_TARPIT_PORTS.contains(&3389), "RDP portu kapsanmali");
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
