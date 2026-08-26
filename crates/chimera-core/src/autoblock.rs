//! autoblock.rs — GEÇİCİ, kendiliğinden süresi dolan IP engelleme
//! ("fail2ban" deseni).
//!
//! Bu modül, projenin "whitelist dışı hiçbir KALICI otomatik aksiyon yok"
//! kuralının **sınırında** durur ve tam olarak neden orada durabildiğini
//! açıklamak zorundadır:
//!
//!   - Uygulanan şey **kalıcı değildir**: her engelin bir TTL'i (yaşam
//!     süresi) vardır ve süresi dolduğunda `expire_due()` tarafından
//!     OTOMATİK olarak kaldırılır. Operatör hiçbir şey yapmasa bile
//!     engel kendiliğinden düşer.
//!   - **Geri alınabilir**: `firewall::unblock_ip` ile aynı kural
//!     silinir; hiçbir veri kaybı veya yapı değişikliği olmaz.
//!   - **Dar**: yalnızca tek bir uzak adres etkilenir; yerel servisler,
//!     kullanıcı oturumları, dosyalar etkilenmez.
//!
//! Bu üç özellik, `remediate.rs`'in başındaki dört şartla aynı ailedendir;
//! bu yüzden `pipeline.rs` whitelist'ine GİREBİLİR. Buna karşılık KALICI
//! engelleme (`chimera-admin block-ip`) hâlâ Shamir(2,3) ister ve bu
//! modül tarafından ASLA otomatik uygulanmaz.
//!
//! ## Kendini kilitleme koruması (`never_block`)
//!
//! Otomatik bir engelleme mekanizmasının en gerçek tehlikesi, makineyi
//! yönetenin kendisini dışarıda bırakmasıdır. Bu yüzden hiçbir koşulda
//! engellenmeyecek adresler VARDIR ve bu kontrol, engelleme yolundaki
//! İLK adımdır.

use std::net::IpAddr;
use std::path::{Path, PathBuf};

/// Varsayılan yaşam süresi: 1 saat. Kısa tutulmuştur — amaç saldırganı
/// yavaşlatmak, kalıcı bir kara liste kurmak değil.
pub const DEFAULT_TTL_SECS: u64 = 3600;

fn state_file(root: &Path) -> PathBuf {
    root.join("state/autoblock.list")
}

/// Operatörün elle doldurabileceği "asla engelleme" listesi. Her satır bir
/// IP adresidir. Üretimde buraya en azından yönetim ağı/atlama sunucusu
/// (jump host) yazılmalıdır.
fn allowlist_file(root: &Path) -> PathBuf {
    root.join("state/never_block.list")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoBlock {
    pub ip: String,
    pub blocked_at: u64,
    pub expires_at: u64,
    pub reason: String,
}

impl AutoBlock {
    fn to_line(&self) -> String {
        format!(
            "{{\"ip\":\"{}\",\"blocked_at\":{},\"expires_at\":{},\"reason\":\"{}\"}}",
            self.ip,
            self.blocked_at,
            self.expires_at,
            self.reason.replace('"', "'").replace('\n', " ")
        )
    }

    fn from_line(line: &str) -> Option<Self> {
        Some(AutoBlock {
            ip: str_field(line, "ip")?,
            blocked_at: num_field(line, "blocked_at")?,
            expires_at: num_field(line, "expires_at")?,
            reason: str_field(line, "reason").unwrap_or_default(),
        })
    }
}

fn str_field(line: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\":\"");
    let start = line.find(&k)? + k.len();
    let rest = &line[start..];
    Some(rest[..rest.find('"')?].to_string())
}

fn num_field(line: &str, key: &str) -> Option<u64> {
    let k = format!("\"{key}\":");
    let start = line.find(&k)? + k.len();
    let rest = &line[start..];
    let end = rest.find([',', '}'])?;
    rest[..end].trim().parse().ok()
}

