//! circuit_breaker.rs — Fidye yazılımı DEVRE KESİCİSİ.
//!
//! `decoy.rs` (tuzağa dokunuldu) veya `heuristic.rs` (kısa pencerede toplu
//! yüksek-entropili yazma) bir şüpheli süreç tespit ettiğinde çağrılır ve
//! DÖRT adım atar:
//!
//!   1. **Askıya alma** — süreç `NtSuspendProcess` ile DONDURULUR.
//!      SONLANDIRILMAZ. Bu bilinçli bir tercihtir: askıya alma GERİ
//!      ALINABİLİR bir işlemdir (`resume-process`), sonlandırma değildir.
//!      Yanlış bir tespitte meşru bir sürecin kaydedilmemiş verisi
//!      kaybolmaz; süreç insan onayıyla kaldığı yerden devam eder.
//!      Ayrıca askıya alma, şifrelemeyi **o anda ve tamamen** durdurur —
//!      "alarm üret, operatör 10 dakika sonra baksın" yaklaşımının
//!      aksine, 10 dakikada kaybedilecek dosyalar kaybedilmez.
//!   2. **Ağ izolasyonu** — o sürecin O ANDAKİ uzak bağlantılarının
//!      adresleri (`GetExtendedTcpTable`, gerçek Win32 API) toplanır ve
//!      `firewall.rs` üzerinden bloklanır. Fidye yazılımının anahtarı
//!      C2 sunucusuna göndermesi/almasi engellenir.
//!   3. **Kanıta-dayanıklı kayıt** — her adım (başarılı VE başarısız)
//!      `auditlog.rs`'in hash-zincirli kaydına yazılır.
//!   4. **İnsan onayı kuyruğu** — süreç `state/suspended.list`'e
//!      `AWAITING_HUMAN_APPROVAL` durumuyla yazılır. Buradan çıkışın TEK
//!      yolu, Shamir(2,3) kapısından geçen bir `resume-process` veya
//!      `terminate-process` komutudur (bkz. `chimera-admin`).
//!
//! **Değişmez kısıt:** KALICI/geri alınamaz hiçbir aksiyon (sonlandırma)
//! bu modül tarafından OTOMATİK çalıştırılmaz. `terminate()` yalnızca
//! `chimera-core`'un istek döngüsündeki, `unlocked()` kontrolünden geçmiş
//! bir IPC isteğiyle çağrılabilir. `pipeline.rs`'in Validator whitelist'i
//! de bunu yansıtır: `Remediation::SuspendSuspectProcess` whitelist'te
//! (geri alınabilir), `Remediation::TerminateSuspendedProcess` DEĞİL —
//! ikincisi her zaman "İNSAN İNCELEMESİ GEREKEN" listesine düşer.
//!
//! **Kernel driver İDDİASI YOKTUR.** Askıya alma, `ntdll.dll`'in
//! `NtSuspendProcess` dışa aktarımıyla yapılır — Ring-3'ten çağrılan,
//! Sysinternals `pssuspend` dahil pek çok aracın kullandığı, kararlı ama
//! Microsoft tarafından RESMEN BELGELENMEMİŞ bir çağrıdır. Bu yüzden
//! bulunamazsa, TAMAMEN BELGELENMİŞ bir yedeğe düşülür: sürecin tüm
//! thread'leri `CreateToolhelp32Snapshot` + `OpenThread` + `SuspendThread`
//! ile tek tek dondurulur. Hangi yolun kullanıldığı dönüş mesajında ve
//! denetim kaydında AÇIKÇA yazar.

use std::path::{Path, PathBuf};

fn state_file(root: &Path) -> PathBuf {
    root.join("state/suspended.list")
}

/// Devre kesicinin neden tetiklendiği. Denetim kaydına ve insan onayı
/// kuyruğuna bu metin yazılır — operatör "neden donduruldu?" sorusunu
/// başka bir yere bakmadan yanıtlayabilmelidir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TripReason {
    /// Bir tuzak (decoy) dosyaya yazıldı/dokunuldu.
    DecoyTouched { path: String },
    /// Kısa bir pencerede çok sayıda farklı dosyaya yüksek entropili yazma.
    MassEncryption { detail: String },
}

