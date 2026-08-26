# CHIMERA EDR — Turn 7 / Faz 1: Aktif Fidye Yazılımı Devre Kesici

Bu tur, "proje gerçek ve test edilmiş ama **PASİF**: tuzağa dokunulması
yalnızca loglanıyor" tespitine karşılık verir. Faz 1'in tek hedefi vardı:
tuzağı **aktif bir devre kesiciye** çevirmek — ama projenin baştan beri
bağlı olduğu kuralları bozmadan (gerçek API, gerçek test, sahte bileşen
yok, whitelist dışı kalıcı otomatik aksiyon yok).

## A) Önceki Seviye (Turn 6 sonu)

| Yetenek | Turn 6 durumu |
|---|---|
| Tuzak dosyaya dokunma | Tespit edilir, **yalnızca loglanır** |
| Dokunan sürecin kimliği | **Bilinmiyor** — PID hiç tespit edilmiyor |
| Şifreleyen sürece müdahale | **Yok** |
| Tuzağa dokunmadan şifreleyen saldırgan | **Tamamen görünmez** |
| Tuzak kapsamı | Tek düz dizin, 6 dosya |
| İnsan onayı kuyruğu | Yok |
| Test sayısı | 67 |

Yani: bir fidye yazılımı tuzak dosyaya dokunduğunda CHIMERA bunu doğru
şekilde *fark ediyor* ama şifrelemeyi durdurmak için hiçbir şey
*yapmıyordu*.

## B) Yapılan Değişiklikler

| # | Değişiklik | Dosya |
|---|---|---|
| 1 | Tuzağa yazan sürecin PID'i **resmî Restart Manager API'si** ile tespit ediliyor (`RmStartSession`/`RmRegisterResources`/`RmGetList`/`RmEndSession`) | `decoy.rs` (yeni `writer_of`) |
| 2 | **Devre kesici**: askıya alma + ağ izolasyonu + hash-zincirli kayıt + insan onayı kuyruğu | `circuit_breaker.rs` (**yeni**) |
| 3 | **Entropi/hız heuristiği**: tuzağa hiç dokunulmasa bile toplu şifrelemeyi yakalar | `heuristic.rs` (**yeni**) |
| 4 | Alfabetik olarak **önce görünen** tuzak seti + simüle edilmiş kullanıcı klasörleri | `decoy.rs` (`EARLY_DECOY_NAMES`, `USER_FOLDERS`) |
| 5 | Askıya alınan süreçler boru hattında **"insan onayı bekliyor"** bulgusu olarak yüzeye çıkıyor | `scanner.rs`, `pipeline.rs` |
| 6 | 3 yeni ayrıcalıklı IPC mesajı + 1 yanıt türü | `chimera-ipc/src/protocol.rs` |
| 7 | 3 yeni Shamir(2,3) korumalı alt komut | `chimera-admin/src/main.rs` |
| 8 | Salt-okunur/dizin olayları artık alarma dönüşmüyor (canlı testte yakalanan gürültü) | `decoy.rs` |
| 9 | `.cargo/config.toml` yeniden oluşturuldu (arşivde yoktu; `release-check.py` onsuz geçmiyordu) | `.cargo/config.toml` |

### B.1 Devre kesicinin dört adımı

`circuit_breaker::trip()` tetiklendiğinde sırayla:

1. **Askıya alma** (`NtSuspendProcess`) — SONLANDIRMA DEĞİL. Askıya alma
   geri alınabilir; yanlış bir tespitte meşru sürecin kaydedilmemiş
   verisi kaybolmaz. Aynı zamanda şifrelemeyi *o anda* durdurur —
   "alarm üret, operatör 10 dakika sonra baksın" yaklaşımının aksine.
2. **Ağ izolasyonu** — yalnızca **o sürecin** o anki uzak adresleri
   (`GetExtendedTcpTable` + `TCP_TABLE_OWNER_PID_ALL`) `firewall.rs`
   üzerinden bloklanır. Loopback ve `0.0.0.0` **asla** bloklanmaz
   (makinenin kendi iç haberleşmesini, CHIMERA'nın kendi IPC'sini
   kırardı).
