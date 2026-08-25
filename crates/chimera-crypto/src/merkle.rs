//! Gercek, dosya uzerinde calisan BLAKE3 Merkle butunluk katmani.
//!
//! `boot.rs`'teki preflight/onarim akisinin arkasindaki GERCEK mekanizma
//! burada. Sahte/simule edilmis bir "dogrulandi" bayragi degil: gercek
//! bayt dizileri parcalanir, gercek BLAKE3 hash'lenir, gercek bir kok
//! karsilastirilir ve bozulma GERCEKTEN hangi 1 MiB'lik (veya testte
//! kucuk) parcada oldugu tespit edilip yalnizca o bayt araligi
//! GERCEKTEN geri yazilir.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const DEFAULT_LEAF_SIZE: usize = 1024 * 1024; // 1 MiB

#[derive(Debug, Clone)]
pub struct MerkleTree {
    pub leaf_size: usize,
    pub leaf_hashes: Vec<[u8; 32]>,
    pub root: [u8; 32],
}

/// Bellekteki bir baytdizisi uzerinden agaci kurar. `leaf_size`'a bolunemeyen
/// son parca oldugu gibi (kisa) hash'lenir.
pub fn build_tree(data: &[u8], leaf_size: usize) -> MerkleTree {
    let leaf_hashes: Vec<[u8; 32]> = data
        .chunks(leaf_size.max(1))
        .map(|chunk| *blake3::hash(chunk).as_bytes())
        .collect();
    let root = merkle_root(&leaf_hashes);
    MerkleTree { leaf_size: leaf_size.max(1), leaf_hashes, root }
}

/// Bir dosyayi tam belleğe okumadan (buffered, sabit boyutlu okuma tamponuyla)
/// agaç kurar — gercek dunyada 20 GB'lik bir .mono dosyasi icin RAM patlamaz.
pub fn build_tree_from_file(path: &Path, leaf_size: usize) -> io::Result<MerkleTree> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; leaf_size.max(1)];
    let mut leaf_hashes = Vec::new();
    loop {
        let n = read_up_to(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        leaf_hashes.push(*blake3::hash(&buf[..n]).as_bytes());
        if n < buf.len() {
            break;
        }
    }
    let root = merkle_root(&leaf_hashes);
    Ok(MerkleTree { leaf_size: leaf_size.max(1), leaf_hashes, root })
}

/// `read_exact` dosyanin sonunda kismi okumada hata verir; burada onun
/// yerine "ne kadar okunabildiyse" donen bir yardimci kullaniyoruz.
fn read_up_to(f: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match f.read(&mut buf[total..])? {
            0 => break,
            n => total += n,
        }
    }
    Ok(total)
}

/// Yaprak hash'lerinden ikili (binary) Merkle agaci ile kok hesaplar.
/// Tek sayida dugum kalan seviyede son dugum kendisiyle eslenir
/// (Bitcoin-tarzi duplicate-last kurali — basit ve iyi bilinen bir secim).
fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return *blake3::hash(b"").as_bytes();
    }
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&pair[0]);
            hasher.update(pair.get(1).unwrap_or(&pair[0]));
            next.push(*hasher.finalize().as_bytes());
        }
        level = next;
    }
    level[0]
}