impl TripReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            TripReason::DecoyTouched { .. } => "decoy_touch",
            TripReason::MassEncryption { .. } => "mass_encryption",
        }
    }
    pub fn detail(&self) -> String {
        match self {
            TripReason::DecoyTouched { path } => format!("tuzak dosyaya yazildi: {path}"),
            TripReason::MassEncryption { detail } => detail.clone(),
        }
    }
}

/// İnsan onayı kuyruğundaki tek bir kayıt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendedRecord {
    pub ts: u64,
    pub pid: u32,
    pub image: String,
    pub reason: String,
    pub suspended: bool,
    pub blocked_ips: Vec<String>,
}

impl SuspendedRecord {
    fn to_line(&self) -> String {
        format!(
            "{{\"ts\":{},\"pid\":{},\"image\":\"{}\",\"reason\":\"{}\",\"suspended\":{},\"blocked\":\"{}\",\"status\":\"AWAITING_HUMAN_APPROVAL\"}}",
            self.ts,
            self.pid,
            sanitize(&self.image),
            sanitize(&self.reason),
            self.suspended,
            self.blocked_ips.join(",")
        )
    }

    fn from_line(line: &str) -> Option<Self> {
        Some(SuspendedRecord {
            ts: field(line, "ts")?.parse().ok()?,
            pid: field(line, "pid")?.parse().ok()?,
            image: field_str(line, "image").unwrap_or_default(),
            reason: field_str(line, "reason").unwrap_or_default(),
            suspended: field(line, "suspended").map(|v| v == "true").unwrap_or(false),
            blocked_ips: field_str(line, "blocked")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
        })
    }
}

/// `auditlog.rs`'teki ile AYNI kasıtlı sadelikteki alan çıkarıcı: bu
/// modülün KENDİ yazdığı, çift tırnaktan arındırılmış sabit format için
/// güvenlidir; genel amaçlı bir JSON ayrıştırıcı DEĞİLDİR.
fn field(line: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\":");
    let start = line.find(&k)? + k.len();
    let rest = &line[start..];
    let end = rest.find([',', '}'])?;
    Some(rest[..end].trim_matches('"').to_string())
}

fn field_str(line: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\":\"");
    let start = line.find(&k)? + k.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn sanitize(s: &str) -> String {
    s.replace('"', "'").replace('\n', " ")
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// **Kendini vurma koruması.** Devre kesicinin ASLA dokunmaması gereken
/// PID'ler. Bu kontrol, platformdan bağımsız katmandadır (yani Linux'ta
/// da test edilebilir) ve `trip()`'in İLK adımıdır — bir yanlış tespit,
/// CHIMERA'nın kendisini, eş bekçisini veya işletim sisteminin çekirdek
/// süreçlerini dondurup makineyi kilitleyemez.
///   - 0 = System Idle Process, 4 = System (Windows çekirdek süreci)
///   - kendi PID'imiz = `chimera-core` (kendini dondurursa devre kesiciyi
///     geri alacak IPC sunucusu da donar — kurtarılamaz durum)
///   - `runtime/sentinel.pid` = `chimera-sentinel` (eş bekçi)
pub fn is_protected_pid(root: &Path, pid: u32) -> bool {
    if pid == 0 || pid == 4 || pid == std::process::id() {
        return true;
    }
    std::fs::read_to_string(root.join("runtime/sentinel.pid"))
        .ok()
        .and_then(|t| t.trim().parse::<u32>().ok())
        .map(|sentinel| sentinel == pid)
        .unwrap_or(false)
}

fn read_records(root: &Path) -> Vec<SuspendedRecord> {
    std::fs::read_to_string(state_file(root))
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).filter_map(SuspendedRecord::from_line).collect())
        .unwrap_or_default()
}

fn write_records(root: &Path, recs: &[SuspendedRecord]) -> std::io::Result<()> {
    use fs4::FileExt;
    let path = state_file(root);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&path)?;
    FileExt::lock(&f)?;
    let body: String = recs.iter().map(|r| r.to_line()).collect::<Vec<_>>().join("\n");
    let res = std::io::Write::write_all(&mut f, body.as_bytes());
    let _ = FileExt::unlock(&f);
    res
}

