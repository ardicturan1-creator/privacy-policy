//! Siber Yanıltma (Cyber Deception) — Bölüm 1: Decoy dosyalar.
//!
//! Gerçekçi isimli, gerçek dosyalar diske yazılır. Bunlara dokunan HERHANGİ
//! bir süreç (yasal bir yedekleme aracı olabilir, ama olağan iş akışında
//! kimsenin dokunmaması gereken dosyalardır) `notify` ile GERÇEK bir
//! dosya sistemi olayı üretir ve bu, gerçek bir denetim kaydına yazılır.
//!
//! **Turn 7'de değişen:** bu tuzak artık PASİF değildir. Windows'ta,
//! tuzağa dokunan sürecin PID'i RESMİ Restart Manager API'siyle
//! (`RmStartSession`/`RmRegisterResources`/`RmGetList`) tespit edilir ve
//! `circuit_breaker.rs`'e devredilir. İmzasız bir kernel driver veya
//! filesystem minifilter YAZILMAMIŞTIR (bkz. `05-TURN6-ULTRA-GUARD.md` §0.1
//! — böyle bir `.sys` Secure Boot açık bir makinede zaten yüklenmez);
//! bunun yerine Windows'un kendi, belgelenmiş, "bu dosyayı hangi süreç
//! açık tutuyor?" sorusunu yanıtlamak için VAR OLAN API'si kullanılır.
//!
//! **Bu yaklaşımın DÜRÜST sınırı:** Restart Manager, dosyayı O ANDA AÇIK
//! TUTAN süreçleri döner. Bir fidye yazılımı dosyayı `open → write → close`
//! ile çok hızlı işlerse, `notify` olayını aldığımız anda tanıtıcı çoktan
//! kapanmış olabilir ve `RmGetList` BOŞ döner. Bu durumda PID `None`
//! olur ve devre kesici o olay için süreci hedefleyemez — yalnızca alarm
//! kaydı düşer. Bu boşluğu kapatan ikinci mekanizma `heuristic.rs`'in
//! hız/entropi sayacıdır (tuzağa hiç dokunulmasa bile çalışır) ve
//! Faz 4'teki ETW tüketicisidir. Bu sınır ölçülmüş bir gerçektir,
//! gizlenmemiştir.

use notify::{RecursiveMode, Watcher};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// Turn 6'dan beri var olan, "ilginç görünen" tuzak seti.
pub const DECOY_NAMES: &[&str] = &[
    "calisan_maaslari_2026.xlsx",
    "yonetim_kurulu_notlari_gizli.docx",
    "musteri_kredi_karti_yedek.csv",
    "vpn_erisim_bilgileri.txt",
    "sirket_sifreleri_master.kdbx",
    "ar-ge_yol_haritasi_2027_TASLAK.pptx",
];

/// Turn 7'de eklendi: **alfabetik sıralamada EN ÖNE düşen** tuzaklar.
///
/// Gerekçe gerçek fidye yazılımı davranışıdır: pek çok aile, bir dizini
/// `FindFirstFileW`/`readdir` ile numaralandırıp dosyaları GELDİĞİ SIRAYLA
/// şifreler; NTFS dizin indeksleri de girdileri Unicode sırasına göre
/// tutar. `!`, `0`, `A` ile başlayan adlar bu sıralamanın BAŞINDA yer alır,
/// yani şifreleme başladıktan sonra **ilk dokunulan dosyalar** bunlar olur.
/// Tuzağın ne kadar erken tetiklendiği, devre kesicinin kaç dosya
/// kaybedilmeden devreye girdiğini doğrudan belirler.
pub const EARLY_DECOY_NAMES: &[&str] = &[
    "!!!_ACIL_yedek_sifre_listesi.xlsx",
    "!_banka_hesap_hareketleri_2026.csv",
    "0001_personel_ozluk_dosyalari.docx",
    "AAA_arsiv_master_parola.kdbx",
];

