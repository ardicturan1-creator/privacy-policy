//! cmdguard.rs — Fidye yazılımının "yedekleri imha et" komutlarının
//! yakalanması.
//!
//! Neredeyse her modern fidye yazılımı, şifrelemeye başlamadan hemen önce
//! kurbanın **kendi kendine kurtulma** yollarını kapatır. Bu komutlar
//! yıllardır neredeyse hiç değişmedi ve hepsi Windows'un KENDİ yönetim
//! araçlarıdır (bu yüzden imza tabanlı antivirüsler için görünmezdirler —
//! `vssadmin.exe` Microsoft imzalıdır):
//!
//!   - `vssadmin delete shadows /all /quiet`  → Gölge Kopyalar (Volume
//!     Shadow Copy) silinir; "Önceki Sürümler" ile dosya kurtarma biter.
//!   - `wmic shadowcopy delete`               → aynı şey, farklı araç.
//!   - `wbadmin delete catalog -quiet`        → Windows Yedekleme
//!     kataloğu silinir; mevcut yedekler kullanılamaz hâle gelir.
//!   - `bcdedit /set {default} recoveryenabled no` → Windows Kurtarma
//!     Ortamı devre dışı bırakılır.
//!   - `bcdedit /set {default} bootstatuspolicy ignoreallfailures` →
//!     otomatik onarım tetiklenmez.
//!   - `wevtutil cl ...` / `wevtutil sl ...`  → olay günlükleri temizlenir
//!     (izlerin silinmesi — CHIMERA'nın kendi 4625/4688 tespitini de
//!     körleştirir).
//!
//! ## Nasıl tespit edilir — ve neden "süreç başlamadan engelleme" YOK
//!
//! Tespit, Windows'un **4688** ("A new process has been created") olayı
//! üzerinden yapılır; `bruteforce.rs`'in 4625 için kullandığı AYNI resmî
//! `wevtapi` altyapısı yeniden kullanılır. 4688 olayı, "Include command
//! line in process creation events" politikası açıksa komut satırının
//! TAMAMINI taşır.
//!
//! **Bir süreci Ring-3'ten BAŞLAMADAN ÖNCE engellemenin dürüst durumu:**
//! bunu gerçekten yapmanın yolu ya bir kernel driver (`PsSetCreateProcess
//! NotifyRoutine`) ya da Image File Execution Options (IFEO) `Debugger`
//! kaydıyla `vssadmin.exe`'yi kalıcı olarak ele geçirmektir. Birincisi
//! bu projenin baştan beri kapsam dışı bıraktığı imzalı sürücü meselesidir
//! (bkz. `05-TURN6-ULTRA-GUARD.md` §0.1). İkincisi teknik olarak
//! mümkündür ama **meşru yönetimi de kırar** — bir yedekleme ürünü veya
//! sistem yöneticisi `vssadmin` çağıramaz hâle gelir — ve kalıcı bir
//! sistem değişikliğidir; yani `remediate.rs`'in dört şartını KARŞILAMAZ
//! ve otomatik uygulanamaz.
//!
//! **Bunun yerine ne yapılıyor:** komut yakalandığı anda EN YÜKSEK
//! öncelikli alarm üretilir ve `circuit_breaker` o PID üzerinde
//! tetiklenir — yani süreç ASKIYA ALINIR. Bu, sahte bir "engelledik"
//! iddiası değildir ama boş bir alarm da değildir: `vssadmin delete
//! shadows /all`, büyük bir birimde saniyeler sürer ve askıya alınan bir
//! süreç silmeye DEVAM EDEMEZ. Yarışı her zaman kazanacağımız
//! iddia EDİLMEZ; kazanma şansımızın gerçek olduğu iddia edilir.

// `classify`/`parse_process_creation` ve yardimcilari yalnizca
// `#[cfg(windows)]` imp tarafindan VE testler tarafindan cagrilir --
// `firewall.rs`/`bruteforce.rs`/`scanner.rs` ile AYNI platforma-bagli
// cagri grafigi durumu (bkz. oradaki notlar).
#![cfg_attr(not(windows), allow(dead_code))]