/// Devre kesicinin bir tetiklenmesinin SONUCU. Hiçbir alan "varsayılan
/// başarılı" değildir: askıya alma başarısız olduysa `suspended=false`
/// olarak kaydedilir ve operatör bunu `list-suspended` çıktısında GÖRÜR —
/// sessizce "hallettik" denmez.
#[derive(Debug, Clone)]
pub struct TripOutcome {
    pub pid: u32,
    pub image: String,
    pub suspended: bool,
    pub suspend_detail: String,
    pub blocked_ips: Vec<String>,
    pub network_detail: String,
    pub skipped_protected: bool,
}

impl TripOutcome {
    pub fn to_text(&self) -> String {
        if self.skipped_protected {
            return format!("DEVRE KESICI ATLANDI: pid={} korumali bir surec (CHIMERA'nin kendisi/sentinel/OS cekirdegi) -- hicbir aksiyon alinmadi.", self.pid);
        }
        format!(
            "DEVRE KESICI: pid={} ({})\n  askiya alindi: {}\n  askiya alma: {}\n  bloklanan adres sayisi: {}\n  ag izolasyonu: {}\n  DURUM: INSAN ONAYI BEKLIYOR (resume-process | terminate-process, her ikisi de Shamir(2,3) gerektirir)",
            self.pid,
            self.image,
            if self.suspended { "EVET" } else { "HAYIR -- surec HALA CALISIYOR" },
            self.suspend_detail,
            self.blocked_ips.len(),
            self.network_detail
        )
    }
}

/// Devre kesiciyi tetikler. `audit` geri çağrısı, `pipeline.rs`'in
/// kullandığı ile AYNI imzadadır (`&Fn(&str, &str)`) — böylece çağıran
/// taraf denetim kaydı yolunu bir kez bağlar.
pub fn trip(root: &Path, pid: u32, reason: &TripReason, audit: &impl Fn(&str, &str)) -> TripOutcome {
    if is_protected_pid(root, pid) {
        audit("circuit_breaker.skipped_protected", &format!("pid={pid} korumali surec, aksiyon alinmadi"));
        return TripOutcome {
            pid,
            image: "(korumali)".into(),
            suspended: false,
            suspend_detail: "atlandi".into(),
            blocked_ips: Vec::new(),
            network_detail: "atlandi".into(),
            skipped_protected: true,
        };
    }

    let image = imp::process_image(pid).unwrap_or_else(|| "(gorunti yolu okunamadi)".to_string());
    audit("circuit_breaker.tripped", &format!("pid={pid} image={image} sebep={} ({})", reason.as_str(), reason.detail()));

    // 1) Askiya alma (GERI ALINABILIR)
    let (suspended, suspend_detail) = match imp::suspend_process(pid) {
        Ok(msg) => {
            audit("circuit_breaker.suspended", &format!("pid={pid}: {msg}"));
            (true, msg)
        }
        Err(e) => {
            audit("circuit_breaker.suspend_failed", &format!("pid={pid}: {e}"));
            (false, format!("BASARISIZ: {e}"))
        }
    };

    // 2) Ag izolasyonu: yalnizca O SURECIN uzak adresleri
    let mut blocked_ips = Vec::new();
    let network_detail = match imp::remote_ips_of_pid(pid) {
        Ok(ips) if ips.is_empty() => "bu surecin acik uzak baglantisi yok".to_string(),
        Ok(ips) => {
            let mut failed = 0usize;
            for ip in &ips {
                match crate::firewall::block_ip(root, ip) {
                    Ok(msg) => {
                        audit("circuit_breaker.ip_blocked", &format!("pid={pid} {msg}"));
                        blocked_ips.push(ip.clone());
                    }
                    Err(e) => {
                        audit("circuit_breaker.ip_block_failed", &format!("pid={pid} {ip}: {e}"));
                        failed += 1;
                    }
                }
            }
            format!("{} adres bloklandi, {} basarisiz (aday: {})", blocked_ips.len(), failed, ips.join(", "))
        }
        Err(e) => {
            audit("circuit_breaker.network_enum_failed", &format!("pid={pid}: {e}"));
            format!("BASARISIZ: baglanti tablosu okunamadi: {e}")
        }
    };

    // 3+4) Insan onayi kuyruguna yaz
    let rec = SuspendedRecord {
        ts: unix_now(),
        pid,
        image: image.clone(),
        reason: reason.detail(),
        suspended,
        blocked_ips: blocked_ips.clone(),
    };
    let mut recs = read_records(root);
    if let Some(existing) = recs.iter_mut().find(|r| r.pid == pid) {
        *existing = rec;
    } else {
        recs.push(rec);
    }
    if let Err(e) = write_records(root, &recs) {
        audit("circuit_breaker.state_write_failed", &format!("pid={pid}: {e}"));
    }

    let outcome =
        TripOutcome { pid, image, suspended, suspend_detail, blocked_ips, network_detail, skipped_protected: false };
    // Devre kesicinin ne yaptiginin TEK SATIRLIK ozeti de zincire yazilir:
    // olay mudahale ekibi, tek tek adim kayitlarini birlestirmek zorunda
    // kalmadan "bu tetiklenmede tam olarak ne oldu" sorusunu yanitlayabilir.
    audit("circuit_breaker.outcome", &outcome.to_text().replace('\n', " | "));
    outcome
}

