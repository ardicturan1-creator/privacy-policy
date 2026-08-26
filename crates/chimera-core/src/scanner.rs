//! scanner.rs — GERÇEK Windows sertleştirme/zafiyet taramaları.
//!
//! Her kontrol, gerçek bir Win32/registry API çağrısına dayanır — hiçbir
//! bulgu simüle edilmez. Kapsam bilinçli olarak DAR tutulmuştur: geniş
//! bir CVE veritabanı/tehdit istihbaratı entegrasyonu YOKTUR (bkz. proje
//! belgeleri); bunun yerine, tek başına çalışan bir ajanın gerçekten
//! doğrulayabileceği somut, iyi belgelenmiş Windows sertleştirme
//! kontrolleri uygulanır (SMBv1, RDP NLA, güvenlik duvarı durumu, açık
//! dinleyen portlar, autorun kayıtları).
//!
//! Her bulgu, `pipeline.rs`'in Validator aşamasının anlayacağı bir
//! `Remediation` taşıyabilir — ama SADECE dar, geri-alınabilir, iyi bilinen
//! bir düzeltmesi varsa. Diğer her şey yalnızca RAPORLANIR, otomatik
//! uygulanmaz.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Low => "DUSUK",
            Severity::Medium => "ORTA",
            Severity::High => "YUKSEK",
            Severity::Critical => "KRITIK",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Remediation {
    EnableFirewall,
    DisableSmb1,
}

impl Remediation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Remediation::EnableFirewall => "enable_firewall",
            Remediation::DisableSmb1 => "disable_smb1",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub remediation: Option<Remediation>,
}