3. **Kanıta-dayanıklı kayıt** — her adım, başarılı VE başarısız, mevcut
   hash-zincirli denetim kaydına yazılır; ayrıca tek satırlık bir
   `circuit_breaker.outcome` özeti düşer.
4. **İnsan onayı kuyruğu** — `state/suspended.list`,
   `AWAITING_HUMAN_APPROVAL` durumuyla.

### B.2 `NtSuspendProcess` hakkında dürüst not

`NtSuspendProcess`, Microsoft tarafından **resmen belgelenmemiş** bir
`ntdll.dll` dışa aktarımıdır (Windows XP'den beri kararlıdır ve
Sysinternals `pssuspend` dahil yaygın araçlar kullanır). Bu yüzden:

- Adres `GetProcAddress` ile çözülür; **bulunamazsa sahte bir "başarılı"
  DÖNÜLMEZ**, tamamen belgelenmiş bir yedeğe düşülür:
  `CreateToolhelp32Snapshot` + `OpenThread` + `SuspendThread` ile sürecin
  tüm thread'leri tek tek dondurulur.
- Yedek yolun **atomik olmadığı** kodda ve dönüş mesajında açıkça yazar:
  biz thread'leri dondururken süreç yeni bir thread yaratabilir.
  `NtSuspendProcess` bu yarışı çekirdek seviyesinde kapatır; bu yüzden
  birincil yol odur.
- Hangi yolun kullanıldığı denetim kaydına **yazılır**.

**Kernel driver iddiası yoktur.** Her iki yol da Ring-3'tür.

### B.3 Kendini vurma koruması

`is_protected_pid()` — devre kesicinin ilk adımı. Şunlara **asla**
dokunulmaz: PID 0 (System Idle), PID 4 (System), `chimera-core`'un kendi
PID'i, ve `runtime/sentinel.pid`'deki eş bekçi. Tek bir yanlış pozitifin
makineyi kurtarılamaz hale getirmesi bu şekilde engellenir. Kontrol
platformdan bağımsız katmandadır, yani **Linux'ta da test edilir**
(`own_process_kernel_and_sentinel_pids_are_protected`,
`tripping_a_protected_pid_takes_no_action_at_all`,
`terminate_refuses_protected_pids_even_when_queued`).

### B.4 Değişmez kuralın koda gömülmesi

| Aksiyon | Geri alınabilir mi | Otomatik çalışır mı | Kapı |
|---|---|---|---|
| Süreci askıya alma | **Evet** | Evet | — |
| Sürecin IP'lerini bloklama | **Evet** (`resume` geri alır) | Evet | — |
| Süreci **sonlandırma** | **HAYIR** | **ASLA** | Shamir(2,3) |

Bu, tek bir yerde değil **üç** yerde birden zorlanır:
`pipeline::is_whitelisted()` `TerminateSuspendedProcess`'i reddeder;
`pipeline::execute()` Validator atlansa bile `panic!` yerine açık bir
`REDDEDILDI` hatası döner; `handle_request` `unlocked()` kontrolünden
geçmeden `Denied` döner. Üçü de testlidir.

## C) Gerçekten Kazanılan Direnç

| Senaryo | Turn 6 | Turn 7 Faz 1 |
|---|---|---|
| Fidye yazılımı tuzak dosyaya yazıyor | Log satırı | Süreç **donduruluyor**, C2 bağlantıları bloklanıyor, kuyruğa düşüyor |
| Fidye yazılımı tuzağa dokunmadan 12+ dosyayı şifreliyor | **Görünmez** | Entropi/hız heuristiği devre kesiciyi tetikliyor |
| Şifreleme kullanıcı klasöründen başlıyor | İzlenmiyor | `Belgeler`/`Masaustu`/`Resimler` tuzakları izleniyor |
| Alfabetik sırayla şifreleyen aile | 6. dosyada yakalanırdı | `!`/`0`/`A` tuzakları sayesinde **ilk dosyalarda** |
| Operatör yanlış pozitif kararı veriyor | — | `resume-process`: süreç devam eder, IP blokları **geri alınır**, heuristik sayacı sıfırlanır |

