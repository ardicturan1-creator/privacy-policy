//! mitigation.rs — Sürecin KENDİSİNİ sertleştirmesi
//! (`SetProcessMitigationPolicy`).
//!
//! Buraya kadar CHIMERA hep BAŞKA süreçlere baktı. Bu modül soruyu tersine
//! çevirir: *CHIMERA'nın kendi süreci ele geçirilmeye karşı ne kadar
//! dayanıklı?* Windows, çalışan bir sürecin kendi üzerine uygulayabileceği,
//! **geri alınamayan** (bilinçli olarak: bir saldırgan kapatamasın diye)
//! bir dizi azaltma politikası sunar. Hepsi Ring-3'tür, hiçbiri sürücü
//! gerektirmez.
//!
//! ## Uygulanan politikalar ve NEDEN
//!
//! | Politika | Neyi engeller | Risk |
//! |---|---|---|
//! | `ProcessExtensionPointDisablePolicy` | Eski "uzantı noktası" DLL enjeksiyonu (AppInit_DLLs, Winsock LSP, global hook'lar) — bir saldırganın CHIMERA'nın adres alanına kod sokmasının en klasik yolu | **Düşük** |
//! | `ProcessStrictHandleCheckPolicy` | Geçersiz tanıtıcı kullanımını sessiz hatadan istisnaya çevirir; tanıtıcı karıştırma (handle confusion) saldırılarını gürültülü hâle getirir | **Düşük** |
//! | `ProcessDynamicCodePolicy` | Sürecin çalışma zamanında YENİ çalıştırılabilir bellek ayırmasını/var olanı çalıştırılabilir yapmasını engeller — kabuk kodu enjeksiyonunun temel adımı | **Orta** (aşağıya bakınız) |
//! | `ProcessSignaturePolicy` | Yalnızca Microsoft imzalı DLL'lerin yüklenmesine izin verir | **Yüksek** — VARSAYILAN KAPALI |
//!
//! ## Dürüst riskler
//!
//! - **Bu politikalar GERİ ALINAMAZ.** Windows bunu bilerek böyle
//!   tasarlamıştır (kapatılabilseydi saldırgan kapatırdı). Yani yanlış bir
//!   politika seçimi, süreci yeniden başlatmadan düzeltilemez.
//! - **`ProcessDynamicCodePolicy`**, adres alanına DLL enjekte eden meşru
//!   yazılımları (bazı EDR/APM ajanları, erişilebilirlik araçları) da
//!   kırabilir. Rust JIT kullanmadığı için CHIMERA'nın KENDİ kodu
//!   etkilenmez.
//! - **`ProcessSignaturePolicy` varsayılan olarak KAPALIDIR** çünkü
//!   Microsoft imzalı olmayan meşru bir DLL'in (kurumsal bir ajan, bir
//!   yerelleştirme katmanı) yüklenmesi gerekiyorsa süreç başlatılamaz
//!   hâle gelir. Açmak için `CHIMERA_MITIGATION_SIGNED_ONLY=1`.
//! - **Hiçbiri `taskkill /F`'e karşı koruma DEĞİLDİR.** Projenin kurucu
//!   kuralı değişmedi: bilinçli OS/yönetici sonlandırmasına direnç YOK ve
//!   eklenmeyecek. Bu politikalar süreci ÖLDÜRÜLEMEZ yapmaz; adres alanına
//!   KOD SOKULMASINI zorlaştırır. Bunlar farklı şeylerdir.

// Bu ogeler yalnizca `#[cfg(windows)]` imp tarafindan VE testler
// tarafindan kullanilir -- diger modullerdeki (firewall/bruteforce/
// cmdguard/etw) AYNI platforma-bagli cagri grafigi durumu.
#![cfg_attr(not(windows), allow(dead_code))]

/// Uygulanması istenen tek bir politika ve sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyResult {
    pub name: &'static str,
    pub applied: bool,
    pub detail: String,
}

/// Tüm sonuçların insan-okunabilir özeti.
pub fn summarize(results: &[PolicyResult]) -> String {
    if results.is_empty() {
        return "SUREC SERTLESTIRME: bu platformda desteklenmiyor.".to_string();
    }
    let ok = results.iter().filter(|r| r.applied).count();
    let mut s = format!("SUREC SERTLESTIRME: {}/{} politika uygulandi\n", ok, results.len());
    for r in results {
        s.push_str(&format!("  [{}] {} -- {}\n", if r.applied { "OK" } else { "HAYIR" }, r.name, r.detail));
    }
    s
}

/// İmzasız DLL yüklemeyi yasaklayan politikanın açık olup olmadığı.
/// Ayrı bir fonksiyondur ki karar mantığı platformdan bağımsız test
/// edilebilsin.
pub fn signed_only_requested() -> bool {
    std::env::var("CHIMERA_MITIGATION_SIGNED_ONLY").as_deref() == Ok("1")
}