pub fn scan() -> Vec<Finding> {
    let mut out = Vec::new();
    imp::check_firewall(&mut out);
    imp::check_smb1(&mut out);
    imp::check_rdp_nla(&mut out);
    imp::check_listening_ports(&mut out);
    imp::check_autoruns(&mut out);
    out
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
        KEY_READ, REG_DWORD,
    };
    use windows::core::PCWSTR;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `HKEY_LOCAL_MACHINE\<subkey>` altındaki bir REG_DWORD değerini okur.
    /// Anahtar veya değer yoksa `None` döner (bu bir hata değildir — pek
    /// çok kontrolde "yok" == "varsayılan/güvenli" anlamına gelir).
    fn read_dword(subkey: &str, value: &str) -> Option<u32> {
        unsafe {
            let mut hkey = HKEY::default();
            let sub_w = wide(subkey);
            let rc = RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(sub_w.as_ptr()), None, KEY_READ, &mut hkey);
            if rc.0 != 0 {
                return None;
            }
            let val_w = wide(value);
            let mut data: u32 = 0;
            let mut size: u32 = std::mem::size_of::<u32>() as u32;
            let mut kind = REG_DWORD;
            let rc = RegQueryValueExW(
                hkey,
                PCWSTR(val_w.as_ptr()),
                None,
                Some(&mut kind),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut size),
            );
            let _ = RegCloseKey(hkey);
            if rc.0 == 0 && kind == REG_DWORD { Some(data) } else { None }
        }
    }

    /// Bir anahtarın altındaki tüm DEĞER ADLARINI (autorun girişlerinde
    /// bunlar genelde uygulama adlarıdır) listeler. Rapor amaçlıdır --
    /// "kötü niyetli" olup olmadığına dair bir yargı İÇERMEZ (bunun için
    /// bir imza/hash veritabanı gerekir, ki bu sürümde yok — bkz. proje
    /// belgeleri); operatöre ham veriyi sunar.
    fn list_value_names(subkey: &str) -> Vec<String> {
        unsafe {
            let mut hkey = HKEY::default();
            let sub_w = wide(subkey);
            let rc = RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(sub_w.as_ptr()), None, KEY_READ, &mut hkey);
            if rc.0 != 0 {
                return Vec::new();
            }
            let mut out = Vec::new();
            let mut index = 0u32;
            loop {
                let mut name_buf = [0u16; 512];
                let mut name_len: u32 = name_buf.len() as u32;
                let rc = RegEnumValueW(
                    hkey,
                    index,
                    Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    None,
                    None,
                    None,
                );
                if rc.0 == 259 {
                    break; // ERROR_NO_MORE_ITEMS
                }
                if rc.0 != 0 && rc.0 != 234 {
                    break; // beklenmeyen hata (234 = ERROR_MORE_DATA, isim yine de yazilmis olabilir)
                }
                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                if !name.is_empty() {
                    out.push(name);
                }
                index += 1;
                if index > 4096 {
                    break; // makul bir ust sinir -- sonsuz donguye karsi
                }
            }
            let _ = RegCloseKey(hkey);
            out
        }
    }

    pub fn check_firewall(out: &mut Vec<Finding>) {
        match crate::firewall::is_enabled() {
            Ok(true) => {}
            Ok(false) => out.push(Finding {
                id: "firewall.disabled".into(),
                severity: Severity::Critical,
                title: "Windows Guvenlik Duvari KAPALI".into(),
                detail: "Aktif ag profili icin Windows Firewall devre disi. CHIMERA'nin IP engelleme ozelligi dahil hicbir guvenlik duvari kurali uygulanmiyor.".into(),
                remediation: Some(Remediation::EnableFirewall),
            }),
            Err(e) => out.push(Finding {
                id: "firewall.unknown".into(),
                severity: Severity::Medium,
                title: "Guvenlik duvari durumu okunamadi".into(),
                detail: e,
                remediation: None,
            }),
        }
    }

    pub fn check_smb1(out: &mut Vec<Finding>) {
        if let Some(v) = read_dword(r"SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters", "SMB1") {
            if v != 0 {
                out.push(Finding {
                    id: "smb1.enabled".into(),
                    severity: Severity::High,
                    title: "SMBv1 protokolu acik".into(),
                    detail: "SMBv1, WannaCry/NotPetya gibi solucanlarin kullandigi, Microsoft'un resmi olarak kaldirilmasini onerdigi eski bir protokoldur.".into(),
                    remediation: Some(Remediation::DisableSmb1),
                });
            }
        }
    }

    pub fn check_rdp_nla(out: &mut Vec<Finding>) {
        let rdp_enabled = read_dword(r"SYSTEM\CurrentControlSet\Control\Terminal Server", "fDenyTSConnections") == Some(0);
        if !rdp_enabled {
            return;
        }
        let nla = read_dword(
            r"SYSTEM\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp",
            "UserAuthentication",
        );
        if nla != Some(1) {
            out.push(Finding {
                id: "rdp.nla_disabled".into(),
                severity: Severity::High,
                title: "RDP acik ama Network Level Authentication (NLA) zorunlu degil".into(),
                detail: "Uzak Masaustu etkin ve NLA kapali/tanimsiz -- bu, kimlik dogrulamasiz oturum acma denemelerine (brute-force, BlueKeep-tarzi zafiyetler) yuzeyi genisletir. NLA acilmasi INSAN ONAYI gerektirir (bu sunucuya nasil eristiginizi etkileyebilir), bu yuzden otomatik duzeltilmez.".into(),
                remediation: None,
            });
        }
    }

    pub fn check_listening_ports(out: &mut Vec<Finding>) {
        match listening_ports() {
            Ok(ports) if !ports.is_empty() => {
                let list: Vec<String> = ports.iter().map(|(p, pid)| format!("{p}/pid={pid}")).collect();
                out.push(Finding {
                    id: "network.listening_ports".into(),
                    severity: Severity::Low,
                    title: format!("{} TCP portu dinleniyor", ports.len()),
                    detail: format!("Inceleme icin: {}", list.join(", ")),
                    remediation: None,
                });
            }
            Ok(_) => {}
            Err(e) => out.push(Finding {
                id: "network.scan_failed".into(),
                severity: Severity::Low,
                title: "Port taramasi basarisiz".into(),
                detail: e,
                remediation: None,
            }),
        }
    }

    fn listening_ports() -> Result<Vec<(u16, u32)>, String> {
        use windows::Win32::NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
        };
        use windows::Win32::Networking::WinSock::AF_INET;
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedTcpTable(None, &mut size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER, 0);
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
                    TCP_TABLE_OWNER_PID_LISTENER,
                    0,
                );
                if rc == 122 {
                    continue; // ERROR_INSUFFICIENT_BUFFER: tablo buyudu, `size` guncellendi, tekrar dene
                }
                if rc != 0 {
                    return Err(format!("GetExtendedTcpTable basarisiz: kod {rc}"));
                }
                let table = buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
                let n = (*table).dwNumEntries as usize;
                let first_row_ptr = core::ptr::addr_of!((*table).table) as *const MIB_TCPROW_OWNER_PID;
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    let row = &*first_row_ptr.add(i);
                    let port = u16::from_be(row.dwLocalPort as u16);
                    out.push((port, row.dwOwningPid));
                }
                return Ok(out);
            }
            Err("GetExtendedTcpTable: arka arkaya ERROR_INSUFFICIENT_BUFFER (tablo surekli degisiyor)".into())
        }
    }

    pub fn check_autoruns(out: &mut Vec<Finding>) {
        let names = list_value_names(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run");
        if !names.is_empty() {
            out.push(Finding {
                id: "autorun.entries".into(),
                severity: Severity::Low,
                title: format!("{} sistem-genelinde autorun girisi", names.len()),
                detail: format!("Inceleme icin (kotu amacli olup olmadigi bir imza veritabani olmadan belirlenemez): {}", names.join(", ")),
                remediation: None,
            });
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn check_firewall(out: &mut Vec<Finding>) {
        out.push(Finding {
            id: "platform.unsupported".into(),
            severity: Severity::Low,
            title: "Tarama bu platformda desteklenmiyor".into(),
            detail: "Gercek sertlestirme kontrolleri yalnizca Windows'ta calisir.".into(),
            remediation: None,
        });
    }
    pub fn check_smb1(_out: &mut Vec<Finding>) {}
    pub fn check_rdp_nla(_out: &mut Vec<Finding>) {}
    pub fn check_listening_ports(_out: &mut Vec<Finding>) {}
    pub fn check_autoruns(_out: &mut Vec<Finding>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_low_to_critical() {
        assert!(Severity::Low < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
        assert!(Severity::High < Severity::Critical);
    }

    #[test]
    fn scan_runs_without_panicking() {
        // Platforma bagli olarak farkli sonuc uretebilir; asil kontrol
        // panic'lememesi (ornegin registry/COM cagrilarindan).
        let _ = scan();
    }
}
