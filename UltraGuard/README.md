# UltraGuard OS Shield

**Taramaz. Anlar.** — Android için davranışsal, cihaz-üstü güvenlik katmanı.

İmza tabanlı tarama yerine **davranış dizisi** üzerine kurulu bir tespit
motoru. Mimari gerekçeler ve ürün tasarımı için:
[tasarım dokümanı](../docs/ultraguard-os-shield.md).

---

## APK indirme

| Kaynak | Nasıl |
|---|---|
| **Releases** | Depo ana sayfasında sağdaki *Releases* bölümü |
| **Actions** | *Actions → UltraGuard APK → son çalışma → Artifacts → UltraGuard-APK* |

**Uyumluluk:** Android 10 (API 29) ve üzeri · arm64-v8a, armeabi-v7a, x86,
x86_64 · **tek universal APK**. ABI bölünmesi bilinçli olarak yapılmıyor:
SQLCipher ve TensorFlow Lite yerel kütüphane taşır, bölünmüş bir APK yanlış
cihazda çalışmaz.

`SHA256SUMS.txt` ile indirdiğiniz dosyayı doğrulayabilirsiniz.

---

## Derleme

| Bileşen | Sürüm |
|---|---|
| JDK | 17 |
| Android Gradle Plugin | 8.7.3 |
| Kotlin | 2.1.0 |
| compileSdk / targetSdk | 35 |
| minSdk | 29 (Android 10) |

```bash
./gradlew testDebugUnitTest     # tüm birim testleri
./gradlew :app:assembleDebug    # universal debug APK
```

`minSdk = 29` bir kısıt değil, gereklilik: paket→UID eşlemesi
(`ConnectivityManager.getConnectionOwnerUid`) ve güvenilir sensör telemetrisi
(`AppOpsManager.startWatchingMode`) bu sürümle geliyor. Bunlar olmadan ağ ve
sensör izleme anonim bir akışa indirgenir — eksik telemetriyle çalışan bir
güvenlik ürünü, korumadığı şeyi koruduğunu iddia eder.

### Yayın derlemesi

Kendini koruma yapılandırması olmadan **kasıtlı olarak başarısız olur**:

```bash
./gradlew :app:assembleRelease \
  -Pultraguard.signingCertSha256=<imza-sertifikasinin-sha256> \
  -Pultraguard.storeFile=/yol/ultraguard.jks \
  -Pultraguard.storePassword=... \
  -Pultraguard.keyAlias=... \
  -Pultraguard.keyPassword=...
```

CI'da bu değerler GitHub Secrets'tan gelir (`KEYSTORE_BASE64`,
`KEYSTORE_PASSWORD`, `KEY_ALIAS`, `KEY_PASSWORD`); imza özeti anahtarın
kendisinden türetilir. Anahtar veya parola depoya hiçbir zaman girmez.

---

## Modül haritası

```
:app                    Compose host, navigation, DI birleştirme, ProtectionService,
                        ThreatResponder, bildirimler, WorkManager işleri
:core:model             Saf domain tipleri (JVM) — SecurityEvent, Verdict, EnforcementAction
:core:common            Dispatcher'lar, Clock soyutlaması, gizlilik farkındalıklı log
:core:designsystem      Compose tema, TrustScoreRing, EvidenceBreakdown, ortak metinler
:core:database          Room + SQLCipher; olay/hüküm/defter kalıcılığı
:core:datastore         Kullanıcı tercihleri (gizlilik ayarları varsayılan KAPALI)
:core:security          StrongBox anahtar yönetimi, root tespiti, kendini koruma, hash zinciri
:core:engine            EventBus, EventNormalizer (L0), korelasyon, ThreatPipeline
:core:policy            EnforcementPlanner, EnforcementExecutor, ActionLedger
:core:ai                L1 kural motoru (17 kural), L2 TFLite sarmalayıcı, RiskFusion
:core:sensors           StaticApkTriage, AppOps, Accessibility, Notification, Clipboard
:core:network           VpnService ZTNA, TLS ClientHello ayrıştırıcı, beacon/DGA tespiti
:feature:*              dashboard · timeline · appdetail · assistant · settings
:module:deepscan        [R] eBPF/Binder köprüsü — ayrı APK, ana uygulama onsuz çalışır
```

**Bağımlılık kuralı:** `feature/*` → `core/*` tek yönlüdür. Feature modülleri
birbirini tanımaz; geçişler yalnızca `:app` içindeki navigasyon grafında
birleşir. Sensörler birbirini tanımaz; hepsi yalnızca `EventBus`'a yazar.

---

## Mimarinin taşıyıcı fikirleri

### Karar zinciri kademelidir

```
Olay akışı → L0 normalizasyon (~%97 elenir, dedup + sistem paketi filtresi)
           → L1 deterministik kural motoru (<1 ms, 17 kural)
           → L2 cihaz-üstü dizi modeli (~30 ms, örneklemeli)
           → RiskFusion → EnforcementPlanner → EnforcementExecutor
```

