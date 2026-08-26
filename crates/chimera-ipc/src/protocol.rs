//! Core <-> Admin/Sentinel arasındaki uygulama seviyesi mesaj protokolü.
//! `SecureChannel::send`/`recv` zaten kimlik doğrulamalı+şifreli bir bayt
//! kanalı sağladığı için burada yalnızca o kanalın taşıdığı mesajların
//! kodlanması var — ayrı bir güvenlik katmanı değil, bir kablo formatı.
//!
//! Sıfır Güven kuralı burada somutlaşır: `unlock` alanı taşıyan istekler
//! (`GetLogs`, `SetDegraded`, `ListDecoyAlerts`) yalnızca admin'in Shamir(2,3)
//! paylarından GERÇEKTEN yeniden kurduğu 32 baytlık master anahtarla eşleşen
//! bir değer taşıyorsa Core tarafından kabul edilir — bu alan olmadan ya da
//! yanlışsa Core `Denied` döner ve hiçbir hassas veri sızmaz.

use std::io;

#[derive(Debug, Clone)]
pub enum Request {
    Ping,
    Status,
    GetLogs { unlock: [u8; 32] },
    SetDegraded { on: bool, unlock: [u8; 32] },
    ListDecoyAlerts { unlock: [u8; 32] },
    Heartbeat { source: String },
    /// Kanita-dayanikli hash-zincirli denetim kaydinin butunlugunu dogrular
    /// (bkz. `chimera-core::auditlog`). Ayricalikli: bir saldirganin "zincir
    /// bozuldu mu" bilgisini bile digital olarak sorgulayamamasi icin diger
    /// ayricalikli komutlarla AYNI Shamir(2,3) kapisina tabidir.
    VerifyAuditLog { unlock: [u8; 32] },
    /// Detector/Validator/Executor boru hattini SENKRON olarak bir kez
    /// calistirir (bkz. `chimera-core::pipeline`) ve bulgu+aksiyon raporunu
    /// dondurur. Ayricalikli: sistem konfigurasyonu (acik portlar, SMBv1,
    /// RDP ayarlari) hassas bilgi sayilir.
    ScanNow { unlock: [u8; 32] },
    /// Verilen IPv4/IPv6 adresini Windows Firewall uzerinden GERCEKTEN
    /// bloklar (bkz. `chimera-core::firewall`). Ayricalikli.
    BlockIp { unlock: [u8; 32], ip: String },
    /// Daha once CHIMERA tarafindan eklenmis bir engelleme kuralini kaldirir.
    /// Ayricalikli.
    UnblockIp { unlock: [u8; 32], ip: String },
    /// CHIMERA tarafindan eklenmis, halen aktif olan engelleme kurallarini
    /// listeler. Ayricalikli.
    ListBlockedIps { unlock: [u8; 32] },
    /// Devre kesicinin (bkz. `chimera-core::circuit_breaker`) askiya alip
    /// INSAN ONAYI kuyruguna koydugu surecleri listeler. Ayricalikli:
    /// hangi surecin yakalandigi, saldirgana kendi araclarinin tespit
    /// edilip edilmedigini soylerdi.
    ListSuspended { unlock: [u8; 32] },
    /// Askiya alinmis bir sureci INSAN ONAYIYLA devam ettirir ve devre
    /// kesicinin uyguladigi IP bloklarini geri alir (yanlis pozitif
    /// karari). GERI ALINABILIR.
    ResumeProcess { unlock: [u8; 32], pid: u32 },
    /// Askiya alinmis bir sureci INSAN ONAYIYLA KALICI olarak sonlandirir.
    /// **Geri alinamaz** — bu yuzden `pipeline.rs` whitelist'ine ASLA
    /// girmez ve yalnizca bu ayricalikli istek uzerinden calisir.
    TerminateProcess { unlock: [u8; 32], pid: u32 },
}