/// Bir adresin **asla** otomatik engellenmemesi gerekiyorsa sebebini
/// döner. `None`, "engellenebilir" demektir.
///
/// Bu liste bilinçli olarak GENİŞTİR — otomatik bir mekanizmanın yanlış
/// bir adresi engellemesinin bedeli, bir saldırganın bir saat daha
/// deneme yapmasından çok daha ağırdır.
pub fn never_block(root: &Path, ip: &str) -> Option<String> {
    let Ok(addr) = ip.parse::<IpAddr>() else {
        return Some("gecerli bir IP adresi degil".into());
    };
    if addr.is_loopback() {
        return Some("loopback (makinenin kendisi)".into());
    }
    if addr.is_unspecified() {
        return Some("belirtilmemis adres (0.0.0.0 / ::)".into());
    }
    if addr.is_multicast() {
        return Some("multicast".into());
    }
    match addr {
        IpAddr::V4(v4) => {
            if v4.is_broadcast() {
                return Some("broadcast".into());
            }
            if v4.is_link_local() {
                // 169.254.0.0/16 -- DHCP yokken kendi kendine atanan adres
                return Some("link-local (169.254.0.0/16)".into());
            }
        }
        IpAddr::V6(v6) => {
            // fe80::/10 -- IPv6 link-local. `is_unicast_link_local` henuz
            // kararli degil, bu yuzden onek ELLE kontrol edilir.
            let seg = v6.segments()[0];
            if (seg & 0xffc0) == 0xfe80 {
                return Some("link-local (fe80::/10)".into());
            }
        }
    }
    // Operatorun elle tanimladigi liste EN SON kontrol edilir ama en
    // yuksek onceliklidir: burada olan bir adres, yukaridaki kurallar ne
    // derse desin engellenmez.
    if read_allowlist(root).iter().any(|a| a == ip) {
        return Some("operator 'never_block.list' dosyasinda tanimlamis".into());
    }
    None
}

fn read_allowlist(root: &Path) -> Vec<String> {
    std::fs::read_to_string(allowlist_file(root))
        .map(|s| {
            s.lines()
                .map(|l| l.split('#').next().unwrap_or("").trim().to_string()) // '#' sonrasi yorum
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn read_blocks(root: &Path) -> Vec<AutoBlock> {
    std::fs::read_to_string(state_file(root))
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).filter_map(AutoBlock::from_line).collect())
        .unwrap_or_default()
}

fn write_blocks(root: &Path, blocks: &[AutoBlock]) -> std::io::Result<()> {
    use fs4::FileExt;
    let path = state_file(root);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&path)?;
    FileExt::lock(&f)?;
    let body: String = blocks.iter().map(|b| b.to_line()).collect::<Vec<_>>().join("\n");
    let res = std::io::Write::write_all(&mut f, body.as_bytes());
    let _ = FileExt::unlock(&f);
    res
}

/// Bir adresi TTL'li olarak engeller. Zaten engelliyse süresi UZATILIR
/// (yeni bir kural eklenmez) — saldırı devam ettiği sürece engel de
/// devam etsin ama defter şişmesin.
pub fn block_with_ttl(
    root: &Path,
    ip: &str,
    ttl_secs: u64,
    reason: &str,
    now: u64,
    audit: &impl Fn(&str, &str),
) -> Result<String, String> {
    if let Some(why) = never_block(root, ip) {
        audit("autoblock.refused", &format!("{ip}: {why}"));
        return Err(format!("{ip} otomatik engellenmez ({why})"));
    }

    let mut blocks = read_blocks(root);
    if let Some(existing) = blocks.iter_mut().find(|b| b.ip == ip) {
        existing.expires_at = now + ttl_secs;
        existing.reason = reason.to_string();
        let _ = write_blocks(root, &blocks);
        audit("autoblock.extended", &format!("{ip} suresi uzatildi (+{ttl_secs}s): {reason}"));
        return Ok(format!("{ip} zaten engelli, suresi {ttl_secs} saniye uzatildi"));
    }

    // GERCEK engelleme: firewall.rs uzerinden, kalici `block-ip` ile AYNI
    // mekanizma. Fark, bizim ayrica bir son kullanma tarihi tutmamiz.
    let msg = crate::firewall::block_ip(root, ip)?;
    blocks.push(AutoBlock {
        ip: ip.to_string(),
        blocked_at: now,
        expires_at: now + ttl_secs,
        reason: reason.to_string(),
    });
    if let Err(e) = write_blocks(root, &blocks) {
        // Defter yazilamadiysa engel KALDIRILIR: aksi halde suresi ASLA
        // dolmayacak, "gecici" oldugu iddia edilen bir engel kalirdi --
        // bu, modulun tum vaadini bozardi.
        audit("autoblock.state_write_failed", &format!("{ip}: {e} -- engel GERI ALINIYOR"));
        let _ = crate::firewall::unblock_ip(root, ip);
        return Err(format!("{ip} engellendi ama defter yazilamadi ({e}); engel geri alindi"));
    }
    audit("autoblock.blocked", &format!("{ip} {ttl_secs} saniyeligine engellendi: {reason} ({msg})"));
    Ok(format!("{ip} GECICI olarak engellendi ({ttl_secs} saniye, otomatik kalkacak): {reason}"))
}

