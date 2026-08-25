//! Siber Yanıltma (Cyber Deception) — Bölüm 1: Decoy dosyalar.
//!
//! Gerçekçi isimli, gerçek dosyalar diske yazılır. Bunlara dokunan HERHANGİ
//! bir süreç (yasal bir yedekleme aracı olabilir, ama olağan iş akışında
//! kimsenin dokunmaması gereken dosyalardır) `notify` ile GERÇEK bir
//! dosya sistemi olayı üretir ve bu, gerçek bir denetim kaydına yazılır.
//! Bu pasif bir tuzaktır: saldırganı oyalamaz, yalnızca ele verir.

use notify::{RecursiveMode, Watcher};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

pub const DECOY_NAMES: &[&str] = &[
    "calisan_maaslari_2026.xlsx",
    "yonetim_kurulu_notlari_gizli.docx",
    "musteri_kredi_karti_yedek.csv",
    "vpn_erisim_bilgileri.txt",
    "sirket_sifreleri_master.kdbx",
    "ar-ge_yol_haritasi_2027_TASLAK.pptx",
];

#[derive(Debug, Clone)]
pub struct DecoyAlert {
    pub ts: u64,
    pub path: String,
    pub kind: String,
}

pub fn create_decoys(dir: &Path) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let mut paths = Vec::new();
    for name in DECOY_NAMES {
        let p = dir.join(name);
        if !p.exists() {
            // Icerik bos degil: bos bir dosya supheli gorunur, gercekci
            // boyutlu bir dosya saldirganin ilk gozden gecirmesinde
            // "gercek" izlenimi birakir.
            let filler = format!("CHIMERA-DECOY\n{}\n", "x".repeat(2048));
            fs::write(&p, filler)?;
        }
        paths.push(p);
    }
    Ok(paths)
}

/// Arka planda calisan bir izleyici baslatir; her decoy olayi kanaldan okunabilir.
/// Donen `notify::RecommendedWatcher` DUSURULMEMELIDIR (drop edilirse izleme durur).
pub fn watch(dir: &Path) -> notify::Result<(notify::RecommendedWatcher, Receiver<DecoyAlert>)> {
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            for path in event.paths {
                let alert = DecoyAlert {
                    ts: unix_now(),
                    path: path.to_string_lossy().into_owned(),
                    kind: format!("{:?}", event.kind),
                };
                let _ = tx.send(alert);
            }
        }
    })?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn touching_a_decoy_produces_a_real_alert() {
        let dir = std::env::temp_dir().join(format!("chimera-core-decoy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let paths = create_decoys(&dir).unwrap();
        assert_eq!(paths.len(), DECOY_NAMES.len());

        let (_watcher, rx) = watch(&dir).unwrap();
        // Watcher'in kurulmasi icin kisa bir bekleme (isletim sistemi
        // dosya sistemi olay kuyrugu asenkron kurulur).
        std::thread::sleep(Duration::from_millis(200));

        // GERCEK bir "saldirgan" dokunusu: decoy dosyasi acilip yazilir.
        fs::write(&paths[0], "dokunuldu").unwrap();

        let alert = rx.recv_timeout(Duration::from_secs(5)).expect("gercek bir dosya olayi alinmali");
        assert!(alert.path.contains(DECOY_NAMES[0]));

        let _ = fs::remove_dir_all(&dir);
    }
}