/// Yakalanan komutun hangi kurtarma yolunu hedeflediği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveIntent {
    /// Gölge kopyalar (Volume Shadow Copy) siliniyor.
    ShadowCopyDeletion,
    /// Windows Yedekleme kataloğu siliniyor.
    BackupCatalogDeletion,
    /// Windows Kurtarma Ortamı / otomatik onarım kapatılıyor.
    RecoveryDisable,
    /// Olay günlükleri temizleniyor (iz silme).
    EventLogClearing,
}

impl DestructiveIntent {
    pub fn as_str(&self) -> &'static str {
        match self {
            DestructiveIntent::ShadowCopyDeletion => "golge_kopya_silme",
            DestructiveIntent::BackupCatalogDeletion => "yedek_katalogu_silme",
            DestructiveIntent::RecoveryDisable => "kurtarma_devre_disi",
            DestructiveIntent::EventLogClearing => "olay_gunlugu_temizleme",
        }
    }

    pub fn human(&self) -> &'static str {
        match self {
            DestructiveIntent::ShadowCopyDeletion => "Golge Kopyalar (VSS) siliniyor -- 'Onceki Surumler' ile kurtarma yok edilir",
            DestructiveIntent::BackupCatalogDeletion => "Windows Yedekleme katalogu siliniyor -- mevcut yedekler kullanilamaz hale gelir",
            DestructiveIntent::RecoveryDisable => "Windows Kurtarma Ortami/otomatik onarim kapatiliyor",
            DestructiveIntent::EventLogClearing => "Olay gunlukleri temizleniyor -- saldirinin izleri ve CHIMERA'nin kendi tespit kaynagi siliniyor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructiveCommand {
    pub pid: u32,
    pub intent: DestructiveIntent,
    pub image: String,
    pub command_line: String,
}

impl DestructiveCommand {
    pub fn as_detail(&self) -> String {
        format!("pid={} [{}] {} | komut: {}", self.pid, self.intent.as_str(), self.intent.human(), self.command_line)
    }
}

/// Bir komut satırının kurtarma yollarını imha edip etmediğine karar verir.
///
/// **Tasarım kararı — neden ham alt-dize eşleşmesi YETMİYOR:** ilk
/// uygulama komut satırında `vssadmin`, `delete` ve `shadow` alt-dizelerini
/// arıyordu. Bu, bu modülün KENDİ yanlış-pozitif testi tarafından
/// yakalandı:
///
/// ```text
/// notepad.exe C:\notlar\vssadmin-delete-shadows-notlarim.txt
/// ```
///
/// Bu komut üç kelimeyi de içerir ama tamamen zararsızdır — yalnızca adı
/// öyle olan bir metin dosyasını açar. Alt-dize eşleşmesi bunu "gölge
/// kopya silme" sanıp meşru bir Notepad oturumunu ASKIYA ALIRDI.
///
/// **Bu yüzden eşleşme BELİRTEÇ (token) düzeyindedir:** komut satırı
/// küçük harfe çevrilip boşluklardan bölünür, ve program adı ayrıca yol
/// önekinden arındırılarak karşılaştırılır. Böylece
/// `C:\Windows\System32\vssadmin.exe` eşleşir ama
/// `...\vssadmin-delete-shadows-notlarim.txt` eşleşmez.
pub fn classify(command_line: &str) -> Option<DestructiveIntent> {
    let toks = tokens(command_line);
    if toks.is_empty() {
        return None;
    }
    let has = |t: &str| toks.iter().any(|x| x == t);
    let prog = |name: &str| toks.iter().any(|x| exe_token_is(x, name));
    let adjacent = |a: &str, b: &str| toks.windows(2).any(|w| w[0] == a && w[1] == b);

    // --- Golge kopya (VSS) silme ---
    // `vssadmin delete shadows ...` -- `vssadmin list shadows` DEGIL.
    if prog("vssadmin") && has("delete") && toks.iter().any(|t| t.starts_with("shadow")) {
        return Some(DestructiveIntent::ShadowCopyDeletion);
    }
    // `wmic shadowcopy delete` -- `wmic shadowcopy list` DEGIL.
    if prog("wmic") && has("shadowcopy") && has("delete") {
        return Some(DestructiveIntent::ShadowCopyDeletion);
    }
    // PowerShell: `Get-WmiObject Win32_Shadowcopy | Remove-WmiObject`
    if has("win32_shadowcopy") && toks.iter().any(|t| t.starts_with("remove-") || t.contains(".delete()")) {
        return Some(DestructiveIntent::ShadowCopyDeletion);
    }

    // --- Yedek katalogu silme ---
    if prog("wbadmin") && has("delete") && (has("catalog") || has("systemstatebackup") || has("backup")) {
        return Some(DestructiveIntent::BackupCatalogDeletion);
    }

    // --- Kurtarma ortamini devre disi birakma ---
    // Deger BITISIK olmali: `recoveryenabled no` evet, `recoveryenabled yes` HAYIR.
    if prog("bcdedit") && (adjacent("recoveryenabled", "no") || adjacent("bootstatuspolicy", "ignoreallfailures")) {
        return Some(DestructiveIntent::RecoveryDisable);
    }

    // --- Olay gunlugu temizleme ---
    // `wevtutil cl <kanal>` / `clear-log` -- `qe`/`gl` (salt-okunur) DEGIL.
    if prog("wevtutil") && (has("cl") || has("clear-log")) {
        return Some(DestructiveIntent::EventLogClearing);
    }
    if toks.iter().any(|t| t.starts_with("clear-eventlog")) {
        return Some(DestructiveIntent::EventLogClearing);
    }

    None
}