/// İnsan onayı kuyruğunu insan-okunabilir biçimde döner (IPC yanıtı).
pub fn list_suspended_text(root: &Path) -> String {
    let recs = read_records(root);
    if recs.is_empty() {
        return "ASKIYA ALINMIS SUREC YOK.".to_string();
    }
    let mut s = format!("INSAN ONAYI BEKLEYEN {} SUREC:\n", recs.len());
    for r in &recs {
        s.push_str(&format!(
            "  pid={} ts={} askiya_alindi={}\n    gorunti: {}\n    sebep  : {}\n    bloklu IP: {}\n",
            r.pid,
            r.ts,
            r.suspended,
            r.image,
            r.reason,
            if r.blocked_ips.is_empty() { "-".to_string() } else { r.blocked_ips.join(", ") }
        ));
    }
    s.push_str("Karar: 'chimera-admin resume-process --pid N' (geri al) veya 'terminate-process --pid N' (KALICI).\n");
    s
}

pub fn pending_count(root: &Path) -> usize {
    read_records(root).len()
}

fn take_record(root: &Path, pid: u32) -> Option<SuspendedRecord> {
    let mut recs = read_records(root);
    let idx = recs.iter().position(|r| r.pid == pid)?;
    let rec = recs.remove(idx);
    let _ = write_records(root, &recs);
    Some(rec)
}

/// İnsan onayıyla süreci devam ettirir ve devre kesicinin uyguladığı ağ
/// bloklarını GERİ ALIR. Kuyruktan düşer. Bu, "yanlış pozitifti" kararının
/// karşılığıdır ve TAM anlamıyla geri alınabilir olmalıdır — bu yüzden IP
/// blokları da kaldırılır.
pub fn resume(root: &Path, pid: u32, audit: &impl Fn(&str, &str)) -> Result<String, String> {
    let Some(rec) = take_record(root, pid) else {
        return Err(format!("pid={pid} insan onayi kuyrugunda bulunamadi"));
    };
    let mut notes = Vec::new();
    match imp::resume_process(pid) {
        Ok(msg) => {
            audit("circuit_breaker.resumed", &format!("pid={pid}: {msg}"));
            notes.push(msg);
        }
        Err(e) => {
            audit("circuit_breaker.resume_failed", &format!("pid={pid}: {e}"));
            notes.push(format!("surec devam ettirilemedi: {e}"));
        }
    }
    for ip in &rec.blocked_ips {
        match crate::firewall::unblock_ip(root, ip) {
            Ok(msg) => { audit("circuit_breaker.ip_unblocked", &format!("pid={pid} {msg}")); }
            Err(e) => { audit("circuit_breaker.ip_unblock_failed", &format!("pid={pid} {ip}: {e}")); notes.push(format!("{ip} blogu kaldirilamadi: {e}")); }
        }
    }
    Ok(format!("pid={pid} insan onayiyla DEVAM ETTIRILDI ({} IP blogu geri alindi). {}", rec.blocked_ips.len(), notes.join("; ")))
}

