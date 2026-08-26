//! heuristic.rs — Fidye yazılımının DAVRANIŞINI yakalayan, saf (platformdan
//! bağımsız) ve tamamen test edilebilir tespit mantığı.
//!
//! `decoy.rs` bir tuzağa DOKUNULMASINI yakalar; ama düzgün yazılmış bir
//! fidye yazılımı tuzak dosyalarımıza hiç dokunmadan da gerçek kullanıcı
//! verisini şifreleyebilir. Bu modül o boşluğu kapatır: bir sürecin KISA
//! BİR PENCEREDE, ÇOK SAYIDA, BİRBİRİNDEN FARKLI dosyayı YÜKSEK ENTROPİLİ
//! içerikle üzerine yazması, şifrelemenin kendisinin bıraktığı istatistiksel
//! izdir (şifreli/sıkıştırılmış veri, düz metin/ofis belgelerinin aksine
//! bayt başına ~8 bit entropiye yaklaşır).
//!
//! **Bilinçli sınırlar — bunlar YANLIŞ POZİTİF kaynaklarıdır ve
//! `08`/`06` numaralı belgelerde açıkça sayılmıştır:**
//!
//!   - Yüksek entropi TEK BAŞINA kötü niyet KANITI DEĞİLDİR: bir yedekleme
//!     aracının yazdığı `.zip`/`.7z`, bir video dışa aktarımı (`.mp4`) veya
//!     meşru bir disk şifreleme aracı da aynı istatistiği üretir. Bu yüzden
//!     tetikleyici tek bir dosya değil, **kısa pencerede çok sayıda FARKLI
//!     dosya** olmak zorundadır.
//!   - Aynı dosyaya arka arkaya yazma (bir dosyanın parça parça yazılması
//!     işletim sisteminden ONLARCA ayrı olay üretir) SAYILMAZ — yalnızca
//!     BENZERSİZ yollar sayılır. Bu, "tek bir büyük arşivi yazan meşru bir
//!     araç" senaryosunun devre kesiciyi tetiklemesini önler.
//!   - Küçük örneklerde entropi matematiksel olarak yanıltıcıdır: `n`
//!     baytlık bir örneğin entropisi en fazla `log2(n)` olabilir, yani
//!     rastgele 16 baytlık bir dosya bile 8.0'a ulaşamaz. Bu yüzden
//!     `MIN_SAMPLE_BYTES`'tan küçük örnekler DEĞERLENDİRİLMEZ (`None`).
//!
//! Bu modül hiçbir aksiyon ALMAZ — yalnızca bir `Trip` üretir. Aksiyonu
//! `circuit_breaker.rs` alır, ve orada da kalıcı/geri alınamaz hiçbir şey
//! insan onayı olmadan yapılmaz.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::Path;

/// Bayt başına bit cinsinden entropi eşiği (teorik üst sınır 8.0).
/// Şifreli/sıkıştırılmış veri pratikte 7.9+ üretir; düz metin ~4.5,
/// tipik bir Office/OOXML belgesi (zaten zip'li) ~7.9'a yakındır — bu
/// yüzden eşik TEK BAŞINA yeterli değildir, hız ile birlikte kullanılır.
pub const HIGH_ENTROPY_BITS: f64 = 7.5;

/// Entropi ölçümü için dosyanın BAŞINDAN okunan en fazla bayt sayısı.
/// Tüm dosyayı okumak, yüzlerce dosyanın saniyeler içinde değiştiği bir
/// fidye yazılımı senaryosunda izleyicinin KENDİSİNİ darboğaza çevirirdi.
pub const SAMPLE_BYTES: usize = 4096;

/// Bu boyutun altındaki örnekler değerlendirilmez (bkz. modül başlığı).
pub const MIN_SAMPLE_BYTES: usize = 512;

/// Varsayılan hız penceresi (saniye).
pub const DEFAULT_WINDOW_SECS: u64 = 60;

/// Varsayılan eşik: bu pencerede bu kadar FARKLI dosya.
pub const DEFAULT_FILE_THRESHOLD: usize = 12;

/// Shannon entropisi, bayt başına bit (0.0 ..= 8.0).
pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    let mut h = 0.0f64;
    for &c in counts.iter() {
        if c == 0 {
            continue;
        }
        let p = c as f64 / len;
        h -= p * p.log2();
    }
    h
}