/// Gerçek bir kullanıcı profilini simüle eden alt klasörler. Yalnızca tek
/// bir "decoys" dizini izlemek, kullanıcı klasörlerinden başlayan bir
/// şifrelemeyi geç yakalardı; bu alt klasörler hem daha inandırıcıdır hem
/// de izlemeyi gerçek verinin durduğu yerlere benzer bir yapıya yayar.
pub const USER_FOLDERS: &[&str] = &["Belgeler", "Masaustu", "Resimler"];

#[derive(Debug, Clone)]
pub struct DecoyAlert {
    pub ts: u64,
    pub path: String,
    pub kind: String,
    /// Tuzağa dokunan sürecin PID'i — Windows'ta Restart Manager ile
    /// tespit edilir. `None` olması bir hata DEĞİLDİR: süreç dosyayı çoktan
    /// kapatmış olabilir (bkz. modül başlığındaki dürüst sınır) veya
    /// platform Windows değildir.
    pub pid: Option<u32>,
    /// PID bulunduysa sürecin Restart Manager'ın bildirdiği adı.
    pub app_name: Option<String>,
}

/// Tuzak dosyaları oluşturur: kök dizine hem klasik hem "alfabetik önce"
/// set, her simüle edilmiş kullanıcı klasörüne de "alfabetik önce" set.
pub fn create_decoys(dir: &Path) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let mut paths = Vec::new();

    for name in DECOY_NAMES.iter().chain(EARLY_DECOY_NAMES.iter()) {
        paths.push(write_one(dir, name)?);
    }

    for folder in USER_FOLDERS {
        let sub = dir.join(folder);
        fs::create_dir_all(&sub)?;
        for name in EARLY_DECOY_NAMES {
            paths.push(write_one(&sub, name)?);
        }
    }

    Ok(paths)
}

fn write_one(dir: &Path, name: &str) -> io::Result<PathBuf> {
    let p = dir.join(name);
    if !p.exists() {
        // Icerik bos degil: bos bir dosya supheli gorunur, gercekci
        // boyutlu bir dosya saldirganin ilk gozden gecirmesinde
        // "gercek" izlenimi birakir. Ayrica `heuristic.rs`'in entropi
        // orneklemesi icin MIN_SAMPLE_BYTES'tan buyuk olmalidir.
        let filler = format!("CHIMERA-DECOY\n{}\n", "x".repeat(2048));
        fs::write(&p, filler)?;
    }
    Ok(p)
}

/// Arka planda calisan bir izleyici baslatir; her decoy olayi kanaldan okunabilir.
/// Donen `notify::RecommendedWatcher` DUSURULMEMELIDIR (drop edilirse izleme durur).
///
/// **Neden `RecursiveMode::Recursive` KULLANILMIYOR:** Turn 7'de once
/// ozyinelemeli mod denendi, cunku kullanici klasoru tuzaklarinin da
/// izlenmesi gerekiyordu. Wine 9.0 altinda CANLI test bunun sessizce
/// CALISMADIGINI gosterdi: Wine'in `ReadDirectoryChangesW` uygulamasi
/// `bWatchSubtree` bayragini onurlandirmiyor -- kok dizindeki olaylar
/// geliyor, alt dizindekiler HIC gelmiyor (yalnizca `notify` kullanan
/// minimal bir tekrar uretimle dogrulandi; ayrintilar
/// `06-TURN7-AKTIF-SAVUNMA.md`). Bu, alt klasor tuzaklarinin SESSIZCE
/// koru olmasi demekti -- bir savunma bileseninde en tehlikeli hata turu.
///
/// Cozum: tuzak dizinlerini BIZ olusturdugumuz icin hangilerinin
/// izlenecegini zaten biliyoruz. Bu yuzden kok dizin VE her kullanici
/// klasoru AYRI AYRI, ozyinelemesiz olarak izlenir. Bu yol hem Wine'da
/// hem Linux'ta hem de gercek Windows'ta calisir ve platformun subtree
/// destegine HIC bagli degildir.
pub fn watch(dir: &Path) -> notify::Result<(notify::RecommendedWatcher, Receiver<DecoyAlert>)> {
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            // Yalnizca GERCEK bir degisiklik (yazma/olusturma/silme) bir
            // alarm sayilir. Ozyinelemeli izleme, alt dizinlere izleyici
            // kaydederken salt-okunur dizin olaylari da uretir
            // (`Access(Open(Any))` gibi) -- bunlari "tuzaga dokunuldu"
            // diye kaydetmek, aldatma kaydini GERCEK bir saldiri sinyali
            // aranamayacak kadar gurultuye bogardi. Bu filtre canli bir
            // testte GERCEKTEN yakalanan bir sorunun duzeltmesidir.
            if !matches!(
                event.kind,
                notify::EventKind::Modify(_) | notify::EventKind::Create(_) | notify::EventKind::Remove(_)
            ) {
                return;
            }
            for path in event.paths {
                // Dizinin KENDISINE ait olaylar (ornegin alt dizin
                // olusturma) bir tuzak dosyaya dokunma DEGILDIR.
                if path.is_dir() {
                    continue;
                }
                let (pid, app_name) = match writer_of(&path) {
                    Some((p, n)) => (Some(p), Some(n)),
                    None => (None, None),
                };
                let alert = DecoyAlert {
                    ts: unix_now(),
                    path: path.to_string_lossy().into_owned(),
                    kind: format!("{:?}", event.kind),
                    pid,
                    app_name,
                };
                let _ = tx.send(alert);
            }
        }
    })?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    for folder in USER_FOLDERS {
        let sub = dir.join(folder);
        if sub.is_dir() {
            watcher.watch(&sub, RecursiveMode::NonRecursive)?;
        }
    }
    Ok((watcher, rx))
}

