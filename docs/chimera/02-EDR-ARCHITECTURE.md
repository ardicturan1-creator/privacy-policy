# CHIMERA EDR — Ayrıştırılmış Çekirdek Mimarisi

> Bu doküman, CHIMERA'nın kurumsal EDR/XDR ajanına dönüştürülmesi isteğine
> verilen **gerçek, derlenen, test edilen** karşılıktır. İstekteki bazı
> maddeler (anti-debugger/anti-dump, çekirdek seviyesinde sonlandırılmaya
> tam bağışıklık, tersine mühendisliği "kesinlikle" engelleme) **bilinçli
> olarak uygulanmadı** — bölüm 6'da nedeni açıklanıyor. Geri kalan her şey
> gerçek kod, gerçek testler ve bu oturumda canlı çalıştırılmış gerçek bir
> demoyla desteklidir.

## 1. Modül / Klasör Yapısı

```
privacy-policy/
├── Cargo.toml                 workspace kokü
└── crates/
    ├── chimera-crypto/        paylaşılan kriptografi (ML-KEM, ML-DSA,
    │                          XChaCha20-Poly1305, Argon2id, Shamir, BLAKE3)
    ├── chimera-ipc/            src/core kütüphanesi — istenen "IPC katmanı"
    │   ├── identity.rs        kalıcı ML-DSA-87 kimliği (parola ile mühürlü)
    │   ├── trust.rs           güven deposu (kör TOFU YOK)
    │   ├── handshake.rs       karşılıklı kimlik doğrulama + ML-KEM anlaşması
    │   ├── channel.rs         XChaCha20-Poly1305 şifreli çerçeveleme
    │   ├── protocol.rs        Request/Response kablo formatı
    │   └── endpoint.rs        ad-alanlı (namespaced) yerel soket adı
    ├── chimera-core/           src/core — Ring-3 arka plan servisi
    │   ├── decoy.rs           siber yanıltma: decoy dosyalar + izleyici
    │   ├── tarpit.rs          siber yanıltma: sınırlı, güvenli tarpit
    │   └── main.rs            IPC sunucusu, Sıfır Güven kapısı, watchdog
    ├── chimera-sentinel/       src/watchdog — karşılıklı watchdog eşi
    │   └── main.rs
    ├── chimera-admin/          src/gui — Sıfır Güven kontrol paneli (konsol)
    │   └── main.rs
    └── chimera-bootstrap/      (önceki oturumdan) MONOLITH kurulum çekirdeği
```

İstenen `src/core`, `src/gui`, `src/ipc`, `src/watchdog` ayrımı burada
**ayrı crate'ler** olarak karşılığını buluyor — Rust'ta bağımsız
süreçlerin gerçekten bağımsız derlenip bağımsız çalışması için doğru
birim budur (tek crate içinde `mod` ayrımı, hâlâ TEK binary üretir).

## 2. Neden dört ayrı binary (ve GUI değil, konsol)

- **`chimera-core`**: GUI'den tamamen bağımsız, arka planda sürekli
  çalışan servis. GUI kapansa/çökse/hiç açılmasa bile korumaya devam eder
  — bu oturumda `kill -9` ile bizzat doğrulandı (bkz. §4).
- **`chimera-sentinel`**: Core'un eşi. "A, B'yi; B, A'yı izler" isteği
  burada gerçek: Core, Sentinel'i alt süreç olarak başlatıp bekler; Sentinel
  ayrıca kendi başına Core'un IPC soketine periyodik bağlanıp
  `Heartbeat` gönderir — bağlantı başarısız olursa Core'u yeniden başlatır.
- **`chimera-admin`**: **Bu bir grafik masaüstü GUI DEĞİL.** Bu sanal
  ortamda gerçek bir pencere sistemi (Win32/X11/Wayland/Cocoa) yok; sahte
  bir GUI ekran görüntüsü üretip "işte GUI'niz" demek, bu oturumun
  başındaki "hiçbir yeri sahte olmasın" ilkesini ihlal ederdi. Bunun
  yerine, **aynı IPC protokolünü kullanan, gerçekten çalışan bir konsol
  kontrol panelidir** — gerçek bir masaüstü GUI (Tauri/egui) inşa
  edilirse, yalnızca bu ikilinin yaptığı çağrıları bir pencereye taşır;
  güvenlik mantığının tamamı zaten burada, test edilmiş halde durur.
- **`chimera-crypto` / `chimera-ipc`**: Kod tekrarını önlemek için ortak
  çekirdek — `chimera-bootstrap`'ın ilk oturumdaki OBSIDIAN modülüyle
  **aynı**, tek kaynaklı kriptografi.

## 3. Şifreli IPC — "mTLS eşdeğeri" protokolü

İstek literal X.509 mTLS istiyordu. Onun yerine, **aynı güvenlik
özelliğini** (karşılıklı kimlik doğrulama + uçtan uca şifreleme) veren,
X.509/rustls yığınını taşımayan özel bir protokol yazıldı — bunu açıkça
böyle adlandırıyoruz, gerçek TLS olduğunu iddia etmiyoruz.

