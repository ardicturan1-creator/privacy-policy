# CHIMERA EDR — Reverse Engineering Sertleştirme Raporu

> Hedef: "Reverse engineer binary'yi hiçbir şekilde açamasın" DEĞİL —
> **"Binary analiz edilse bile kritik sırlar, private key'ler, trust
> credentials ve EDR'nin temel detection intelligence'ı elde edilemesin;
> binary değiştirildiğinde veya sahte bir bileşen sisteme sokulduğunda
> güven zinciri bunu tespit edebilsin."**
>
> Bu raporda **"yapıldı"** dediğim her şey bu oturumda gerçekten
> derlendi ve gerçekten doğrulandı (PE header parse, `strings` taraması,
> gerçek testler, `x86_64-pc-windows-gnu` cross-compile). **"Önerilir"**
> dediğim her şey ise gerçek altyapı (Windows makinesi, kod imzalama
> sertifikası, ayrı bir backend sunucu) gerektirdiği için bu oturumda
> uygulanamadı — bunu açıkça ayırıyorum.

---

## A) Önceki Güvenlik Seviyesi

- ASLR/DEP zaten rustc/mingw varsayılanıyla açıktı ama **doğrulanmamış ve
  açıkça yazılmamıştı** — gelecekte sessizce kapanabilirdi.
- Binary'de yapı makinesinin **tam ev dizini yolu** (`/home/user/...`) ve
  **cargo kayıt defteri önbellek yolu** (`/root/.cargo/registry/...`)
  138+ kez açık metin olarak duruyordu.
- IPC kanalı kimlik doğrulamalı ve şifreliydi (önceki oturum) ama
  **replay/yeniden-sıralama koruması yoktu** ve protokol sürümü
  denetlenmiyordu.
- Kimlik anahtarı çalınırsa (ör. disk erişimi olan bir saldırgan
  tarafından), **değiştirilmiş bir ikili aynı anahtarla geçerli bir el
  sıkışma yapabilirdi** — ikilinin kendisinin bütünlüğünü doğrulayan
  hiçbir mekanizma yoktu.
- Windows Named Pipe **varsayılan ACL**'iyle açılıyordu (aynı makinedeki
  her hesap bağlanabilir).
- Panic mesajları tam dosya yolu + satır numarası + iç Rust tür bilgisini
  doğrudan stderr'e basıyordu.
- Anti-analiz katmanı yoktu (talep zaten "tek başına kullanma" dediği
  için bu kritik bir eksiklik değildi — sadece "yok"tu).

## B) Yapılan Değişiklikler

