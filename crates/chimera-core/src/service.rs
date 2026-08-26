//! service.rs — Windows Servisi olarak kurulum ve çalışma.
//!
//! `05-TURN6-ULTRA-GUARD.md` §6'da dürüstçe belgelenen bir sınır vardı:
//!
//! > *"Doğrulanamayan tek şey, bu ikilinin şu anda bir Windows SERVİSİ
//! > olarak KURULU olmadığıdır (düz bir konsol/arka plan exe'sidir) — bir
//! > Windows Servisi olarak kurulsaydı, `SERVICE_CONTROL_STOP` farklı bir
//! > mekanizma (`RegisterServiceCtrlHandlerEx`) gerektirirdi."*
//!
//! Bu modül tam olarak o boşluğu kapatır. Servis olmak üç somut şey
//! kazandırır:
//!
//!   1. **Kullanıcı oturumundan bağımsız çalışma** — kimse oturum açmasa
//!      bile CHIMERA çalışır; oturum kapatma onu durdurmaz.
//!   2. **Makine açılışında otomatik başlama** (`SERVICE_AUTO_START`).
//!   3. **Servis Denetim Yöneticisi (SCM) tarafından denetlenen, TEMİZ
//!      durdurma** — `net stop` / `sc stop`, konsol `CTRL_C_EVENT`'inden
//!      tamamen farklı bir yoldur ve mevcut `ctrlc` işleyicisi bunu
//!      GÖRMEZ. Bu yüzden `SERVICE_CONTROL_STOP` ayrıca ele alınır ve
//!      var olan `runtime/stop.flag` mekanizmasına bağlanır — yani temiz
//!      durdurma mantığı ÇOĞALTILMAZ, yeniden kullanılır.
//!
//! ## Dürüst sınırlar
//!
//!   - **Kurulum yönetici hakkı gerektirir.** `OpenSCManagerW` ayrıcalıksız
//!     bir hesapta başarısız olur ve bu açıkça raporlanır.
//!   - **Servis olmak, öldürülemezlik DEĞİLDİR.** Bir yönetici
//!     `sc delete` veya `taskkill /F` ile servisi durdurabilir/silebilir.
//!     Projenin kurucu kuralı değişmedi.
//!   - **`SERVICE_CONTROL_STOP` mekanizması bu ortamda CANLI
//!     DOĞRULANAMADI** — Wine'ın Servis Denetim Yöneticisi kısmidir.
//!     Kod, gerçek `advapi32` bağlarına karşı derlenir ve çağrı dizisi
//!     Microsoft'un belgelediği sıradır; ama "gerçek bir Windows'ta
//!     `net stop chimera-core` temiz durduruyor" iddiası yalnızca gerçek
//!     bir Windows makinesinde kanıtlanabilir.

// Bu ogeler yalnizca `#[cfg(windows)]` imp tarafindan VE testler
// tarafindan kullanilir -- diger modullerdeki (firewall/bruteforce/
// cmdguard/etw) AYNI platforma-bagli cagri grafigi durumu.
#![cfg_attr(not(windows), allow(dead_code))]

/// Servisin SCM'deki adı (`sc query chimera-core` ile görünen ad).
pub const SERVICE_NAME: &str = "chimera-core";
/// Hizmetler panelinde görünen okunabilir ad.
pub const DISPLAY_NAME: &str = "CHIMERA EDR Core";

/// Kurulum/kaldırma sonucunun insan-okunabilir hâli.
pub type ServiceResult = Result<String, String>;

/// Servisi SCM'e kaydeder. `root`, servise `--root` argümanı olarak
/// geçirilir — servis, kurulduğu andaki kök dizinle çalışır.
pub fn install(root: &std::path::Path) -> ServiceResult {
    imp::install(root)
}

/// Servisi SCM'den siler.
pub fn uninstall() -> ServiceResult {
    imp::uninstall()
}