/// Bir dosyanın BAŞINDAN en fazla `SAMPLE_BYTES` okuyup entropisini döner.
/// Örnek `MIN_SAMPLE_BYTES`'tan küçükse `None` — "bilmiyorum" demek,
/// küçük bir dosyayı yanlışlıkla "düşük entropili/zararsız" saymaktan
/// daha dürüsttür.
pub fn sample_file_entropy(path: &Path) -> Option<f64> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; SAMPLE_BYTES];
    let mut filled = 0usize;
    // `read` kısa okuma yapabilir; dosya sonuna ya da tampon dolana kadar
    // döngüyle okuruz (tek bir `read` cagrisina guvenmek gercek bir hata
    // kaynagidir).
    loop {
        match f.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => {
                filled += n;
                if filled == buf.len() {
                    break;
                }
            }
            Err(_) => return None,
        }
    }
    if filled < MIN_SAMPLE_BYTES {
        return None;
    }
    Some(shannon_entropy(&buf[..filled]))
}

/// Devre kesiciyi tetikleyen kanıt paketi.
#[derive(Debug, Clone, PartialEq)]
pub struct Trip {
    pub pid: u32,
    /// Pencerede yüksek entropili yazma görülen BENZERSİZ dosya sayısı.
    pub distinct_files: usize,
    pub window_secs: u64,
    /// Denetim kaydına yazılmak üzere birkaç örnek yol (tamamı değil —
    /// yüzlerce yolun denetim kaydını şişirmesini önlemek için).
    pub sample_paths: Vec<String>,
}

impl Trip {
    pub fn as_detail(&self) -> String {
        format!(
            "pid={} {} saniyelik pencerede {} FARKLI dosyaya yuksek entropili ({:.1}+ bit/bayt) yazdi. Ornek: {}",
            self.pid,
            self.window_secs,
            self.distinct_files,
            HIGH_ENTROPY_BITS,
            self.sample_paths.join(", ")
        )
    }
}

/// PID başına kayan pencere sayacı. Saf bir veri yapısıdır: saat DIŞARIDAN
/// (`now` parametresi) verilir, böylece testler gerçek zamanı beklemek
/// zorunda kalmaz ve pencere mantığı GERÇEKTEN doğrulanabilir.
pub struct RansomHeuristic {
    window_secs: u64,
    file_threshold: usize,
    min_entropy: f64,
    /// pid -> (zaman, yol) kuyruğu; yol BENZERSİZDİR.
    per_pid: HashMap<u32, VecDeque<(u64, String)>>,
    /// Bir kez tetiklenen PID tekrar tekrar tetiklenmez — devre kesici
    /// zaten süreci askıya aldı ve insan onayı bekliyor; her yeni olayda
    /// yeni bir alarm üretmek denetim kaydını GÜRÜLTÜYE boğardı.
    tripped: HashSet<u32>,
}

impl Default for RansomHeuristic {
    fn default() -> Self {
        Self::new(DEFAULT_WINDOW_SECS, DEFAULT_FILE_THRESHOLD, HIGH_ENTROPY_BITS)
    }
}

impl RansomHeuristic {
    pub fn new(window_secs: u64, file_threshold: usize, min_entropy: f64) -> Self {
        Self { window_secs, file_threshold, min_entropy, per_pid: HashMap::new(), tripped: HashSet::new() }
    }

    /// Tek bir "şu PID şu dosyayı şu entropiyle yazdı" gözlemini işler.
    /// Eşik aşıldıysa bir `Trip` döner (ve aynı PID için bir daha dönmez,
    /// `forget` çağrılana kadar).
    pub fn observe(&mut self, pid: u32, path: &str, entropy: f64, now: u64) -> Option<Trip> {
        if entropy < self.min_entropy {
            return None; // dusuk entropi: sifreleme izi degil, sayilmaz
        }
        if self.tripped.contains(&pid) {
            return None;
        }

        let q = self.per_pid.entry(pid).or_default();
        let cutoff = now.saturating_sub(self.window_secs);
        while q.front().map(|(ts, _)| *ts < cutoff).unwrap_or(false) {
            q.pop_front();
        }

        // AYNI dosyaya tekrar yazma yeni bir kanit DEGILDIR (bkz. modul
        // basligi) -- yalnizca zaman damgasi tazelenir.
        if let Some(slot) = q.iter_mut().find(|(_, p)| p == path) {
            slot.0 = now;
        } else {
            q.push_back((now, path.to_string()));
        }

        if q.len() >= self.file_threshold {
            let sample_paths: Vec<String> = q.iter().rev().take(3).map(|(_, p)| p.clone()).collect();
            let distinct_files = q.len();
            self.tripped.insert(pid);
            return Some(Trip { pid, distinct_files, window_secs: self.window_secs, sample_paths });
        }
        None
    }

