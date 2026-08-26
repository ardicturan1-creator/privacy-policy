//! bruteforce.rs — RDP/SMB parola deneme (brute-force) tespiti.
//!
//! Kaynak: Windows Güvenlik olay günlüğündeki **4625** ("An account failed
//! to log on") kayıtları. Bu, Microsoft'un kendi belgelediği, her Windows
//! kurulumunda var olan olay kimliğidir; özel bir ajan/hook/driver
//! GEREKTİRMEZ. Okuma, resmî `wevtapi.dll` API'siyle yapılır
//! (`EvtQuery`/`EvtNext`/`EvtRender`) — `wevtutil.exe` ve Olay
//! Görüntüleyici'nin de kullandığı aynı genel arayüz.
//!
//! Olay içindeki `LogonType` alanı saldırı yüzeyini ayırt etmemizi sağlar:
//!
//! | LogonType | Anlamı | Bizim için |
//! |---|---|---|
//! | 3  | Network | **SMB** (dosya paylaşımı) üzerinden deneme |
//! | 10 | RemoteInteractive | **RDP** (Uzak Masaüstü) üzerinden deneme |
//! | 2/4/5/7/... | Interactive/Batch/Service/Unlock | ilgilenmiyoruz |
//!
//! **Bilinçli sınır — bu bir denetim (audit) politikası bağımlılığıdır:**
//! 4625 kayıtları yalnızca "Audit Logon Failure" denetim politikası AÇIKSA
//! üretilir. Varsayılan olarak Windows'ta açıktır, ama bir yönetici bunu
//! kapatmışsa bu modül HİÇBİR ŞEY GÖREMEZ ve bunu sessizce "saldırı yok"
//! diye yorumlamaz — `scanner.rs` sorgunun başarısız/boş olduğunu ayrı bir
//! bulgu olarak raporlar.
//!
//! **İkinci bilinçli sınır:** 4625'in `IpAddress` alanı, kaynak IP
//! bilinmiyorsa `-` olur (örneğin yerel konsol denemeleri). Bu kayıtlar
//! ATILIR — bir IP uydurmak yerine "kaynak bilinmiyor" demek doğrudur.

// Bu modulun ayristirma katmani (`parse_failed_logon` ve yardimcilari)
// yalnizca `#[cfg(windows)]` imp tarafindan VE testler tarafindan
// cagrilir. Linux'ta test-disi bir derlemede kullanilmadigi icin
// "dead_code" uyarisi uretir; bu, gercek bir olu kod DEGIL, platforma
// bagli bir cagri grafigidir -- bu yuzden yalnizca Windows-disinda ve
// ACIKCA bastiriliyor.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::HashMap;

/// Varsayılan pencere: bu kadar saniye içinde...
pub const DEFAULT_WINDOW_SECS: u64 = 300;
/// ...bu kadar başarısız oturum açma denemesi eşiği aşar.
pub const DEFAULT_FAIL_THRESHOLD: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogonSurface {
    /// LogonType 3 — SMB/ağ paylaşımı
    Smb,
    /// LogonType 10 — RDP/Uzak Masaüstü
    Rdp,
}