/// Süresi dolmuş TÜM engelleri kaldırır. `pipeline.rs`'in arka plan
/// döngüsü tarafından her turda çağrılır — TTL vaadini GERÇEK kılan
/// mekanizma budur.
pub fn expire_due(root: &Path, now: u64, audit: &impl Fn(&str, &str)) -> Vec<String> {
    let blocks = read_blocks(root);
    let (expired, live): (Vec<AutoBlock>, Vec<AutoBlock>) = blocks.into_iter().partition(|b| b.expires_at <= now);
    if expired.is_empty() {
        return Vec::new();
    }
    let mut removed = Vec::new();
    for b in &expired {
        match crate::firewall::unblock_ip(root, &b.ip) {
            Ok(msg) => {
                audit("autoblock.expired", &format!("{} suresi doldu, engel kaldirildi ({msg})", b.ip));
                removed.push(b.ip.clone());
            }
            Err(e) => {
                // Kural kaldirilamadi ama defterden DUSURUYORUZ: aksi halde
                // her turda tekrar denenip denetim kaydini doldururdu.
                // Operatör bunu denetim kaydinda GORUR.
                audit("autoblock.expire_failed", &format!("{}: {e} -- defterden dusuruldu, kural ELLE kontrol edilmeli", b.ip));
            }
        }
    }
    let _ = write_blocks(root, &live);
    removed
}

pub fn active_count(root: &Path) -> usize {
    read_blocks(root).len()
}

