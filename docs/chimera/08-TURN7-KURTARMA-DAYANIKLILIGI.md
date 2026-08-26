# CHIMERA EDR — Turn 7 / Faz 3: Kurtarma Dayanıklılığı

Faz 1 şifrelemeyi durdurdu, Faz 2 saldırganın makineye ulaşmasını
zorlaştırdı. Faz 3, **her ikisi de başarısız olursa** ne kaldığını ele
alır: fidye yazılımının kurbanın kurtulma yollarını imha etmesini yakalamak,
ve geri dönülecek **doğrulanmış** bir noktanın gerçekten var olmasını
sağlamak.

## A) Önceki Seviye (Faz 2 sonu)

| Yetenek | Faz 2 durumu |
|---|---|
| `vssadmin delete shadows` benzeri komutlar | **Hiç izlenmiyor** |
| Yedekleme | **Hiç yok** — `state/` dizini korumasız |
| Yedeğin bozulup bozulmadığı | — |
| Test sayısı | 126 |

Bu, gerçek bir boşluktu: CHIMERA kendi kimlik anahtarlarını, güven
listelerini ve mühürlü kasasını `state/` altında tutuyordu ve bunların
**hiçbir yedeği yoktu**.

## B) Yapılan Değişiklikler

| # | Değişiklik | Dosya |
|---|---|---|
| 1 | **Yedek imha komutlarının tespiti** (4688 + belirteç düzeyinde eşleşme) | `cmdguard.rs` (**yeni**) |
| 2 | Yakalanan komutun sürecinde **devre kesici tetiklenmesi** | `pipeline.rs` (`guard_recovery_paths`) |
| 3 | **İmzalı, Merkle tabanlı periyodik anlık görüntüler** | `backup.rs` (**yeni**) |
| 4 | Arka plan döngüsünde **yedek al + DOĞRULA + buda** | `pipeline.rs` (`backup_cycle`) |
| 5 | Yedeğin yokluğu **KRİTİK**, bayatlığı **YÜKSEK** bulgu | `scanner.rs` |
| 6 | 4688 okunamıyorsa **körlük** olarak raporlama | `scanner.rs` |
| 7 | 3 yeni Shamir(2,3) komutu: `backup-now`, `list-backups`, `verify-backup` | `chimera-admin`, `chimera-ipc` |
| 8 | Kullanıcı verisi kapsama dahil (`CHIMERA_BACKUP_INCLUDE`) | `backup.rs` |

### B.1 Neden "süreç başlamadan engelleme" YOK — ve yerine ne var

Bir süreci Ring-3'ten **başlamadan önce** engellemenin iki yolu vardır:

1. **Kernel driver** (`PsSetCreateProcessNotifyRoutineEx`) — projenin
   baştan beri kapsam dışı bıraktığı imzalı sürücü meselesi
   (bkz. `05-TURN6-ULTRA-GUARD.md` §0.1).
2. **Image File Execution Options (IFEO) `Debugger` kaydı** — teknik
   olarak mümkündür ve tamamen belgelenmiştir. Ama `vssadmin.exe`'yi
   kalıcı olarak ele geçirmek, **meşru yönetimi de kırar** (yedekleme
   ürünleri ve sistem yöneticileri `vssadmin` çağıramaz hâle gelir) ve
   kalıcı bir sistem değişikliğidir — yani `remediate.rs`'in dört şartını
   KARŞILAMAZ, otomatik uygulanamaz.

**Yerine ne yapılıyor:** komut 4688 üzerinden yakalanır, en yüksek
öncelikli alarm üretilir ve `circuit_breaker` o PID'de tetiklenir — süreç
**askıya alınır**. `vssadmin delete shadows /all`, büyük bir birimde
saniyeler sürer; askıya alınan bir süreç silmeye devam edemez.
**Yarışı her zaman kazanacağımız İDDİA EDİLMEZ**; kazanma şansının
gerçek olduğu iddia edilir.

### B.2 Belirteç düzeyinde eşleşme — GERÇEK bir yanlış pozitifin düzeltilmesi

İlk uygulama komut satırında `vssadmin`, `delete`, `shadow` alt-dizelerini
arıyordu. Bu, modülün **kendi yanlış-pozitif testi tarafından yakalandı**:

```
notepad.exe C:\notlar\vssadmin-delete-shadows-notlarim.txt
```

Bu komut üç kelimeyi de içerir ama tamamen zararsızdır. Alt-dize
eşleşmesi bunu "gölge kopya silme" sanıp **meşru bir Notepad oturumunu
askıya alırdı**. Düzeltme: komut satırı belirteçlere bölünür ve program
adı **yol önekinden arındırılmış temel ad** olarak karşılaştırılır
(`exe_token_is`). Böylece `C:\Windows\System32\vssadmin.exe` eşleşir,
`...\vssadmin-delete-shadows-notlarim.txt` eşleşmez.

