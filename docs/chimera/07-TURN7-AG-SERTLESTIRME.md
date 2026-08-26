# CHIMERA EDR — Turn 7 / Faz 2: Ağ Saldırısı Sertleştirme

Faz 1 fidye yazılımını *makine üzerinde* durdurdu. Faz 2, saldırganın
makineye **ulaşma** yolunu hedefler: RDP/SMB parola deneme saldırılarının
tespiti, geçici ve kendiliğinden kalkan otomatik IP engelleme, ve tarpit'in
gerçek saldırı yüzeylerine genişletilmesi.

## A) Önceki Seviye (Faz 1 sonu)

| Yetenek | Faz 1 durumu |
|---|---|
| RDP/SMB parola deneme saldırısı | **Hiç izlenmiyor** |
| Saldıran IP'ye otomatik yanıt | **Yok** — yalnızca elle `block-ip` (Shamir 2/3) |
| Tarpit | Tek port, `127.0.0.1:0` (rastgele, kimsenin taramadığı bir port) |
| Tarpit'e bağlanan IP | Yalnızca loglanır |
| Engellerin süresi | Kalıcı (yalnızca elle kaldırılır) |
| Test sayısı | 94 |

## B) Yapılan Değişiklikler

| # | Değişiklik | Dosya |
|---|---|---|
| 1 | **4625 tabanlı RDP/SMB brute-force tespiti** (`EvtQuery`/`EvtNext`/`EvtRender`) | `bruteforce.rs` (**yeni**) |
| 2 | **TTL'li, kendiliğinden kalkan otomatik engelleme** + kendini kilitleme koruması | `autoblock.rs` (**yeni**) |
| 3 | Tarpit **çoklu porta** (445/SMB, 3389/RDP) genişletildi | `tarpit.rs` |
| 4 | Tarpit'e bağlanan IP, eşik aşılınca **oto-bloka besleniyor** | `tarpit.rs`, `main.rs` |
| 5 | Oto-blok `pipeline.rs` **whitelist'ine eklendi** (geçici + geri alınabilir olduğu için) | `pipeline.rs`, `scanner.rs` |
| 6 | Arka plan döngüsü her turda **süresi dolan engelleri kaldırıyor** | `pipeline.rs` |
| 7 | Olay günlüğü okunamıyorsa bu **körlük olarak** raporlanıyor (sessizce "saldırı yok" değil) | `scanner.rs` |
| 8 | Platforma bağlı `dead_code` uyarıları açıkça bastırıldı — derleme artık **0 uyarı** | `firewall.rs`, `scanner.rs`, `bruteforce.rs` |

### B.1 Neden 4625 ve neden XPath

4625 ("An account failed to log on") her Windows kurulumunda üretilen,
Microsoft'un belgelediği olaydır — özel bir ajan/hook/driver gerekmez.
`LogonType` alanı saldırı yüzeyini ayırır: **3 = SMB**, **10 = RDP**.
Diğer türler (interaktif, servis, batch) bilinçli olarak sayılmaz.

Filtreleme, Olay Günlüğü'nün **kendi XPath sorgu diline** yaptırılır:

```
*[System[(EventID=4625) and TimeCreated[timediff(@SystemTime) <= 300000]]]
```

Tüm günlüğü çekip Rust tarafında elemek, büyük bir sunucuda dakikalar
sürerdi. Ayrıca en fazla 2000 olay okunur — brute-force tespiti için
fazlasıyla yeterli, sınırsız tarama ise bir kaynak riskidir.

### B.2 Neden TTL'li oto-blok "kalıcı otomatik aksiyon yok" kuralını bozmuyor

Projenin değişmez kuralı **kalıcı** otomatik aksiyonu yasaklar. Oto-blok
üç şartı da sağlar ve bu yüzden whitelist'e girebilir:

| Şart | Nasıl sağlanıyor |
|---|---|
| Kalıcı değil | Her engelin TTL'i var (varsayılan 1 saat); `expire_due()` her arka plan turunda süresi dolanları kaldırır. Operatör hiçbir şey yapmasa bile engel düşer. |
| Geri alınabilir | Aynı `firewall::unblock_ip` kuralı silinir; veri kaybı/yapı değişikliği yok. |
| Dar | Tek bir uzak adres; yerel servisler, oturumlar, dosyalar etkilenmez. |