/// Verilen dosyayı O ANDA açık tutan İLK sürecin (PID, ad) çiftini döner.
/// Bulunamazsa `None` — bkz. modül başlığındaki dürüst sınır.
pub fn writer_of(path: &Path) -> Option<(u32, String)> {
    imp::writer_of(path)
}

#[cfg(windows)]
mod imp {
    use std::path::Path;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY, RM_PROCESS_INFO,
    };

    /// Restart Manager ile "bu dosyayı kim açık tutuyor?" sorusunu sorar.
    ///
    /// Bu, Windows Installer'ın ve Windows Update'in "şu dosyayı
    /// güncelleyeceğim, hangi uygulamaları kapatmam gerek?" sorusunu
    /// yanıtlamak için kullandığı RESMİ, belgelenmiş API'dir
    /// (`rstrtmgr.dll`). Ring-3'ten çalışır, hiçbir sürücü/imza
    /// gerektirmez.
    pub fn writer_of(path: &Path) -> Option<(u32, String)> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        unsafe {
            let mut session: u32 = 0;
            // RmStartSession, `CCH_RM_SESSION_KEY + 1` genisliginde bir
            // tampon BEKLER ve icine oturum anahtarini YAZAR; daha kucuk
            // bir tampon vermek tampon tasmasi demektir.
            let mut key = vec![0u16; CCH_RM_SESSION_KEY as usize + 1];
            if RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())).0 != 0 {
                return None;
            }

            let result = (|| {
                let files = [PCWSTR(wide.as_ptr())];
                if RmRegisterResources(session, Some(&files), None, None).0 != 0 {
                    return None;
                }

                let mut needed: u32 = 0;
                let mut count: u32 = 0;
                let mut reasons: u32 = 0;
                // Ilk cagri yalnizca KAC kayit gerektigini ogrenir
                // (`pnprocinfo = 0`, tampon yok).
                let _ = RmGetList(session, &mut needed, &mut count, None, &mut reasons);
                if needed == 0 {
                    return None; // dosyayi acik tutan surec YOK
                }

                let mut infos = vec![RM_PROCESS_INFO::default(); needed as usize];
                count = needed;
                if RmGetList(session, &mut needed, &mut count, Some(infos.as_mut_ptr()), &mut reasons).0 != 0 {
                    return None;
                }

                let first = infos.first()?;
                let name_end = first.strAppName.iter().position(|&c| c == 0).unwrap_or(first.strAppName.len());
                let name = String::from_utf16_lossy(&first.strAppName[..name_end]);
                Some((first.Process.dwProcessId, name))
            })();

            let _ = RmEndSession(session);
            result
        }
    }

    use std::os::windows::ffi::OsStrExt;
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;
    /// Linux'ta `notify` olayı bize yazan süreci SÖYLEMEZ ve `/proc`
    /// üzerinden bunu geriye dönük olarak güvenilir biçimde çıkarmanın
    /// bir yolu yoktur (fanotify/audit altyapısı gerekir, bu da ayrı bir
    /// ayrıcalık ve kapsam meselesidir). Sahte bir PID uydurmak yerine
    /// dürüstçe `None` dönülür — mevcut `cfg(not(windows))`
    /// "desteklenmiyor" deseniyle tutarlı.
    pub fn writer_of(_path: &Path) -> Option<(u32, String)> {
        None
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Testlerin ORTAK temizligi: izleyici, izlenen dizinler SILINMEDEN
    /// ONCE dusurulur.
    ///
    /// Bu sira KEYFI DEGILDIR. Wine 9.0 altinda, bir ALT dizini aktif
    /// olarak izlenen bir agaci `remove_dir_all` ile silmek SONSUZA KADAR
    /// ASILIYOR (yalnizca `notify` kullanan minimal bir tekrar uretimle
    /// dogrulandi: kok-yalniz izlemede 3 ms'de biterken, alt dizin
    /// izlenirken hic donmuyor). Izleyiciyi once dusurmek bunu her
    /// platformda cozer ve zaten dogru kaynak yonetimidir. Uretim kodu
    /// tuzak dizinini calisirken SILMEDIGI icin bu bir test-hijyeni
    /// meselesidir, bir urun hatasi degil.
    fn cleanup(watcher: notify::RecommendedWatcher, dir: &Path) {
        drop(watcher);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn touching_a_decoy_produces_a_real_alert() {
        let dir = std::env::temp_dir().join(format!("chimera-core-decoy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let paths = create_decoys(&dir).unwrap();
        // kok dizin: klasik + erken set; her kullanici klasoru: erken set
        let expected = DECOY_NAMES.len() + EARLY_DECOY_NAMES.len() + USER_FOLDERS.len() * EARLY_DECOY_NAMES.len();
        assert_eq!(paths.len(), expected);

        let (watcher, rx) = watch(&dir).unwrap();
        // Watcher'in kurulmasi icin kisa bir bekleme (isletim sistemi
        // dosya sistemi olay kuyrugu asenkron kurulur).
        std::thread::sleep(Duration::from_millis(200));

        // GERCEK bir "saldirgan" dokunusu: decoy dosyasi acilip yazilir.
        fs::write(&paths[0], "dokunuldu").unwrap();

        let alert = rx.recv_timeout(Duration::from_secs(5)).expect("gercek bir dosya olayi alinmali");
        assert!(
            alert.path.contains(DECOY_NAMES[0]),
            "ilk alarm hedef tuzaga ait olmali (salt-okunur dizin olaylari filtrelenmis olmali), gelen: {alert:?}"
        );

        cleanup(watcher, &dir);
    }

    /// Turn 7'de eklenen kullanıcı-klasörü tuzakları GERÇEKTEN izleniyor mu?
    /// Bu test, `watch()`'un her kullanıcı klasörünü AYRI AYRI izlemesinin
    /// kanıtıdır — ve platformun `bWatchSubtree` desteğine bağlı olmadığı
    /// için Wine altında da geçer (ozyinelemeli mod ile GEÇMİYORDU).
    #[test]
    fn touching_a_decoy_inside_a_simulated_user_folder_is_also_detected() {
        let dir = std::env::temp_dir().join(format!("chimera-core-decoy-sub-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        create_decoys(&dir).unwrap();

        let (watcher, rx) = watch(&dir).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        let target = dir.join(USER_FOLDERS[0]).join(EARLY_DECOY_NAMES[0]);
        assert!(target.exists(), "kullanici klasoru tuzagi olusturulmus olmali");
        fs::write(&target, "sifrelendi").unwrap();

        // Ilgisiz olaylar da gelebilir; hedefe ait olani ARAYARAK bekle.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut seen = false;
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(a) if a.path.contains(EARLY_DECOY_NAMES[0]) && a.path.contains(USER_FOLDERS[0]) => { seen = true; break; }
                Ok(_) => continue,
                Err(_) => continue,
            }
        }
        assert!(seen, "alt klasordeki tuzak izlenmiyor -- her klasorun AYRI izlenmesi CALISMIYOR");

        cleanup(watcher, &dir);
    }

    /// Erken tuzakların GERÇEKTEN alfabetik olarak önde olduğunu doğrular —
    /// iddianın kendisi test edilir, "öyle olduğunu varsayıyoruz" denmez.
    #[test]
    fn early_decoys_really_sort_before_the_classic_ones() {
        let mut all: Vec<&str> = DECOY_NAMES.iter().chain(EARLY_DECOY_NAMES.iter()).copied().collect();
        all.sort();
        let first_four: Vec<&str> = all.iter().take(EARLY_DECOY_NAMES.len()).copied().collect();
        for name in EARLY_DECOY_NAMES {
            assert!(first_four.contains(name), "{name} siralamada ONDE degil -- 'erken tuzak' iddiasi GECERSIZ");
        }
    }

    /// Ozyinelemeli izlemenin urettigi salt-okunur/dizin olaylari alarma
    /// DONUSMEMELI. Bu test, canli calistirmada gercekten gozlemlenen
    /// `Access(Open(Any))` gurultusunun geri gelmesini onler.
    #[test]
    fn read_only_and_directory_events_never_become_alerts() {
        let dir = std::env::temp_dir().join(format!("chimera-core-decoy-noise-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        create_decoys(&dir).unwrap();

        let (watcher, rx) = watch(&dir).unwrap();
        std::thread::sleep(Duration::from_millis(300));

        // Salt-okunur erisim: tuzaklari okumak ve yeni bir DIZIN acmak.
        for name in DECOY_NAMES {
            let _ = fs::read(dir.join(name));
        }
        fs::create_dir_all(dir.join("YeniKlasor")).unwrap();
        std::thread::sleep(Duration::from_millis(400));

        // KESIN bir sure sinirinda topla. Sinirsiz bir `while let Ok(..)`
        // tuketme dongusu, olaylarin surekli aktigi bir platformda ASLA
        // bitmezdi -- bu, Wine altinda GERCEKTEN yasanan bir asilmaydi.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            let Ok(alert) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
            assert!(
                !alert.path.ends_with("YeniKlasor"),
                "dizin olayi alarma donusmus: {alert:?}"
            );
            assert!(
                alert.kind.starts_with("Modify") || alert.kind.starts_with("Create") || alert.kind.starts_with("Remove"),
                "salt-okunur olay alarma donusmus: {alert:?}"
            );
        }

        cleanup(watcher, &dir);
    }

    #[test]
    fn decoy_contents_are_large_enough_for_entropy_sampling() {
        let dir = std::env::temp_dir().join(format!("chimera-core-decoy-size-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let paths = create_decoys(&dir).unwrap();
        for p in &paths {
            let len = fs::metadata(p).unwrap().len() as usize;
            assert!(len >= crate::heuristic::MIN_SAMPLE_BYTES, "{p:?} entropi ornegi icin cok kucuk");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// Linux'ta `writer_of` dürüstçe `None` döner (sahte PID uydurmaz);
    /// Windows'ta gerçek bir API çağrısı yapar ve panic'lememelidir.
    #[test]
    fn writer_of_is_honest_about_what_it_cannot_know() {
        let dir = std::env::temp_dir().join(format!("chimera-core-decoy-writer-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let p = dir.join("test.bin");
        fs::write(&p, "x").unwrap();
        let result = writer_of(&p);
        #[cfg(not(windows))]
        assert_eq!(result, None, "Linux'ta sahte bir PID UYDURULMAMALI");
        #[cfg(windows)]
        let _ = result; // Windows'ta sonuc ortama baglidir; asil kontrol panic'lememesi
        let _ = fs::remove_dir_all(&dir);
    }
}