/// Komut satırını karşılaştırmaya hazır belirteçlere böler: küçük harf,
/// boşluklardan bölünmüş, kenar tırnakları atılmış.
fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split_whitespace()
        .map(|t| t.trim_matches(|c| c == '"' || c == '\'').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Bir belirtecin, yol önekinden bağımsız olarak belirli bir programı
/// gösterip göstermediğine karar verir.
///
/// `C:\Windows\System32\vssadmin.exe` → `vssadmin` ile eşleşir.
/// `C:\notlar\vssadmin-delete-shadows.txt` → EŞLEŞMEZ (bu fonksiyonun
/// varlık sebebi olan gerçek yanlış pozitif).
fn exe_token_is(token: &str, name: &str) -> bool {
    let base = token.rsplit(['\\', '/']).next().unwrap_or(token);
    base == name || base == format!("{name}.exe")
}

/// 4688 ("A new process has been created") olayının XML gösteriminden
/// yıkıcı bir komut çıkarır. `None` = bu olay ilgilendiğimiz bir şey değil.
pub fn parse_process_creation(xml: &str) -> Option<DestructiveCommand> {
    let cmd = data_field(xml, "CommandLine")?;
    let intent = classify(&cmd)?;
    // 4688'de PID ONALTILIK bir dize olarak yazilir ("0x1a2c"). Onluk
    // varsayip `parse::<u32>()` demek, YANLIS bir PID'e -- yani yanlis bir
    // surece -- aksiyon uygulamak demek olurdu.
    let pid = data_field(xml, "NewProcessId").and_then(|s| parse_hex_pid(&s))?;
    let image = data_field(xml, "NewProcessName").unwrap_or_else(|| "(bilinmiyor)".into());
    Some(DestructiveCommand { pid, intent, image, command_line: cmd })
}

fn parse_hex_pid(s: &str) -> Option<u32> {
    let t = s.trim();
    let hex = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))?;
    u32::from_str_radix(hex, 16).ok()
}

fn data_field(xml: &str, name: &str) -> Option<String> {
    for quote in ['\'', '"'] {
        let key = format!("<Data Name={quote}{name}{quote}>");
        if let Some(start) = xml.find(&key) {
            let rest = &xml[start + key.len()..];
            let end = rest.find("</Data>")?;
            return Some(decode_entities(rest[..end].trim()));
        }
    }
    None
}

fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Son `window_secs` saniyedeki 4688 olaylarını okur ve yıkıcı olanları
/// döner. Windows dışında açık bir "desteklenmiyor" hatası döner.
pub fn recent_destructive_commands(window_secs: u64) -> Result<Vec<DestructiveCommand>, String> {
    imp::read_process_creations(window_secs)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::PCWSTR;
    use windows::Win32::System::EventLog::{
        EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender,
        EvtRenderEventXml, EVT_HANDLE,
    };

    const MAX_EVENTS: usize = 4000;
    const BATCH: usize = 64;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn read_process_creations(window_secs: u64) -> Result<Vec<DestructiveCommand>, String> {
        let query = format!(
            "*[System[(EventID=4688) and TimeCreated[timediff(@SystemTime) <= {}]]]",
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
                format!(
                    "Surec olusturma (4688) kayitlari sorgulanamadi: {e} -- 'Audit Process Creation' VE \
                     'Include command line in process creation events' politikalarinin acik olmasi gerekir"
                )
            })?;

            let mut out = Vec::new();
            let mut seen = 0usize;
            loop {
                let mut batch = [0isize; BATCH];
                let mut returned: u32 = 0;
                if EvtNext(h, &mut batch, 5000, 0, &mut returned).is_err() {
                    break; // ERROR_NO_MORE_ITEMS -- dongunun normal cikisi
                }
                if returned == 0 {
                    break;
                }
                for &raw in batch.iter().take(returned as usize) {
                    let ev = EVT_HANDLE(raw);
                    if let Some(xml) = render_xml(ev) {
                        if let Some(d) = parse_process_creation(&xml) {
                            out.push(d);
                        }
                    }
                    let _ = EvtClose(ev);
                    seen += 1;
                }
                if seen >= MAX_EVENTS {
                    break;
                }
            }
            let _ = EvtClose(h);
            Ok(out)
        }
    }

    fn render_xml(ev: EVT_HANDLE) -> Option<String> {
        unsafe {
            let mut needed: u32 = 0;
            let mut props: u32 = 0;
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
    pub fn read_process_creations(_window_secs: u64) -> Result<Vec<DestructiveCommand>, String> {
        Err("bu platform desteklenmiyor: 4688 surec olusturma kayitlari yalnizca Windows'ta okunur".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Gercek fidye yazilimi komutlari YAKALANMALI ----------

    #[test]
    fn the_classic_vssadmin_shadow_deletion_is_caught() {
        for cmd in [
            r"vssadmin.exe delete shadows /all /quiet",
            r"C:\Windows\System32\vssadmin.exe Delete Shadows /All /Quiet",
            r"VSSADMIN    DELETE   SHADOWS  /ALL",
            r#""C:\Windows\SysWOW64\vssadmin.exe" delete shadows /for=C: /oldest"#,
        ] {
            assert_eq!(classify(cmd), Some(DestructiveIntent::ShadowCopyDeletion), "YAKALANAMADI: {cmd}");
        }
    }

    #[test]
    fn wmic_and_powershell_shadow_deletion_variants_are_caught() {
        assert_eq!(classify("wmic shadowcopy delete /nointeractive"), Some(DestructiveIntent::ShadowCopyDeletion));
        assert_eq!(
            classify("powershell.exe -Command Get-WmiObject Win32_Shadowcopy | Remove-WmiObject"),
            Some(DestructiveIntent::ShadowCopyDeletion)
        );
    }

    #[test]
    fn wbadmin_catalog_deletion_is_caught() {
        assert_eq!(classify("wbadmin delete catalog -quiet"), Some(DestructiveIntent::BackupCatalogDeletion));
        assert_eq!(
            classify(r"C:\Windows\System32\wbadmin.exe DELETE SYSTEMSTATEBACKUP -keepVersions:0"),
            Some(DestructiveIntent::BackupCatalogDeletion)
        );
    }

    #[test]
    fn bcdedit_recovery_disabling_is_caught() {
        assert_eq!(
            classify("bcdedit /set {default} recoveryenabled No"),
            Some(DestructiveIntent::RecoveryDisable)
        );
        assert_eq!(
            classify("bcdedit.exe /set {default} bootstatuspolicy ignoreallfailures"),
            Some(DestructiveIntent::RecoveryDisable)
        );
    }

    #[test]
    fn event_log_clearing_is_caught() {
        assert_eq!(classify("wevtutil cl Security"), Some(DestructiveIntent::EventLogClearing));
        assert_eq!(classify("wevtutil.exe cl System"), Some(DestructiveIntent::EventLogClearing));
        assert_eq!(classify("powershell Clear-EventLog -LogName Application"), Some(DestructiveIntent::EventLogClearing));
    }

    // ---------- MESRU komutlar YAKALANMAMALI (yanlis pozitif koruması) ----------

    /// **En önemli yanlış-pozitif testi.** Bunlar bir sistem yöneticisinin
    /// veya yedekleme ürününün her gün çalıştırdığı, tamamen meşru,
    /// çoğu SALT-OKUNUR komutlardır. Bunları yakalamak, meşru bir
    /// yedekleme işini askıya almak demektir.
    #[test]
    fn legitimate_administrative_commands_are_never_flagged() {
        for cmd in [
            "vssadmin list shadows",
            "vssadmin list shadowstorage",
            "vssadmin create shadow /for=C:",
            "vssadmin resize shadowstorage /for=c: /on=c: /maxsize=10gb",
            "wbadmin get status",
            "wbadmin start backup -backupTarget:E: -include:C: -quiet",
            "wbadmin get versions",
            "bcdedit /enum",
            "bcdedit /set {default} recoveryenabled Yes",
            "bcdedit /set {default} description \"Windows 11\"",
            "wevtutil qe Security /c:10",
            "wevtutil gl Security",
            "wmic process list brief",
            "wmic shadowcopy list",
            "powershell Get-WmiObject Win32_Shadowcopy",
            "powershell Get-EventLog -LogName Security -Newest 10",
            r"C:\Program Files\Yedekleme\backup.exe --full",
            "notepad.exe C:\\notlar\\vssadmin-delete-shadows-notlarim.txt",
        ] {
            assert_eq!(classify(cmd), None, "MESRU komut yanlislikla yakalandi: {cmd}");
        }
    }

    /// Bu testin varlık sebebi GERÇEK bir yanlış pozitiftir: ilk
    /// uygulama alt-dize eşleşmesi kullanıyordu ve aşağıdaki ilk komutu
    /// "gölge kopya silme" sanıyordu. Belirteç düzeyinde eşleşmeye
    /// geçilerek düzeltildi.
    #[test]
    fn a_file_merely_named_like_a_destructive_command_is_not_flagged() {
        for cmd in [
            r"notepad.exe C:\notlar\vssadmin-delete-shadows-notlarim.txt",
            r"7z.exe a arsiv.7z C:\raporlar\wbadmin-delete-catalog-raporu.docx",
            r"explorer.exe C:\Users\ali\Desktop\bcdedit-recoveryenabled-no.lnk",
        ] {
            assert_eq!(classify(cmd), None, "dosya ADI yuzunden yanlis yakalandi: {cmd}");
        }
    }

    /// Program adı yol önekinden bağımsız tanınmalı, ama YALNIZCA gerçek
    /// çalıştırılabilir adı olarak — alt-dize olarak DEĞİL.
    #[test]
    fn the_program_token_is_matched_by_basename_not_by_substring() {
        assert!(exe_token_is(r"c:\windows\system32\vssadmin.exe", "vssadmin"));
        assert!(exe_token_is("vssadmin", "vssadmin"));
        assert!(exe_token_is("vssadmin.exe", "vssadmin"));
        assert!(!exe_token_is(r"c:\notlar\vssadmin-delete.txt", "vssadmin"));
        assert!(!exe_token_is("myvssadmin.exe", "vssadmin"));
    }

    /// `bcdedit /set {default} recoveryenabled Yes` MESRU bir onarimdir --
    /// degerin BITISIK okunmasi bunu ayirt eder.
    #[test]
    fn enabling_recovery_is_not_confused_with_disabling_it() {
        assert_eq!(classify("bcdedit /set {default} recoveryenabled Yes"), None);
        assert_eq!(
            classify("bcdedit /set {default} recoveryenabled No"),
            Some(DestructiveIntent::RecoveryDisable)
        );
    }

    #[test]
    fn an_empty_or_harmless_command_line_is_not_flagged() {
        assert_eq!(classify(""), None);
        assert_eq!(classify("   "), None);
        assert_eq!(classify("explorer.exe"), None);
    }

    // ---------- 4688 olay ayristirma ----------

    fn sample_4688(pid_hex: &str, cmd: &str) -> String {
        format!(
            r#"<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'>
 <System><EventID>4688</EventID><Channel>Security</Channel></System>
 <EventData>
  <Data Name='SubjectUserName'>KURBAN$</Data>
  <Data Name='NewProcessId'>{pid_hex}</Data>
  <Data Name='NewProcessName'>C:\Windows\System32\vssadmin.exe</Data>
  <Data Name='ParentProcessName'>C:\Temp\sifrele.exe</Data>
  <Data Name='CommandLine'>{cmd}</Data>
 </EventData>
</Event>"#
        )
    }

    #[test]
    fn a_real_4688_destructive_event_is_parsed_with_the_right_pid() {
        let d = parse_process_creation(&sample_4688("0x1a2c", "vssadmin.exe delete shadows /all /quiet"))
            .expect("yikici 4688 ayristirilmali");
        assert_eq!(d.pid, 0x1a2c, "PID ONALTILIK cozulmeli");
        assert_eq!(d.pid, 6700);
        assert_eq!(d.intent, DestructiveIntent::ShadowCopyDeletion);
        assert!(d.image.ends_with("vssadmin.exe"));
        assert!(d.as_detail().contains("pid=6700"));
    }

    /// PID onaltılık okunmazsa YANLIŞ bir sürece aksiyon uygulanır —
    /// bu testin varlık sebebi budur.
    #[test]
    fn a_decimal_looking_pid_is_still_read_as_hex() {
        let d = parse_process_creation(&sample_4688("0x20", "vssadmin delete shadows /all")).unwrap();
        assert_eq!(d.pid, 32, "0x20 = 32 olmali, 20 DEGIL");
    }

    #[test]
    fn a_4688_without_a_hex_pid_is_rejected_rather_than_guessed() {
        let xml = sample_4688("6700", "vssadmin delete shadows /all");
        assert_eq!(parse_process_creation(&xml), None, "onaltilik olmayan PID TAHMIN EDILMEMELI");
    }

    #[test]
    fn a_harmless_4688_produces_nothing() {
        assert_eq!(parse_process_creation(&sample_4688("0x1a2c", "notepad.exe")), None);
    }

    #[test]
    fn xml_entities_in_command_lines_are_decoded_before_matching() {
        // `&amp;&amp;` ile zincirlenmis bir komut: cozulmezse eslesme kacabilir.
        let xml = sample_4688("0x10", "cmd.exe /c echo x &amp;&amp; vssadmin delete shadows /all");
        let d = parse_process_creation(&xml).expect("cozulmus komut yakalanmali");
        assert_eq!(d.intent, DestructiveIntent::ShadowCopyDeletion);
        assert!(d.command_line.contains("&&"));
    }

    #[test]
    fn every_intent_has_a_distinct_machine_tag_and_human_text() {
        let all = [
            DestructiveIntent::ShadowCopyDeletion,
            DestructiveIntent::BackupCatalogDeletion,
            DestructiveIntent::RecoveryDisable,
            DestructiveIntent::EventLogClearing,
        ];
        let mut tags: Vec<&str> = all.iter().map(|i| i.as_str()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), all.len(), "her niyetin BENZERSIZ bir etiketi olmali");
        assert!(all.iter().all(|i| !i.human().is_empty()));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_platforms_report_an_explicit_unsupported_error() {
        let e = recent_destructive_commands(300).unwrap_err();
        assert!(e.contains("desteklenmiyor"));
    }
}