Aynı disiplin şunları da ayırır: `vssadmin list shadows` (meşru, salt
okunur) eşleşmez; `bcdedit ... recoveryenabled Yes` (onarım) eşleşmez,
`... recoveryenabled No` eşleşir; `wevtutil qe`/`gl` (okuma) eşleşmez,
`wevtutil cl` (temizleme) eşleşir.

### B.3 "Immutable" ve "offsite" sözcüklerinin DÜRÜST karşılığı

Bu iki sözcük pazarlama metinlerinde çok kolay kullanılır. Burada tam
olarak ne sağlanıp ne **sağlanmadığı** yazılıdır:

| İddia | GERÇEKTE olan | GERÇEKTE OLMAYAN |
|---|---|---|
| Değiştirilemez | Anlık görüntü dizini zaman damgalıdır ve **asla üzerine yazılmaz**; dosyalar salt-okunur işaretlenir; her şey **ML-DSA-87 ile imzalıdır** | Yerel yönetici salt-okunur bayrağını kaldırıp **silebilir**. Gerçek değiştirilemezlik WORM/object-lock depolama gerektirir — bu ortamda YOK. Garanti **"silinemez" değil, "sessizce bozulamaz"dır.** |
| Offsite | Hedef dizin `CHIMERA_BACKUP_DIR` ile operatör tarafından verilir; bir ağ paylaşımı/çıkarılabilir disk bağlama noktası olabilir | Bu modül **hiçbir ağ protokolü konuşmaz** — S3/SFTP/rsync istemcisi YAZILMAMIŞTIR. "Offsite" ancak operatörün verdiği yol gerçekten başka bir makinedeyse gerçektir; kod bunu doğrulayamaz ve doğruladığını iddia etmez. |

### B.4 Neden imza şart — ve neyi durdurduğu

Doğrulama üç aşamalıdır ve **sıra önemlidir**:

1. Manifestonun **kendi içinde** tutarlılığı (satırlar ↔ ilan edilen kök).
2. O kökün **core'un ML-DSA-87 anahtarıyla imzalanmış** olması.
3. Diskteki dosyaların manifestodaki Merkle kökleriyle uyuşması.

Bu sıra sayesinde: saldırgan bir dosyayı bozar → 3'te yakalanır.
Saldırgan hem dosyayı hem manifestoyu tutarlı biçimde yeniden yazar →
**2'de yakalanır**, çünkü core'un özel anahtarı yoktur. Saldırgan kendi
anahtar çiftini üretip yedeği baştan imzalar → yine 2'de yakalanır,
çünkü doğrulama core'un **bilinen** açık anahtarıyla yapılır. Üç senaryo
da testlidir.

## C) Gerçekten Kazanılan Direnç