**Kalıcı** engelleme (`chimera-admin block-ip`) hâlâ Shamir(2,3) ister ve
bu modül tarafından asla otomatik uygulanmaz.

### B.3 Kendini kilitleme koruması (`never_block`)

Otomatik engellemenin en gerçek tehlikesi, makineyi yönetenin kendisini
dışarıda bırakmasıdır. `never_block()` engelleme yolundaki **ilk** adımdır
ve şunları koşulsuz reddeder: loopback, `0.0.0.0`/`::`, multicast,
broadcast, link-local (`169.254.0.0/16` ve `fe80::/10`), IP olmayan her
şey, ve operatörün `state/never_block.list` dosyasına yazdığı adresler
(`#` ile yorum desteklenir).

Ayrıca: defter yazılamazsa engel **geri alınır**. Aksi halde süresi asla
dolmayacak, "geçici" olduğu iddia edilen bir engel kalırdı — bu modülün
tüm vaadini bozardı.

## C) Gerçekten Kazanılan Direnç

| Senaryo | Faz 1 | Faz 2 |
|---|---|---|
| Bir IP'den RDP'ye 40 parola denemesi | Görünmez | Tespit edilir → **1 saatliğine otomatik engellenir** |
| Aynı IP hem SMB hem RDP deniyor | Görünmez | Tek bulguda **her iki yüzey** raporlanır |
| Saldırgan 445/3389 tarıyor ama servis kapalı | Görünmez | Tarpit yakalar, oyalar, eşikten sonra engeller |
| Meşru kullanıcı parolasını 3 kez yanlış giriyor | — | Eşik altı → **hiçbir şey olmaz** |
| Denetim politikası kapalı | — | "Tespit **KÖRDÜR**" diye raporlanır, "saldırı yok" denmez |
| Engel yanlışlıkla kondu | Elle kaldırma gerekir | **1 saat sonra kendiliğinden kalkar** |

## D) Kalan Zayıflıklar (Dürüst)

- **Denetim politikası bağımlılığı.** 4625 kayıtları "Audit Logon Failure"
  politikası açıksa üretilir. Kapalıysa bu modül hiçbir şey göremez.
  Kod bunu gizlemez (bkz. §B, madde 7) ama **açamaz** da — politikayı
  otomatik değiştirmek, whitelist'in dört şartını sağlamayan bir sistem
  değişikliği olurdu.
- **Kaynak adresi taklidi (IP spoofing) ile üçüncü tarafı engelletme.**
  Teorik olarak mümkündür. Neden pratikte sınırlı:
  (a) `accept()` yalnızca TCP el sıkışması **tamamlandıktan sonra** döner,
  yani saldırganın SYN-ACK'i görmesi gerekir — kör spoofing yetmez;
  (b) engel geçicidir (1 saat);
  (c) `never_block.list` ile kritik adresler korunabilir.
  **Yine de bu, Faz 2'nin en ciddi kalan riskidir** ve internete açık bir
  makinede tarpit portlarının oto-bloka beslenmesi operatör kararı
  olmalıdır.
- **TTL çözünürlüğü arka plan turuna bağlı.** `expire_due()` 30 dakikada
  bir çalışır, yani bir engel süresinden en fazla bir tur (≈30 dk) fazla
  kalabilir. Bilinçli: ayrı bir zamanlayıcı thread'i eklemek, kazandığı
  hassasiyete değmezdi.
- **Yalnızca IPv4 bağlantı tablosu.** `remote_ips_of_pid` (Faz 1) ve
  dinleyen port taraması `AF_INET` kullanır; IPv6 bağlantılar
  sayılmaz. 4625 tarafı IPv6'yı **destekler** (test edilmiştir), ama
  süreç-bazlı ağ izolasyonu desteklemez. Belgelenmiş bir eksiktir.