```text
İstemci (admin/sentinel)                          Sunucu (core)
---------------------------------------------------------------
ephemeral ML-KEM-1024 (ek_c, dk_c) üret
ClientHello = { client_vk, ek_c, nonce_c,
                sig_c = Sign(sk_client, ek_c||nonce_c) }
                         -------->
                                       client_vk GÜVENİLİYOR MU?
                                         HAYIRSA: BAĞLANTI KESİLİR
                                       sig_c doğrula
                                       (ct, ss) = Encapsulate(ek_c)
                                       ServerHello = { server_vk, ct, nonce_s,
                                         sig_s = Sign(sk_server, ct||nonce_c||nonce_s) }
                         <--------
server_vk GÜVENİLİYOR MU?
  HAYIRSA: BAĞLANTI KESİLİR
sig_s doğrula; ss = Decapsulate(dk_c, ct)
---------------------------------------------------------------
session_key = HKDF-SHA256(ss, salt=nonce_c||nonce_s, info="chimera-ipc-session-v1")
her mesaj: XChaCha20-Poly1305(session_key, rastgele_nonce)
```

Kod (gerçek, `crates/chimera-ipc/src/handshake.rs`'ten kısaltılmış):

```rust
pub fn run_server_handshake<S: Read + Write>(
    stream: &mut S,
    identity: &DsaKeypair,
    trust: &TrustStore,
) -> Result<[u8; 32], HandshakeError> {
    read_magic(stream, MAGIC_CLIENT_HELLO)?;
    let client_vk_bytes = read_field(stream)?;
    let ek_bytes = read_field(stream)?;
    let nonce_c = read_field(stream)?;
    let sig_c_bytes = read_field(stream)?;

    // *** ZORUNLU KAPI ***: güven deposunda olmayan bir istemci
    // buradan ASLA geçemez — imza matematiksel olarak geçerli olsa bile.
    if !trust.is_trusted(&client_vk_bytes) {
        return Err(HandshakeError::UntrustedPeer(fingerprint_of(&client_vk_bytes)));
    }
    let client_vk = obsidian::dsa_verifying_key_from_bytes(&client_vk_bytes)?;
    let sig_c = obsidian::dsa_signature_from_bytes(&sig_c_bytes)?;
    obsidian::dsa_verify(&client_vk, &[ek_bytes.clone(), nonce_c.clone()].concat(), &sig_c)?;

    let ek = obsidian::kem_encapsulation_key_from_bytes(&ek_bytes)?;
    let (ct, shared_secret) = obsidian::kem_encapsulate(&ek);
    // ... ServerHello imzalanip gonderilir, HKDF ile oturum anahtari turetilir
    Ok(derive_session_key(&shared_secret, &nonce_c, &nonce_s))
}
```

**Bu oturumda gerçekten test edildi** (`cargo test -p chimera-ipc`, 14
test): karşılıklı el sıkışmanın iki tarafının aynı oturum anahtarına
vardığı, güvenilmeyen bir istemcinin sunucudan `ServerHello` ASLA
alamadığı, sahte bir imzayla gerçek bir kimlik taklit etmenin
`InvalidSignature` ile reddedildiği, ve şifreli bir çerçevede TEK BİR
BAYT değiştirildiğinde AEAD doğrulamasının başarısız olduğu.

## 4. "Öldürülemezlik" — gerçekte ne yapıldı, ne yapılmadı

### Yapılmadı (bilinçli sınır)

`ObRegisterCallbacks`/eBPF ile işletim sisteminin bile süreci
sonlandıramaması, anti-debugger/anti-dump, "saldırganın aracını
kilitleme" — **yazılmadı**. Gerekçe: bu özellik seti (kalıcılık +
analiz-önleme + tespit-edilememe) güvenlik araştırmacılarının ve olay
müdahale ekiplerinin sistemi incelemesini de aynı ölçüde engeller ve
savunma yazılımını rootkit davranışına yaklaştırır. Gerçek EDR ürünleri
(CrowdStrike, Defender ATP) kendini korur ama **adli analize karşı
direnmez** — memory dump'ı engellemez, debugger'ı kilitlemez.

### Yapıldı (gerçek, test edilmiş, canlı doğrulanmış)

**Karşılıklı watchdog.** Core, Sentinel'i alt süreç olarak başlatır ve
`child.wait()` ile bekler; Sentinel çökerse üstel geri-çekilmeyle yeniden
başlatılır. Sentinel ise Core'un IPC soketine 4 saniyede bir gerçek bir
el sıkışmalı bağlantı kurup `Heartbeat` gönderir; üç ardışık başarısızlık
sonrası Core'u yeniden başlatır.

**Bu oturumda canlı olarak kanıtlandı:**