pub fn list_text(root: &Path, now: u64) -> String {
    let blocks = read_blocks(root);
    if blocks.is_empty() {
        return "GECICI (TTL'li) OTOMATIK ENGEL YOK.".to_string();
    }
    let mut s = format!("{} GECICI OTOMATIK ENGEL:\n", blocks.len());
    for b in &blocks {
        let kalan = b.expires_at.saturating_sub(now);
        s.push_str(&format!("  {} -- kalan {} saniye -- sebep: {}\n", b.ip, kalan, b.reason));
    }
    s.push_str("Bu engeller suresi dolunca KENDILIGINDEN kalkar. Kalici engelleme icin: block-ip (Shamir 2/3).\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("chimera-ab-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("state")).unwrap();
        p
    }

    fn noaudit() -> impl Fn(&str, &str) {
        |_: &str, _: &str| {}
    }

    /// **En kritik güvenlik testi:** otomatik mekanizma, makineyi
    /// yönetenin kendisini dışarıda bırakabilecek adresleri ASLA
    /// engellememelidir.
    #[test]
    fn dangerous_addresses_are_never_auto_blocked() {
        let root = temp_root("never");
        for ip in ["127.0.0.1", "::1", "0.0.0.0", "::", "255.255.255.255", "224.0.0.1", "169.254.1.1", "fe80::1"] {
            assert!(never_block(&root, ip).is_some(), "{ip} ASLA otomatik engellenmemeli");
        }
        // Gercek, yonlendirilebilir bir dis adres engellenebilir olmali.
        assert!(never_block(&root, "203.0.113.7").is_none());
        assert!(never_block(&root, "2001:db8::1").is_none());
        // IP olmayan bir sey de reddedilmeli.
        assert!(never_block(&root, "kotu-makine").is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_operator_allowlist_is_honoured_including_comments() {
        let root = temp_root("allow");
        std::fs::write(
            root.join("state/never_block.list"),
            "# yonetim atlama sunucusu\n203.0.113.7   # asla engelleme\n\n198.51.100.9\n",
        )
        .unwrap();
        assert!(never_block(&root, "203.0.113.7").is_some(), "operator listesi ONURLANDIRILMALI");
        assert!(never_block(&root, "198.51.100.9").is_some());
        assert!(never_block(&root, "192.0.2.5").is_none(), "listede olmayan adres engellenebilir");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn blocking_a_protected_address_is_refused_and_audited() {
        let root = temp_root("refuse");
        let events = std::sync::Mutex::new(Vec::new());
        let audit = |e: &str, d: &str| events.lock().unwrap().push(format!("{e}|{d}"));
        let err = block_with_ttl(&root, "127.0.0.1", 3600, "test", 1000, &audit).unwrap_err();
        assert!(err.contains("otomatik engellenmez"));
        assert_eq!(active_count(&root), 0);
        assert!(events.lock().unwrap().iter().any(|e| e.starts_with("autoblock.refused")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn block_records_round_trip_through_disk() {
        let root = temp_root("roundtrip");
        let b = AutoBlock {
            ip: "203.0.113.7".into(),
            blocked_at: 1000,
            expires_at: 4600,
            reason: "RDP brute-force: 40 deneme".into(),
        };
        write_blocks(&root, &[b.clone()]).unwrap();
        assert_eq!(read_blocks(&root), vec![b]);
        let text = list_text(&root, 1600);
        assert!(text.contains("203.0.113.7"));
        assert!(text.contains("3000 saniye"), "kalan sure hesabi yanlis: {text}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// TTL vaadinin GERÇEK testi: süresi dolan kayıt defterden düşmeli,
    /// dolmayan kalmalı.
    #[test]
    fn only_expired_blocks_are_removed() {
        let root = temp_root("expire");
        write_blocks(
            &root,
            &[
                AutoBlock { ip: "203.0.113.7".into(), blocked_at: 0, expires_at: 1000, reason: "eski".into() },
                AutoBlock { ip: "198.51.100.9".into(), blocked_at: 0, expires_at: 9000, reason: "taze".into() },
            ],
        )
        .unwrap();

        // Linux'ta firewall::unblock_ip "desteklenmiyor" doner; expire_due
        // yine de defteri temizlemeli (aksi halde ayni kayit sonsuza kadar
        // yeniden denenirdi).
        expire_due(&root, 5000, &noaudit());
        let left = read_blocks(&root);
        assert_eq!(left.len(), 1, "yalnizca suresi DOLMAYAN kayit kalmali");
        assert_eq!(left[0].ip, "198.51.100.9");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_expires_before_its_time() {
        let root = temp_root("notyet");
        write_blocks(
            &root,
            &[AutoBlock { ip: "203.0.113.7".into(), blocked_at: 0, expires_at: 9000, reason: "x".into() }],
        )
        .unwrap();
        assert!(expire_due(&root, 8999, &noaudit()).is_empty());
        assert_eq!(active_count(&root), 1, "suresi DOLMAMIS engel kaldirilmamali");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_empty_ledger_reports_nothing_active() {
        let root = temp_root("empty");
        assert_eq!(active_count(&root), 0);
        assert!(list_text(&root, 0).contains("YOK"));
        assert!(expire_due(&root, 99999, &noaudit()).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Aynı IP iki kez engellenmeye çalışılırsa ikinci kural EKLENMEZ,
    /// süresi uzatılır. (Linux'ta `firewall::block_ip` başarısız olduğu
    /// için ilk kaydı elle koyuyoruz — test edilen şey defter mantığıdır.)
    #[test]
    fn re_blocking_an_active_ip_extends_it_instead_of_duplicating() {
        let root = temp_root("extend");
        write_blocks(
            &root,
            &[AutoBlock { ip: "203.0.113.7".into(), blocked_at: 100, expires_at: 200, reason: "ilk".into() }],
        )
        .unwrap();
        let msg = block_with_ttl(&root, "203.0.113.7", 3600, "ikinci", 1000, &noaudit()).unwrap();
        assert!(msg.contains("uzatildi"));
        let blocks = read_blocks(&root);
        assert_eq!(blocks.len(), 1, "AYNI IP icin ikinci kayit OLUSMAMALI");
        assert_eq!(blocks[0].expires_at, 4600);
        assert_eq!(blocks[0].reason, "ikinci");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn quotes_in_a_reason_cannot_corrupt_the_ledger() {
        let root = temp_root("quotes");
        write_blocks(
            &root,
            &[AutoBlock { ip: "203.0.113.7".into(), blocked_at: 1, expires_at: 2, reason: "a\"b\nc".into() }],
        )
        .unwrap();
        let back = read_blocks(&root);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].ip, "203.0.113.7");
        assert!(!back[0].reason.contains('"'));
        let _ = std::fs::remove_dir_all(&root);
    }
}