/// İnsan onayıyla süreci KALICI olarak sonlandırır. **Geri alınamaz.**
/// Bu yüzden `pipeline.rs` whitelist'ine ASLA girmez ve yalnızca
/// Shamir(2,3) kapısından geçmiş bir IPC isteğiyle çağrılır. Ağ blokları
/// bilinçli olarak KALDIRILMAZ: operatör bunları `unblock-ip` ile ayrıca,
/// bilinçli bir kararla kaldırır.
pub fn terminate(root: &Path, pid: u32, audit: &impl Fn(&str, &str)) -> Result<String, String> {
    if is_protected_pid(root, pid) {
        audit("circuit_breaker.terminate_refused_protected", &format!("pid={pid}"));
        return Err(format!("pid={pid} korumali bir surec (CHIMERA/sentinel/OS cekirdegi) -- sonlandirma REDDEDILDI"));
    }
    let Some(rec) = take_record(root, pid) else {
        return Err(format!("pid={pid} insan onayi kuyrugunda bulunamadi (yalnizca devre kesicinin askiya aldigi surecler sonlandirilabilir)"));
    };
    match imp::terminate_process(pid) {
        Ok(msg) => {
            audit("circuit_breaker.terminated", &format!("pid={pid} image={} insan onayiyla: {msg}", rec.image));
            Ok(format!("pid={pid} ({}) insan onayiyla SONLANDIRILDI. {} IP blogu KASITLI olarak yerinde birakildi (kaldirmak icin: unblock-ip).", rec.image, rec.blocked_ips.len()))
        }
        Err(e) => {
            audit("circuit_breaker.terminate_failed", &format!("pid={pid}: {e}"));
            Err(format!("pid={pid} sonlandirilamadi: {e}"))
        }
    }
}

#[cfg(windows)]
mod imp {
    use windows::core::{s, PCSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, NTSTATUS};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Threading::{
        OpenProcess, OpenThread, QueryFullProcessImageNameW, ResumeThread, SuspendThread, TerminateProcess,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SUSPEND_RESUME, PROCESS_TERMINATE,
        THREAD_SUSPEND_RESUME,
    };

    type NtProcFn = unsafe extern "system" fn(HANDLE) -> NTSTATUS;

    /// `ntdll.dll`'den `NtSuspendProcess`/`NtResumeProcess` adresini çözer.
    /// Bu iki çağrı Microsoft tarafından RESMEN BELGELENMEMİŞTİR (ama
    /// Windows XP'den beri kararlıdır ve Sysinternals `pssuspend` dahil
    /// yaygın araçlar bunları kullanır). Bulunamazsa `None` döner ve
    /// çağıran TAMAMEN BELGELENMİŞ thread-bazlı yola düşer — sahte bir
    /// "başarılı" DÖNMEZ.
    unsafe fn ntdll_proc(name: PCSTR) -> Option<NtProcFn> {
        unsafe {
            let ntdll = GetModuleHandleW(windows::core::w!("ntdll.dll")).ok()?;
            let addr = GetProcAddress(ntdll, name)?;
            Some(core::mem::transmute::<unsafe extern "system" fn() -> isize, NtProcFn>(addr))
        }
    }