- **`bruteforce.rs` her taramada baştan sayar.** Pencere durumu tur
  arasında saklanmaz; 30 dakikalık tur ile 5 dakikalık pencere birlikte,
  turlar arasına düşen saldırıların kaçırılabileceği anlamına gelir.
  Gerçek bir dağıtımda tur aralığı pencereden kısa olmalıdır.

## E) Performans / Maliyet

- **Olay günlüğü sorgusu:** XPath ile günlük motorunda filtrelenir, en
  fazla 2000 olay, 64'lük gruplar hâlinde. Tur başına bir kez (30 dk).
- **Tarpit:** eşzamanlı bağlantı sınırı (`MAX_CONCURRENT = 64`) ve
  bağlantı başına süre sınırı (`MAX_DURATION = 300 sn`) Turn 6'dan beri
  aynı — çoklu porta geçmek bu sınırları değiştirmedi, her dinleyici
  kendi sayacını tutar.
- **Oto-blok defteri:** `fs4` OS kilidi altında düz metin; engel sayısı
  onlarca mertebesinde kalır (TTL sayesinde birikmez).
- **Ölçülebilir regresyon yok:** Wine altında tam test paketi ~2.9 sn.

## F) Yanlış-Pozitif / Kararlılık Riskleri

- **En yüksek risk: NAT arkasındaki paylaşılan çıkış IP'si.** Bir ofisin
  tamamı tek bir genel IP'den çıkıyorsa, tek bir kullanıcının parolasını
  tekrar tekrar yanlış girmesi **tüm ofisi** 1 saat dışarıda bırakabilir.
  Azaltma: eşik 5 dakikada 10 denemedir (tek kullanıcı için yüksek), engel
  geçicidir, ve `never_block.list` vardır. **Üretimde bu eşik ortama göre
  ayarlanmalıdır.**
- **Zafiyet tarayıcıları ve izleme sistemleri** tarpit portlarına
  bağlanabilir ve eşiği aşabilir. Bunlar `never_block.list`'e
  yazılmalıdır.
- **445/3389'a bağlanamama normaldir.** Bunlar ayrıcalıklı/kullanımda olan
  portlardır; gerçek bir SMB/RDP servisi çalışan bir makinede tarpit o
  portu **alamaz** ve almamalıdır. Bu durum sessizce yutulmaz, hem
  konsola hem denetim kaydına `tarpit.bind_failed` olarak yazılır.

## G) Canlı Doğrulama — GERÇEK Sonuçlar

### G.1 Doğrulanan (kanıtlı)

- **`cargo test --workspace` (native Linux): 126/126** (Faz 1 sonu: 94 —
  **32 yeni test**: `bruteforce` 14, `autoblock` 9, `tarpit` +6,
  `scanner` +2, `pipeline` +1).
