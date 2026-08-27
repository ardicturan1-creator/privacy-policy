# UltraGuard OS Shield

**Taramaz. Anlar.** — Android için davranışsal, cihaz-üstü güvenlik katmanı.

Bu depo, [mimari tasarım dokümanında](../docs/ultraguard-os-shield.md) tarif
edilen ürünün MVP kod tabanıdır. İmza tabanlı tarama yerine **davranış dizisi**
üzerine kurulu bir tespit motoru içerir.

---

## Derleme durumu

> **Bu proje henüz bir CI ortamında derlenmemiştir.** Kod, Android SDK ve
> Google Maven erişimi olmayan bir ortamda yazıldı; derleme doğrulaması
> Android Studio'da yapılmalıdır. Aşağıdaki "İlk derleme" bölümü beklenen
> adımları verir.

### Gereksinimler

| Bileşen | Sürüm |
|---|---|
| JDK | 17 |
| Android Gradle Plugin | 8.7.3 |
| Kotlin | 2.1.0 |
| compileSdk / targetSdk | 35 |
| minSdk | 29 (Android 10) |

`minSdk = 29` bilinçli bir karardır: `ConnectivityManager.getConnectionOwnerUid`
(paket→UID eşlemesi) ve `AppOpsManager.startWatchingMode`'un güvenilir
davranışı bu sürümle gelir. Bunlar olmadan ağ ve sensör telemetrisi anonim
bir akışa indirgenir — eksik telemetriyle çalışan bir güvenlik ürünü,
korumadığı şeyi koruduğunu iddia eder.

### İlk derleme

```bash
./gradlew :core:model:test           # saf JVM, en hızlı geri bildirim
./gradlew testDebugUnitTest          # tüm birim testleri
./gradlew :app:assembleDebug
```

Release derlemesi, kendini koruma yapılandırması olmadan **kasıtlı olarak
başarısız olur**:

```bash
./gradlew :app:assembleRelease -Pultraguard.signingCertSha256=<64-hane-sha256>
```

---

## Modül haritası

```
:app                    Compose host, navigation, DI birleştirme, ProtectionService
:core:model             Saf domain tipleri (JVM) — SecurityEvent, Verdict, EnforcementAction
:core:common            Dispatcher'lar, Clock soyutlaması, gizlilik farkındalıklı log
:core:designsystem      Compose tema, TrustScoreRing, EvidenceBreakdown
:core:database          Room + SQLCipher; olay/hüküm/defter kalıcılığı
:core:datastore         Kullanıcı tercihleri (gizlilik ayarları varsayılan KAPALI)
:core:security          StrongBox anahtar yönetimi, root tespiti, kendini koruma, hash zinciri
:core:engine            EventBus, EventNormalizer (L0), korelasyon, ThreatPipeline
:core:policy            EnforcementPlanner, ActionLedger (geri alınabilir yaptırım)
:core:ai                L1 kural motoru (17 kural), L2 TFLite sarmalayıcı, RiskFusion
:core:sensors           StaticApkTriage, AppOps, Accessibility, Notification, Settings
:core:network           VpnService ZTNA, TLS ClientHello ayrıştırıcı, beacon/DGA tespiti
:feature:*              dashboard · timeline · appdetail · assistant · settings
:module:deepscan        [R] eBPF/Binder köprüsü — ayrı APK, ana uygulama onsuz çalışır
```

**Bağımlılık kuralı:** `feature/*` → `core/*` tek yönlüdür. Feature modülleri
birbirini tanımaz; geçişler yalnızca `:app` içindeki navigasyon grafında
birleşir. Sensörler birbirini tanımaz; hepsi yalnızca `EventBus`'a yazar.

---

## Mimarinin üç taşıyıcı fikri

### 1. Karar zinciri kademelidir

```
Olay akışı → L0 normalizasyon (%97 elenir)
           → L1 deterministik kural motoru (<1 ms)
           → L2 cihaz-üstü dizi modeli (~30 ms, örneklemeli)
           → RiskFusion → EnforcementPlanner
```

L2 her olayda çalışmaz. `MonitoringStateMachine`, BASELINE durumunda olayların
%5'ini modele çıkarır ve HEIGHTENED durumu **her zaman süre sınırlıdır**
(10 dk). Sürekli yüksek tüketim mimari olarak oluşamaz.

### 2. Otonom eylem yalnızca geri alınabilir olabilir

`EnforcementPlan`'ın `init` bloğu bunu derleme sonrası ilk çalıştırmada
zorlar: `reversible = false` olan hiçbir eylem otonom kuyruğa giremez.
Uygulama kaldırma her zaman kullanıcı onayına gider. Yanlış pozitifin
kullanıcıya kalıcı zarar vermesi böylece **yapısal olarak** imkânsızdır.

### 3. Açıklanamayan hüküm üretilemez

`Verdict`'in `init` bloğu boş `attributions` listesini reddeder. Kural
motorunda kanıt toplama, koşul kontrolünün yan ürünüdür (`RuleContext.need`),
dolayısıyla "eşleşti ama nedenini bilmiyoruz" durumu oluşamaz.

---

## Gizlilik sözleşmeleri (kod düzeyinde)

Bunlar yorum değil, uygulanan kısıtlardır:

| Yer | Kısıt |
|---|---|
| `accessibility_service_config.xml` | `canRetrieveWindowContent="false"` — ekran metnini okuma yeteneği **platform düzeyinde** kapalı |
| `UltraGuardAccessibilityService` | `getText()` çağrısı yok ve eklenmemeli |
| `SensitivePatternMatcher` | Girdi metni alır, **yalnızca etiket** döndürür; metin hiçbir yere yazılmaz |
| `NotificationCollector` | Bildirim içeriği RAM'de sınıflandırılır, olaya yalnızca `matched_pattern` yazılır |
| `data_extraction_rules.xml` | Hiçbir şey yedeklenmez — güvenlik geçmişi buluta gitmez |
| `SettingsStore` | Bulut konsültasyonu ve federated learning varsayılan **KAPALI** |
| `UltraGuardVpnService` | Tünel çıkışı yoktur; trafik hiçbir sunucuya gönderilmez |

---

## Test

Test edilebilirlik, `Clock` soyutlaması ve saf Kotlin katmanları sayesinde
emülatörsüzdür:

```bash
./gradlew testDebugUnitTest
```

Kapsanan alanlar: kural motoru eşleşme ve eşleşmeme senaryoları, korelasyon
penceresi sıralama mantığı, yaptırım planlayıcının geri-alınabilirlik
değişmezi, hash zinciri kurcalama tespiti, izleme durum makinesi geçişleri,
TLS ClientHello ayrıştırma (bozuk girdi dahil), beacon/DGA sınıflandırma,
hassas desen eşleme, entropi hesabı.

---

## Bilinen boşluklar (MVP kapsamı)

- **L2 model dosyası yok.** `behavior_seq_int8.tflite` henüz eğitilmedi;
  `LazyModelProvider` bunu algılar ve L1 tek başına çalışmaya devam eder.
- **Compose ekranları kısmi.** Dashboard tam; timeline/appdetail/assistant/
  settings ViewModel katmanı hazır, ekran bileşenleri eksik.
- **`:module:deepscan` yalnızca arayüz.** eBPF program yükleme ve Binder
  tracepoint uygulaması v2.0 kapsamındadır.
- **Tehdit istihbaratı beslemesi yok.** `reputation` alanları şu an boş.
- **Federated learning istemcisi yok.** v1.5 kapsamındadır.