## D) Kalan Zayıflıklar (Dürüst)

- **Restart Manager, dosyayı O ANDA açık tutan süreci döner.** Bir fidye
  yazılımı `open → write → close` döngüsünü çok hızlı yaparsa, `notify`
  olayını aldığımız anda tanıtıcı kapanmış olabilir ve PID `None` döner.
  Bu durumda devre kesici **hedeflenemez** — yalnızca
  `decoy.touch_unattributed` alarmı düşer. Kod bunu "engelledik" diye
  YAZMAZ. Bu boşluğu kapatması beklenen şey Faz 4'teki ETW tüketicisidir.
- **Entropi heuristiği yanlış pozitif üretebilir.** Yedekleme aracının
  yazdığı `.zip`/`.7z`, video dışa aktarımı veya meşru disk şifreleme
  aynı istatistiği üretir. Azaltma: (a) tek dosya değil, kısa pencerede
  **12 farklı** dosya gerekir, (b) aynı dosyaya tekrar yazma sayılmaz,
  (c) sonuç sonlandırma değil **geri alınabilir** askıya almadır.
  Yine de bu, Faz 1'in en yüksek FP riskidir.
- **Yedek thread-bazlı askıya alma atomik değildir** (bkz. §B.2).
- **`terminate` ağ bloklarını kaldırmaz** — bilinçli. Operatör bunları
  `unblock-ip` ile ayrıca, bilinçli bir kararla kaldırır.
- **Askıya alınmış bir süreç, CHIMERA çökerse askıda kalır.** Sentinel
  core'u yeniden başlatır ama askıya alınmış süreçler otomatik
  çözülmez — kuyruk diskte kalıcıdır, operatör `list-suspended` ile
  görür. Bu bilinçli: otomatik çözme, saldırganın core'u çökertip
  şifrelemeye devam etmesini sağlardı.
- **Bilinçli OS/yönetici sonlandırmasına direnç YOK** — projenin kurucu
  kuralı, değişmedi.

## E) Performans / Maliyet

- **`writer_of` çağrısı:** yalnızca Modify/Create/Remove olaylarında ve
  yalnızca dosyalar için. Wine'da ölçüldü: `RmStartSession` ~12 µs,
  `RmRegisterResources` ~1 µs, `RmGetList` ~0.4 µs. Gerçek Windows'ta
  daha yüksek olması beklenir (gerçek bir servisle konuşur) ama tuzak
  dizini birkaç düzine dosyadır, sürekli bir yük değildir.
- **Entropi örneklemesi:** dosya başına en fazla 4 KiB okunur (tüm dosya
  DEĞİL) — yüzlerce dosyanın saniyeler içinde değiştiği bir senaryoda
  izleyicinin kendisinin darboğaz olmaması için.
- **Heuristik sayaç:** PID başına kayan pencere, `HashMap` + `VecDeque`;
  bellek maliyeti pencere içindeki farklı dosya sayısıyla sınırlıdır.
- **Ölçülebilir bir regresyon gözlenmedi:** Wine altında tam test paketi
  Turn 6'daki ile aynı mertebede (~4 s) tamamlanıyor.

## F) Yanlış-Pozitif / Kararlılık Riskleri

- **En yüksek risk entropi heuristiğidir** (bkz. §D). Varsayılan eşikler
  (60 sn / 12 farklı dosya / 7.5 bit) `heuristic.rs`'te sabit olarak
  durur ve gerçek bir dağıtımda ortama göre ayarlanmalıdır.
- **Askıya alma, meşru bir süreçte veri kaybına yol açmaz** ama uzun
  süreli askı, o sürecin ağ eşlerinde zaman aşımına yol açabilir.