/// Süreci sertleştirir. Her politika BAĞIMSIZ uygulanır: biri
/// başarısız olursa (eski bir Windows sürümü, politika desteklenmiyor)
/// diğerleri yine de denenir ve sonuç dürüstçe raporlanır.
pub fn harden_self() -> Vec<PolicyResult> {
    imp::harden_self()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::System::Threading::{
        SetProcessMitigationPolicy, ProcessDynamicCodePolicy, ProcessExtensionPointDisablePolicy,
        ProcessSignaturePolicy, ProcessStrictHandleCheckPolicy, PROCESS_MITIGATION_POLICY,
    };

    /// Politikanın bayrak alanını doğrudan `Flags: u32` birleşim üyesi
    /// üzerinden yazarız. Bunun sebebi, `windows` crate'inin bit-alanı
    /// erişimcilerinin (`_bitfield`) sürümler arasında değişebilmesidir;
    /// `Flags` ise Windows'un belgelediği, KARARLI ikili düzendir.
    /// Tüm bu politikalarda ilgili bayrak **bit 0**'tır.
    const FLAG_BIT0: u32 = 0x0000_0001;

    fn apply(policy: PROCESS_MITIGATION_POLICY, flags: u32, name: &'static str) -> PolicyResult {
        // Her politika yapisi ayni ikili duzene sahiptir: tek bir u32
        // bayrak alani (birlesimin `Flags` uyesi).
        let value: u32 = flags;
        let res = unsafe {
            SetProcessMitigationPolicy(
                policy,
                &value as *const u32 as *const core::ffi::c_void,
                core::mem::size_of::<u32>(),
            )
        };
        match res {
            Ok(()) => PolicyResult { name, applied: true, detail: "uygulandi (GERI ALINAMAZ)".into() },
            Err(e) => PolicyResult { name, applied: false, detail: format!("uygulanamadi: {e}") },
        }
    }

    pub fn harden_self() -> Vec<PolicyResult> {
        let mut out = vec![
            apply(ProcessExtensionPointDisablePolicy, FLAG_BIT0, "ExtensionPointDisable (eski DLL enjeksiyonu)"),
            apply(ProcessStrictHandleCheckPolicy, FLAG_BIT0, "StrictHandleCheck (tanitici karistirma)"),
            apply(ProcessDynamicCodePolicy, FLAG_BIT0, "DynamicCode (kabuk kodu enjeksiyonu)"),
        ];
        if signed_only_requested() {
            out.push(apply(ProcessSignaturePolicy, FLAG_BIT0, "SignatureMicrosoftSignedOnly (imzasiz DLL)"));
        } else {
            out.push(PolicyResult {
                name: "SignatureMicrosoftSignedOnly (imzasiz DLL)",
                applied: false,
                detail: "ISTENMEDI: varsayilan KAPALI (acmak icin CHIMERA_MITIGATION_SIGNED_ONLY=1) -- imzasiz mesru bir DLL gerekiyorsa surec baslatilamaz hale gelebilir".into(),
            });
        }
        out
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    /// Linux'ta `SetProcessMitigationPolicy` YOKTUR ve eşdeğeri de yoktur.
    /// Sahte bir "sertleştirildi" raporu üretmek yerine BOŞ liste dönülür;
    /// `summarize` bunu açıkça "desteklenmiyor" diye yazar.
    pub fn harden_self() -> Vec<PolicyResult> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_reports_unsupported_platforms_honestly() {
        let text = summarize(&[]);
        assert!(text.contains("desteklenmiyor"), "bos sonuc 'basarili' gibi GORUNMEMELI: {text}");
    }

    #[test]
    fn summarize_counts_applied_and_failed_separately() {
        let results = vec![
            PolicyResult { name: "A", applied: true, detail: "uygulandi".into() },
            PolicyResult { name: "B", applied: false, detail: "uygulanamadi: hata".into() },
            PolicyResult { name: "C", applied: true, detail: "uygulandi".into() },
        ];
        let text = summarize(&results);
        assert!(text.contains("2/3"), "uygulanan sayisi yanlis: {text}");
        assert!(text.contains("[OK] A"));
        assert!(text.contains("[HAYIR] B"));
    }

    /// İmzasız-DLL yasağı VARSAYILAN OLARAK KAPALI olmalı — açık olsaydı,
    /// imzasız meşru bir DLL kullanan bir ortamda süreç başlatılamazdı.
    #[test]
    fn the_signed_only_policy_is_opt_in_not_default() {
        // Ortam degiskeni testler arasinda paylasildigi icin ONCE mevcut
        // degeri saklayip SONRA geri koyuyoruz.
        let prev = std::env::var("CHIMERA_MITIGATION_SIGNED_ONLY").ok();

        std::env::remove_var("CHIMERA_MITIGATION_SIGNED_ONLY");
        assert!(!signed_only_requested(), "varsayilan KAPALI olmali");

        std::env::set_var("CHIMERA_MITIGATION_SIGNED_ONLY", "0");
        assert!(!signed_only_requested(), "'0' acik SAYILMAMALI");

        std::env::set_var("CHIMERA_MITIGATION_SIGNED_ONLY", "1");
        assert!(signed_only_requested(), "'1' acik saymali");

        match prev {
            Some(v) => std::env::set_var("CHIMERA_MITIGATION_SIGNED_ONLY", v),
            None => std::env::remove_var("CHIMERA_MITIGATION_SIGNED_ONLY"),
        }
    }

    /// `harden_self` hiçbir platformda panic'lememeli. Windows'ta gerçek
    /// bir sistem çağrısı yapar; Linux'ta boş liste döner.
    #[test]
    fn hardening_never_panics_and_reports_something_coherent() {
        let results = harden_self();
        let text = summarize(&results);
        assert!(!text.is_empty());
        #[cfg(not(windows))]
        assert!(results.is_empty(), "Linux'ta sahte bir politika sonucu URETILMEMELI");
        #[cfg(windows)]
        {
            assert_eq!(results.len(), 4, "dort politikanin dordu de RAPORLANMALI");
            // Her sonucun bos olmayan bir aciklamasi olmali -- "uygulanamadi"
            // bile olsa SEBEBI yazmali.
            assert!(results.iter().all(|r| !r.detail.is_empty()));
        }
    }
}