/// Süreci SCM'e bir servis olarak bağlar ve `serve_body` çalıştırır.
/// SCM bağlantısı kurulamazsa (yani süreç servis olarak DEĞİL, normal
/// konsoldan başlatılmışsa) `Err` döner — çağıran o zaman normal konsol
/// yoluna düşer.
pub fn run_as_service(serve_body: fn(&std::path::Path) -> i32) -> Result<(), String> {
    imp::run_as_service(serve_body)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::OnceLock;
    // `Stream::connect` bir OZELLIK (trait) metodudur; `main.rs` bunu
    // `prelude::*` uzerinden aliyor, burada ACIKCA ice aktariyoruz.
    use interprocess::local_socket::traits::Stream as _;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::System::Services::{
        CloseServiceHandle, CreateServiceW, DeleteService, OpenSCManagerW, OpenServiceW,
        RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW, SERVICE_AUTO_START,
        SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP,
        SERVICE_ERROR_NORMAL, SERVICE_RUNNING, SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED,
        SERVICE_STOP_PENDING, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS, SC_MANAGER_ALL_ACCESS,
        SERVICE_ALL_ACCESS,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn install(root: &Path) -> ServiceResult {
        let exe = std::env::current_exe().map_err(|e| format!("kendi yolumuz okunamadi: {e}"))?;
        // SCM, komut satirini oldugu gibi calistirir; bosluklu yollar icin
        // exe yolu TIRNAK icine alinmalidir.
        let cmd = format!("\"{}\" serve --root \"{}\"", exe.display(), root.display());

        unsafe {
            let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS)
                .map_err(|e| format!("Servis Denetim Yoneticisi acilamadi (YONETICI hakki gerekir): {e}"))?;

            let name = wide(SERVICE_NAME);
            let display = wide(DISPLAY_NAME);
            let bin = wide(&cmd);

            let result = CreateServiceW(
                scm,
                PCWSTR(name.as_ptr()),
                PCWSTR(display.as_ptr()),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START, // makine acilisinda otomatik baslar
                SERVICE_ERROR_NORMAL,
                PCWSTR(bin.as_ptr()),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(), // LocalSystem hesabi
                PCWSTR::null(),
            );

            let out = match result {
                Ok(svc) => {
                    let _ = CloseServiceHandle(svc);
                    Ok(format!(
                        "Servis kuruldu: {SERVICE_NAME} ({DISPLAY_NAME})\n  komut: {cmd}\n  baslangic: OTOMATIK (makine acilisinda)\n  hesap: LocalSystem\nBaslatmak icin: sc start {SERVICE_NAME}"
                    ))
                }
                Err(e) => Err(format!("servis olusturulamadi (zaten kurulu olabilir): {e}")),
            };
            let _ = CloseServiceHandle(scm);
            out
        }
    }

    pub fn uninstall() -> ServiceResult {
        unsafe {
            let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS)
                .map_err(|e| format!("Servis Denetim Yoneticisi acilamadi (YONETICI hakki gerekir): {e}"))?;
            let name = wide(SERVICE_NAME);
            let out = match OpenServiceW(scm, PCWSTR(name.as_ptr()), SERVICE_ALL_ACCESS) {
                Ok(svc) => {
                    let r = DeleteService(svc).map(|_| format!("Servis silindi: {SERVICE_NAME}")).map_err(|e| format!("servis silinemedi: {e}"));
                    let _ = CloseServiceHandle(svc);
                    r
                }
                Err(e) => Err(format!("servis acilamadi (kurulu degil olabilir): {e}")),
            };
            let _ = CloseServiceHandle(scm);
            out
        }
    }

    // --- Servis calisma zamani durumu ---
    //
    // SCM geri cagrilari, bizim secmedigimiz bir thread'den ve C ABI
    // uzerinden gelir; bu yuzden durum GLOBAL olmak zorundadir. Kapsam
    // bilincli olarak minimumda tutuldu: bir tanitici, bir bayrak ve
    // servisin kok dizini.
    // `static mut` KULLANILMIYOR: `static mut`'a paylasimli referans almak
    // Rust 2024'te tanimsiz davranis riski tasir (`static_mut_refs`
    // uyarisi). Bunun yerine `OnceLock` ("bir kez yaz, cok kez oku") ve
    // `AtomicIsize` (tanitici) kullaniliyor -- ayni islev, SAGLAM bellek
    // modeli.
    static STATUS_HANDLE: AtomicIsize = AtomicIsize::new(0);
    static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
    static SERVICE_ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    static SERVE_BODY: OnceLock<fn(&Path) -> i32> = OnceLock::new();

    fn set_status(state: windows::Win32::System::Services::SERVICE_STATUS_CURRENT_STATE, accept: u32, exit_code: u32) {
        let raw = STATUS_HANDLE.load(Ordering::SeqCst);
        if raw == 0 {
            return; // henuz SCM'e kaydolmadik
        }
        let h = SERVICE_STATUS_HANDLE(raw as *mut core::ffi::c_void);
        unsafe {
            let status = SERVICE_STATUS {
                dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                dwCurrentState: state,
                dwControlsAccepted: accept,
                dwWin32ExitCode: exit_code,
                dwServiceSpecificExitCode: 0,
                dwCheckPoint: 0,
                dwWaitHint: 3000,
            };
            let _ = SetServiceStatus(h, &status);
        }
    }

    /// SCM'den gelen denetim kodlarını işler. **`SERVICE_CONTROL_STOP`,
    /// konsol `CTRL_C_EVENT`'inden TAMAMEN farklı bir yoldur** — mevcut
    /// `ctrlc` işleyicisi bunu görmez. Bu yüzden burada, var olan temiz
    /// durdurma mekanizmasının aynısı tetiklenir: `runtime/stop.flag`
    /// yazılır ve bloklayan `accept()` çağrısı kendi soketimize sahte bir
    /// bağlantı açılarak uyandırılır (bkz. `main.rs`'teki aynı desen).
    unsafe extern "system" fn control_handler(
        control: u32,
        _event_type: u32,
        _event_data: *mut core::ffi::c_void,
        _context: *mut core::ffi::c_void,
    ) -> u32 {
        if control == SERVICE_CONTROL_STOP || control == SERVICE_CONTROL_SHUTDOWN {
            STOP_REQUESTED.store(true, Ordering::SeqCst);
            set_status(SERVICE_STOP_PENDING, 0, 0);
            if let Some(root) = SERVICE_ROOT.get() {
                let _ = std::fs::create_dir_all(root.join("runtime"));
                let _ = std::fs::write(root.join("runtime/stop.flag"), b"1");
                if let Ok(wake) = chimera_ipc::socket_name(root) {
                    let _ = interprocess::local_socket::Stream::connect(wake);
                }
            }
        }
        0 // NO_ERROR
    }

    unsafe extern "system" fn service_main(_argc: u32, _argv: *mut PWSTR) {
        unsafe {
            let name = wide(SERVICE_NAME);
            let handle = match RegisterServiceCtrlHandlerExW(PCWSTR(name.as_ptr()), Some(control_handler), None) {
                Ok(h) => h,
                Err(_) => return, // SCM'e baglanamadik; yapabilecegimiz bir sey yok
            };
            STATUS_HANDLE.store(handle.0 as isize, Ordering::SeqCst);
            set_status(SERVICE_RUNNING, SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN, 0);

            let code = match (SERVE_BODY.get(), SERVICE_ROOT.get()) {
                (Some(body), Some(root)) => body(root),
                _ => 1,
            };

            set_status(SERVICE_STOPPED, 0, code as u32);
        }
    }

    pub fn run_as_service(serve_body: fn(&Path) -> i32) -> Result<(), String> {
        // Kok dizin, servis komut satirindaki `--root` argumanindan
        // okunur (kurulumda oraya yazilmisti).
        let args: Vec<String> = std::env::args().collect();
        let root = args
            .iter()
            .position(|a| a == "--root")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "C:\\ProgramData\\chimera-core".to_string());

        let _ = SERVICE_ROOT.set(std::path::PathBuf::from(root));
        let _ = SERVE_BODY.set(serve_body);

        let name = wide(SERVICE_NAME);
        let table = [
            SERVICE_TABLE_ENTRYW { lpServiceName: PWSTR(name.as_ptr() as *mut u16), lpServiceProc: Some(service_main) },
            SERVICE_TABLE_ENTRYW { lpServiceName: PWSTR::null(), lpServiceProc: None },
        ];

        // Bu cagri, surec GERCEKTEN SCM tarafindan baslatildiysa servis
        // bitene kadar BLOKLAR. Normal bir konsoldan calistirildiysa
        // hemen ERROR_FAILED_SERVICE_CONTROLLER_CONNECT (1063) ile
        // basarisiz olur -- cagiran bunu gorup normal konsol yoluna duser.
        unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) }
            .map_err(|e| format!("SCM'e baglanilamadi (surec bir servis olarak baslatilmamis): {e}"))
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    const UNSUPPORTED: &str = "bu platform desteklenmiyor: Windows Servisi kurulumu yalnizca Windows'ta calisir";
    pub fn install(_root: &std::path::Path) -> ServiceResult {
        Err(UNSUPPORTED.into())
    }
    pub fn uninstall() -> ServiceResult {
        Err(UNSUPPORTED.into())
    }
    pub fn run_as_service(_serve_body: fn(&std::path::Path) -> i32) -> Result<(), String> {
        Err(UNSUPPORTED.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_service_identity_is_stable_and_non_empty() {
        // Servis adi SCM'de bir ANAHTARDIR: degisirse, onceki kurulum
        // kaldirilamaz hale gelir. Bu test onu kazara degistirmeye karsi
        // bir bariyerdir.
        assert_eq!(SERVICE_NAME, "chimera-core");
        assert_eq!(DISPLAY_NAME, "CHIMERA EDR Core");
        assert!(!SERVICE_NAME.contains(' '), "servis adi bosluk ICERMEMELI (sc komutlarini kirar)");
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_platforms_refuse_explicitly_instead_of_pretending() {
        let e = install(std::path::Path::new("/tmp/x")).unwrap_err();
        assert!(e.contains("desteklenmiyor"));
        assert!(uninstall().unwrap_err().contains("desteklenmiyor"));
        assert!(run_as_service(|_| 0).unwrap_err().contains("desteklenmiyor"));
    }
}