`MonitoringStateMachine`, BASELINE durumunda olayların %5'ini modele çıkarır
ve HEIGHTENED durumu **her zaman süre sınırlıdır** (10 dk). Sürekli yüksek
tüketim mimari olarak oluşamaz. Pil %15 altına inince `BATTERY_GUARD` devreye
girer: L2 askıya alınır, L1 ve ağ kalkanı çalışmaya devam eder.

### Otonom eylem yalnızca geri alınabilir olabilir

`EnforcementPlan`'ın `init` bloğu bunu çalışma zamanında zorlar:
`reversible = false` olan hiçbir eylem otonom kuyruğa giremez. Uygulama
kaldırma her zaman kullanıcı onayına gider. Yanlış pozitifin kullanıcıya
kalıcı zarar vermesi böylece **yapısal olarak** imkânsızdır.

`EnforcementExecutor` ayrıca: başarısız eylemi `APPLIED` değil `FAILED`
kaydeder, yetki yoksa `SKIPPED_NO_CAPABILITY` yazar ve sessizce atlamaz.

### Açıklanamayan hüküm üretilemez

`Verdict`'in `init` bloğu boş `attributions` listesini reddeder. Kural
motorunda kanıt toplama, koşul kontrolünün yan ürünüdür
(`RuleContext.need`), dolayısıyla "eşleşti ama nedenini bilmiyoruz" durumu
oluşamaz.

---

## Gizlilik sözleşmeleri (kod düzeyinde)

Bunlar yorum değil, uygulanan kısıtlardır:

| Yer | Kısıt |
|---|---|
| `accessibility_service_config.xml` | `canRetrieveWindowContent="false"` — ekran metnini okuma yeteneği **platform düzeyinde** kapalı |
| `UltraGuardAccessibilityService` | `getText()` çağrısı yok ve eklenmemeli |
| `SensitivePatternMatcher` | Girdi metni alır, **yalnızca etiket** döndürür; metin hiçbir yere yazılmaz |
| `NotificationCollector` / `ClipboardMonitor` | İçerik RAM'de sınıflandırılır, olaya yalnızca `matched_pattern` yazılır |
| `data_extraction_rules.xml` | Hiçbir şey yedeklenmez — güvenlik geçmişi buluta gitmez |
| `SettingsStore` | Bulut konsültasyonu ve federated learning varsayılan **KAPALI** |
| `UltraGuardVpnService` | Tünel çıkışı yoktur; trafik hiçbir sunucuya gönderilmez |
| `DatabaseKeyProvider` | Anahtar StrongBox → TEE → yazılım kademeli; ulaşılan seviye kayıt altında |

**Manifest'te bilinçli olarak istemediğimiz izinler** gerekçeleriyle
listelenmiştir: `CAMERA`, `RECORD_AUDIO`, `READ_SMS`,
`ACCESS_FINE_LOCATION`, `READ_CONTACTS`.

---

## Test

`Clock` soyutlaması ve saf Kotlin katmanları sayesinde emülatörsüz:

```bash
./gradlew testDebugUnitTest
```

| Alan | Kapsam |
|---|---|
| Kural motoru | Eşleşme ve **eşleşmeme** senaryoları, skor tavanı, yetki filtresi |
| Korelasyon penceresi | Sıralama, zaman penceresi sınırları |
| Yaptırım planlayıcı | Geri-alınabilirlik değişmezi, mod eşikleri, yetki eksikliği |
| Yaptırım uygulayıcı | Defter kaydı, başarısızlık, geri alma, çift geri alma reddi |
| Hash zinciri | Kayıt değiştirme ve silme tespiti, alan sınırı çakışması |
| Durum makinesi | Yükselme, süre aşımı, CONTAINMENT kalıcılığı |
| Ağ | TLS ClientHello (bozuk girdi dahil), JA4 kararlılığı, beacon, DGA, IP başlığı |
| Sensör | Hassas desen eşleme, entropi hesabı |

---

## Bilinen sınırlar

- **L2 model dosyası yok.** `behavior_seq_int8.tflite` henüz eğitilmedi;
  `LazyModelProvider` bunu algılar ve L1 tek başına çalışmaya devam eder.
  Koruma azalır, kesilmez.
- **`:module:deepscan` yalnızca arayüz.** eBPF program yükleme ve Binder
  tracepoint uygulaması ayrı bir root paketiyle gelecek.
- **Tehdit istihbaratı beslemesi bağlı değil.** `reputation` alanları boş.
- **Federated learning istemcisi yok.** Ayar mevcut ama istemci yazılmadı.
- **Fleet (kurumsal) modu kısmi.** Device Owner yaptırımları çalışır, uzaktan
  politika dağıtımı ve SIEM aktarımı yok.