| # | Değişiklik | Dosya |
|---|---|---|
| 1 | ASLR/DEP linker bayrakları **açıkça** yazıldı (`--dynamicbase,--nxcompat,--high-entropy-va`) | `.cargo/config.toml` |
| 2 | `--remap-path-prefix` ile derleme-zamanı yol gizleme (standart rustc özelliği) | `.cargo/config.toml` |
| 3 | Linker seviyesinde `--strip-all` (Cargo `strip=true`'ya ek katman) | `.cargo/config.toml` |
| 4 | IPC: AEAD **associated-data tabanlı replay/sıra-dışı koruması** + protokol sürüm baytı | `chimera-ipc/src/channel.rs`, `handshake.rs` |
| 5 | **Karşılıklı ikili (binary) attestation**: her el sıkışmada BLAKE3 özet imzalanıp gönderilir, operatörün önceden `attest`/`attest-core` ile sabitlediği özetle karşılaştırılır | `chimera-ipc/src/attestation.rs` (yeni) |
| 6 | Windows Named Pipe **ACL kısıtlaması** (yalnızca SYSTEM + Administrators) | `chimera-core/src/main.rs` |
| 7 | Tek, opsiyonel, **log-only** `IsDebuggerPresent` katmanı (varsayılan kapalı) | `chimera-core/src/main.rs` |
| 8 | Panic hook sertleştirmesi: konsola genel mesaj, tam detay güvenli log dosyasına | `chimera-core`, `chimera-sentinel`, `chimera-admin` `main.rs` |
| 9 | Otomatik **hardening doğrulama script'i** (PE bitleri + yasaklı dize taraması + sembol kontrolü) | `scripts/release-check.py` (yeni) |

Toplam **53 gerçek test** (`cargo test --workspace`), yeni eklenenler
dahil (replay reddi, sıra-dışı çerçeve reddi, çalınmış anahtarla bile
ikili-özet uyuşmazlığının reddi, protokol sürüm uyuşmazlığının reddi).
`release-check.py` çalıştırıldı ve **gerçekten bir sorun buldu**
(`chimera-bootstrap.exe`'de kalıntı `TODO(ffi)` yorum dizeleri — bkz. §D).

## C) Reverse Engineering'e Karşı Kazanılan Dayanıklılık

- **Statik keşif (Seviye 1-2) zorlaştı:** yapı makinesi dosya yapısı,
  kullanıcı adı, geliştirme ortamı bilgisi artık `strings` ile
  çıkarılamıyor. Bu, saldırganın "hedef nasıl inşa edilmiş" sorusuna
  ücretsiz cevap almasını engeller.
- **Kimlik hırsızlığı tek başına yetmiyor:** bir saldırgan `identity.sealed`
  dosyasını ve onu açacak parolayı ele geçirse bile (imzalama anahtarını
  çalsa bile), **pinlenmiş bir ikili özeti varsa** değiştirilmiş bir
  ikiliyle bağlanamaz — bu, testte gerçekten doğrulandı
  (`pinned_binary_hash_mismatch_is_rejected_even_with_valid_signature`).
- **Ağ/IPC seviyesinde tekrar oynatma (replay) artık işe yaramıyor:**
  yakalanmış geçerli bir şifreli çerçeve ikinci kez gönderildiğinde AEAD
  doğrulaması başarısız olur (test edildi).
- **Windows'ta yerel ayrıcalıksız bir süreç artık pipe'a bağlanamıyor**
  (ACL), bu da IPC protokolünü fuzzing'lemeyi/analiz etmeyi güçleştirir —
  önce yönetici hakkı gerekir.

## D) Kalan Zayıflıklar (Dürüst)

- **CFG (Control Flow Guard) YOK.** GNU ld (mingw) bunu üretemiyor;
  gerçek CFG için `x86_64-pc-windows-msvc` hedefi + gerçek bir Windows
  makinesinde MSVC/Windows SDK gerekir. Bu Linux ortamında ne kurulabilir
  ne doğrulanabilir.
- **`chimera-bootstrap.exe`'de kalıntı `TODO(ffi)` yorum dizeleri var**
  (önceki oturumdan, MONOLITH kurulum çekirdeği) — `release-check.py`
  bunu gerçekten yakaladı. Bu üç EDR binary'sini (core/sentinel/admin)
  etkilemiyor ama aynı build pipeline'ından geçtiği için düzeltilmeli.
- **Attestation "sonraki el sıkışmada" yakalar, "o anda" değil.** Çalışan
  bir sürecin BELLEĞİ o an değiştirilirse (Seviye 4-5 saldırgan), bir
  sonraki bağlantıya kadar fark edilmez.
- **Argv'de hâlâ hassas veri var:** `--password`, `--pubkey`,
  `--share` değerleri `ps`/Görev Yöneticisi'nden aynı makinedeki başka
  bir yerel kullanıcı tarafından görülebilir. Dosya/`stdin` alternatifi
  henüz eklenmedi (bkz. §G, önerilen).
- **Anti-debug tek bir standart API'ye dayanıyor** (`IsDebuggerPresent`,
  PEB bayrağı) — bilinçli olarak kolayca atlatılabilir, sadece bir sinyal
  katmanıdır (talebin kendisi de bunu istiyordu).
- **Gerçek bir backend/telemetri korelasyon katmanı yok** — bkz. §G.

## E) Performans Maliyeti

- **AEAD associated-data (replay koruması):** ölçülemez düzeyde (aynı
  şifreleme çağrısına birkaç bayt ekleniyor, ek bir kriptografik işlem
  yok).
- **İkili özet attestation:** el sıkışma başına bir kez `current_exe()`
  okunup BLAKE3 hash'lenir — birkaç MB'lık bir binary için ~1 ms
  mertebesinde, ve yalnızca bağlantı kurulumunda (mesaj başına değil).
- **Path remap / strip:** yalnızca derleme zamanını etkiler, çalışma
  zamanı maliyeti sıfırdır.
- **Anti-debug kontrolü:** varsayılan kapalı; açıldığında tek bir Win32
  API çağrısı, süreç başına bir kez.
- **Genel değerlendirme: ölçülebilir bir performans regresyonu yok.**

## F) False-Positive / Kararlılık Riskleri

- **Attestation:** bir ikili **meşru** bir güncellemeyle değiştirildiğinde
  (yeni sürüm) ve operatör `attest`/`attest-core`'u yeniden çalıştırmayı
  unutursa, o bileşenle el sıkışma `BinaryMismatch` ile reddedilir. Bu,
  **kasıtlı bir güvenlik-kullanılabilirlik dengesidir** — sessizce
  görmezden gelmek yerine operatörün bilinçli bir adım atmasını zorunlu
  kılar. Gerçek dağıtımda bu adım güncelleme betiğine otomatik
  eklenmelidir (bkz. §G).
- **Anti-debug:** varsayılan KAPALI olması tam olarak bu riski
  önlemek içindir — meşru bir APM/hata ayıklama aracı bağlıyken
  yanlışlıkla tetiklenmesin diye.
- **Named Pipe ACL:** Core farklı bir hesap altında (ör. normal kullanıcı)
  çalıştırılırsa ve Admin/Sentinel SYSTEM/Administrators değilse,
  bağlantı OS seviyesinde reddedilir. **Üretimde Core'un servis hesabı
  netleştirilmeden bu ACL'i olduğu gibi dağıtmayın** — bu, kilitlenmeyi
  önlemek için SDDL'nin dağıtım ortamına göre ayarlanması gerektiği
  anlamına gelir.

## G) Server-Side'a Taşınması Gereken Mantık

Bu oturumda **gerçek bir backend altyapısı kurulmadı** (ağ/sunucu
provizyonu bu ortamın kapsamı dışında) — ama mimari olarak şunlar
taşınmalı:

1. **Trust/attestation kayıtlarının merkezi dağıtımı.** Bugün her
   `chimera-core`/`admin`/`sentinel` kendi yerel dosyasında güven ve
   attestation listesi tutuyor. Kurumsal ölçekte bunlar merkezi bir
   yönetim sunucusundan imzalı olarak itilmeli (MDM benzeri).
2. **Detection correlation/threat intelligence.** Bu derlemede
   detection mantığı yok (endpoint şu an yalnızca decoy/tarpit
   telemetrisi topluyor) — ileride eklenecek kural motoru, itibar
   sorguları ve çapraz-host korelasyon **sunucu tarafında** olmalı;
   endpoint yalnızca ham olay göndermeli.
3. **Güncelleme dağıtımı ve imza doğrulama otoritesi.** Release imzalama
   anahtarının kendisi asla endpoint'te olmamalı (zaten değil) —
   güncelleme sunucusu tarafında, ayrı bir HSM/imzalama hizmetinde
   olmalı.
4. **Uzun ömürlü Shamir paylarının saklanması.** Şu an operatör bunları
   kendi sorumluluğunda saklıyor (mimari gereği — bkz. önceki oturum);
   kurumsal dağıtımda Pay A/C bir kurumsal HSM/anahtar kasasına
   gidebilir.

## H) Binary'de Kesinlikle Bulunmaması Gereken Bilgiler

**Bugün itibarıyla binary'lerde bulunmayanlar (doğrulandı):**
- Uzun ömürlü master anahtar veya imzalama anahtarı (asla üretilip
  derlemeye gömülmedi — her zaman çalışma zamanında üretilip yerel
  parola ile mühürleniyor).
- Yapı makinesi dosya yolu/kullanıcı adı (bu oturumda temizlendi).

**Hâlâ dikkat edilmesi gerekenler:**
- `strings` taramasında görülen komut adları (`provision`, `attest`,
  `--share`) — bunlar **sır değil**, protokol/CLI yüzeyidir; gizlenmesi
  gerekmez (Kerckhoffs ilkesi: güvenlik gizli protokole değil anahtarlara
  dayanmalı, ki burada da öyle).
- `release-check.py`'ın yakaladığı `TODO(ffi)` kalıntıları (bkz. §D) —
  bunlar sır değil ama "bu özellik eksik" bilgisini saldırgana bedava
  veriyor; temizlenmeli.

## I) Windows Mitigation/Hardening Durumu

| Mitigation | Durum | Nasıl doğrulandı |
|---|---|---|
| ASLR (DYNAMIC_BASE + HIGH_ENTROPY_VA) | ✅ Açık | PE header parse (bu oturumda) |
| DEP (NX_COMPAT) | ✅ Açık | PE header parse |
| CFG (Control Flow Guard) | ❌ Yok | MSVC linker gerektirir, mingw'de yok |
| Sembol/PDB stripping | ✅ Uygulandı | `strip=true` + linker `--strip-all`, `release-check.py` ile doğrulandı |
| Authenticode imzalama | ❌ Yapılmadı | Gerçek bir kod imzalama sertifikası gerektirir (bkz. §L) |
| Named Pipe ACL | ✅ Kısıtlandı | Cross-compile ile derleme doğrulandı (Windows'ta çalıştırılamadı) |

## J) IPC Güvenlik Durumu

| Özellik | Durum |
|---|---|
| Karşılıklı kimlik doğrulama (mTLS-eşdeğeri) | ✅ Önceki oturumdan, testli |
| Uçtan uca şifreleme (XChaCha20-Poly1305) | ✅ Testli |
| Replay koruması (sıra numarası + AAD) | ✅ **Bu oturumda eklendi**, testli |
| Sıra-dışı/enjeksiyon koruması | ✅ **Bu oturumda eklendi**, testli |
| Protokol versiyonlama | ✅ **Bu oturumda eklendi**, testli |
| Karşılıklı ikili (binary) attestation | ✅ **Bu oturumda eklendi**, testli |
| Windows ACL (OS-seviyesi erişim kısıtı) | ✅ **Bu oturumda eklendi**, cross-compile doğrulandı |
| Kör TOFU | ❌ Yok (kasıtlı — hem önceki hem bu oturumda) |

## K) Cryptographic Key-Management Durumu

- **Anahtar yaşam döngüsü:** kimlik anahtarları (ML-DSA-87) süreç
  başlangıcında bir kez üretilir/yüklenir, oturum anahtarları (ML-KEM-1024
  + HKDF) her el sıkışmada yeniden türetilir (ileri gizlilik).
- **Nonce benzersizliği:** XChaCha20-Poly1305'in 192-bit genişletilmiş
  nonce'u rastgele üretiliyor — çakışma riski ihmal edilebilir; ek olarak
  artık AAD'de sıra numarası var (çift katman).
- **Anahtar rotasyonu:** oturum anahtarı bağlantı başına türetilir (doğal
  rotasyon). Kalıcı kimlik anahtarı rotasyonu için ayrı bir mekanizma
  YOK — bu, kurumsal dağıtımda eklenecek bir sonraki adımdır.
- **Yeniden oynatma direnci:** ✅ bu oturumda eklendi (§J).
- **Downgrade direnci:** el sıkışma sürümü artık denetleniyor
  (`UnsupportedVersion`), ama henüz yalnızca TEK bir sürüm var — gerçek
  bir downgrade senaryosu (v2 varken v1'e zorlama) test edilemedi.
- **Güvenli silme:** `zeroize` crate'i master anahtar/türetilmiş anahtar
  belleklerinde kullanılıyor (önceki oturumdan).

## L) Supply-Chain/Update Güvenliği

- **Bu oturumda GERÇEK bir güncelleme mekanizması uygulanmadı** — mevcut
  ML-DSA-87 + Merkle altyapısı (önceki oturumdan, `chimera-bootstrap`)
  bunun için hazır bir temel sağlıyor ama EDR üçlüsüne (core/sentinel/
  admin) henüz bağlanmadı.
- **Authenticode imzalama YAPILMADI** — gerçek bir kod imzalama
  sertifikası (DigiCert/Sectigo EV ya da Azure Trusted Signing) ve
  gerçek bir Windows imzalama makinesi (`signtool.exe`) gerektirir;
  bunlar bu Linux sanal ortamında ne temin edilebilir ne test edilebilir.
  **Önerilen adım:** release pipeline'ının son adımı olarak
  `signtool sign /fd SHA256 /tr <timestamp-authority> /td SHA256` — CI
  runner'ı Windows olmalı, imzalama anahtarı bir HSM/KSP'de tutulmalı
  (özel anahtar asla CI ortamına indirilmemeli).
- **Anti-downgrade:** el sıkışma protokol sürümü kontrolü var (§K) ama
  UYGULAMA sürümü (binary'nin kendisi) için ayrı bir monotonluk sayacı
  YOK — bu, gerçek güncelleme mekanizmasıyla birlikte eklenmelidir.

## M) Bileşen Bazında Sertleştirme Skoru (10 üzerinden)

| Bileşen | Skor | Gerekçe |
|---|---|---|
| **chimera-core** | 7/10 | ASLR/DEP/path-remap/ACL/attestation/anti-debug/panic-hook hepsi var; CFG ve imzalama eksik |
| **chimera-sentinel** | 6/10 | Aynı temel sertleştirmeler ama ACL/anti-debug eklenmedi (kendi dinleyicisi yok, gerek de yok) |
| **chimera-admin** | 6/10 | Aynı; ayrıca argv'deki Shamir payları hâlâ süreç listesinde görünür (§D) |
| **chimera-ipc (protokol)** | 8/10 | Replay/versiyon/attestation testli ve sağlam; kör-TOFU'suz tasarım zaten güçlüydü |
| **Build pipeline** | 5/10 | Gerçek bir otomatik doğrulama script'i var ve çalışıyor, ama imzalama ve gerçek CI entegrasyonu yok |
| **Supply-chain/update** | 2/10 | Tasarım hazır (mevcut Merkle+imza altyapısı) ama EDR'ye henüz bağlanmadı |

---

## Tehdit Modeli — Saldırgan Seviyelerine Göre

| Seviye | Elde edebileceği | Elde EDEMEYECEĞİ | Bypass edebileceği | Server-side olmalı |
|---|---|---|---|---|
| **1** (strings/imports) | CLI komut adları, protokol alan adları, hata metinleri (artık genel) | Yapı makinesi bilgisi (temizlendi), gömülü sır (zaten yoktu) | — | — |
| **2** (Ghidra/IDA) | Genel kontrol akışı, kripto kütüphane kullanım şekli (RustCrypto/ml-kem/ml-dsa çağrıları tanınabilir) | Hangi anahtarın hangi ikili özetle eşleştiği (attestation dosyaları ayrı, şifreli değil ama diskte, erişim OS izinlerine bağlı) | Anti-debug (tek katman, bilinçli) | Detection kuralları (şu an zaten endpoint'te yok) |
| **3** (debugger/runtime) | O anki bellek içeriği (oturum anahtarı dahil — kaçınılmaz, TLS/mTLS'in kendisi de aynı sınırı taşır) | Kalıcı kimlik anahtarının PAROLASI (mühürlü, RAM'de yalnızca kullanım anında) | `IsDebuggerPresent` katmanı | Kalıcı sır saklama (zaten yapılmıyor) |
| **4** (yerel admin/root) | Diskteki her dosya (sealed identity, attest listeleri, trust listeleri) | Sealed identity'nin İÇERİĞİ (parola olmadan Argon2id+XChaCha20 ile korunur) | Named Pipe ACL (kendi hesabı zaten yetkiliyse) | TPM/donanım-bağlı anahtar saklama (bu ortamda yok, önceki oturumda belgelendi) |
| **5** (profesyonel, tam erişim) | İkilinin TAMAMININ statik+dinamik analizi, algoritma detayları | Merkezi backend'deki (varsa) threat intelligence/korelasyon kuralları — **ÇÜNKÜ ENDPOINT'TE DEĞİLLER** | Attestation (ikiliyi değiştirip pin'i de değiştirebilir — pin operatör tarafından ayrıca doğrulanmalı) | Tüm detection intelligence (§G) |

---

*Önceki oturumların mimari dokümanlarıyla birlikte okunmalıdır:
`00-ARCHITECTURE.md` (CHIMERA çekirdek tasarımı), `02-EDR-ARCHITECTURE.md`
(Core/Sentinel/Admin ayrıştırılmış mimarisi). Bu dosya yalnızca bu
oturumdaki sertleştirme değişikliklerini kapsar.*
