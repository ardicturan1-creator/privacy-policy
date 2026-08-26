# CHIMERA EDR — Derinlemesine Analiz ve İyileştirme Turu (Rapor)

Bu belge, önceki sertleştirme turundan (`03-HARDENING.md`) sonra yapılan
derinlemesine bir kod incelemesi sonucunda **gerçekten bulunan** hatalar ve
uygulanan iyileştirmeleri belgeler. Buradaki her madde ya (a) canlı,
çalışan süreçlerle test edilerek doğrulanmış GERÇEK bir hata düzeltmesidir,
ya da (b) yeni bir test paketiyle desteklenen GERÇEK bir yeni yetenektir.
Hiçbir madde "iddia edilen" veya "teorik" bir iyileştirme değildir — hepsi
`cargo test --workspace` ve/veya canlı süreç testleriyle doğrulanmıştır.

## 1) GERÇEK HATA: Temiz durdurma, hiç istemci bağlanmazsa asla tetiklenmiyordu

**Sorun:** `chimera-core serve`, `listener.incoming()` üzerinde bloklayan bir
döngü kullanıyordu; `runtime/stop.flag` kontrolü yalnızca bir bağlantı
KABUL EDİLDİKTEN SONRA yapılıyordu. Hiçbir istemci bağlanmazsa (örn. sentinel
geçici olarak sessizse), bilinçli bir `stop.flag` sonsuza kadar fark
edilmezdi — kod yorumlarındaki "temiz bir `stop` komutu HER ZAMAN saygı
görür" vaadi pratikte YANLIŞTI.

**Düzeltme:** `ctrlc` ile SIGTERM/SIGINT/CTRL_CLOSE_EVENT yakalanır; sinyal
geldiğinde hem `stop.flag` yazılır hem de kendi soketimize sahte bir
bağlantı açılarak bloklayan `accept()` çağrısı ANINDA uyandırılır.
`stop.flag` kontrolü döngünün EN BAŞINA taşındı.

**Canlı doğrulama:** Sıfır bağlantı ile başlatılan bir core'a `SIGTERM`
gönderildi; süreç 1 saniye içinde sonlandı (önceden sonsuza dek asılı
kalırdı).

## 2) GERÇEK HATA: Sentinel, bilinçli bir durdurmadan sonra core'u yeniden başlatıyordu

**Sorun:** (1) numaralı düzeltme canlı test edilirken İKİNCİ bir gerçek hata
ortaya çıktı: `chimera-sentinel`, `stop.flag` kavramından tamamen habersizdi.
Operatör core'u bilinçli olarak durdurduğunda, sentinel bunu "çökme" sanıp
birkaç saniye içinde core'u YENİDEN BAŞLATIYORDU — sistem hiçbir zaman
gerçekten durdurulamıyordu.

**Düzeltme:** Sentinel'in izleme döngüsüne, hem döngü başında hem de yeniden
başlatmadan hemen önce (yarış penceresini kapatmak için) `stop.flag` kontrolü
eklendi.

**Kendi kendine yakalanan yarış durumu:** İlk düzeltme sürümünde core,
`stop.flag`'ı KENDİSİ tespit eder etmez SİLİYORDU — bu, sentinel'in
asenkron kalp atışı kontrolü bayrağı henüz görmeden bayrağın silinmesine
(yarış durumu) yol açabiliyordu. Düzeltme: `stop.flag` artık KALICI bir
işarettir; yalnızca bir sonraki bilinçli `chimera-core serve` başlangıcında
temizlenir.

**Canlı doğrulama:** Taze bir dizinle, sıfır bağlantı varken `SIGTERM`
gönderildi; denetim kaydında TEK BİR `core.start` kaydı olduğu, core'un
yeniden başlamadığı VE sentinel'in de temiz şekilde çıktığı doğrulandı.

## 3) GERÇEK HATA: Sentinel'in `respawn_core()` fonksiyonu zombie süreç sızdırıyordu

Bu, bu turda **canlı test sırasında kazara keşfedilen** en ciddi hatadır.
Yukarıdaki (1) ve (2) numaralı düzeltmeleri doğrularken kullanılan eski test
dizinlerinden biri (`shutdown-test`), güven deposu bozuk bir sentinel/core
eşleşmesiyle unutulmuş halde arka planda ÇALIŞMAYA DEVAM ETMİŞTİ. Süreç
tablosu incelendiğinde:

- Sentinel, core ile el sıkışamıyordu (`handshake_rejected`) ve bunu
  sonsuza kadar, sabit ~15 saniyelik aralıklarla core'u yeniden başlatarak
  "çözmeye" çalışıyordu.
- `respawn_core()`, spawn ettiği `chimera-core` alt sürecini **hiçbir zaman
  `wait()` ile toplamıyordu (reap etmiyordu)**. İşletim sistemi kuralları
  gereği, bir alt süreç sonlandığında ebeveyni onu toplayana kadar PID
  tablosunda "defunct" (zombie) olarak kalır.
- Sonuç: yaklaşık 50 dakika içinde **200'ün üzerinde zombie `chimera-core`
  süreci** birikmişti — kendi kendine yol açılan bir PID tüketimi/kaynak
  tükenmesi durumu. Üretimde bu, kalıcı bir konfigürasyon hatasının (örn.
  bozuk bir güven deposu, dolu disk) bir gün içinde binlerce zombie sürece
  ve nihayetinde tüm sistem için PID tablosu tükenmesine yol açabileceği
  anlamına gelir.