#[derive(Debug, Clone)]
pub enum Response {
    Pong,
    StatusOk(String),
    LogsOk(String),
    DecoyAlertsOk(String),
    Denied,
    HeartbeatAck,
    AuditVerifyOk(String),
    ScanReportOk(String),
    BlockIpOk(String),
    /// Devre kesici komutlarinin (list/resume/terminate) ortak yaniti.
    CircuitBreakerOk(String),
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn get_str(buf: &[u8], off: &mut usize) -> io::Result<String> {
    let len = get_u32(buf, off)? as usize;
    let s = buf.get(*off..*off + len).ok_or_else(bad)?;
    *off += len;
    String::from_utf8(s.to_vec()).map_err(|_| bad())
}

fn get_u32(buf: &[u8], off: &mut usize) -> io::Result<u32> {
    let b = buf.get(*off..*off + 4).ok_or_else(bad)?;
    *off += 4;
    Ok(u32::from_le_bytes(b.try_into().unwrap()))
}

fn get_32(buf: &[u8], off: &mut usize) -> io::Result<[u8; 32]> {
    let b = buf.get(*off..*off + 32).ok_or_else(bad)?;
    *off += 32;
    Ok(b.try_into().unwrap())
}

fn bad() -> io::Error {
    io::Error::other("gecersiz protokol cercevesi")
}

impl Request {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Request::Ping => out.push(0x01),
            Request::Status => out.push(0x02),
            Request::GetLogs { unlock } => { out.push(0x03); out.extend_from_slice(unlock); }
            Request::SetDegraded { on, unlock } => {
                out.push(0x04);
                out.push(if *on { 1 } else { 0 });
                out.extend_from_slice(unlock);
            }
            Request::ListDecoyAlerts { unlock } => { out.push(0x05); out.extend_from_slice(unlock); }
            Request::Heartbeat { source } => { out.push(0x06); put_str(&mut out, source); }
            Request::VerifyAuditLog { unlock } => { out.push(0x07); out.extend_from_slice(unlock); }
            Request::ScanNow { unlock } => { out.push(0x08); out.extend_from_slice(unlock); }
            Request::BlockIp { unlock, ip } => { out.push(0x09); out.extend_from_slice(unlock); put_str(&mut out, ip); }
            Request::UnblockIp { unlock, ip } => { out.push(0x0A); out.extend_from_slice(unlock); put_str(&mut out, ip); }
            Request::ListBlockedIps { unlock } => { out.push(0x0B); out.extend_from_slice(unlock); }
            Request::ListSuspended { unlock } => { out.push(0x0C); out.extend_from_slice(unlock); }
            Request::ResumeProcess { unlock, pid } => { out.push(0x0D); out.extend_from_slice(unlock); out.extend_from_slice(&pid.to_le_bytes()); }
            Request::TerminateProcess { unlock, pid } => { out.push(0x0E); out.extend_from_slice(unlock); out.extend_from_slice(&pid.to_le_bytes()); }
        }
        out
    }

    pub fn decode(buf: &[u8]) -> io::Result<Self> {
        let tag = *buf.first().ok_or_else(bad)?;
        let mut off = 1usize;
        Ok(match tag {
            0x01 => Request::Ping,
            0x02 => Request::Status,
            0x03 => Request::GetLogs { unlock: get_32(buf, &mut off)? },
            0x04 => {
                let on = *buf.get(off).ok_or_else(bad)? != 0;
                off += 1;
                Request::SetDegraded { on, unlock: get_32(buf, &mut off)? }
            }
            0x05 => Request::ListDecoyAlerts { unlock: get_32(buf, &mut off)? },
            0x06 => Request::Heartbeat { source: get_str(buf, &mut off)? },
            0x07 => Request::VerifyAuditLog { unlock: get_32(buf, &mut off)? },
            0x08 => Request::ScanNow { unlock: get_32(buf, &mut off)? },
            0x09 => {
                let unlock = get_32(buf, &mut off)?;
                Request::BlockIp { unlock, ip: get_str(buf, &mut off)? }
            }
            0x0A => {
                let unlock = get_32(buf, &mut off)?;
                Request::UnblockIp { unlock, ip: get_str(buf, &mut off)? }
            }
            0x0B => Request::ListBlockedIps { unlock: get_32(buf, &mut off)? },
            0x0C => Request::ListSuspended { unlock: get_32(buf, &mut off)? },
            0x0D => {
                let unlock = get_32(buf, &mut off)?;
                Request::ResumeProcess { unlock, pid: get_u32(buf, &mut off)? }
            }
            0x0E => {
                let unlock = get_32(buf, &mut off)?;
                Request::TerminateProcess { unlock, pid: get_u32(buf, &mut off)? }
            }
            _ => return Err(bad()),
        })
    }
}

