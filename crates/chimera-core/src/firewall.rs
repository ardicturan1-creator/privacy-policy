//! firewall.rs — Windows Firewall (`INetFwPolicy2` COM API) üzerinden
//! GERÇEK ağ saldırısı engelleme.
//!
//! Burada özel bir kernel driver YAZILMADI (bkz. proje belgeleri — imzasız
//! bir `.sys` gerçek bir Windows makinesinde Secure Boot/driver imza
//! zorunluluğu nedeniyle zaten YÜKLENMEZ). Bunun yerine bu modül,
//! Windows'un KENDİ kernel-seviyeli filtreleme motorunu (WFP), o motorun
//! resmi kullanıcı-modu COM arayüzü üzerinden yönetir — Windows Güvenlik
//! Duvarı GUI'sinin, `netsh advfirewall`'ın ve PowerShell
//! `NetSecurity` modülünün ARKA PLANDA kullandığı, Vista'dan beri
//! değişmeyen aynı kararlı genel API. Yani: engelleme gerçekten kernel
//! seviyesinde uygulanır, ama bunu uygulayan kod bizim yazdığımız/imzalamamız
//! gereken bir sürücü değil, işletim sisteminin kendisidir.
//!
//! Her engelleme İKİ kural olarak eklenir (gelen VE giden) — yalnızca gelen
//! yönü engellemek, ele geçirilmiş bir sürecin o adrese GİDEN bağlantı
//! kurmasını (örn. C2 sunucusuna "eve telefon etmek") engellemez.

// Kural adi ureticileri ve yerel aday defteri yalnizca `#[cfg(windows)]`
// imp tarafindan VE testler tarafindan kullanilir; Linux'ta test-disi bir
// derlemede cagrilmadiklari icin "dead_code" uyarisi uretirler. Bu gercek
// bir olu kod DEGIL, platforma bagli bir cagri grafigidir (ayni desen
// `bruteforce.rs`'te de var) -- ACIKCA bastiriliyor ki ILERIDE cikacak
// GERCEK uyarilar bu gurultunun icinde kaybolmasin.
#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use std::path::{Path, PathBuf};

const RULE_PREFIX: &str = "CHIMERA-block-";

fn state_file(root: &Path) -> PathBuf {
    root.join("state/blocked_ips.list")
}

/// Girdinin gerçekten bir IPv4/IPv6 adresi olduğunu doğrular. Bu, hem
/// erken/anlaşılır bir hata vermek hem de rastgele bir metnin COM API'sine
/// "adres" olarak geçmesini önlemek içindir.
fn validate_ip(ip: &str) -> Result<(), String> {
    ip.parse::<std::net::IpAddr>().map(|_| ()).map_err(|_| format!("gecersiz IP adresi: {ip}"))
}

fn rule_name_in(ip: &str) -> String { format!("{RULE_PREFIX}IN-{ip}") }
fn rule_name_out(ip: &str) -> String { format!("{RULE_PREFIX}OUT-{ip}") }

fn read_candidates(root: &Path) -> Vec<String> {
    std::fs::read_to_string(state_file(root))
        .map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

fn write_candidates(root: &Path, ips: &[String]) -> std::io::Result<()> {
    use fs4::FileExt;
    let path = state_file(root);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&path)?;
    FileExt::lock(&f)?;
    let body = ips.join("\n");
    let res = std::io::Write::write_all(&mut f, body.as_bytes());
    let _ = FileExt::unlock(&f);
    res
}

fn remember(root: &Path, ip: &str) {
    let mut ips = read_candidates(root);
    if !ips.iter().any(|x| x == ip) {
        ips.push(ip.to_string());
        let _ = write_candidates(root, &ips);
    }
}

