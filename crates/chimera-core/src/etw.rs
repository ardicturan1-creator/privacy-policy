//! etw.rs — Event Tracing for Windows (ETW) tüketicisi: **Ring-3'ten
//! kernel-kaynaklı telemetri.**
//!
//! ## Bu NE DEĞİLDİR
//!
//! Bu bir kernel driver DEĞİLDİR. Hiçbir `.sys` yazılmamıştır, hiçbir
//! kernel geri çağrısı (`PsSetCreateProcessNotifyRoutine`,
//! filesystem minifilter) kurulmamıştır ve öyle bir iddia YOKTUR.
//!
//! ## Bu NEDİR
//!
//! Windows çekirdeği, dosya ve süreç olaylarını **zaten** ETW üzerinden
//! yayınlar. Bu modül, o yayının resmî kullanıcı-modu tüketicisidir
//! (`StartTraceW` → `EnableTraceEx2` → `OpenTraceW` → `ProcessTrace`);
//! `logman`, `xperf`, Windows Performance Recorder ve pek çok ticari EDR
//! ürününün kullanıcı-modu bileşeni de tam olarak bu API'yi kullanır.
//!
//! Yani telemetri **kernel kaynaklıdır** ama onu toplayan kod Ring-3'te,
//! imzasız, sıradan bir süreçtir. `firewall.rs`'in Windows'un kendi
//! kernel filtreleme motorunu (WFP) kullanıcı-modu COM arayüzünden
//! yönetmesiyle **aynı desendir** (bkz. `05-TURN6-ULTRA-GUARD.md` §0.1).
//!
//! ## Neden bu Faz 1 ve 3'ün boşluğunu kapatır
//!
//! - `decoy.rs`'in Restart Manager tabanlı PID tespiti, dosyayı hızlıca
//!   açıp kapatan bir sürece **yetişemiyordu** (bkz. `06-*.md` §D).
//! - `cmdguard.rs`'in 4688 tespiti **olaydan sonradır** ve olay
//!   günlüğü gecikmesine tabidir (bkz. `08-*.md` §D).
//!
//! ETW her ikisinde de gecikmeyi milisaniyeler mertebesine indirir ve
//! olay, dosya tanıtıcısı kapansa bile **PID'i taşır**.
//!
//! ## Bu turda BİLİNÇLİ olarak yapılmayan: yük (payload) çözümü
//!
//! Bir ETW olayının gövdesini (dosya adı, komut satırı) çözmek
//! `TdhGetEventInformation`/`TdhFormatProperty` ile ayrı ve büyük bir
//! şema-çözme katmanı gerektirir. Bu turda **uygulanmadı** ve
//! uygulandığı iddia EDİLMEZ. Bunun yerine her olayın **başlığından**
//! (`EVENT_HEADER`, şema gerektirmez) okunabilen alanlar kullanılır:
//! sağlayıcı GUID'i, olay kimliği/opcode, **süreç kimliği**, thread
//! kimliği ve zaman damgası.
//!
//! Bu, "hangi dosya" sorusunu yanıtlamaz ama **"hangi PID, ne hızda,
//! ne tür kernel olayı üretiyor"** sorusunu gerçek zamanlı yanıtlar — ki
//! toplu şifreleme tespitinde asıl sinyal budur.

// Saglayici GUID'leri, olay kimlikleri ve oturum yonetimi yalnizca
// `#[cfg(windows)]` imp tarafindan VE testler tarafindan kullanilir --
// `firewall.rs`/`bruteforce.rs`/`cmdguard.rs` ile AYNI platforma-bagli
// cagri grafigi durumu.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::HashMap;

/// `Microsoft-Windows-Kernel-File` sağlayıcı GUID'i.
/// (Windows'un yayımladığı sabit değer; `logman query providers` ile
/// doğrulanabilir.)
pub const KERNEL_FILE_GUID: (u32, u16, u16, [u8; 8]) =
    (0xEDD08927, 0x9CC4, 0x4E65, [0xB9, 0x70, 0xC2, 0x56, 0x0F, 0xB5, 0xC2, 0x89]);