impl LogonSurface {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogonSurface::Smb => "SMB",
            LogonSurface::Rdp => "RDP",
        }
    }

    fn from_logon_type(t: &str) -> Option<Self> {
        match t.trim() {
            "3" => Some(LogonSurface::Smb),
            "10" => Some(LogonSurface::Rdp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedLogon {
    pub ip: String,
    pub user: String,
    pub surface: LogonSurface,
}

/// 4625 olayının XML gösteriminden ilgilendiğimiz alanları çıkarır.
///
/// Bu, genel amaçlı bir XML ayrıştırıcı DEĞİLDİR ve öyle olduğunu iddia
/// etmez: Windows'un ürettiği `<Data Name='X'>DEGER</Data>` yapısına özel,
/// dar bir çıkarıcıdır. Bir XML kütüphanesi bağımlılığı eklemek yerine bu
/// tercih edildi çünkü girdi, işletim sisteminin kendi ürettiği SABİT
/// şemalı bir belgedir — keyfi/düşmanca bir XML değil.
///
/// `None` dönen durumlar (hepsi meşru): olay 4625 değil, `LogonType`
/// ilgilendiğimiz yüzeylerden biri değil, ya da `IpAddress` yok/`-`.
pub fn parse_failed_logon(xml: &str) -> Option<FailedLogon> {
    let ip = data_field(xml, "IpAddress")?;
    // "-" = kaynak IP bilinmiyor (yerel konsol denemesi gibi). Bir IP
    // UYDURMAK yerine bu kaydi atiyoruz.
    if ip == "-" || ip.is_empty() {
        return None;
    }
    // Gercek bir IP olmayan hicbir sey ilerlemez: firewall'a "adres" diye
    // rastgele bir metin gecmesini onlemenin ILK katmani burasidir
    // (`firewall::validate_ip` ikinci katman).
    if ip.parse::<std::net::IpAddr>().is_err() {
        return None;
    }
    let surface = LogonSurface::from_logon_type(&data_field(xml, "LogonType")?)?;
    let user = data_field(xml, "TargetUserName").unwrap_or_else(|| "(bilinmiyor)".into());
    Some(FailedLogon { ip, user, surface })
}

/// `<Data Name='ALAN'>DEGER</Data>` desenini bulur. Windows hem tek hem
/// cift tirnak kullanabildigi icin ikisi de denenir.
fn data_field(xml: &str, name: &str) -> Option<String> {
    for quote in ['\'', '"'] {
        let key = format!("<Data Name={quote}{name}{quote}>");
        if let Some(start) = xml.find(&key) {
            let rest = &xml[start + key.len()..];
            let end = rest.find("</Data>")?;
            return Some(decode_entities(rest[..end].trim()));
        }
        // Bos alanlar Windows tarafindan kendi kendini kapatan etiketle
        // yazilabilir: <Data Name='X'/>
        let empty = format!("<Data Name={quote}{name}{quote}/>");
        if xml.contains(&empty) {
            return Some(String::new());
        }
    }
    None
}

/// XML'in beş standart varlık kaçışını çözer. Kullanıcı adları `&amp;`
/// gibi kaçışlar içerebilir; bunları çözmemek denetim kaydına yanlış bir
/// kullanıcı adı yazardı.
fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Kaynak IP başına kayan pencere sayacı. `heuristic.rs`'teki ile aynı
/// desende, ama PID yerine IP anahtarlı ve "aynı olayı iki kez sayma"
/// kuralı YOK — burada her başarısız deneme ayrı ve gerçek bir denemedir.
pub struct BruteForceWindow {
    window_secs: u64,
    threshold: usize,
    per_ip: HashMap<String, Vec<(u64, LogonSurface)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BruteForceHit {
    pub ip: String,
    pub attempts: usize,
    pub window_secs: u64,
    pub surfaces: Vec<&'static str>,
}

impl BruteForceHit {
    pub fn as_detail(&self) -> String {
        format!(
            "{} kaynagindan {} saniyede {} basarisiz oturum acma denemesi ({})",
            self.ip,
            self.window_secs,
            self.attempts,
            self.surfaces.join("+")
        )
    }
}

impl Default for BruteForceWindow {
    fn default() -> Self {
        Self::new(DEFAULT_WINDOW_SECS, DEFAULT_FAIL_THRESHOLD)
    }
}

impl BruteForceWindow {
    pub fn new(window_secs: u64, threshold: usize) -> Self {
        Self { window_secs, threshold, per_ip: HashMap::new() }
    }

    pub fn observe(&mut self, ev: &FailedLogon, now: u64) {
        let cutoff = now.saturating_sub(self.window_secs);
        let entry = self.per_ip.entry(ev.ip.clone()).or_default();
        entry.retain(|(ts, _)| *ts >= cutoff);
        entry.push((now, ev.surface));
    }

    /// Eşiği aşan TÜM kaynak IP'leri döner. `observe` ile ayrılmıştır:
    /// bir tarama turunda yüzlerce olay beslenir, karar SONUNDA bir kez
    /// verilir.
    pub fn offenders(&self, now: u64) -> Vec<BruteForceHit> {
        let cutoff = now.saturating_sub(self.window_secs);
        let mut out = Vec::new();
        for (ip, events) in &self.per_ip {
            let live: Vec<&(u64, LogonSurface)> = events.iter().filter(|(ts, _)| *ts >= cutoff).collect();
            if live.len() < self.threshold {
                continue;
            }
            let mut surfaces: Vec<&'static str> = live.iter().map(|(_, s)| s.as_str()).collect();
            surfaces.sort_unstable();
            surfaces.dedup();
            out.push(BruteForceHit { ip: ip.clone(), attempts: live.len(), window_secs: self.window_secs, surfaces });
        }
        // Kararli sira: en cok deneyen once (raporun okunabilirligi icin).
        out.sort_by(|a, b| b.attempts.cmp(&a.attempts).then(a.ip.cmp(&b.ip)));
        out
    }
}

/// Güvenlik günlüğünden son 4625 olaylarını okur ve eşiği aşan kaynak
/// IP'leri döner. Windows dışında açık bir "desteklenmiyor" hatası döner.
pub fn recent_offenders(window_secs: u64, threshold: usize) -> Result<Vec<BruteForceHit>, String> {
    let events = imp::read_failed_logons(window_secs)?;
    let now = unix_now();
    let mut w = BruteForceWindow::new(window_secs, threshold);
    for ev in &events {
        // Olayin KENDI zaman damgasini kullanmak yerine "simdi" kullanmak
        // yeterlidir: sorgu zaten yalnizca son `window_secs` icindeki
        // olaylari getirir (asagidaki XPath), yani hepsi pencere icindedir.
        w.observe(ev, now);
    }
    Ok(w.offenders(now))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::PCWSTR;
    use windows::Win32::System::EventLog::{
        EvtClose, EvtNext, EvtQuery, EvtRender, EvtQueryChannelPath, EvtQueryReverseDirection,
        EvtRenderEventXml, EVT_HANDLE,
    };

    /// Tek seferde okunacak en fazla olay sayısı. Brute-force tespiti için
    /// birkaç yüz olay fazlasıyla yeterlidir; sınırsız okumak, günlüğü
    /// devasa bir makinede taramayı dakikalarca sürdürebilirdi.
    const MAX_EVENTS: usize = 2000;
    const BATCH: usize = 64;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn read_failed_logons(window_secs: u64) -> Result<Vec<FailedLogon>, String> {
        // XPath, Windows Olay Gunlugu'nun KENDI sorgu dilidir: filtrelemeyi
        // gunluk motoruna yaptirmak, tum gunlugu cekip Rust tarafinda
        // elemekten kat kat ucuzdur.
        //   - EventID=4625  -> yalnizca basarisiz oturum acma
        //   - timediff(@SystemTime) <= N  -> yalnizca son N milisaniye
        let query = format!(
            "*[System[(EventID=4625) and TimeCreated[timediff(@SystemTime) <= {}]]]",
            window_secs * 1000
        );
        let channel = wide("Security");
        let query_w = wide(&query);

        unsafe {
            let h = EvtQuery(
                None,
                PCWSTR(channel.as_ptr()),
                PCWSTR(query_w.as_ptr()),
                (EvtQueryChannelPath.0 | EvtQueryReverseDirection.0) as u32,
            )
            .map_err(|e| {
                format!("Guvenlik gunlugu sorgulanamadi (yonetici hakki ve 'Audit Logon Failure' politikasi gerekir): {e}")
            })?;

            let mut out = Vec::new();
            loop {
                let mut batch = [0isize; BATCH];
                let mut returned: u32 = 0;
                // EvtNext, daha fazla olay kalmadiginda HATA doner
                // (ERROR_NO_MORE_ITEMS). Bu bir basarisizlik DEGILDIR --
                // dongunun normal cikisidir.
                if EvtNext(h, &mut batch, 5000, 0, &mut returned).is_err() {
                    break;
                }
                if returned == 0 {
                    break;
                }
                for &raw in batch.iter().take(returned as usize) {
                    let ev = EVT_HANDLE(raw);
                    if let Some(xml) = render_xml(ev) {
                        if let Some(f) = parse_failed_logon(&xml) {
                            out.push(f);
                        }
                    }
                    let _ = EvtClose(ev);
                }
                if out.len() >= MAX_EVENTS {
                    break;
                }
            }
            let _ = EvtClose(h);
            Ok(out)
        }
    }

    /// Tek bir olayı XML'e dönüştürür. İki aşamalı çağrı deseni
    /// (`EvtRender` önce gereken tampon boyutunu bildirir) Windows'un
    /// standart desenidir.
    fn render_xml(ev: EVT_HANDLE) -> Option<String> {
        unsafe {
            let mut needed: u32 = 0;
            let mut props: u32 = 0;
            // Ilk cagri KASITLI olarak basarisiz olur (ERROR_INSUFFICIENT_BUFFER)
            // ve `needed`'i doldurur.
            let _ = EvtRender(None, ev, EvtRenderEventXml.0, 0, None, &mut needed, &mut props);
            if needed == 0 {
                return None;
            }
            let mut buf = vec![0u8; needed as usize];
            EvtRender(
                None,
                ev,
                EvtRenderEventXml.0,
                needed,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                &mut needed,
                &mut props,
            )
            .ok()?;
            // Tampon UTF-16 doludur; sondaki NUL'u kirp.
            let u16s: Vec<u16> = buf
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&c| c != 0)
                .collect();
            Some(String::from_utf16_lossy(&u16s))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn read_failed_logons(_window_secs: u64) -> Result<Vec<FailedLogon>, String> {
        Err("bu platform desteklenmiyor: 4625 olay gunlugu okumasi yalnizca Windows'ta calisir".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gerçek bir Windows 4625 olayının XML gösterimi (Microsoft'un
    /// belgelediği şema; alanlar kısaltıldı ama YAPI birebir korundu).
    fn sample_4625(ip: &str, logon_type: &str, user: &str) -> String {
        format!(
            r#"<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'>
 <System>
  <Provider Name='Microsoft-Windows-Security-Auditing' Guid='{{54849625-5478-4994-A5BA-3E3B0328C30D}}'/>
  <EventID>4625</EventID>
  <Level>0</Level>
  <TimeCreated SystemTime='2026-08-26T10:11:12.3456789Z'/>
  <Channel>Security</Channel>
 </System>
 <EventData>
  <Data Name='SubjectUserSid'>S-1-0-0</Data>
  <Data Name='TargetUserName'>{user}</Data>
  <Data Name='TargetDomainName'>WORKGROUP</Data>
  <Data Name='Status'>0xc000006d</Data>
  <Data Name='LogonType'>{logon_type}</Data>
  <Data Name='WorkstationName'>KALI</Data>
  <Data Name='IpAddress'>{ip}</Data>
  <Data Name='IpPort'>49512</Data>
 </EventData>
</Event>"#
        )
    }

    #[test]
    fn a_real_smb_failed_logon_is_parsed() {
        let f = parse_failed_logon(&sample_4625("203.0.113.7", "3", "administrator")).expect("4625 ayristirilmali");
        assert_eq!(f.ip, "203.0.113.7");
        assert_eq!(f.user, "administrator");
        assert_eq!(f.surface, LogonSurface::Smb);
    }

    #[test]
    fn a_real_rdp_failed_logon_is_parsed() {
        let f = parse_failed_logon(&sample_4625("198.51.100.9", "10", "yonetici")).unwrap();
        assert_eq!(f.surface, LogonSurface::Rdp);
        assert_eq!(f.ip, "198.51.100.9");
    }

    /// Kaynak IP'si bilinmeyen (yerel konsol) denemeler SESSIZCE ATILMALI —
    /// aksi halde `-` bir "adres" olarak firewall'a kadar giderdi.
    #[test]
    fn events_without_a_source_ip_are_discarded() {
        assert_eq!(parse_failed_logon(&sample_4625("-", "3", "kullanici")), None);
        assert_eq!(parse_failed_logon(&sample_4625("", "3", "kullanici")), None);
    }

    /// IP alanina IP OLMAYAN bir sey yazilmissa (bozuk/uydurma kayit)
    /// ilerlememeli.
    #[test]
    fn events_with_a_non_ip_source_are_discarded() {
        assert_eq!(parse_failed_logon(&sample_4625("KOTU-BILGISAYAR", "3", "x")), None);
        assert_eq!(parse_failed_logon(&sample_4625("999.999.999.999", "10", "x")), None);
    }

    /// Ilgilenmedigimiz oturum acma turleri (interaktif, servis, batch)
    /// brute-force sayilmamali.
    #[test]
    fn irrelevant_logon_types_are_ignored() {
        for t in ["2", "4", "5", "7", "8", "9", "11"] {
            assert_eq!(parse_failed_logon(&sample_4625("203.0.113.7", t, "x")), None, "LogonType {t} sayilmamali");
        }
    }

    #[test]
    fn double_quoted_attributes_are_also_supported() {
        let xml = r#"<EventData><Data Name="LogonType">10</Data><Data Name="IpAddress">203.0.113.7</Data><Data Name="TargetUserName">admin</Data></EventData>"#;
        let f = parse_failed_logon(xml).expect("cift tirnakli sema de ayristirilmali");
        assert_eq!(f.surface, LogonSurface::Rdp);
    }

    #[test]
    fn xml_entities_in_user_names_are_decoded() {
        let xml = r#"<EventData><Data Name='LogonType'>3</Data><Data Name='IpAddress'>203.0.113.7</Data><Data Name='TargetUserName'>ar&amp;ge</Data></EventData>"#;
        assert_eq!(parse_failed_logon(xml).unwrap().user, "ar&ge");
    }

    #[test]
    fn ipv6_sources_are_accepted() {
        let f = parse_failed_logon(&sample_4625("2001:db8::1", "10", "x")).unwrap();
        assert_eq!(f.ip, "2001:db8::1");
    }

    /// Eşiğin ALTINDA kalan denemeler alarm üretmemeli — meşru bir
    /// kullanıcının parolasını birkaç kez yanlış girmesi normaldir.
    #[test]
    fn a_few_failures_never_trigger() {
        let mut w = BruteForceWindow::new(300, 10);
        let ev = FailedLogon { ip: "203.0.113.7".into(), user: "a".into(), surface: LogonSurface::Rdp };
        for i in 0..9 {
            w.observe(&ev, 1000 + i);
        }
        assert!(w.offenders(1010).is_empty(), "9 deneme (esik 10) alarm URETMEMELI");
    }

    #[test]
    fn crossing_the_threshold_reports_the_source_ip() {
        let mut w = BruteForceWindow::new(300, 10);
        let ev = FailedLogon { ip: "203.0.113.7".into(), user: "administrator".into(), surface: LogonSurface::Rdp };
        for i in 0..10 {
            w.observe(&ev, 1000 + i);
        }
        let hits = w.offenders(1010);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].ip, "203.0.113.7");
        assert_eq!(hits[0].attempts, 10);
        assert_eq!(hits[0].surfaces, vec!["RDP"]);
        assert!(hits[0].as_detail().contains("203.0.113.7"));
    }

    /// Pencere GERÇEKTEN kayıyor mu: eski denemeler birikip sonradan
    /// yanlışlıkla eşiği aşmamalı.
    #[test]
    fn attempts_older_than_the_window_expire() {
        let mut w = BruteForceWindow::new(300, 10);
        let ev = FailedLogon { ip: "203.0.113.7".into(), user: "a".into(), surface: LogonSurface::Smb };
        for i in 0..9 {
            w.observe(&ev, 1000 + i);
        }
        // 1 saat sonra tek bir deneme daha: eski 9 kayit PENCEREDEN DUSMELI
        w.observe(&ev, 4600);
        assert!(w.offenders(4600).is_empty(), "pencere disi denemeler birikmis -- kayan pencere CALISMIYOR");
    }

    #[test]
    fn different_source_ips_are_counted_independently() {
        let mut w = BruteForceWindow::new(300, 5);
        for i in 0..5 {
            w.observe(&FailedLogon { ip: "203.0.113.7".into(), user: "a".into(), surface: LogonSurface::Rdp }, 1000 + i);
            w.observe(&FailedLogon { ip: "198.51.100.9".into(), user: "b".into(), surface: LogonSurface::Smb }, 1000 + i);
        }
        for i in 0..3 {
            w.observe(&FailedLogon { ip: "192.0.2.5".into(), user: "c".into(), surface: LogonSurface::Rdp }, 1000 + i);
        }
        let hits = w.offenders(1010);
        assert_eq!(hits.len(), 2, "yalnizca esigi asan IKI IP raporlanmali");
        assert!(hits.iter().all(|h| h.ip != "192.0.2.5"));
    }

    /// Hem SMB hem RDP'yi deneyen bir saldırgan, tek bir bulguda HER İKİ
    /// yüzeyle birlikte raporlanmalı (operatör saldırının kapsamını görsün).
    #[test]
    fn an_attacker_probing_both_surfaces_reports_both() {
        let mut w = BruteForceWindow::new(300, 4);
        for i in 0..2 {
            w.observe(&FailedLogon { ip: "203.0.113.7".into(), user: "a".into(), surface: LogonSurface::Rdp }, 1000 + i);
            w.observe(&FailedLogon { ip: "203.0.113.7".into(), user: "a".into(), surface: LogonSurface::Smb }, 1000 + i);
        }
        let hits = w.offenders(1005);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].surfaces, vec!["RDP", "SMB"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_platforms_report_an_explicit_unsupported_error() {
        let e = recent_offenders(300, 10).unwrap_err();
        assert!(e.contains("desteklenmiyor"), "sahte bir 'saldiri yok' DONULMEMELI");
    }
}
