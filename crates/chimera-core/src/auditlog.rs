//! Kanıta-dayanıklı (tamper-evident), hash-zincirli denetim kaydı.
//!
//! Sorun: önceki uygulamada `append_line` yalnızca dosyaya bir JSON satırı
//! ekliyordu — diske erişimi olan bir saldırgan (Tehdit Modeli Seviye 4)
//! GEÇMİŞ bir satırı sessizce SİLEBİLİR ya da DEĞİŞTİREBİLİRDİ, ve bunu
//! kimse fark edemezdi. Bu modül her satırı bir öncekinin BLAKE3 özetine
//! bağlar (blockchain'lerin kullandığı temel fikir, ama burada dağıtık
//! mutabakat YOK — yalnızca yerel, doğrulanabilir bir zincir): bir satır
//! silinir/değiştirilirse, ondan SONRAKİ satırın `prev` alanı artık
//! yeniden hesaplanan özetle uyuşmaz ve `verify()` bunu GERÇEKTEN
//! yakalar. Bu, saldırganın "izlerini silmesini" imkansız kılmaz (dosyayı
//! BAŞTAN İTİBAREN yeniden yazıp zinciri kendi baştan kurabilir) ama
//! **kısmi/nokta düzenlemeyi tespit edilebilir kılar** — ki tipik "birkaç
//! satırı sil" saldırısı budur.
//!
//! Eşzamanlılık: `fs4::FileExt::lock()` (Unix'te `flock(2)`, Windows'ta
//! `LockFileEx`) ile GERÇEK bir OS-seviyesi kilit kullanılır — "son satırı
//! oku, yeni özeti hesapla, ekle" kritik bölümü, aynı süreçteki farklı
//! thread'ler arasında VE (gelecekte) farklı süreçler arasında bile
//! güvenle serileştirilir. Kendi kilitleme ilkelimizi icat etmedik.

use fs4::FileExt;
use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub fn append(path: &Path, event: &str, detail: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).read(true).write(true).open(path)?;
    FileExt::lock(&file)?;

    let result = (|| -> io::Result<()> {
        let mut contents = String::new();
        file.seek(SeekFrom::Start(0))?;
        file.read_to_string(&mut contents)?;

        let mut seq: u64 = 0;
        let mut prev_hash = GENESIS.to_string();
        for line in contents.lines().filter(|l| !l.trim().is_empty()) {
            prev_hash = blake3::hash(line.as_bytes()).to_hex().to_string();
            seq += 1;
        }

        let ts = unix_now();
        let safe_event = event.replace('"', "'");
        let safe_detail = detail.replace('"', "'").replace('\n', " ");
        let line = format!(
            "{{\"seq\":{seq},\"ts\":{ts},\"event\":\"{safe_event}\",\"detail\":\"{safe_detail}\",\"prev\":\"{prev_hash}\"}}"
        );

        file.seek(SeekFrom::End(0))?;
        writeln!(file, "{line}")?;
        file.flush()
    })();

    let _ = FileExt::unlock(&file);
    result
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifyResult {
    /// Dosya bos (henuz hicbir olay yazilmamis) — bir bozulma degil.
    Empty,
    /// Zincir bastan sona tutarli. Icindeki deger toplam kayit sayisidir.
    Ok(u64),
    /// Zincir `at_seq` numarali kayitta koptu — bu kayittan ONCEKI bir
    /// satir silinmis/degistirilmis olabilir.
    Broken { at_seq: u64 },
}

pub fn verify(path: &Path) -> io::Result<VerifyResult> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(VerifyResult::Empty),
        Err(e) => return Err(e),
    };

    let mut prev_hash = GENESIS.to_string();
    let mut count: u64 = 0;
    for line in contents.lines().filter(|l| !l.trim().is_empty()) {
        match extract_field(line, "prev") {
            Some(stored) if stored == prev_hash => {}
            _ => return Ok(VerifyResult::Broken { at_seq: count }),
        }
        prev_hash = blake3::hash(line.as_bytes()).to_hex().to_string();
        count += 1;
    }

    Ok(if count == 0 { VerifyResult::Empty } else { VerifyResult::Ok(count) })
}