    /// Sürecin TÜM thread'lerini tek tek dondurur/çözer. Yalnızca
    /// belgelenmiş Win32 API'leri kullanır (`CreateToolhelp32Snapshot`,
    /// `OpenThread`, `SuspendThread`/`ResumeThread`).
    ///
    /// Bilinen sınır — DÜRÜSTÇE: bu yol ATOMİK DEĞİLDİR. Biz thread'leri
    /// dondururken süreç YENİ bir thread oluşturabilir ve o thread
    /// dondurulmamış olarak çalışmaya devam eder. `NtSuspendProcess` bu
    /// yarışı çekirdek seviyesinde kapatır; bu yüzden birincil yol odur ve
    /// bu yalnızca YEDEKtir.
    unsafe fn for_each_thread(pid: u32, suspend: bool) -> Result<String, String> {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
                .map_err(|e| format!("thread anlik goruntusu alinamadi: {e}"))?;
            let mut te = THREADENTRY32 { dwSize: core::mem::size_of::<THREADENTRY32>() as u32, ..Default::default() };
            let mut touched = 0u32;
            let mut failed = 0u32;
            if Thread32First(snap, &mut te).is_ok() {
                loop {
                    if te.th32OwnerProcessID == pid {
                        match OpenThread(THREAD_SUSPEND_RESUME, false, te.th32ThreadID) {
                            Ok(h) => {
                                let rc = if suspend { SuspendThread(h) } else { ResumeThread(h) };
                                if rc == u32::MAX { failed += 1 } else { touched += 1 }
                                let _ = CloseHandle(h);
                            }
                            Err(_) => failed += 1,
                        }
                    }
                    te.dwSize = core::mem::size_of::<THREADENTRY32>() as u32;
                    if Thread32Next(snap, &mut te).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
            if touched == 0 {
                return Err(format!("surecin hicbir thread'i acilamadi ({failed} deneme basarisiz) -- surec olmus veya yetki yetersiz olabilir"));
            }
            Ok(format!(
                "{} thread {} (BELGELENMIS yedek yol: OpenThread+{}Thread; {} thread acilamadi). UYARI: bu yol atomik degildir.",
                touched,
                if suspend { "donduruldu" } else { "cozuldu" },
                if suspend { "Suspend" } else { "Resume" },
                failed
            ))
        }
    }

    fn nt_whole_process(pid: u32, suspend: bool) -> Option<Result<String, String>> {
        unsafe {
            let name = if suspend { s!("NtSuspendProcess") } else { s!("NtResumeProcess") };
            let f = ntdll_proc(name)?;
            let handle = match OpenProcess(PROCESS_SUSPEND_RESUME, false, pid) {
                Ok(h) => h,
                Err(e) => return Some(Err(format!("surec acilamadi (PROCESS_SUSPEND_RESUME): {e}"))),
            };
            let status = f(handle);
            let _ = CloseHandle(handle);
            if status.0 < 0 {
                return Some(Err(format!("{} basarisiz: NTSTATUS 0x{:08x}", if suspend { "NtSuspendProcess" } else { "NtResumeProcess" }, status.0)));
            }
            Some(Ok(format!(
                "surecin TAMAMI {} (ntdll!{})",
                if suspend { "donduruldu" } else { "cozuldu" },
                if suspend { "NtSuspendProcess" } else { "NtResumeProcess" }
            )))
        }
    }

    fn suspend_or_resume(pid: u32, suspend: bool) -> Result<String, String> {
        match nt_whole_process(pid, suspend) {
            Some(Ok(msg)) => Ok(msg),
            // ntdll cagrisi bulundu ama BASARISIZ oldu: belgelenmis yola
            // dusmek yine de bir sans -- sessizce vazgecmeyiz.
            Some(Err(nt_err)) => unsafe {
                for_each_thread(pid, suspend).map(|m| format!("{m} (once denenen: {nt_err})")).map_err(|e| format!("{nt_err}; yedek yol da basarisiz: {e}"))
            },
            None => unsafe { for_each_thread(pid, suspend) },
        }
    }

    pub fn suspend_process(pid: u32) -> Result<String, String> {
        suspend_or_resume(pid, true)
    }

    pub fn resume_process(pid: u32) -> Result<String, String> {
        suspend_or_resume(pid, false)
    }

    pub fn terminate_process(pid: u32) -> Result<String, String> {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| format!("surec acilamadi (PROCESS_TERMINATE): {e}"))?;
            let res = TerminateProcess(handle, 1).map_err(|e| format!("TerminateProcess basarisiz: {e}"));
            let _ = CloseHandle(handle);
            res.map(|_| "TerminateProcess ile sonlandirildi".to_string())
        }
    }

    pub fn process_image(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 32768];
            let mut size = buf.len() as u32;
            let res = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size);
            let _ = CloseHandle(handle);
            res.ok()?;
            Some(String::from_utf16_lossy(&buf[..size as usize]))
        }
    }

    /// Verilen PID'in O ANDAKİ uzak TCP adreslerini toplar. `scanner.rs`
    /// ile AYNI `GetExtendedTcpTable` API'sini kullanır, ama
    /// `TCP_TABLE_OWNER_PID_ALL` sınıfıyla (yalnızca dinleyenler değil,
    /// KURULMUŞ bağlantılar da gerekli).
    ///
    /// Bilinçli filtre: loopback (`127.0.0.0/8`) ve `0.0.0.0` ASLA
    /// bloklanmaz — bunları bloklamak makinenin kendi iç haberleşmesini
    /// (CHIMERA'nın kendi IPC'si dahil) kırardı.
    pub fn remote_ips_of_pid(pid: u32) -> Result<Vec<String>, String> {
        use windows::Win32::NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
        };
        use windows::Win32::Networking::WinSock::AF_INET;
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedTcpTable(None, &mut size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0);
            if size == 0 {
                return Ok(Vec::new());
            }
            for _attempt in 0..3 {
                let mut buf = vec![0u8; size as usize];
                let rc = GetExtendedTcpTable(
                    Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                    &mut size,
                    false,
                    AF_INET.0 as u32,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                );
                if rc == 122 {
                    continue; // ERROR_INSUFFICIENT_BUFFER: tablo buyudu, tekrar dene
                }
                if rc != 0 {
                    return Err(format!("GetExtendedTcpTable basarisiz: kod {rc}"));
                }
                let table = buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
                let n = (*table).dwNumEntries as usize;
                let first = core::ptr::addr_of!((*table).table) as *const MIB_TCPROW_OWNER_PID;
                let mut out: Vec<String> = Vec::new();
                for i in 0..n {
                    let row = &*first.add(i);
                    if row.dwOwningPid != pid {
                        continue;
                    }
                    let o = row.dwRemoteAddr.to_le_bytes();
                    let ip = std::net::Ipv4Addr::new(o[0], o[1], o[2], o[3]);
                    if ip.is_loopback() || ip.is_unspecified() || ip.is_broadcast() {
                        continue;
                    }
                    let s = ip.to_string();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
                return Ok(out);
            }
            Err("GetExtendedTcpTable: arka arkaya ERROR_INSUFFICIENT_BUFFER (baglanti tablosu surekli degisiyor)".into())
        }
    }
}