    /// Bir PID'in geçmişini ve "tetiklendi" işaretini siler. İnsan onayıyla
    /// `resume-process` yapıldığında çağrılır: aksi halde devam eden meşru
    /// bir süreç bir daha ASLA alarm üretemezdi.
    pub fn forget(&mut self, pid: u32) {
        self.per_pid.remove(&pid);
        self.tripped.remove(&pid);
    }

    pub fn has_tripped(&self, pid: u32) -> bool {
        self.tripped.contains(&pid)
    }

    /// Bir PID'i, hiz sayacindan BAGIMSIZ olarak (ornegin bir tuzaga
    /// dokundugu icin) "zaten yakalandi" isaretler. Boylece ayni surec
    /// icin arka arkaya gelen olaylar yeni alarm uretmez.
    pub fn mark_tripped(&mut self, pid: u32) {
        self.tripped.insert(pid);
        self.per_pid.remove(&pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_of_uniform_bytes_is_zero_and_of_full_range_is_eight() {
        assert!(shannon_entropy(&[0u8; 4096]) < 0.001, "tek bir bayt degeri -> 0 bit");
        // 0..=255'in her biri esit sayida -> tam 8.0 bit/bayt
        let all: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        assert!((shannon_entropy(&all) - 8.0).abs() < 0.001);
        assert_eq!(shannon_entropy(&[]), 0.0);
    }

    #[test]
    fn plain_turkish_text_has_clearly_lower_entropy_than_the_threshold() {
        let text = "Sayin yetkili, ekteki sozlesme taslagini incelemenizi rica ederiz. \
                    Toplantiyi onumuzdeki hafta sali gunu yapmayi planliyoruz. Saygilarimizla."
            .repeat(40);
        let h = shannon_entropy(text.as_bytes());
        assert!(h < HIGH_ENTROPY_BITS, "duz metin esigin ALTINDA kalmali, olculen: {h}");
    }

    #[test]
    fn small_samples_are_refused_rather_than_guessed() {
        let dir = std::env::temp_dir().join(format!("chimera-heur-small-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("kucuk.bin");
        std::fs::write(&p, vec![0xABu8; MIN_SAMPLE_BYTES - 1]).unwrap();
        assert_eq!(sample_file_entropy(&p), None, "MIN_SAMPLE_BYTES altinda TAHMIN YAPILMAZ");
        std::fs::write(&p, vec![0xABu8; MIN_SAMPLE_BYTES]).unwrap();
        assert!(sample_file_entropy(&p).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// GERÇEK bir dosyadan okunan entropi: sözde-rastgele içerik eşiği
    /// aşmalı, sıfırlarla dolu bir dosya aşmamalı.
    #[test]
    fn file_entropy_separates_random_content_from_flat_content() {
        let dir = std::env::temp_dir().join(format!("chimera-heur-file-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let mut rnd = vec![0u8; SAMPLE_BYTES];
        getrandom::fill(&mut rnd).unwrap();
        let rp = dir.join("sifreli_gorunumlu.dat");
        std::fs::write(&rp, &rnd).unwrap();
        let h = sample_file_entropy(&rp).unwrap();
        assert!(h > HIGH_ENTROPY_BITS, "gercek rastgele icerik esigi asmali, olculen: {h}");

        let fp = dir.join("duz.dat");
        std::fs::write(&fp, vec![0u8; SAMPLE_BYTES]).unwrap();
        assert!(sample_file_entropy(&fp).unwrap() < 0.001);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn many_distinct_high_entropy_writes_in_the_window_trip_the_breaker() {
        let mut h = RansomHeuristic::new(60, 5, HIGH_ENTROPY_BITS);
        for i in 0..4 {
            assert!(h.observe(1234, &format!("C:/Users/x/belge{i}.docx.locked"), 7.99, 1000 + i as u64).is_none());
        }
        let trip = h.observe(1234, "C:/Users/x/belge4.docx.locked", 7.99, 1004).expect("5. FARKLI dosyada tetiklenmeli");
        assert_eq!(trip.pid, 1234);
        assert_eq!(trip.distinct_files, 5);
        assert!(!trip.sample_paths.is_empty());
        assert!(trip.as_detail().contains("pid=1234"));
    }

    /// YANLIŞ POZİTİF koruması: tek bir büyük dosyanın parça parça
    /// yazılması işletim sisteminden onlarca olay üretir — bu ASLA
    /// tetiklememeli.
    #[test]
    fn one_file_written_many_times_never_trips() {
        let mut h = RansomHeuristic::new(60, 5, HIGH_ENTROPY_BITS);
        for i in 0..50 {
            assert!(
                h.observe(1234, "D:/yedek/arsiv.7z", 7.99, 1000 + i).is_none(),
                "ayni dosyaya {i}. yazimda YANLIS tetiklendi"
            );
        }
        assert!(!h.has_tripped(1234));
    }

    #[test]
    fn low_entropy_writes_are_never_counted() {
        let mut h = RansomHeuristic::new(60, 3, HIGH_ENTROPY_BITS);
        for i in 0..20 {
            assert!(h.observe(1234, &format!("rapor{i}.txt"), 4.2, 1000 + i).is_none());
        }
        assert!(!h.has_tripped(1234));
    }

    /// Pencere GERÇEKTEN kayıyor mu: eşiğin bir eksiği kadar dosya yazılıp
    /// pencere dolduktan SONRA yenileri gelirse tetiklenmemeli.
    #[test]
    fn events_older_than_the_window_are_pruned_and_do_not_accumulate() {
        let mut h = RansomHeuristic::new(60, 5, HIGH_ENTROPY_BITS);
        for i in 0..4 {
            h.observe(7, &format!("eski{i}.dat"), 7.99, 1000 + i);
        }
        // 200 saniye sonra: eski 4 kayit pencereden DUSMELI, bu yuzden
        // 4 yeni dosya da tek basina esigi (5) asmamali.
        for i in 0..4 {
            assert!(
                h.observe(7, &format!("yeni{i}.dat"), 7.99, 1200 + i).is_none(),
                "pencere disi kayitlar birikmis -- kayan pencere CALISMIYOR"
            );
        }
        assert!(!h.has_tripped(7));
    }

    #[test]
    fn a_tripped_pid_does_not_trip_again_until_forgotten() {
        let mut h = RansomHeuristic::new(60, 2, HIGH_ENTROPY_BITS);
        h.observe(9, "a.dat", 7.99, 100);
        assert!(h.observe(9, "b.dat", 7.99, 101).is_some());
        assert!(h.observe(9, "c.dat", 7.99, 102).is_none(), "ayni PID tekrar tekrar alarm uretmemeli");

        h.forget(9); // insan onayiyla resume-process yapildi
        assert!(!h.has_tripped(9));
        // Tuzaga dokunma yolu: hiz sayacindan bagimsiz isaretleme
        h.mark_tripped(9);
        assert!(h.has_tripped(9));
        h.forget(9);
        h.observe(9, "d.dat", 7.99, 200);
        assert!(h.observe(9, "e.dat", 7.99, 201).is_some(), "unutulduktan sonra yeniden alarm uretebilmeli");
    }

    #[test]
    fn different_pids_are_counted_independently() {
        let mut h = RansomHeuristic::new(60, 3, HIGH_ENTROPY_BITS);
        for i in 0..2 {
            h.observe(1, &format!("a{i}"), 7.99, 100);
            h.observe(2, &format!("b{i}"), 7.99, 100);
        }
        assert!(h.observe(1, "a2", 7.99, 100).is_some());
        assert!(!h.has_tripped(2), "bir PID'in tetiklenmesi digerini ETKILEMEMELI");
    }
}