- **Wine altında (gerçek Windows PE ikilileri): `chimera-core` 73/73.**
  (Linux'taki 75 ile fark, `#[cfg(not(windows))]` ile işaretli 2 testtir.)
- **Çapraz derleme temiz, 0 uyarı** — yeni `wevtapi` (EventLog) FFI dahil.
- **Çoklu port tarpit GERÇEKTEN bağlandı ve GERÇEKTEN hizmet verdi.**
  Canlı Wine testinde 445 ve 3389'a açılan **8 gerçek TCP bağlantısının
  hepsi** kabul edildi ve sahte SSH bannerının ilk baytı (`b'S'`)
  karşı tarafa ulaştı.
- **Bağımsız çapraz doğrulama:** `scan` çıktısındaki dinleyen-port
  kontrolü (tamamen ayrı bir Win32 API, `GetExtendedTcpTable`) tarpit'in
  açtığı portları **kendi başına gördü**: `6 TCP portu dinleniyor ...
  3389/pid=0, 445/pid=0`. Yani "tarpit dinliyor" iddiası, o iddiayı yapan
  koddan bağımsız bir API tarafından teyit edildi.
- **Eşik mantığı TAM OLARAK BİR KEZ tetiklendi.** 3389'a 7 bağlantı
  açıldı (eşik 5); denetim kaydında **tek bir** `tarpit.autoblock_refused`
  var — 6. ve 7. bağlantılar tekrar tetiklemedi. "Tekrarlı engelleme yok"
  iddiası canlı olarak doğrulandı.
- **`never_block` kendini kilitleme koruması CANLI çalıştı** — bu, Faz
  2'nin en önemli güvenlik özelliğidir ve yalnızca birim testinde değil,
  çalışan bir sistemde gösterildi:
  ```
  "event":"autoblock.refused","detail":"127.0.0.1: loopback (makinenin kendisi)"
  "event":"tarpit.autoblock_refused","detail":"127.0.0.1: 127.0.0.1 otomatik engellenmez (loopback (makinenin kendisi))"
  ```
- **Körlüğün dürüstçe raporlanması CANLI çalıştı** (aşağıya bakınız).
- Denetim zinciri her senaryoda **SAĞLAM**.

### G.2 DOĞRULANAMAYAN — Wine sınırları (dürüstçe)

- **4625 okuması Wine'da doğrulanamadı.** Wine'ın bir Güvenlik olay
  günlüğü kanalı yoktur: `EvtQuery` canlı testte
  `File not found. (0x80070002)` döndü. **Ama bu, tasarımın çalıştığının
  kanıtıdır:** kod bunu sessizce "saldırı yok" diye yorumlamak yerine
  `bruteforce.unavailable [ORTA]` bulgusu üretti ve raporda açıkça
  *"Bu, 'saldiri yok' anlamina GELMEZ; brute-force tespiti su anda
  KORDUR"* yazdı. XPath sorgusunun ve XML ayrıştırıcının doğruluğu
  **14 birim testiyle** gerçek 4625 şemasına karşı doğrulandı; ama
  "gerçek bir Windows'ta gerçek bir saldırı yakalanıyor mu" sorusunun
  cevabı yalnızca **gerçek bir Windows makinesinde** alınabilir.
- **Oto-blokun GERÇEKTEN paket engellediği doğrulanamadı** — Turn 6'da
  belgelenen Wine COM firewall sınırı aynen geçerli. Canlı testte tek
  aday adres loopback olduğu için zaten `never_block` tarafından
  reddedildi; yani engelleme yolunun COM'a kadar giden kısmı bu ortamda
  hiç çalışmadı.
- **`chimera-ipc`'de 1 test Wine'da hâlâ başarısız**
  (`handshake::mismatched_protocol_version_is_rejected`,
  `WSAECONNRESET`) — Faz 1'de belgelendi, bu turda değişmedi, native
  Linux'ta geçiyor.

## H) Test Özeti

| Modül | Yeni test | Neyi kanıtlıyor |
|---|---|---|
| `bruteforce.rs` | 14 | Gerçek 4625 XML ayrıştırma (tek/çift tırnak, XML varlıkları, IPv6), `-`/geçersiz IP reddi, ilgisiz LogonType reddi, eşik altı sessizlik, kayan pencere, IP bağımsızlığı, çift yüzey raporu |
| `autoblock.rs` | 9 | 8 tehlikeli adres sınıfının asla engellenmemesi, operatör listesi (+yorum), defter round-trip, yalnızca süresi dolanın kaldırılması, tekrar engellemede uzatma, tırnak enjeksiyonu |
| `tarpit.rs` | 6 | Salt IP ayıklama, çoklu port bağlama + başarısızlığın dürüst raporlanması, tek bağlantının asla tetiklememesi, eşiğin tam bir kez ateşlenmesi, pencerenin kayması |
| `scanner.rs` | 2 | Körlüğün "güvenlik" olarak raporlanmaması, aktif engellerin yüzeye çıkması |
| `pipeline.rs` | 1 | TTL'li oto-blokun whitelist'te olması AMA korumalı adresi yine de reddetmesi |
| **Toplam** | **32** | 94 → **126** |

---

*Faz 1 için bkz. `06-TURN7-AKTIF-SAVUNMA.md`. Faz 3 (`08-*.md`) ve
Faz 4 (`09-*.md`) bu belgenin devamıdır.*