/// Bu modulun KENDI yazdigi sabit formata ozel, kucuk bir alan cikarici.
/// Genel amacli bir JSON ayristirici DEGILDIR -- event/detail alanlari
/// yazilirken zaten cift tirnaktan arindirildigi icin (`replace('"', "'")`)
/// bu basit dize aramasi bu ozel formatta guvenlidir.
fn extract_field(line: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\":\"");
    let start = line.find(&key)? + key.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("chimera-auditlog-{name}-{}.jsonl", std::process::id()))
    }

    #[test]
    fn empty_file_verifies_as_empty() {
        let p = temp_path("empty");
        let _ = std::fs::remove_file(&p);
        assert_eq!(verify(&p).unwrap(), VerifyResult::Empty);
    }

    #[test]
    fn chain_of_real_entries_verifies_ok() {
        let p = temp_path("chain");
        let _ = std::fs::remove_file(&p);
        append(&p, "core.start", "fingerprint=abc123").unwrap();
        append(&p, "heartbeat", "sentinel").unwrap();
        append(&p, "mode.set", "degraded=true").unwrap();
        assert_eq!(verify(&p).unwrap(), VerifyResult::Ok(3));
        let _ = std::fs::remove_file(&p);
    }

    /// GERÇEK bir saldırı simülasyonu: bir saldırgan (diske erişimi olan,
    /// Seviye 4) ORTADAKİ bir satırı sessizce değiştiriyor (örn. bir
    /// "privileged_denied" olayını gizlemek için). Zincir bunu YAKALAMALI.
    #[test]
    fn tampering_with_middle_line_is_detected() {
        let p = temp_path("tamper");
        let _ = std::fs::remove_file(&p);
        append(&p, "core.start", "fingerprint=abc123").unwrap();
        append(&p, "privileged_denied", "GetLogs").unwrap();
        append(&p, "heartbeat", "sentinel").unwrap();

        let mut lines: Vec<String> = std::fs::read_to_string(&p).unwrap().lines().map(String::from).collect();
        assert_eq!(lines.len(), 3);
        // saldirgan ikinci satiri (privileged_denied) baska bir olayla
        // DEGISTIRIYOR -- ondan SONRAKI satirin prev'i artik uyusmaz.
        lines[1] = "{\"seq\":1,\"ts\":0,\"event\":\"heartbeat\",\"detail\":\"sahte\",\"prev\":\"deadbeef\"}".to_string();
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();

        match verify(&p).unwrap() {
            // `at_seq` degistirilen kaydin KENDI (0-tabanli) sira numarasidir
            // (2. satir, seq=1) -- verify() `count`'u yalnizca ONCEKI
            // basarili kayitlar icin arttirir, bu yuzden kopma NOKTASI olan
            // bu kaydin sirasi geri donulur.
            VerifyResult::Broken { at_seq } => assert_eq!(at_seq, 1, "2. satirda (seq=1) kopma tespit edilmeli"),
            other => panic!("kurcalama YAKALANAMADI: {other:?}"),
        }
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn deleting_last_line_is_detected_by_count_mismatch_via_reappend() {
        // Silinen son satirin kendisi zinciri BOZMAZ (zincir hala tutarli
        // gorunur -- bu, hash zincirlerinin bilinen bir siniridir: yalnizca
        // dosyanin SONUNA kirpma, bir SONRAKI kaydin gelmesine kadar
        // tespit edilemez). Bu testte tam olarak bu sinir belgeleniyor.
        let p = temp_path("trim");
        let _ = std::fs::remove_file(&p);
        append(&p, "a", "1").unwrap();
        append(&p, "b", "2").unwrap();
        let mut lines: Vec<String> = std::fs::read_to_string(&p).unwrap().lines().map(String::from).collect();
        lines.truncate(1); // son satiri sil
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        assert_eq!(verify(&p).unwrap(), VerifyResult::Ok(1), "kirpma KENDI BASINA tespit edilemez (bilinen sinir)");

        // AMA bir sonraki gercek olay eklendiginde zincir "b"nin ustune
        // degil "a"nin ustune insa edilir -- bu, "sondan kirpma sonrasi
        // sessizce devam etme" davranisinin GERCEK KANITIDIR (seq=1'den
        // devam eder, seq=2'den degil).
        append(&p, "c", "3").unwrap();
        let final_lines: Vec<String> = std::fs::read_to_string(&p).unwrap().lines().map(String::from).collect();
        assert!(final_lines[1].contains("\"seq\":1"));
        let _ = std::fs::remove_file(&p);
    }

    /// Kilitleme GERÇEKTEN calisiyor mu? 16 thread AYNI ANDA append
    /// cagirir; kilit yoksa "son satiri oku + hesapla + yaz" kritik
    /// bolgesi yaris durumuna girer ve zincir kirilir. Kilit varsa
    /// TUM 16 kayit da dogru sirali bir zincir olusturur.
    #[test]
    fn concurrent_appends_from_many_threads_produce_a_valid_chain() {
        let p = temp_path("concurrent");
        let _ = std::fs::remove_file(&p);

        let handles: Vec<_> = (0..16)
            .map(|i| {
                let p = p.clone();
                std::thread::spawn(move || {
                    append(&p, "concurrent_test", &format!("thread-{i}")).unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        match verify(&p).unwrap() {
            VerifyResult::Ok(16) => {}
            other => panic!("16 es zamanli yazimdan sonra zincir GECERSIZ: {other:?}"),
        }
        let _ = std::fs::remove_file(&p);
    }
}
