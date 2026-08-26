//! remediate.rs — `pipeline.rs`'in Executor aşamasının çağırdığı, DAR ve
//! GERİ ALINABİLİR düzeltme fonksiyonları. Buradaki her fonksiyon:
//!   (a) yalnızca TEK, iyi tanımlanmış bir sistem ayarını değiştirir,
//!   (b) tersine çevrilebilir (aynı ayarı geri açmak/kapatmak yeterlidir),
//!   (c) hiçbir veri kaybına/kalıcı yapı değişikliğine yol açmaz,
//!   (d) reboot/servis yeniden başlatma gibi KESİNTİ yaratan ek adımları
//!       OTOMATİK tetiklemez — bunu gerektiren durumlarda dönüş mesajı
//!       operatöre bunu açıkça söyler.
//! `pipeline.rs`'teki whitelist'e YENİ bir fonksiyon eklemek isteyen biri,
//! önce burada bu dört şartı da karşılayan bir fonksiyon yazmak zorundadır.

#[cfg(windows)]
mod imp {
    pub fn enable_firewall() -> Result<String, String> {
        use windows::Win32::Foundation::VARIANT_TRUE;
        use windows::Win32::NetworkManagement::WindowsFirewall::{
            INetFwPolicy2, NET_FW_PROFILE2_DOMAIN, NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC,
        };
        use windows::Win32::System::Com::{CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
        use windows::core::w;

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let clsid = CLSIDFromProgID(w!("HNetCfg.FwPolicy2")).map_err(|e| e.to_string())?;
            let pol: INetFwPolicy2 = CoCreateInstance(&clsid, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            for p in [NET_FW_PROFILE2_DOMAIN, NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC] {
                pol.put_FirewallEnabled(p, VARIANT_TRUE).map_err(|e| e.to_string())?;
            }
        }
        Ok("Windows Guvenlik Duvari her 3 profilde (Domain/Private/Public) acildi".into())
    }

    pub fn disable_smb1() -> Result<String, String> {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_DWORD,
        };
        use windows::core::PCWSTR;

        let subkey: Vec<u16> = r"SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let value: Vec<u16> = "SMB1".encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let mut hkey = HKEY::default();
            let rc = RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(subkey.as_ptr()), None, KEY_SET_VALUE, &mut hkey);
            if rc.0 != 0 {
                return Err(format!("LanmanServer\\Parameters acilamadi (yonetici hakki gerekebilir): kod {}", rc.0));
            }
            let data = 0u32.to_le_bytes();
            let rc = RegSetValueExW(hkey, PCWSTR(value.as_ptr()), None, REG_DWORD, Some(&data));
            let _ = RegCloseKey(hkey);
            if rc.0 != 0 {
                return Err(format!("SMB1 degeri yazilamadi: kod {}", rc.0));
            }
        }
        Ok("SMB1=0 yazildi. Tam etkin olmasi icin LanmanServer servisinin yeniden baslatilmasi (veya yeniden baslatma) gerekir -- bu OTOMATIK tetiklenmedi.".into())
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn enable_firewall() -> Result<String, String> {
        Err("desteklenmiyor: yalnizca Windows".into())
    }
    pub fn disable_smb1() -> Result<String, String> {
        Err("desteklenmiyor: yalnizca Windows".into())
    }
}

pub use imp::{disable_smb1, enable_firewall};