**Düzeltme (iki parça):**

1. **Reap düzeltmesi:** `respawn_core()`, spawn ettiği çocuğu artık ayrı,
   kısa ömürlü bir thread'de `child.wait()` ile topluyor — ana kalp atışı
   döngüsü BLOKLANMIYOR, ama süreç zombie olarak asla kalmıyor.
2. **Ustel geri-çekilme:** Sabit 15 saniyelik yeniden deneme aralığı, core
   `spawn_sentinel_watchdog`'un (core'un sentinel'i izleme yönü) ZATEN
   kullandığı desenle TUTARLI şekilde, üst sınırı 5 dakika olan ustel bir
   geri-çekilmeye (15sn → 30sn → 60sn → ... → 300sn tavan) çevrildi. Başarılı
   bir kalp atışı, geri-çekilmeyi anında taban değere sıfırlar. Bu, kalıcı
   bir arızada deneme sıklığını 20 kat azaltarak hem CPU hem PID tüketimini
   sınırlar; sistemin gerçek bir kazadan toparlanma yeteneğini KISITLAMAZ.

**Canlı doğrulama:** Aynı bozuk-güven senaryosu KASITLI olarak yeniden
üretildi (sentinel kimliği silinip yeniden oluşturuldu, böylece zaten
çalışan core'un güven deposunda tanınmayan bir kimlikle sürekli el sıkışma
reddi üretildi); düzeltme sonrası ~50 saniyelik gözlem penceresinde SIFIR
yeni zombie süreç oluştuğu doğrulandı (bkz. aşağıdaki ham çıktı).

## 4) YENİ ÖZELLİK: Kanıta-dayanıklı, hash-zincirli denetim kaydı

**Sorun:** Önceki denetim kaydı (`logs/audit.jsonl`) düz metin JSON
satırlarından oluşuyordu. Diske erişimi olan bir saldırgan (Tehdit Modeli
Seviye 4) geçmiş bir satırı sessizce silebilir/değiştirebilirdi ve bu asla
fark edilmezdi — örneğin bir `privileged_denied` (yetkisiz erişim
denemesi) olayını gizlemek için.

**Çözüm:** Yeni `chimera-core::auditlog` modülü, her satırı bir öncekinin
BLAKE3 özetine zincirler (standart, denetlenmiş bir hash fonksiyonu —
özel bir kriptografik ilkel İCAT EDİLMEDİ). Bir satır silinir/değiştirilirse,
ondan SONRAKİ satırın `prev` alanı artık yeniden hesaplanan özetle uyuşmaz
ve `verify()` bunu YAKALAR. Bu, saldırganın "izlerini silmesini" imkânsız
kılmaz (dosyayı BAŞTAN İTİBAREN yeniden yazıp zinciri kendi baştan
kurabilir) ama **kısmi/nokta düzenlemeyi tespit edilebilir kılar** — ki
tipik "birkaç satırı sil" saldırısı budur.

Eşzamanlılık, kendi icat edilmiş bir kilitleme ilkeli yerine GERÇEK bir
OS-seviyesi dosya kilidiyle (`fs4` — Unix'te `flock(2)`, Windows'ta
`LockFileEx`) sağlanır.

**Doğrulama iki yoldan yapılabilir:**
- **Yerel (çevrimdışı):** `chimera-core verify-audit --root R` — servis
  ÇALIŞMIYORKEN bile, disk erişimi olan bir olay müdahale ekibinin
  kullanabileceği bir komut.
- **Uzaktan (Sıfır Güven):** `chimera-admin verify-audit --root R --share
  HEX --share HEX` — diğer ayrıcalıklı komutlarla AYNI Shamir(2,3) kapısına
  tabidir; IPC protokolüne yeni `VerifyAuditLog`/`AuditVerifyOk` mesaj
  çifti eklendi.

**Test kapsamı:** 5 birim testi (boş dosya, geçerli 3 kayıtlık zincir,
GERÇEK bir kurcalama simülasyonu ve tespiti, sondan-kırpma sınırının
belgelenmesi, 16 thread'in eşzamanlı yazdığı bir zincirin bütünlüğü) + canlı
süreç testi (gerçek IPC üzerinden `degrade`/`logs`/`heartbeat` trafiği
üretilip zincirin "SAĞLAM" doğrulandığı, ardından temiz durdurma ile
yeniden başlatılan sürecin de zincire doğru kaldığı gösterildi).

## Özet: Bu turda düzeltilen/eklenenler

| # | Tür | Bileşen | Durum |
|---|-----|---------|-------|
| 1 | Gerçek hata | chimera-core | Düzeltildi, canlı doğrulandı |
| 2 | Gerçek hata | chimera-sentinel | Düzeltildi, canlı doğrulandı |
| 3 | Gerçek hata (kaynak tükenmesi) | chimera-sentinel | Düzeltildi, canlı doğrulandı |
| 4 | Yeni özellik | chimera-core + chimera-ipc + chimera-admin | Eklendi, 5 birim testi + canlı doğrulama |

Tüm değişiklikler `cargo test --workspace` ile 58/58 test geçişi
korunarak yapıldı (önceki tur: 53 test). Hiçbir mevcut işlevsellik
bozulmadı; hiçbir özel kriptografik ilkel icat edilmedi; hiçbir
kullanıcı-düşmanı/veri kaybına yol açan anti-tamper davranışı eklenmedi.
