# chimera-bootstrap

MONOLITH'in gercek, derlenen, test edilen kurulum ve kriptografi cekirdegi.
Bu dosya neyin **gercekten calistigini**, neyin **bilerek kapsam disinda**
birakildigini ve nasil dogrulayabileceginizi anlatir. Mimari tasarimin
tamami icin bkz. `../../docs/chimera/00-ARCHITECTURE.md`.

## Hizli baslangic

```bash
cargo test --release          # 29 gercek test, mock/placeholder yok
cargo build --release
./target/release/chimera-bootstrap probe
./target/release/chimera-bootstrap install --root /tmp/chimera
./target/release/chimera-bootstrap verify   --root /tmp/chimera
./target/release/chimera-bootstrap obsidian-demo
```

Windows'ta kaynaktan derlemek icin `build_windows.bat` (Rust gerektirir).
Depoda ayrica onceden cross-compile edilmis bir `.exe` bulunabilir; kendi
makinenizde dogrulamak isterseniz betigi calistirin.

## Ne GERCEKTEN calisiyor (test edilmis, `cargo test` ile dogrulanan)

| Alan | Kutuphane | Ne test ediliyor |
|---|---|---|
| ML-KEM-1024 (FIPS 203) | `ml-kem` | Anahtar uretimi, kapsulleme/kapsul acma, paylasilan sirrin iki tarafta eslesmesi |
| ML-DSA-87 (FIPS 204) | `ml-dsa` | Imzalama, dogrulama, kurcalama tespiti, bayt-duzeyinde seri hale getirme |
| XChaCha20-Poly1305 | `chacha20poly1305` | Muhurleme/acma, yanlis anahtarla acilamama, AEAD kurcalama tespiti |
| Argon2id | `argon2` | Parola tabanli anahtar turetme, yanlis parolanin basarisiz olmasi |
| Shamir(2,3) | `sharks` | 3 parcanin herhangi 2'sinden kurtarma, tek parcadan kurtarilamama |
| BLAKE3 Merkle agaci | `blake3` | Tek bayt bozulmanin dogru yaprakta tespiti, GERCEK dosyalardan parcali onarim |
| Ortogonal vektor donusumu | (elle, f64) | Kosinus benzerliginin donusumden sonra da korunmasi (1e-9 hassasiyetle) |
| Donanim tespiti (CPU/RAM) | `std`/`/proc`/`/sys` | SMT tekillestirme, P/E-core ayrimi, `MemAvailable` okuma |
| Donanim tespiti (GPU) | `nvidia-smi`, amdgpu sysfs, `ash` (Vulkan) | Surucu/GPU yoksa zarifce bos liste — bu ortamda GERCEKTEN calistirilip dogrulandi |
| Kuantizasyon planlayicisi | (elle) | Guvenlik butcesi asilmiyor, kanarya reddinde kademe inisi, GQA KV-cache dogru hesabi |
| Watchdog / self-healing | `std::process` | Cokme dongusu tespiti, ustel backoff, GERCEK alt surec baslatma |
| Butunluk zinciri (uctan uca) | yukaridakilerin hepsi birlikte | `chimera install` -> `corrupt-test` -> `verify` -> `verify --repair` -> dosyalar golden ile birebir ayni |

Toplam **29 test**, hepsi `cargo test` ile calisir, hicbiri mock/stub
kullanmaz — gercek kriptografik islemler, gercek dosya G/C'si, gercek alt
surecler.

## `cargo build --target x86_64-pc-windows-gnu` ile dogrulanan

Windows'a ozgu DXGI (donanim tespiti) kodu bu ortamda **calistirilamadi**
(fiziksel/sanal Windows makinesi yok) ama `windows` crate'inin resmi
bindings'ine karsi **derlenip baglanarak** dogrulandi — bu, donanimsiz
yapilabilecek en guclu statik dogrulama adimidir. Cikan `.exe` gecerli bir
PE32+ dosyasidir (`file` komutuyla dogrulanmistir).

## Bilerek kapsam DISINDA birakilanlar (ve neden)

Bu proje "hicbir yeri sahte olmasin" ilkesiyle yazildi. Asagidaki parcalar,
bu sanal/kapali ortamda **dogrulanamayacagi** icin koda hic girmedi —
calisiyormus gibi gorunen ama test edilememis kod, testli koddan daha
kotudur:

- **Metal (macOS GPU tespiti).** Apple'in framework/SDK baglantilari bu
  Linux ortaminda ne kurulabilir ne derlenebilir. Gercek bir Mac + Xcode
  gerekir.
- **TPM 2.0 donanim muhurleme.** Bu konteynerde `/dev/tpm0` yok. Yerine,
  mimarinin zaten tanimladigi yazilim-yolu (Argon2id parola + Shamir(2,3))
  tam olarak calisir durumda. Gercek bir TPM oldugunda Share A'nin
  `tss-esapi` ile donanima baglanmasi, mevcut API degismeden eklenebilir.
- **eBPF/XDP tabanli ag mutasyonu (Proteus).** Kok/kernel BPF yetkisi ve
  gercek bir ag arayuzu gerektirir; bu konteynerde guvenle ne yazilip ne
  de test edilebilir bir sey degil.
- **Gercek GGUF model agirliklariyla LLM cikarsama.** Bu ortamda cok-GB'lik
  model dosyalari yok. Butunluk-koruma zinciri (Merkle + imza + onarim)
  YINE DE gercek dosyalar uzerinde tam olarak calisir (`chimera install` /
  `verify` / `corrupt-test` ile canli gorulebilir); yalnizca "yerinde
  yeniden kuantizasyon" (`fallocate` numarasi) ve `llama.cpp` baglantisi
  gercek bir GGUF dosyasi gerektirdigi icin bu derlemede yok.
- **NVML (dogrudan `dlopen`).** `nvidia-smi` alt surecine cikmak tercih
  edildi cunku NVML'in ham ABI'sini bu ortamda gercek bir surucuye karsi
  dogrulayamiyorduk; `nvidia-smi` NVIDIA'nin kendi resmi araci oldugu icin
  onun ciktisini ayristirmak, dogrulanabilir bir yoldur.

Bu maddelerin hepsi `docs/chimera/00-ARCHITECTURE.md`'de tasarim olarak
tanimlidir; burada yalnizca "bu derlemede kod olarak yok, cunku burada
dogrulanamazdi" farki netlestiriliyor.