fn forget(root: &Path, ip: &str) {
    let ips: Vec<String> = read_candidates(root).into_iter().filter(|x| x != ip).collect();
    let _ = write_candidates(root, &ips);
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::{w, BSTR, Result as WinResult};
    use windows::Win32::Foundation::VARIANT_TRUE;
    use windows::Win32::NetworkManagement::WindowsFirewall::{
        INetFwPolicy2, INetFwRule, NET_FW_ACTION_BLOCK, NET_FW_IP_PROTOCOL_ANY,
        NET_FW_PROFILE2_ALL, NET_FW_RULE_DIR_IN, NET_FW_RULE_DIR_OUT,
    };
    use windows::Win32::System::Com::{
        CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    /// COM'u bu thread için başlatır. Zaten başlatılmışsa (S_FALSE) veya
    /// farklı bir modda başlatılmışsa (RPC_E_CHANGED_MODE — başka bir kod
    /// yolu zaten farklı bir threading modeliyle başlatmış demektir) bu
    /// GÜVENLE yok sayılır: her iki durumda da COM zaten kullanılabilir
    /// durumdadır, biz yalnızca "hiç başlatılmamış" durumunu düzeltmek
    /// istiyoruz.
    fn ensure_com() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn policy() -> WinResult<INetFwPolicy2> {
        ensure_com();
        unsafe {
            let clsid = CLSIDFromProgID(w!("HNetCfg.FwPolicy2"))?;
            CoCreateInstance(&clsid, None, CLSCTX_ALL)
        }
    }

    fn new_rule_obj() -> WinResult<INetFwRule> {
        ensure_com();
        unsafe {
            let clsid = CLSIDFromProgID(w!("HNetCfg.FWRule"))?;
            CoCreateInstance(&clsid, None, CLSCTX_ALL)
        }
    }

    fn build_rule(name: &str, ip: &str, dir: windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIRECTION) -> Result<INetFwRule, String> {
        let rule = new_rule_obj().map_err(|e| format!("kural nesnesi olusturulamadi: {e}"))?;
        unsafe {
            rule.SetName(&BSTR::from(name)).map_err(|e| e.to_string())?;
            rule.SetDescription(&BSTR::from("CHIMERA EDR tarafindan otomatik eklendi")).map_err(|e| e.to_string())?;
            rule.SetRemoteAddresses(&BSTR::from(ip)).map_err(|e| e.to_string())?;
            rule.SetDirection(dir).map_err(|e| e.to_string())?;
            rule.SetAction(NET_FW_ACTION_BLOCK).map_err(|e| e.to_string())?;
            rule.SetProtocol(NET_FW_IP_PROTOCOL_ANY.0).map_err(|e| e.to_string())?;
            rule.SetProfiles(NET_FW_PROFILE2_ALL.0).map_err(|e| e.to_string())?;
            rule.SetEnabled(VARIANT_TRUE).map_err(|e| e.to_string())?;
        }
        Ok(rule)
    }

    pub fn block_ip(root: &Path, ip: &str) -> Result<String, String> {
        validate_ip(ip)?;
        let pol = policy().map_err(|e| format!("firewall politikasi acilamadi (Windows Firewall servisi kapali olabilir): {e}"))?;
        let rules = unsafe { pol.Rules() }.map_err(|e| e.to_string())?;

        let in_rule = build_rule(&rule_name_in(ip), ip, NET_FW_RULE_DIR_IN)?;
        unsafe { rules.Add(&in_rule) }.map_err(|e| format!("gelen-yon kurali eklenemedi: {e}"))?;

        let out_rule = build_rule(&rule_name_out(ip), ip, NET_FW_RULE_DIR_OUT)?;
        unsafe { rules.Add(&out_rule) }.map_err(|e| format!("giden-yon kurali eklenemedi: {e}"))?;

        remember(root, ip);
        Ok(format!("bloklandi: {ip} (gelen+giden, tum profiller)"))
    }

    pub fn unblock_ip(root: &Path, ip: &str) -> Result<String, String> {
        validate_ip(ip)?;
        let pol = policy().map_err(|e| e.to_string())?;
        let rules = unsafe { pol.Rules() }.map_err(|e| e.to_string())?;
        let mut removed = 0u32;
        for name in [rule_name_in(ip), rule_name_out(ip)] {
            let bstr = BSTR::from(name.as_str());
            if unsafe { rules.Remove(&bstr) }.is_ok() {
                removed += 1;
            }
        }
        forget(root, ip);
        Ok(format!("kaldirildi: {ip} ({removed}/2 kural bulundu ve silindi)"))
    }

    /// Yerel aday listesini GERÇEK güvenlik duvarı durumuna karşı doğrular:
    /// her IP için hâlâ en az bir kural (IN veya OUT) canlı mı diye
    /// `Item()` ile sorgular. Dışarıdan (örn. bir yönetici `netsh` ile)
    /// silinmiş kurallar listeden düşer — bu yüzden çıktı her zaman güncel
    /// gerçek durumu yansıtır, yalnızca kendi defterimizi değil.
    pub fn list_blocked_ips(root: &Path) -> Result<Vec<String>, String> {
        let pol = policy().map_err(|e| e.to_string())?;
        let rules = unsafe { pol.Rules() }.map_err(|e| e.to_string())?;
        let candidates = read_candidates(root);
        let mut live = Vec::new();
        for ip in &candidates {
            let still_in = unsafe { rules.Item(&BSTR::from(rule_name_in(ip).as_str())) }.is_ok();
            let still_out = unsafe { rules.Item(&BSTR::from(rule_name_out(ip).as_str())) }.is_ok();
            if still_in || still_out {
                live.push(ip.clone());
            }
        }
        if live.len() != candidates.len() {
            let _ = write_candidates(root, &live);
        }
        Ok(live)
    }

    /// O anki aktif ağ profili için Windows Firewall'ın açık olup
    /// olmadığını sorgular (kapalıysa CHIMERA'nın eklediği kurallar dahil
    /// hiçbir kural uygulanmaz — bu yüzden `scanner.rs` bunu ayrı bir
    /// bulgu olarak raporlar).
    pub fn is_enabled() -> Result<bool, String> {
        let pol = policy().map_err(|e| e.to_string())?;
        let current = unsafe { pol.CurrentProfileTypes() }.map_err(|e| e.to_string())?;
        use windows::Win32::NetworkManagement::WindowsFirewall::{
            NET_FW_PROFILE2_DOMAIN, NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC,
        };
        for p in [NET_FW_PROFILE2_DOMAIN, NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC] {
            if current & p.0 != 0 {
                let enabled = unsafe { pol.get_FirewallEnabled(p) }.map_err(|e| e.to_string())?;
                return Ok(enabled.0 != 0);
            }
        }
        Err("aktif ag profili belirlenemedi".into())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    const UNSUPPORTED: &str = "bu platform desteklenmiyor: gercek guvenlik duvari kontrolu yalnizca Windows'ta calisir";
    pub fn block_ip(_root: &Path, ip: &str) -> Result<String, String> { validate_ip(ip)?; Err(UNSUPPORTED.into()) }
    pub fn unblock_ip(_root: &Path, ip: &str) -> Result<String, String> { validate_ip(ip)?; Err(UNSUPPORTED.into()) }
    pub fn list_blocked_ips(root: &Path) -> Result<Vec<String>, String> { Ok(read_candidates(root)) }
    pub fn is_enabled() -> Result<bool, String> { Err(UNSUPPORTED.into()) }
}

pub use imp::{block_ip, is_enabled, list_blocked_ips, unblock_ip};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ip_accepts_v4_and_v6() {
        assert!(validate_ip("203.0.113.7").is_ok());
        assert!(validate_ip("2001:db8::1").is_ok());
    }

    #[test]
    fn validate_ip_rejects_garbage() {
        assert!(validate_ip("not-an-ip").is_err());
        assert!(validate_ip("999.999.999.999").is_err());
        assert!(validate_ip("").is_err());
    }

    #[test]
    fn rule_names_are_stable_and_direction_scoped() {
        assert_eq!(rule_name_in("203.0.113.7"), "CHIMERA-block-IN-203.0.113.7");
        assert_eq!(rule_name_out("203.0.113.7"), "CHIMERA-block-OUT-203.0.113.7");
        assert_ne!(rule_name_in("203.0.113.7"), rule_name_out("203.0.113.7"));
    }

    #[test]
    fn candidate_list_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("chimera-fw-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        remember(&dir, "203.0.113.7");
        remember(&dir, "203.0.113.8");
        remember(&dir, "203.0.113.7"); // duplicate should not double-add
        let list = read_candidates(&dir);
        assert_eq!(list.len(), 2);
        forget(&dir, "203.0.113.7");
        let list = read_candidates(&dir);
        assert_eq!(list, vec!["203.0.113.8".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