- **`is_protected_pid` yalnızca 4 PID'i korur.** Kritik bir üçüncü taraf
  servisi (yedekleme ajanı, veritabanı) yanlışlıkla yakalanırsa askıya
  alınır. Üretimde bu listenin operatör tarafından genişletilebilir
  olması gerekir — **bu sürümde yoktur**, bilinçli bir eksiktir.

## G) Canlı Doğrulama — GERÇEK Sonuçlar

### G.1 Doğrulanan (kanıtlı)

- **`cargo test --workspace` (native Linux): 94/94 geçti** (Turn 6 sonu:
  67 — **27 yeni test**).
- **`x86_64-pc-windows-gnu` çapraz derlemesi: İLK denemede temiz**, tüm
  yeni FFI (RestartManager / ToolHelp / Threading / LibraryLoader /
  IpHelper) dahil, **sıfır uyarı**. API imzaları tahmin edilmedi,
  üretilmiş `windows` crate kaynağından okundu.
- **YENİ: Windows ikilileri artık Wine altında GERÇEKTEN test ediliyor.**
  `.cargo/config.toml`'a `runner = "wine"` eklendi; `cargo test --target
  x86_64-pc-windows-gnu` derlenmiş `.exe` test ikililerini Wine 9.0
  altında çalıştırır. Sonuç: **`chimera-core` 43/43 geçti.**
- **Tam yaşam döngüsü Wine'da canlı çalıştı** (`scripts/wine-faz1-test.sh`):
  identity → trust → provision → serve → yeni komutlar → tuzağa dokunma →
  scan → verify-audit → temiz durdurma. Denetim zinciri **30 kayıtta
  SAĞLAM**, 0 zombie süreç.
- **Shamir kapısı yeni komutlarda GERÇEKTEN çalışıyor:** paysız
  `list-suspended` → çıkış kodu 1 (KASA KAPALI); 2 geçerli payla → doğru
  yanıt. Kuyrukta olmayan bir PID için `resume-process` ve
  `terminate-process` **reddedildi**.
- **Yeni tuzaklar diskte gerçekten oluştu** ve alfabetik sıralamada
  gerçekten önde: canlı `ls | sort | head -3` çıktısı
  `!!!_ACIL_...`, `!_banka_...`, `0001_personel_...` verdi.
- **Aldatma kaydı artık `pid` ve `app` alanlarını taşıyor.**

### G.2 Bu turda ÇÖZÜLEN iki gerçek hata (canlı testte yakalandı)

1. **Özyinelemeli izleme Wine'da sessizce çalışmıyordu.**
   İlk uygulama `RecursiveMode::Recursive` kullanıyordu. Wine altında alt
   klasör tuzakları **hiç olay üretmedi**. Yalnızca `notify` kullanan
   minimal bir tekrar üretimle doğrulandı: kök dizin olayları geliyor,
   alt dizin olayları gelmiyor — Wine'ın `ReadDirectoryChangesW`
   uygulaması `bWatchSubtree` bayrağını onurlandırmıyor. Bu, alt klasör
   tuzaklarının **sessizce kör** olması demekti.
   **Düzeltme:** tuzak dizinlerini biz oluşturduğumuz için hangilerinin
   izleneceğini zaten biliyoruz — kök ve her kullanıcı klasörü **ayrı
   ayrı, özyinelemesiz** izleniyor. Bu, platformun subtree desteğine
   hiç bağlı değil ve üç ortamda da (Linux/Wine/gerçek Windows) çalışır.
2. **Salt-okunur olaylar alarm üretiyordu.** Özyinelemeli izleme, alt
   dizinlere izleyici kaydederken `Access(Open(Any))` gibi olaylar da
   üretiyordu ve bunlar "tuzağa dokunuldu" diye kaydediliyordu. Canlı
   çalıştırmada gerçekten görüldü; artık yalnızca Modify/Create/Remove
   ve yalnızca **dosya** olayları alarma dönüşüyor.

### G.3 DOĞRULANAMAYAN — Wine sınırları (dürüstçe)

- **Restart Manager PID tespiti Wine'da doğrulanamadı.** Wine'ın
  `rstrtmgr.dll`'i bir **stub**'dır: bu ölçüldü — `RmStartSession`,
  `RmRegisterResources`, `RmGetList` hepsi `0` (başarılı) dönüyor,
  mikrosaniyeler sürüyor, ama dosyayı **bizzat açık tutan sürecin
  kendisini bile** bildirmiyor (`needed=0`). Yani canlı testte tüm
  tuzak olayları `pid:null` düştü ve devre kesici hiç tetiklenemedi.
  **Sonuç:** çağrı dizisi ve tip imzaları gerçek Windows bindings'e karşı
  doğrulandı ve Wine altında çökmeden çalıştı; ama "gerçek Windows'ta
  PID GERÇEKTEN bulunuyor mu" sorusunun cevabı yalnızca **gerçek bir
  Windows makinesinde** alınabilir.
- **`NtSuspendProcess` / thread-bazlı askıya alma Wine'da doğrulanmadı.**
  Devre kesici hiç tetiklenemediği için (yukarıdaki sebeple) bu kod yolu
  canlı çalışmadı. Birim testleri korumalı-PID mantığını ve kuyruk
  durumunu doğruluyor; ama gerçek bir sürecin GERÇEKTEN donduğu yalnızca
  gerçek Windows'ta gösterilebilir.
- **Firewall bloklama hâlâ doğrulanamıyor** — Turn 6'da belgelenen Wine
  COM sınırı aynen geçerli (`Rules().Add()` S_OK diyor ama `Item()`
  bulamıyor).
- **`chimera-ipc` altında 1 test Wine'da başarısız:**
  `handshake::mismatched_protocol_version_is_rejected`, Wine'ın loopback
  soketinde temiz EOF yerine `WSAECONNRESET (10054)` dönmesi yüzünden.
  Bu test **bu turda değiştirilmedi** (Turn 5/6 kodu) ve native Linux'ta
  geçiyor — bir Wine soket semantiği farkıdır, yeni bir regresyon değil.
  Wine altında workspace toplamı: **93/94**.
- **Wine'da bulunan üçüncü bir sınır (test hijyeni):** bir **alt dizini**
  aktif izlenen ağacı `remove_dir_all` ile silmek Wine'da sonsuza kadar
  asılıyor (minimal tekrar üretimle doğrulandı: kök-yalnız izlemede 3 ms,
  alt dizin izlenirken hiç dönmüyor). Testler artık izleyiciyi
  silmeden önce açıkça düşürüyor — zaten doğru kaynak yönetimi. Üretim
  kodu tuzak dizinini çalışırken silmediği için bu bir ürün hatası
  değildir.

## H) Test Özeti

| Modül | Yeni test | Neyi kanıtlıyor |
|---|---|---|
| `heuristic.rs` | 9 | Entropi matematiği, küçük örnek reddi, kayan pencere, tek-dosya FP koruması, PID izolasyonu |
| `circuit_breaker.rs` | 8 | Korumalı PID'ler, kuyruk round-trip, tırnak enjeksiyonu, kuyrukta olmayan PID reddi |
| `decoy.rs` | 4 | Alt klasör izleme, alfabetik sıra iddiası, gürültü filtresi, entropi örneği boyutu |
| `scanner.rs` | 1 | Kuyruğun whitelist DIŞI kritik bulgu olarak yüzeye çıkması |
| `pipeline.rs` | 2 | Sonlandırmanın asla whitelist'e girmemesi; uçtan uca "insan onayı" akışı |
| `protocol.rs` | 2 | PID'in kabloda bozulmaması, eksik alanın panic yerine hata dönmesi |
| **Toplam** | **27** | 67 → **94** |

---

*Faz 2 (`07-*.md`), Faz 3 (`08-*.md`) ve Faz 4 (`09-*.md`) bu belgenin
devamıdır. `05-TURN6-ULTRA-GUARD.md`'deki iki bilinçli kapsam kararı
(kernel driver yok, otonom AI yaması yok) bu turda da aynen geçerlidir.*
