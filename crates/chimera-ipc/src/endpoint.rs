//! Yerel soket adının GERÇEK dünyada karşılaştığı bir sorunu çözer:
//! dosya-yolu tabanlı Unix domain soketleri `sun_path` (tipik olarak 108
//! bayt) sınırını aşabilir — uzun bir kurulum dizini altında bu GERÇEKTEN
//! olur (bu oturumda birebir yaşandı). Bunun yerine ad-alanlı (namespaced)
//! bir soket adı kullanılır: adın kendisi `--root` yolunun BLAKE3 özetinden
//! türetilir, dosya sisteminden bağımsızdır ve Windows adlandırılmış
//! borularıyla (named pipe) aynı soyutlamayı paylaşır — `interprocess`
//! kütüphanesi platform farkını zaten gizler.

use interprocess::local_socket::{GenericNamespaced, Name, ToNsName};
use std::io;
use std::path::Path;

pub fn socket_name(root: &Path) -> io::Result<Name<'static>> {
    let digest = blake3::hash(root.to_string_lossy().as_bytes());
    let short = &digest.to_hex()[..24];
    let owned = format!("chimera-core-{short}");
    owned.to_ns_name::<GenericNamespaced>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_root_produces_same_name_different_roots_differ() {
        let a1 = socket_name(Path::new("/opt/chimera-core")).unwrap();
        let a2 = socket_name(Path::new("/opt/chimera-core")).unwrap();
        let b = socket_name(Path::new("/opt/chimera-core-2")).unwrap();
        assert_eq!(format!("{a1:?}"), format!("{a2:?}"));
        assert_ne!(format!("{a1:?}"), format!("{b:?}"));
    }

    #[test]
    fn very_long_root_path_still_produces_a_valid_name() {
        // Bu oturumda GERCEKTEN karsilasilan sorun: uzun bir dizin yolu
        // dosya-tabanli bir Unix soketinde sun_path sinirini asiyordu.
        // Ad-alanli isim, yol UZUNLUGUNDAN bagimsizdir.
        let long = "/tmp/".to_string() + &"x".repeat(300) + "/root";
        assert!(socket_name(Path::new(&long)).is_ok());
    }
}