impl Response {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Response::Pong => out.push(0x81),
            Response::StatusOk(s) => { out.push(0x82); put_str(&mut out, s); }
            Response::LogsOk(s) => { out.push(0x83); put_str(&mut out, s); }
            Response::DecoyAlertsOk(s) => { out.push(0x85); put_str(&mut out, s); }
            Response::Denied => out.push(0x84),
            Response::HeartbeatAck => out.push(0x86),
            Response::AuditVerifyOk(s) => { out.push(0x87); put_str(&mut out, s); }
            Response::ScanReportOk(s) => { out.push(0x88); put_str(&mut out, s); }
            Response::BlockIpOk(s) => { out.push(0x89); put_str(&mut out, s); }
            Response::CircuitBreakerOk(s) => { out.push(0x8A); put_str(&mut out, s); }
        }
        out
    }

    pub fn decode(buf: &[u8]) -> io::Result<Self> {
        let tag = *buf.first().ok_or_else(bad)?;
        let mut off = 1usize;
        Ok(match tag {
            0x81 => Response::Pong,
            0x82 => Response::StatusOk(get_str(buf, &mut off)?),
            0x83 => Response::LogsOk(get_str(buf, &mut off)?),
            0x85 => Response::DecoyAlertsOk(get_str(buf, &mut off)?),
            0x84 => Response::Denied,
            0x86 => Response::HeartbeatAck,
            0x87 => Response::AuditVerifyOk(get_str(buf, &mut off)?),
            0x88 => Response::ScanReportOk(get_str(buf, &mut off)?),
            0x89 => Response::BlockIpOk(get_str(buf, &mut off)?),
            0x8A => Response::CircuitBreakerOk(get_str(buf, &mut off)?),
            _ => return Err(bad()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips() {
        let cases = vec![
            Request::Ping,
            Request::Status,
            Request::GetLogs { unlock: [1u8; 32] },
            Request::SetDegraded { on: true, unlock: [2u8; 32] },
            Request::ListDecoyAlerts { unlock: [3u8; 32] },
            Request::Heartbeat { source: "sentinel".into() },
            Request::VerifyAuditLog { unlock: [4u8; 32] },
            Request::ScanNow { unlock: [5u8; 32] },
            Request::BlockIp { unlock: [6u8; 32], ip: "203.0.113.7".into() },
            Request::UnblockIp { unlock: [7u8; 32], ip: "203.0.113.7".into() },
            Request::ListBlockedIps { unlock: [8u8; 32] },
            Request::ListSuspended { unlock: [9u8; 32] },
            Request::ResumeProcess { unlock: [10u8; 32], pid: 4242 },
            Request::TerminateProcess { unlock: [11u8; 32], pid: 0xDEAD_BEEF },
        ];
        for req in cases {
            let encoded = req.encode();
            let decoded = Request::decode(&encoded).unwrap();
            assert_eq!(format!("{req:?}"), format!("{decoded:?}"));
        }
    }

    #[test]
    fn response_round_trips() {
        let cases = vec![
            Response::Pong,
            Response::StatusOk("mode=Full".into()),
            Response::LogsOk("...".into()),
            Response::DecoyAlertsOk("[]".into()),
            Response::Denied,
            Response::HeartbeatAck,
            Response::AuditVerifyOk("SAGLAM: 3 kayitlik zincir bastan sona tutarli".into()),
            Response::ScanReportOk("[]".into()),
            Response::BlockIpOk("bloklandi: 203.0.113.7".into()),
            Response::CircuitBreakerOk("INSAN ONAYI BEKLEYEN 1 SUREC".into()),
        ];
        for resp in cases {
            let encoded = resp.encode();
            let decoded = Response::decode(&encoded).unwrap();
            assert_eq!(format!("{resp:?}"), format!("{decoded:?}"));
        }
    }

    #[test]
    fn truncated_frame_is_rejected_not_panicking() {
        assert!(Request::decode(&[0x03, 1, 2, 3]).is_err());
        assert!(Request::decode(&[]).is_err());
        assert!(Response::decode(&[0x82, 0, 0]).is_err());
        // unlock TAM ama pid alani eksik: panic degil, temiz bir hata.
        let mut short = vec![0x0Eu8];
        short.extend_from_slice(&[7u8; 32]);
        short.extend_from_slice(&[0, 0]); // pid icin 4 yerine 2 bayt
        assert!(Request::decode(&short).is_err(), "eksik pid alani REDDEDILMELI");
    }

    /// PID'in kablo uzerinde BOZULMADAN tasindigini dogrular — devre
    /// kesici komutlarinda yanlis bir PID, YANLIS bir surecin
    /// sonlandirilmasi demektir.
    #[test]
    fn process_ids_survive_the_wire_exactly() {
        for pid in [0u32, 1, 4242, u32::MAX] {
            let req = Request::TerminateProcess { unlock: [1u8; 32], pid };
            match Request::decode(&req.encode()).unwrap() {
                Request::TerminateProcess { pid: back, .. } => assert_eq!(back, pid),
                other => panic!("yanlis varyant cozuldu: {other:?}"),
            }
        }
    }
}