| Senaryo | Faz 2 | Faz 3 |
|---|---|---|
| `vssadmin delete shadows /all /quiet` | Görünmez | Yakalanır → süreç **askıya alınır** |
| `wbadmin delete catalog` / `bcdedit recoveryenabled no` | Görünmez | Aynı |
| `wevtutil cl Security` (iz silme) | Görünmez | Yakalanır (CHIMERA'nın kendi tespit kaynağını korur) |
| `state/` dizini şifrelenir/silinir | **Kurtarılamaz** | İmzalı anlık görüntüden geri dönülebilir |
| Kullanıcı belgeleri | Yedeklenmiyor | `CHIMERA_BACKUP_INCLUDE` ile kapsama girer |
| Yedek sessizce bozulur | — | **Doğrulama yakalar**, çıkış kodu 1 |
| Yedek hiç yok | Fark edilmez | Taramada **KRİTİK** bulgu |

## D) Kalan Zayıflıklar (Dürüst)

- **4688 iki ayrı denetim politikasına bağlıdır:** "Audit Process
  Creation" **ve** "Include command line in process creation events".
  İkincisi Windows'ta **varsayılan olarak KAPALIDIR** — yani bu özellik,
  operatör o politikayı açmadıkça komut satırını göremez. Bu, Faz 3'ün
  en önemli operasyonel ön koşuludur ve kod bunu açamaz (kalıcı sistem
  değişikliği olurdu).
- **Tespit her zaman OLAYDAN SONRADIR.** 4688 süreç *oluşturulduktan*
  sonra yazılır ve olay günlüğüne yazılması ile bizim okumamız arasında
  gecikme vardır. Arka plan turu 30 dakikada bir çalıştığından, en kötü
  durumda komut yarım saat önce çalışmış olabilir — o zaman askıya alma
  hiçbir şeyi kurtarmaz. **`ScanNow` (elle tarama) bu gecikmeyi kısaltır
  ama sıfırlamaz.** Gerçek zamanlı tespit, Faz 4'teki ETW tüketicisinin
  konusudur.
- **Yedek şifrelenmiyor.** Anlık görüntüler bütünlük açısından korunur
  (imza + Merkle) ama **gizlilik açısından korunmaz**: `state/` içindeki
  dosyalar zaten kendi başlarına mühürlüdür (`identity.sealed`,
  `vault.sealed`), ama `CHIMERA_BACKUP_INCLUDE` ile eklenen kullanıcı
  verisi **düz metin olarak kopyalanır**. Yedek dizinine erişebilen biri
  o veriyi okuyabilir. Bu belgelenmiş bir eksiktir.
- **Geri yükleme (restore) YOK.** Bu sürüm yedek *alır* ve *doğrular*;
  otomatik geri yükleme uygulanmamıştır. Canlı `state/` dizininin üzerine
  yazmak, yanlış bir kararla sistemi bozabilecek yıkıcı bir işlemdir ve
  hak ettiği tasarımı ayrı bir turda almalıdır. Şu an operatör,
  doğrulanmış bir anlık görüntüden **elle** kopyalar.
- **Aynı makinede duran bir yedek, fidye yazılımına karşı yarım
  korumadır.** Varsayılan hedef `<root>/backups`'tır; makineyi ele
  geçiren bir saldırgan bunu da şifreleyebilir. `CHIMERA_BACKUP_DIR`
  gerçekten başka bir makineye bağlı bir yola ayarlanmadıkça bu risk
  DEVAM EDER.
- **Sembolik bağlantılar izlenmez** (bilinçli — yedekleyicinin `C:\`'ye
  yönlendirilip diski doldurmasını önler), ama bu, bir sembolik bağlantı
  arkasındaki gerçek verinin yedeklenmeyeceği anlamına da gelir.

## E) Performans / Maliyet

- **Yedek maliyeti dosya boyutuyla doğrusaldır** (BLAKE3 + kopyalama).
  `state/` birkaç yüz KB'dır; kullanıcı verisi eklenirse maliyet o
  verinin boyutuna bağlıdır. Merkle ağacı `build_tree_from_file` ile
  **akış hâlinde** kurulur — 20 GB'lık bir dosya için bile RAM patlamaz.
- **Varsayılan aralık 6 saat, saklanan görüntü 8** → yaklaşık 2 günlük
  geçmiş. Budama, silinen her görüntüyü denetim kaydına yazar.
- **Doğrulama her turda EN YENİ görüntü için çalışır** (hepsi için
  değil) — aksi halde maliyet görüntü sayısıyla çarpılırdı.
- **4688 sorgusu** 4000 olayla sınırlıdır ve XPath ile günlük motorunda
  filtrelenir.

## F) Yanlış-Pozitif / Kararlılık Riskleri

- **Yedekleme ürünleri `vssadmin create shadow` çalıştırır** — bu
  eşleşmez (testlidir). Ama bir yedekleme ürünü **eski gölge kopyaları
  temizlemek için** `vssadmin delete shadows /oldest` çalıştırabilir ve
  bu **eşleşir**. Sonuç: meşru yedekleme süreci askıya alınır.
  Azaltma: askıya alma geri alınabilir (`resume-process`) ve
  `is_protected_pid` genişletilebilir olmalıdır (bkz. `06-*.md` §F).
  **Bu, Faz 3'ün en olası yanlış pozitifidir.**
- **Disk dolması:** 8 anlık görüntü × kullanıcı veri boyutu. Büyük bir
  kapsam verilirse disk dolabilir; `DEFAULT_KEEP` ortama göre
  ayarlanmalıdır.
- **Salt-okunur işaretleme**, bazı yedekleme/senkronizasyon araçlarının
  yedek dizinini işlemesini zorlaştırabilir.

## G) Canlı Doğrulama — GERÇEK Sonuçlar

### G.1 Doğrulanan (kanıtlı)

- **`cargo test --workspace` (native Linux): 161/161** (Faz 2 sonu: 126 —
  **35 yeni test**).
- **Wine altında (gerçek Windows PE ikilileri): 157/157 geçti.**
  Linux'taki 161 ile fark, `#[cfg(not(windows))]` ile işaretli **tam 4**
  testtir (2× "desteklenmiyor" hatası, 2× körlük raporu).
- **Çapraz derleme temiz, 0 uyarı.**
- **Arka plan yedek döngüsü CANLI çalıştı:** `serve` başladıktan sonra
  operatör hiçbir şey yapmadan `snapshot-1787760353` oluştu
  (`backup.cycle_ok`), ardından elle `backup-now` ikinci görüntüyü aldı.
- **Kullanıcı verisi GERÇEKTEN kapsama girdi:**
  `veri/kullanici0/sozlesme.docx` ve `veri/kullanici0/tablo.xlsx`
  yedekte fiziksel olarak bulundu.
- **Manifesto gerçek BLAKE3 Merkle kökleri taşıyor** (14 dosya, her biri
  ayrı kök) ve `manifest.sig` gerçek bir ML-DSA-87 imzası içeriyor.
- **`verify-backup` → `SAGLAM: imza gecerli, 14 dosyanin tamami
  manifestoyla uyusuyor`, çıkış kodu 0.**
- **ASIL TEST — sessiz bozulma CANLI yakalandı.** Yedekteki kullanıcı
  belgesi diskte doğrudan değiştirildi:
  ```
  snapshot-...: BOZULMA TESPIT EDILDI: imza gecerli ama 1 dosya
  manifestoyla UYUSMUYOR: kullanici0/sozlesme.docx
    cikis: 1
  ```
  Bozulan dosya **adıyla** raporlandı ve komut **sıfır olmayan çıkış
  koduyla** döndü — yani bir zamanlanmış görev/CI bunu otomatik
  yakalayabilir.
- **Denetim kaydı**: `backup.verify_ok` ×3, `backup.verify_FAILED` ×1,
  `backup.snapshot` ×2. Zincir **SAĞLAM** (23 kayıt).
- **Körlük dürüstçe raporlandı** (aşağıya bakınız).
- **Faz 1/2 regresyonu yok:** aynı çalıştırmada tarpit, devre kesici ve
  Shamir kapısı davranışları değişmedi.

### G.2 DOĞRULANAMAYAN — Wine sınırları (dürüstçe)

- **4688 okuması Wine'da doğrulanamadı** — 4625 ile aynı sebep (Wine'ın
  Güvenlik kanalı yok). Canlı taramada beklendiği gibi:
  ```
  [ORTA] cmdguard.unavailable -- Surec olusturma (4688) kayitlari okunamadi
  ```
  ve detayda *"Bu, 'boyle bir komut calismadi' anlamina GELMEZ"*.
  Yani **tasarım doğru çalıştı** ama gerçek bir `vssadmin delete shadows`
  komutunun yakalandığı **gösterilemedi**. Ayrıştırıcı ve sınıflandırıcı
  **20 birim testiyle** gerçek 4688 şemasına ve gerçek fidye yazılımı
  komutlarına karşı doğrulandı.
- **Devre kesicinin yıkıcı komut yolundaki askıya alması doğrulanamadı** —
  komut hiç yakalanamadığı için bu kod yolu canlı çalışmadı (Faz 1'deki
  `NtSuspendProcess` sınırının aynısı).
- **`chimera-ipc` handshake testi hakkında düzeltme:** Faz 1/2'de bu
  testin Wine altında `WSAECONNRESET` ile başarısız olduğunu
  belgelemiştik. Faz 3 çalıştırmasında **aynı kod değişmeden geçti**
  (22/22). Yani bu başarısızlık **deterministik değil, aralıklı bir
  zamanlama duyarlılığıdır** — önceki iki belgede "başarısız" olarak
  yazılması, tek bir gözleme dayanıyordu. Native Linux'ta her zaman
  geçiyor.

## H) Test Özeti

| Modül | Yeni test | Neyi kanıtlıyor |
|---|---|---|
| `cmdguard.rs` | 15 | 4 yıkıcı komut ailesinin yakalanması, **18 meşru komutun yakalanmaması**, dosya-adı yanlış pozitifi, `recoveryenabled Yes/No` ayrımı, 4688 onaltılık PID çözümü, XML varlıkları |
| `backup.rs` | 14 | Uçtan uca imzalı görüntü, tek bit bozulmanın yakalanması, manifesto yeniden yazımının imzayla yakalanması, yabancı anahtar reddi, üzerine yazmama, sıradan bağımsız kök, budama |
| `pipeline.rs` | 2 | Yedek döngüsünün alma+doğrulama akışı, bozuk yedeğin sessizce geçilmemesi |
| `scanner.rs` | 2 | Yedek yokluğunun KRİTİK raporlanması, 4688 körlüğü |
| `protocol.rs` | (mevcut testlere 3 mesaj eklendi) | Yeni yedek mesajlarının round-trip'i |
| **Toplam** | **35** | 126 → **161** |

---

*Faz 1: `06-TURN7-AKTIF-SAVUNMA.md`, Faz 2: `07-TURN7-AG-SERTLESTIRME.md`.
Faz 4 (`09-*.md`) bu belgenin devamıdır.*