/// `Microsoft-Windows-Kernel-Process` sağlayıcı GUID'i.
pub const KERNEL_PROCESS_GUID: (u32, u16, u16, [u8; 8]) =
    (0x22FB2CD6, 0x0E7B, 0x422B, [0xA0, 0xC7, 0x2F, 0xAD, 0x1F, 0xD0, 0xE7, 0x16]);

/// Kernel-File sağlayıcısında yazma işlemine karşılık gelen olay
/// kimliği. (Kernel-File şeması: 12=Create, 14=Close, 15=Read,
/// **16=Write**, 26=DeletePath, 27=RenamePath.)
pub const KERNEL_FILE_WRITE_ID: u16 = 16;
/// Kernel-Process sağlayıcısında süreç başlatma olayı.
pub const KERNEL_PROCESS_START_ID: u16 = 1;

/// Oturum adı. Aynı anda iki CHIMERA çalışırsa ikincisi bu adı alamaz ve
/// bunu dürüstçe raporlar (sessizce ilkinin oturumunu ele geçirmez).
pub const SESSION_NAME: &str = "CHIMERA-ETW";

/// Yüksek yazma hızının rapor edileceği pencere.
pub const RATE_WINDOW_SECS: u64 = 30;
/// Bu pencerede bu kadar yazma olayı "olağandışı yüksek" sayılır.
pub const RATE_THRESHOLD: u32 = 2000;

/// PID başına kernel olay sayacı (kayan pencere).
///
/// **Neden bu SAYAÇ tek başına devre kesiciyi TETİKLEMEZ:** yüksek dosya
/// yazma hızı, fidye yazılımına özgü DEĞİLDİR — bir derleyici, bir
/// veritabanı, bir video dönüştürücü veya bir yedekleme aracı da aynı
/// hızı üretir. Bu yüzden bu sayaç yalnızca bir **bulgu** üretir
/// (`scanner.rs` üzerinden, insan incelemesine düşer). Otomatik askıya
/// alma, `heuristic.rs`'in ENTROPİ artı hız birleşimine ya da tuzağa
/// dokunmaya bağlı kalmaya devam eder. Bu, bilinçli bir yanlış-pozitif
/// tercihidir.
#[derive(Default)]
pub struct EventRates {
    /// pid -> (pencere baslangici, sayac)
    per_pid: HashMap<u32, (u64, u32)>,
    /// Pencere icinde gorulen surec BASLATMA olayi sayisi (baglam icin).
    process_starts: u32,
    process_window_start: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighRate {
    pub pid: u32,
    pub events: u32,
    pub window_secs: u64,
}

impl HighRate {
    pub fn as_detail(&self) -> String {
        format!(
            "pid={} son {} saniyede {} kernel dosya-yazma olayi uretti (esik {})",
            self.pid, self.window_secs, self.events, RATE_THRESHOLD
        )
    }
}

impl EventRates {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tek bir yazma olayını kaydeder. Pencere dolduğunda sayaç sıfırlanır
    /// (kayan pencere yerine sabit pencere: milisaniyede binlerce olay
    /// gelebilen bir yolda her olay için kuyruk budamak ÖLÇEKLENMEZDİ).
    pub fn observe_write(&mut self, pid: u32, now: u64) {
        let e = self.per_pid.entry(pid).or_insert((now, 0));
        if now.saturating_sub(e.0) >= RATE_WINDOW_SECS {
            *e = (now, 0);
        }
        e.1 = e.1.saturating_add(1);
    }

    /// Eşiği aşan PID'leri döner (en yüksekten düşüğe).
    pub fn high_rates(&self, now: u64) -> Vec<HighRate> {
        let mut out: Vec<HighRate> = self
            .per_pid
            .iter()
            .filter(|(_, (start, count))| *count >= RATE_THRESHOLD && now.saturating_sub(*start) < RATE_WINDOW_SECS)
            .map(|(pid, (_, count))| HighRate { pid: *pid, events: *count, window_secs: RATE_WINDOW_SECS })
            .collect();
        out.sort_by(|a, b| b.events.cmp(&a.events).then(a.pid.cmp(&b.pid)));
        out
    }

    /// Penceresi dolmuş kayıtları temizler — uzun süre çalışan bir
    /// serviste `HashMap`'in sınırsız büyümesini önler.
    pub fn evict_stale(&mut self, now: u64) {
        self.per_pid.retain(|_, (start, _)| now.saturating_sub(*start) < RATE_WINDOW_SECS * 4);
    }