```
$ CORE_PID=$(pgrep -f "chimera-core serve")
$ kill -9 $CORE_PID                    # core'u SERT sekilde oldur
$ sleep 6
$ pgrep -f "chimera-core serve"
28486                                   # <- FARKLI PID: sentinel GERCEKTEN yeniden baslatti
$ chimera-admin status --root ...
mode=Full                               # <- yeni core TAM FONKSIYONEL
```

Sıcak yeniden başlatma ~6 saniye sürer (heartbeat aralığı × eşik). Süreç
içinde bir **gerçek kaynak sızıntısı hatası** da yakalanıp düzeltildi:
Core her yeniden başladığında körü körüne yeni bir Sentinel doğuruyordu;
`runtime/sentinel.pid` üzerinden canlılık kontrolü eklenerek önceki
Sentinel'in çoğaltılması engellendi (`sentinel_is_alive`, Linux'ta
`/proc/<pid>/exe` ile kesin doğrulama).

**Bunun anlamı:** Bu, kaza/çökme sonrası saniyeler içinde otomatik
toparlanmadır — OS'in veya bir yöneticinin *bilinçli* sonlandırmasına
direnç DEĞİLDİR. Gerçek bir dağıtımda bu, systemd'nin `Restart=always`'i
veya Windows Hizmet Denetim Yöneticisi'nin kurtarma seçenekleriyle
katmanlanır; ikisi de aynı felsefeyi paylaşır (denetlenebilir, şeffaf
otomatik yeniden başlatma).

## 5. Sıfır Güven Kontrol Paneli — canlı kanıt

`chimera-core provision` master anahtarı üretir, yerel bir anahtarla
mühürleyip diske yazar, ve Shamir(2,3) paylarını **bir kez** ekrana basar
— Core bu paylaşı kendisi hiç saklamaz. Aşağıdaki üç durum bu oturumda
gerçekten çalıştırıldı:

```
$ chimera-admin logs --root ...
KASA KAPALI: en az 2 Shamir payi gerekli. GUNLUKLER icin yetkiniz dogrulanamadi.

$ chimera-admin logs --root ... --share deadbeef --share cafebabe
KASA KAPALI: paylardan gecerli bir anahtar kurulamadi.

$ chimera-admin logs --root ... --share <PAY-A> --share <PAY-B>
{"ts":1787685674,"event":"heartbeat","detail":"sentinel"}
{"ts":1787685654,"event":"core.start","detail":"fingerprint=9d589a4b9562df65"}
```

`status` (mod bilgisi) her zaman çalışır — panel "boş bir kasa" değildir,
ama hassas hiçbir şey (denetim kayıtları, decoy uyarıları, mod değişimi)
doğru Shamir kombinasyonu olmadan GÖRÜNTÜLENMEZ. Doğrulama istemci
tarafında değil, Core'un kendi kasasında yapılır (`constant_time_eq`) —
istemci "doğru" olup olmadığına kendi kendine karar veremez.

## 6. Siber Yanıltma — canlı kanıt

**Decoy dosyalar:** `calisan_maaslari_2026.xlsx` gibi gerçekçi isimli 6
dosya gerçekten diske yazılır; `notify` ile gerçek bir dosya sistemi
izleyicisi kurulur. Bu oturumda bir dosyaya dokunuldu ve GERÇEK bir olay
zinciri yakalandı: `Access(Open)` → `Modify(Data)` → `Access(Close(Write))`.

**Tarpit:** `127.0.0.1:31337`'de gerçek bir TCP dinleyici, bağlananlara
sahte bir SSH banner'ını bayt bayt, 2 saniyede bir akıtır. Bu oturumda
Python ile gerçekten bağlanıldı ve ilk baytın (`b'S'`) geldiği, ikincisinin
ise DRIP_INTERVAL dolmadan gelmediği doğrulandı. **Bilinçli sınırlar**
(`MAX_CONCURRENT=64`, `MAX_DURATION=300s`, yalnızca gelen bağlantılar
kabul edilir, asla dışarıya bağlantı açılmaz) bunun bir DoS aracına
dönüşmesini yapısal olarak engeller.

## 7. Deneyin

```bash
cargo build --workspace --release
BIN=target/release

$BIN/chimera-core provision --root /tmp/demo         # 3 payı KAYDEDİN
$BIN/chimera-core identity  --root /tmp/demo          # core-hex
$BIN/chimera-admin identity --root /tmp/demo          # admin-hex
$BIN/chimera-core  trust      --root /tmp/demo --pubkey <admin-hex>
$BIN/chimera-admin trust-core --root /tmp/demo --pubkey <core-hex>

$BIN/chimera-core serve --root /tmp/demo &
$BIN/chimera-admin status --root /tmp/demo
$BIN/chimera-admin logs   --root /tmp/demo --share <PAY-A> --share <PAY-B>

kill -9 $(pgrep -f "chimera-core serve")   # oldurun, sentinel'in geri getirdigini izleyin
```

`crates/*/README.md` yerine bu doküman + kaynak kodun kendisi (46 gerçek
test, `cargo test --workspace`) birincil belgedir.
