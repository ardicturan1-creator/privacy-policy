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
}

#[derive(Debug, Clone)]
pub enum Response {
    Pong,
    StatusOk(String),
    LogsOk(String),
    DecoyAlertsOk(String),
    Denied,
    HeartbeatAck,
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
    }
}