    /// Kernel-Process sağlayıcısından gelen bir süreç başlatma olayını
    /// kaydeder. Bu sayı tek başına bir alarm ÜRETMEZ; yüksek yazma hızı
    /// raporuna **bağlam** olarak eklenir (bir fidye yazılımı dalgası
    /// sırasında süreç başlatma sayısı da tipik olarak yükselir).
    pub fn observe_process_start(&mut self, now: u64) {
        if now.saturating_sub(self.process_window_start) >= RATE_WINDOW_SECS {
            self.process_window_start = now;
            self.process_starts = 0;
        }
        self.process_starts = self.process_starts.saturating_add(1);
    }

    pub fn process_starts_in_window(&self, now: u64) -> u32 {
        if now.saturating_sub(self.process_window_start) >= RATE_WINDOW_SECS {
            0
        } else {
            self.process_starts
        }
    }

    pub fn tracked_pids(&self) -> usize {
        self.per_pid.len()
    }
}

/// ETW oturumunun durumu (yalnızca raporlama için).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtwStatus {
    /// Oturum kuruldu ve olay akıyor.
    Running { session: String },
    /// Bu platformda ETW yok. (Windows derlemesinde hiç üretilmez —
    /// `cfg(not(windows))` yolunun tek dönüş değeridir.)
    #[cfg_attr(windows, allow(dead_code))]
    Unsupported,
    /// Oturum kurulamadı — sebebi ile birlikte.
    Failed { reason: String },
}

impl EtwStatus {
    pub fn to_text(&self) -> String {
        match self {
            EtwStatus::Running { session } => format!("ETW: '{session}' oturumu calisiyor (kernel dosya/surec telemetrisi)"),
            EtwStatus::Unsupported => "ETW: bu platformda desteklenmiyor".to_string(),
            EtwStatus::Failed { reason } => format!("ETW: oturum KURULAMADI ({reason}) -- gercek zamanli kernel telemetrisi YOK"),
        }
    }
}

/// Yüksek yazma hızı gözlemlerinin yazıldığı durum dosyası.
/// `scanner.rs` bunu okuyup operatöre bir bulgu olarak sunar.
pub fn state_file(root: &std::path::Path) -> std::path::PathBuf {
    root.join("state/etw_highrate.list")
}

/// Sayacı periyodik olarak yoklayan ve eşiği aşan PID'leri diske +
/// denetim kaydına yazan hafif bir gözlemci thread'i başlatır.
///
/// Neden ayrı bir thread: `pipeline.rs`'in arka plan turu 30 DAKİKADA bir
/// çalışır, ama ETW hız penceresi 30 SANİYEDİR — tura bırakılsaydı
/// gözlemlerin neredeyse tamamı kaçırılırdı.
pub fn spawn_rate_watcher(
    rates: std::sync::Arc<std::sync::Mutex<EventRates>>,
    root: std::path::PathBuf,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    audit: impl Fn(&str, &str) + Send + 'static,
) {
    use std::sync::atomic::Ordering;
    std::thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            for _ in 0..RATE_WINDOW_SECS {
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let (hits, tracked, starts) = {
                let mut r = match rates.lock() {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let hits = r.high_rates(now);
                let starts = r.process_starts_in_window(now);
                r.evict_stale(now);
                (hits, r.tracked_pids(), starts)
            };
            if hits.is_empty() {
                continue;
            }
            audit(
                "etw.high_write_rate",
                &format!("{} surec esigi asti (izlenen PID: {tracked}, penceredeki surec baslatma: {starts})", hits.len()),
            );
            let mut lines = String::new();
            for h in &hits {
                audit("etw.high_write_rate.detail", &h.as_detail());
                lines.push_str(&format!("{{\"ts\":{},\"pid\":{},\"events\":{}}}\n", now, h.pid, h.events));
            }
            let path = state_file(&root);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Her tur ustune YAZILIR (biriktirilmez): bu dosya bir gecmis
            // kaydi degil, "SU AN ne oluyor" anlik goruntusudur. Kalici
            // gecmis zaten denetim kaydindadir.
            let _ = std::fs::write(&path, lines);
        }
    });
}