/// Iki agacin yaprak hash'lerini karsilastirir; FARKLI olan yaprak
/// indekslerini dondurur. Bu, "preflight" adiminin gercek tespit motorudur.
pub fn diff_leaves(golden: &MerkleTree, candidate: &MerkleTree) -> Vec<usize> {
    golden
        .leaf_hashes
        .iter()
        .zip(candidate.leaf_hashes.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .chain(
            // Boy uyusmazligi da bir bozulmadir: fazla/eksik yapraklar da isaretlenir.
            candidate.leaf_hashes.len().min(golden.leaf_hashes.len())
                ..golden.leaf_hashes.len().max(candidate.leaf_hashes.len()),
        )
        .collect()
}

/// Bozuk dosyadaki YALNIZCA belirtilen yaprak araliklarini, altin (golden)
/// dosyadan gercek `pread`/`pwrite` ile geri yazar. Tam dosya kopyalanmaz.
pub fn repair_leaves_from_golden(
    golden_path: &Path,
    target_path: &Path,
    leaf_size: usize,
    bad_leaves: &[usize],
) -> io::Result<usize> {
    if bad_leaves.is_empty() {
        return Ok(0);
    }
    let mut golden = File::open(golden_path)?;
    let mut target = OpenOptions::new().read(true).write(true).open(target_path)?;
    let leaf_size = leaf_size.max(1);
    let mut buf = vec![0u8; leaf_size];
    let mut repaired = 0usize;

    for &idx in bad_leaves {
        let offset = (idx * leaf_size) as u64;
        golden.seek(SeekFrom::Start(offset))?;
        let n = read_up_to(&mut golden, &mut buf)?;
        if n == 0 {
            continue; // golden'da bu yaprak yok (dosya kisaldi) — atla
        }
        target.seek(SeekFrom::Start(offset))?;
        target.write_all(&buf[..n])?;
        repaired += 1;
    }
    target.flush()?;
    target.sync_data()?;
    Ok(repaired)
}

/// ML-DSA-87 ile agac kokunu imzalar/dogrular — `MANIFEST.sig` konseptinin
/// gercek karsiligi. `crate::obsidian` uzerinden gercek imzalama kullanilir.
pub fn sign_root(sk: &ml_dsa::SigningKey<ml_dsa::MlDsa87>, root: &[u8; 32]) -> ml_dsa::Signature<ml_dsa::MlDsa87> {
    crate::obsidian::dsa_sign(sk, root)
}

pub fn verify_root(
    vk: &ml_dsa::VerifyingKey<ml_dsa::MlDsa87>,
    root: &[u8; 32],
    sig: &ml_dsa::Signature<ml_dsa::MlDsa87>,
) -> bool {
    crate::obsidian::dsa_verify(vk, root, sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(name: &str, data: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("chimera-merkle-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = File::create(&path).unwrap();
        f.write_all(data).unwrap();
        path
    }

    #[test]
    fn identical_buffers_produce_identical_roots() {
        let data = vec![0x42u8; 10_000];
        let a = build_tree(&data, 1024);
        let b = build_tree(&data, 1024);
        assert_eq!(a.root, b.root);
        assert!(diff_leaves(&a, &b).is_empty());
    }

    #[test]
    fn single_byte_flip_is_detected_in_exactly_one_leaf() {
        let mut data = vec![0u8; 10_000];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        let golden = build_tree(&data, 1024);

        let mut corrupted = data.clone();
        corrupted[5123] ^= 0xFF; // leaf index 5

        let candidate = build_tree(&corrupted, 1024);
        let diffs = diff_leaves(&golden, &candidate);
        assert_eq!(diffs, vec![5], "yalnizca bir yaprak farkli olmali");
        assert_ne!(golden.root, candidate.root);
    }

    #[test]
    fn corrupted_file_is_repaired_leaf_by_leaf_from_golden() {
        let mut data = vec![0u8; 8192];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i * 7 % 256) as u8;
        }
        let golden_path = write_temp("golden.bin", &data);

        let mut corrupted = data.clone();
        corrupted[100] ^= 0xAA; // leaf 0 (leaf_size=1024)
        corrupted[3000] ^= 0x55; // leaf 2
        let target_path = write_temp("active.bin", &corrupted);

        let golden_tree = build_tree_from_file(&golden_path, 1024).unwrap();
        let candidate_tree = build_tree_from_file(&target_path, 1024).unwrap();
        let bad = diff_leaves(&golden_tree, &candidate_tree);
        assert_eq!(bad, vec![0, 2]);

        let repaired = repair_leaves_from_golden(&golden_path, &target_path, 1024, &bad).unwrap();
        assert_eq!(repaired, 2);

        let healed = std::fs::read(&target_path).unwrap();
        assert_eq!(healed, data, "onarim sonrasi dosya golden ile birebir ayni olmali");

        let healed_tree = build_tree_from_file(&target_path, 1024).unwrap();
        assert_eq!(healed_tree.root, golden_tree.root);
    }

    #[test]
    fn signed_root_verifies_and_tamper_is_rejected() {
        let kp = crate::obsidian::dsa_generate_keypair();
        let data = vec![9u8; 4096];
        let tree = build_tree(&data, 512);

        let sig = sign_root(&kp.signing_key, &tree.root);
        assert!(verify_root(&kp.verifying_key, &tree.root, &sig));

        let mut tampered_root = tree.root;
        tampered_root[0] ^= 1;
        assert!(!verify_root(&kp.verifying_key, &tampered_root, &sig));
    }

    #[test]
    fn empty_data_has_deterministic_root() {
        let a = build_tree(&[], 1024);
        let b = build_tree(&[], 1024);
        assert_eq!(a.root, b.root);
    }
}