#[cfg(not(windows))]
mod imp {
    const UNSUPPORTED: &str = "bu platform desteklenmiyor: surec askiya alma/sonlandirma yalnizca Windows'ta calisir";
    pub fn suspend_process(_pid: u32) -> Result<String, String> { Err(UNSUPPORTED.into()) }
    pub fn resume_process(_pid: u32) -> Result<String, String> { Err(UNSUPPORTED.into()) }
    pub fn terminate_process(_pid: u32) -> Result<String, String> { Err(UNSUPPORTED.into()) }
    pub fn process_image(_pid: u32) -> Option<String> { None }
    pub fn remote_ips_of_pid(_pid: u32) -> Result<Vec<String>, String> { Err(UNSUPPORTED.into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("chimera-cb-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("state")).unwrap();
        std::fs::create_dir_all(p.join("runtime")).unwrap();
        p
    }

    /// **En kritik güvenlik testi:** devre kesici CHIMERA'nın KENDİSİNİ
    /// veya eş bekçisini ASLA donduramamalıdır — aksi halde tek bir yanlış
    /// pozitif, sistemi kurtarılamaz hale getirirdi.
    #[test]
    fn own_process_kernel_and_sentinel_pids_are_protected() {
        let root = temp_root("protected");
        std::fs::write(root.join("runtime/sentinel.pid"), "424242").unwrap();

        assert!(is_protected_pid(&root, std::process::id()), "kendi PID'imiz KORUMALI olmali");
        assert!(is_protected_pid(&root, 0), "System Idle (0) KORUMALI olmali");
        assert!(is_protected_pid(&root, 4), "System (4) KORUMALI olmali");
        assert!(is_protected_pid(&root, 424242), "sentinel PID'i KORUMALI olmali");
        assert!(!is_protected_pid(&root, 999999), "ilgisiz bir PID korumali OLMAMALI");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Korumalı bir PID için `trip()` GERÇEKTEN hiçbir şey yapmamalı:
    /// ne askıya alma, ne kuyruğa yazma.
    #[test]
    fn tripping_a_protected_pid_takes_no_action_at_all() {
        let root = temp_root("protected-trip");
        let events = std::sync::Mutex::new(Vec::new());
        let audit = |e: &str, d: &str| events.lock().unwrap().push(format!("{e}:{d}"));

        let out = trip(&root, std::process::id(), &TripReason::DecoyTouched { path: "x".into() }, &audit);
        assert!(out.skipped_protected);
        assert!(!out.suspended);
        assert_eq!(pending_count(&root), 0, "korumali PID kuyruga YAZILMAMALI");
        assert!(out.to_text().contains("ATLANDI"));
        assert!(events.lock().unwrap().iter().any(|e| e.starts_with("circuit_breaker.skipped_protected")));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn suspended_record_round_trips_through_disk() {
        let root = temp_root("roundtrip");
        let rec = SuspendedRecord {
            ts: 1700000000,
            pid: 4242,
            image: r"C:\Users\kurban\AppData\Local\Temp\sifrele.exe".into(),
            reason: "tuzak dosyaya yazildi: calisan_maaslari_2026.xlsx".into(),
            suspended: true,
            blocked_ips: vec!["203.0.113.7".into(), "198.51.100.9".into()],
        };
        write_records(&root, &[rec.clone()]).unwrap();
        let back = read_records(&root);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0], rec, "diskten geri okunan kayit birebir ayni olmali");

        let text = list_suspended_text(&root);
        assert!(text.contains("pid=4242"));
        assert!(text.contains("203.0.113.7"));
        assert!(text.contains("INSAN ONAYI BEKLEYEN"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn quotes_in_image_paths_cannot_break_the_record_format() {
        let root = temp_root("quotes");
        let rec = SuspendedRecord {
            ts: 1,
            pid: 7,
            image: "C:\\kotu\"niyetli\".exe".into(),
            reason: "a\"b".into(),
            suspended: false,
            blocked_ips: vec![],
        };
        write_records(&root, &[rec]).unwrap();
        let back = read_records(&root);
        assert_eq!(back.len(), 1, "cift tirnak kaydi BOZMAMALI");
        assert_eq!(back[0].pid, 7);
        assert!(!back[0].image.contains('"'));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_queue_reports_nothing_pending() {
        let root = temp_root("empty");
        assert_eq!(pending_count(&root), 0);
        assert!(list_suspended_text(&root).contains("YOK"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resume_and_terminate_refuse_pids_that_are_not_in_the_queue() {
        let root = temp_root("notqueued");
        let audit = |_: &str, _: &str| {};
        assert!(resume(&root, 31337, &audit).is_err(), "kuyrukta olmayan PID devam ettirilemez");
        assert!(terminate(&root, 31337, &audit).is_err(), "kuyrukta olmayan PID SONLANDIRILAMAZ");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Sonlandırma, kuyrukta OLSA BİLE korumalı bir PID için reddedilmeli.
    #[test]
    fn terminate_refuses_protected_pids_even_when_queued() {
        let root = temp_root("term-protected");
        let me = std::process::id();
        write_records(
            &root,
            &[SuspendedRecord { ts: 1, pid: me, image: "x".into(), reason: "y".into(), suspended: true, blocked_ips: vec![] }],
        )
        .unwrap();
        let audit = |_: &str, _: &str| {};
        let err = terminate(&root, me, &audit).unwrap_err();
        assert!(err.contains("REDDEDILDI"));
        assert_eq!(pending_count(&root), 1, "reddedilen sonlandirma kaydi kuyruktan DUSURMEMELI");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trip_reason_carries_a_human_readable_detail() {
        let a = TripReason::DecoyTouched { path: "gizli.xlsx".into() };
        assert_eq!(a.as_str(), "decoy_touch");
        assert!(a.detail().contains("gizli.xlsx"));
        let b = TripReason::MassEncryption { detail: "pid=5 30 dosya".into() };
        assert_eq!(b.as_str(), "mass_encryption");
        assert!(b.detail().contains("30 dosya"));
    }
}