/// Durum dosyasındaki güncel yüksek-hız gözlemlerini okur. `max_age`'ten
/// eski kayıtlar ATILIR — bayat bir dosya, geçmiş bir olayı "şu an
/// oluyor" gibi göstermemelidir.
pub fn recent_high_rates(root: &std::path::Path, now: u64, max_age: u64) -> Vec<HighRate> {
    let Ok(text) = std::fs::read_to_string(state_file(root)) else { return Vec::new() };
    text.lines()
        .filter_map(|l| {
            let ts = num_field(l, "ts")?;
            if now.saturating_sub(ts) > max_age {
                return None;
            }
            Some(HighRate {
                pid: num_field(l, "pid")? as u32,
                events: num_field(l, "events")? as u32,
                window_secs: RATE_WINDOW_SECS,
            })
        })
        .collect()
}

fn num_field(line: &str, key: &str) -> Option<u64> {
    let k = format!("\"{key}\":");
    let start = line.find(&k)? + k.len();
    let rest = &line[start..];
    let end = rest.find([',', '}'])?;
    rest[..end].trim().parse().ok()
}

/// ETW tüketicisini arka planda başlatır. Dönen `EtwStatus`, oturumun
/// GERÇEKTEN kurulup kurulmadığını söyler — kurulamadıysa sahte bir
/// "çalışıyor" DÖNMEZ.
pub fn spawn(rates: std::sync::Arc<std::sync::Mutex<EventRates>>) -> EtwStatus {
    imp::spawn(rates)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use windows::core::{GUID, PCWSTR, PWSTR};
    use windows::Win32::System::Diagnostics::Etw::{
        CloseTrace, ControlTraceW, EnableTraceEx2, OpenTraceW, ProcessTrace, StartTraceW,
        CONTROLTRACE_HANDLE, EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_RECORD, EVENT_TRACE_CONTROL_STOP,
        EVENT_TRACE_LOGFILEW, EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE,
        PROCESS_TRACE_MODE_EVENT_RECORD, PROCESS_TRACE_MODE_REAL_TIME, TRACE_LEVEL_INFORMATION,
        WNODE_FLAG_TRACED_GUID,
    };

    fn guid(g: (u32, u16, u16, [u8; 8])) -> GUID {
        GUID::from_values(g.0, g.1, g.2, g.3)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // ETW geri cagrisi C ABI uzerinden, bizim secmedigimiz bir thread'den
    // gelir ve kullanici verisi tasimak icin yalnizca ham bir pointer
    // sunar; bu yuzden sayac global olmak zorundadir.
    //
    // `static mut` DEGIL `OnceLock` kullaniliyor: `static mut`'a paylasimli
    // referans almak Rust 2024'te tanimsiz davranis riski tasir
    // (`static_mut_refs` uyarisi) ve bir GUVENLIK urununde bunu kabul
    // etmek dogru olmaz. `OnceLock` ayni "bir kez yaz, cok kez oku"
    // desenini SAGLAM sekilde verir.
    static RATES: std::sync::OnceLock<Arc<Mutex<EventRates>>> = std::sync::OnceLock::new();
    static RUNNING: AtomicBool = AtomicBool::new(false);

    fn unix_now() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }

    /// Her ETW olayı için çağrılır. **Sıcak yol:** saniyede on binlerce
    /// kez çalışabilir, bu yüzden burada tahsis, biçimlendirme veya
    /// disk erişimi YOKTUR — yalnızca bir sayaç artırımı.
    unsafe extern "system" fn event_callback(record: *mut EVENT_RECORD) {
        if record.is_null() {
            return;
        }
        let rec = unsafe { &*record };
        let provider = rec.EventHeader.ProviderId;
        let event_id = rec.EventHeader.EventDescriptor.Id;
        let pid = rec.EventHeader.ProcessId;

        // Yalnizca Kernel-File YAZMA olaylari sayilir.
        let Some(rates) = RATES.get() else { return };
        // try_lock BASARISIZ olursa olay DUSURULUR. Bu bilinclidir: sicak
        // yolda kilit beklemek, ETW tamponlarinin tasmasina ve olay
        // KAYBINA yol acar -- birkac olayi saymamak, oturumu bogmaktan
        // iyidir.
        let Ok(mut r) = rates.try_lock() else { return };

        if provider == guid(KERNEL_FILE_GUID) && event_id == KERNEL_FILE_WRITE_ID {
            r.observe_write(pid, unix_now());
        } else if provider == guid(KERNEL_PROCESS_GUID) && event_id == KERNEL_PROCESS_START_ID {
            r.observe_process_start(unix_now());
        }
    }

    /// `EVENT_TRACE_PROPERTIES`, arkasında oturum adının da bulunduğu
    /// **değişken uzunlukta** bir tampon bekler. Bu yüzden yapı, düz bir
    /// bayt tamponunun başına yerleştirilir ve ad, `LoggerNameOffset`
    /// ile gösterilen konuma yazılır. Bu, Microsoft'un belgelediği
    /// zorunlu düzendir.
    fn properties_buffer(session: &str) -> Vec<u8> {
        let name_w = wide(session);
        let prop_size = core::mem::size_of::<EVENT_TRACE_PROPERTIES>();
        let total = prop_size + name_w.len() * 2 + 64; // ad + emniyet payi
        let mut buf = vec![0u8; total];

        unsafe {
            let p = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            (*p).Wnode.BufferSize = total as u32;
            (*p).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*p).Wnode.ClientContext = 1; // QPC zaman damgasi
            (*p).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
            (*p).BufferSize = 64; // KB
            (*p).MinimumBuffers = 4;
            (*p).MaximumBuffers = 32;
            (*p).FlushTimer = 1;
            (*p).LoggerNameOffset = prop_size as u32;

            let name_dst = buf.as_mut_ptr().add(prop_size) as *mut u16;
            core::ptr::copy_nonoverlapping(name_w.as_ptr(), name_dst, name_w.len());
        }
        buf
    }

    pub fn spawn(rates: Arc<Mutex<EventRates>>) -> EtwStatus {
        if RUNNING.swap(true, Ordering::SeqCst) {
            return EtwStatus::Failed { reason: "oturum zaten calisiyor".into() };
        }
        let _ = RATES.set(Arc::clone(&rates));

        let session_w = wide(SESSION_NAME);
        let mut props = properties_buffer(SESSION_NAME);
        let mut handle = CONTROLTRACE_HANDLE::default();

        // Onceki bir cokmeden kalma ayni isimli oturum varsa once
        // durdurulur; aksi halde StartTraceW ERROR_ALREADY_EXISTS doner.
        unsafe {
            let mut stop_props = properties_buffer(SESSION_NAME);
            let _ = ControlTraceW(
                CONTROLTRACE_HANDLE::default(),
                PCWSTR(session_w.as_ptr()),
                stop_props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES,
                EVENT_TRACE_CONTROL_STOP,
            );
        }

        let rc = unsafe {
            StartTraceW(
                &mut handle,
                PCWSTR(session_w.as_ptr()),
                props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES,
            )
        };
        if rc.0 != 0 {
            RUNNING.store(false, Ordering::SeqCst);
            return EtwStatus::Failed {
                reason: format!("StartTraceW basarisiz: kod {} (YONETICI hakki gerekir)", rc.0),
            };
        }

        // Iki kernel saglayicisini bu oturuma bagla.
        for g in [KERNEL_FILE_GUID, KERNEL_PROCESS_GUID] {
            let id = guid(g);
            let rc = unsafe {
                EnableTraceEx2(
                    handle,
                    &id,
                    EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
                    TRACE_LEVEL_INFORMATION as u8,
                    0, // MatchAnyKeyword = 0 -> saglayicinin varsayilan kumesi
                    0,
                    0,
                    None,
                )
            };
            if rc.0 != 0 {
                let mut stop_props = properties_buffer(SESSION_NAME);
                unsafe {
                    let _ = ControlTraceW(handle, PCWSTR::null(), stop_props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_STOP);
                }
                RUNNING.store(false, Ordering::SeqCst);
                return EtwStatus::Failed { reason: format!("EnableTraceEx2 basarisiz: kod {}", rc.0) };
            }
        }

        // `ProcessTrace` BLOKLAR (oturum durdurulana kadar donmez), bu
        // yuzden kendi thread'inde calisir -- mevcut arka plan thread'i
        // desenleriyle tutarli.
        //
        // **Neden thread'in ONAYINI BEKLIYORUZ:** oturumun KURULMASI
        // (`StartTraceW`) ile olaylarin AKMASI ayri sorulardir. Wine
        // altinda canli olcum bunu somut olarak gosterdi: `StartTraceW`
        // ve `EnableTraceEx2` 0 (basarili) donuyor ama `OpenTraceW`
        // GECERSIZ tanitici donduruyor ve TEK BIR olay bile teslim
        // edilmiyor. Ilk uygulama bu noktada "calisiyor" diyordu --
        // yani operatore GERCEK ZAMANLI TELEMETRIM VAR diye YANLIS bilgi
        // veriyordu. Artik tuketici thread'i `OpenTraceW` sonucunu geri
        // bildirmeden `Running` DONULMEZ.
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        std::thread::spawn(move || {
            let mut logfile = EVENT_TRACE_LOGFILEW::default();
            let mut name = wide(SESSION_NAME);
            logfile.LoggerName = PWSTR(name.as_mut_ptr());
            logfile.Anonymous1.ProcessTraceMode = PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
            logfile.Anonymous2.EventRecordCallback = Some(event_callback);

            let trace = unsafe { OpenTraceW(&mut logfile) };
            // INVALID_PROCESSTRACE_HANDLE, 64-bit'te u64::MAX'tir.
            if trace.Value == u64::MAX {
                let _ = tx.send(Err("OpenTraceW GECERSIZ tanitici dondu -- oturum kuruldu ama olay TESLIM EDILEMIYOR".into()));
                RUNNING.store(false, Ordering::SeqCst);
                return;
            }
            let _ = tx.send(Ok(()));
            let _ = unsafe { ProcessTrace(&[trace], None, None) };
            let _ = unsafe { CloseTrace(trace) };
            RUNNING.store(false, Ordering::SeqCst);
        });

        let confirmation = rx.recv_timeout(std::time::Duration::from_secs(5));
        let failure = match confirmation {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(e),
            Err(_) => Some("tuketici thread'i 5 saniyede yanit vermedi".to_string()),
        };
        if let Some(reason) = failure {
            // Yarim kalmis oturumu birakmayiz: aksi halde sistemde
            // hicbir sey tuketmeyen bir ETW oturumu asili kalirdi.
            let mut stop_props = properties_buffer(SESSION_NAME);
            unsafe {
                let _ = ControlTraceW(handle, PCWSTR::null(), stop_props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_STOP);
            }
            RUNNING.store(false, Ordering::SeqCst);
            return EtwStatus::Failed { reason };
        }

        EtwStatus::Running { session: SESSION_NAME.to_string() }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn spawn(_rates: std::sync::Arc<std::sync::Mutex<EventRates>>) -> EtwStatus {
        EtwStatus::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_guids_match_the_values_windows_publishes() {
        // Bu GUID'ler Windows'un SABIT, yayimlanmis degerleridir; kazara
        // degistirilmeleri ETW oturumunu SESSIZCE bos birakirdi.
        assert_eq!(KERNEL_FILE_GUID.0, 0xEDD08927);
        assert_eq!(KERNEL_PROCESS_GUID.0, 0x22FB2CD6);
        assert_ne!(KERNEL_FILE_GUID, KERNEL_PROCESS_GUID);
    }

    #[test]
    fn a_quiet_process_never_reports_a_high_rate() {
        let mut r = EventRates::new();
        for i in 0..100 {
            r.observe_write(1234, 1000 + i % 10);
        }
        assert!(r.high_rates(1005).is_empty(), "100 olay esigin ({RATE_THRESHOLD}) COK altinda");
    }

    #[test]
    fn crossing_the_threshold_reports_the_pid() {
        let mut r = EventRates::new();
        for _ in 0..RATE_THRESHOLD {
            r.observe_write(4242, 1000);
        }
        let hits = r.high_rates(1005);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pid, 4242);
        assert_eq!(hits[0].events, RATE_THRESHOLD);
        assert!(hits[0].as_detail().contains("pid=4242"));
    }

    /// Pencere dolduğunda sayaç sıfırlanmalı — aksi halde günlerce çalışan
    /// meşru bir süreç eninde sonunda eşiği aşar ve YANLIŞ alarm üretirdi.
    #[test]
    fn the_counter_resets_when_the_window_rolls_over() {
        let mut r = EventRates::new();
        for _ in 0..RATE_THRESHOLD {
            r.observe_write(7, 1000);
        }
        assert_eq!(r.high_rates(1000).len(), 1);

        // Pencere doldu: yeni bir olay sayaci SIFIRDAN baslatmali.
        r.observe_write(7, 1000 + RATE_WINDOW_SECS);
        let hits = r.high_rates(1000 + RATE_WINDOW_SECS);
        assert!(hits.is_empty(), "pencere donduktan sonra eski sayim TASINMAMALI");
    }

    #[test]
    fn different_pids_are_counted_independently() {
        let mut r = EventRates::new();
        for _ in 0..RATE_THRESHOLD {
            r.observe_write(1, 1000);
        }
        r.observe_write(2, 1000);
        let hits = r.high_rates(1000);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pid, 1);
        assert_eq!(r.tracked_pids(), 2);
    }

    /// Uzun süre çalışan bir serviste sayaç haritası sınırsız büyümemeli.
    #[test]
    fn stale_pids_are_evicted_so_memory_does_not_grow_without_bound() {
        let mut r = EventRates::new();
        for pid in 0..500u32 {
            r.observe_write(pid, 1000);
        }
        assert_eq!(r.tracked_pids(), 500);
        r.evict_stale(1000 + RATE_WINDOW_SECS * 5);
        assert_eq!(r.tracked_pids(), 0, "eski kayitlar TEMIZLENMELI");

        // Taze kayitlar KORUNMALI.
        r.observe_write(9, 2000);
        r.evict_stale(2001);
        assert_eq!(r.tracked_pids(), 1);
    }

    #[test]
    fn status_text_never_claims_success_when_it_failed() {
        assert!(EtwStatus::Failed { reason: "kod 5".into() }.to_text().contains("KURULAMADI"));
        assert!(EtwStatus::Unsupported.to_text().contains("desteklenmiyor"));
        assert!(EtwStatus::Running { session: "X".into() }.to_text().contains("calisiyor"));
    }

    #[test]
    fn process_start_counting_also_respects_the_window() {
        let mut r = EventRates::new();
        for _ in 0..5 {
            r.observe_process_start(1000);
        }
        assert_eq!(r.process_starts_in_window(1000), 5);
        // Pencere dondugunde sayac SIFIRLANMALI.
        assert_eq!(r.process_starts_in_window(1000 + RATE_WINDOW_SECS), 0);
        r.observe_process_start(1000 + RATE_WINDOW_SECS);
        assert_eq!(r.process_starts_in_window(1000 + RATE_WINDOW_SECS), 1);
    }

    #[test]
    fn recent_high_rates_ignores_stale_observations() {
        let root = std::env::temp_dir().join(format!("chimera-etw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("state")).unwrap();
        std::fs::write(
            state_file(&root),
            "{\"ts\":1000,\"pid\":11,\"events\":5000}\n{\"ts\":100,\"pid\":22,\"events\":9000}\n",
        )
        .unwrap();

        // max_age=120: yalnizca ts=1000 taze sayilir.
        let hits = recent_high_rates(&root, 1050, 120);
        assert_eq!(hits.len(), 1, "bayat gozlem 'su an oluyor' gibi GOSTERILMEMELI");
        assert_eq!(hits[0].pid, 11);
        assert_eq!(hits[0].events, 5000);

        // Hepsi bayatsa hicbiri donmemeli.
        assert!(recent_high_rates(&root, 99999, 120).is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_state_file_yields_no_observations() {
        let root = std::env::temp_dir().join(format!("chimera-etw-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert!(recent_high_rates(&root, 1000, 120).is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_platforms_report_unsupported_rather_than_pretending() {
        let rates = std::sync::Arc::new(std::sync::Mutex::new(EventRates::new()));
        assert_eq!(spawn(rates), EtwStatus::Unsupported);
    }
}
