# UltraGuard OS Shield
### Android için Otonom Güvenlik İşletim Katmanı — Mimari ve Ürün Tasarımı (2026–2030)

> **Doküman durumu:** Mimari tasarım v0.9 · Hedef platform: Android 10+ (API 29+) · Hedef lansman: 2027 Q1 (v1.0)

---

## 0. Bu Dokümanın Okunma Biçimi

Bu doküman iki katmanda yazılmıştır:

- **Ürün katmanı** (§1, §4, §5, §6, §7): Ne yaptığımız ve neden.
- **Mühendislik katmanı** (§2, §3, §8): Nasıl yaptığımız ve **neyin Android'de gerçekten mümkün olduğu.**

Mobil güvenlik pazarındaki en yaygın hata, masaüstü EDR yeteneklerini Android'e vaat etmektir. Android, uygulamaları birbirinden SELinux ve uygulama-başına UID sandbox'ı ile ayırır; bir kullanıcı uygulaması diğer uygulamanın belleğini okuyamaz, syscall'larını göremez, dosyalarına erişemez. Bu doküman boyunca her yeteneğin yanında **[R]** (root/KernelSU gerekir), **[E]** (Enterprise / Device Owner gerekir) veya **[U]** (root'suz, standart kullanıcı cihazı) etiketi bulunur. Etiketsiz vaat vermiyoruz.

---

## 1. Ürün Vizyonu ve Konumlandırma

### 1.1 Vizyon

> Antivirüs, kötü yazılımın **ne olduğunu** bilmeye çalışır. UltraGuard, uygulamanın **ne yapmak üzere olduğunu** bilir.

İmza tabanlı tarama 2026'da ölü bir teknolojidir. Modern mobil tehdit — AI ile üretilen polimorfik dropper'lar, meşru uygulamaların içine enjekte edilen SDK'lar, accessibility hizmetini kötüye kullanan bankacılık trojanları, ticari stalkerware — imza bırakmaz. Bunların ortak noktası **davranış dizisi**dir: izin al → görünürlükten çık → veri topla → dışarı sızdır.

UltraGuard OS Shield, cihazda çalışan bir **davranışsal gözlem ve otonom müdahale katmanıdır**. Uygulamaları dosya olarak değil, **zaman içindeki eylem dizisi** olarak modeller.

### 1.2 Slogan Setleri

**Ana slogan (uluslararası):**
> **UltraGuard OS Shield — It doesn't scan. It understands.**

**Türkçe ana slogan:**
> **UltraGuard OS Shield — Taramaz. Anlar.**

**Destekleyici slogan alternatifleri:**

| Bağlam | Slogan |
|---|---|
| Mağaza / ASO | *Telefonunuzun bağışıklık sistemi.* |
| Teknik kitle | *Behavior is the signature.* (Davranış, imzanın kendisidir.) |
| Gizlilik odaklı kampanya | *Sizi korumak için sizi izlemek zorunda değiliz.* |
| Kurumsal (B2B) | *Sıfır güven, cihazın kendisinden başlar.* |
| Otonomi vurgusu | *Sormaz. Durdurur. Sonra anlatır.* |

### 1.3 Ürün Konumu

UltraGuard bir "antivirüs uygulaması" değil, **cihaz üstü güvenlik denetleyicisi**dir. Üç ürün hattı:

| Hat | Kitle | Dağıtım | Yetki modeli |
|---|---|---|---|
| **UltraGuard Personal** | Bireysel kullanıcı | Google Play | [U] Root'suz |
| **UltraGuard Sentinel** | Güç kullanıcısı / güvenlik araştırmacısı | Doğrudan APK + F-Droid | [U] + [R] modülleri |
| **UltraGuard Fleet** | Kurumsal / MDM | Managed Play, Device Owner | [E] Tam politika kontrolü |

---

## 2. Sistem Mimarisi

### 2.1 Katman Diyagramı

```
┌─────────────────────────────────────────────────────────────────────┐
│  KATMAN 6 — SUNUM (:app, :feature-*)                                │
│  Jetpack Compose · Guardian AI Asistan · Risk Haritası · Zaman Çiz. │
├─────────────────────────────────────────────────────────────────────┤
│  KATMAN 5 — POLİTİKA & OTONOMİ (:core-policy)                       │
│  Mod motoru (Active/Stealth/Paranoid/Fleet) · Yaptırım kararı ·     │
│  Geri alınabilir eylem günlüğü (Action Ledger) · Onay akışı         │
├─────────────────────────────────────────────────────────────────────┤
│  KATMAN 4 — KARAR (:core-ai)                                        │
│  L1 Kural Motoru → L2 On-device Model → L3 Bulut Konsültasyonu      │
│  Risk skoru füzyonu · Açıklanabilirlik (attribution) üretimi        │
├─────────────────────────────────────────────────────────────────────┤
│  KATMAN 3 — KORELASYON (:core-graph)                                │
│  Olay normalizasyonu · Varlık grafiği (uygulama↔süreç↔soket↔dosya)  │
│  Kayan pencere davranış dizisi · Gürültü bastırma                   │
├─────────────────────────────────────────────────────────────────────┤
│  KATMAN 2 — TELEMETRİ (:core-sensors)                               │
│  ┌───────────┬───────────┬───────────┬───────────┬───────────────┐  │
│  │ AppOps &  │ Network   │ Package   │ UI/Overlay│ Integrity     │  │
│  │ Perm izi  │ (VpnSvc)  │ Yaşam d.  │ Accessib. │ Boot/Key/TEE  │  │
│  └───────────┴───────────┴───────────┴───────────┴───────────────┘  │
│  [R] Genişletme: eBPF · Binder IPC · Netlink · inotify · procfs     │
├─────────────────────────────────────────────────────────────────────┤
│  KATMAN 1 — DAYANIKLILIK (:core-runtime)                            │
│  Foreground Service · WorkManager · Watchdog · Self-protection      │
│  Şifreli depo (SQLCipher + StrongBox) · Tamper-evident log zinciri  │
└─────────────────────────────────────────────────────────────────────┘
                                 ▲
                    ┌────────────┴────────────┐
                    │  Android Platform APIs  │
                    │  (+ [R] Kernel yüzeyi)  │
                    └─────────────────────────┘
```

### 2.2 Telemetri Motoru — Gerçekte Ne Görebiliriz?

Bu, ürünün en kritik ve en çok abartılan bölümüdür. Kaynak bazında dürüst envanter:

#### [U] Root'suz cihazda erişilebilen sinyaller

| Sinyal kaynağı | API | Ne verir | Sınır |
|---|---|---|---|
| **Paket olayları** | `PackageManager`, `ACTION_PACKAGE_ADDED`, `PackageInstaller.SessionCallback` | Kurulum/güncelleme/kaldırma, **installer kaynağı** (yan yükleme tespiti), imza sertifikası, hedef SDK | Anlık, güvenilir |
| **İzin & AppOps** | `PackageManager.getPackageInfo(GET_PERMISSIONS)`, `AppOpsManager.startWatchingMode()` | Hangi uygulama **hangi anda** kamera/mikrofon/konum/clipboard kullandı | `startWatchingMode` bazı op'lar için sistem izni ister; kalanı `getHistoricalOps` ile geriye dönük |
| **Ağ akışı** | `VpnService` (lokal, tünelsiz) | Her paketin hedef IP/port'u, SNI/ECH öncesi TLS ClientHello, DNS sorguları, **paket→UID eşlemesi** (`ConnectivityManager.getConnectionOwnerUid`) | **TLS payload'ı okunamaz.** Aşağıda §2.2.1 |
| **Erişilebilirlik** | `AccessibilityService` | Ekrandaki pencere sahibi paket, overlay varlığı, tıklama hedefleri, **başka bir accessibility servisinin kötüye kullanımı** | Play Store politikası: sadece güvenlik amaçlı beyan + kullanıcı onay ekranı zorunlu |
| **Ekran üstü çizim** | `TYPE_APPLICATION_OVERLAY` sayımı + `AccessibilityWindowInfo` | Overlay saldırısı / tapjacking anında tespit | Android 12+ `HIDE_OVERLAY_WINDOWS` ile korumalı ekranlarda otomatik gizleme |
| **Bildirim** | `NotificationListenerService` | Phishing bildirimi, OTP sızdırma denemesi, sahte sistem uyarısı | Kullanıcı onayı gerekir |
| **Kullanım istatistiği** | `UsageStatsManager` | Arka plan uyanma sıklığı, foreground süresi, davranış temeli | 1 gün altı granülerlik sınırlı |
| **Bütünlük** | `KeyStore` + `StrongBox`, `Play Integrity API`, `KeyMint attestation` | Bootloader kilidi, verified boot durumu, cihaz bütünlüğü, **kendi APK imzamızın doğrulanması** | Donanım destekli, taklit edilmesi çok zor |
| **Kendi süreç durumumuz** | `/proc/self/*`, `ActivityManager` | Kendi süreçlerimizin öldürülmesi, hafıza baskısı | Sadece kendi sürecimiz — Android 9'dan sonra `/proc` diğer UID'ler için gizli |

#### [R] Root / KernelSU / Zygisk ile açılan derin katman

| Yetenek | Teknoloji | Ne verir |
|---|---|---|
| **Syscall izleme** | eBPF (`kprobe`/`tracepoint`, GKI 5.10+ kernel'lerde `CONFIG_BPF` açıksa) | `execve`, `openat`, `connect`, `ptrace`, `memfd_create` çağrıları — fileless yükleme tespiti |
| **Binder IPC** | `binder_transaction` tracepoint | Hangi uygulama hangi sistem servisine ne istekle gitti — izin-atlatma ve IPC istismarı tespiti |
| **Ağ olayları** | Netlink `NETLINK_INET_DIAG` | Soket→UID eşlemesi, VPN yükü olmadan |
| **Dosya sistemi** | `fanotify` (`FAN_OPEN_EXEC_PERM`) | Çalıştırma anında **engelleme** (izin tabanlı, sadece kernel seviyesinde mümkün) |
| **Süreç dondurma** | cgroup v2 `freezer` | Şüpheli süreci öldürmeden askıya alma — kanıt korunur |
| **Anti-tampering** | Zygisk modülü + Magisk `denylist` tespiti | Kendini gizleyen root'ların yakalanması |

**Mimari kural:** [R] yetenekleri **ayrı bir modülde** (`:module-deepscan`, opsiyonel APK eklentisi) yaşar. Ana uygulama bunlar olmadan tam işlevseldir. Bu, hem Play Store uyumluluğu hem de saldırı yüzeyi izolasyonu içindir.

#### 2.2.1 TLS İçeriği Hakkında Dürüst Not

"TLS içeriği dahil ağ izleme" teknik olarak **tüketici ürününde yapılmamalıdır**:

- Android 7+ (API 24) itibarıyla uygulamalar kullanıcı-yüklü CA'lara varsayılan güvenmez. MITM için her uygulamanın network security config'ini değiştirmek gerekir — bu root ister ve **uygulamaların certificate pinning'ini kırar**.
- Kırıldığında UltraGuard'ın kendisi tüm banka trafiğinin merkezi bir zafiyet noktası olur. Bu, çözdüğümüzden daha büyük bir risk üretir.

**Bunun yerine ne yapıyoruz — Encrypted Traffic Analysis (ETA):** Şifreyi çözmeden sınıflandırma.

| Metadata özelliği | Ne ele verir |
|---|---|
| SNI / ECH varlığı, DNS-over-HTTPS resolver kimliği | C2 alan adı, DGA (algoritma üretimi domain) paterni |
| JA4/JA4S TLS parmak izi | İstemcinin gerçek kütüphanesi — "meşru bankacılık SDK'sı" mı yoksa gömülü Go/Rust dropper mı |
| Paket boyutu & zamanlama histogramı | Beacon paterni (sabit aralıklı küçük paketler = C2 kalp atışı), toplu veri sızdırma |
| Bağlantı grafiği | ASN itibarı, bulletproof hosting, yeni kayıtlı domain |

Bu yaklaşım şifrelemeye dokunmaz, %100 lokal çalışır ve modern C2 tespitinde payload analizinden daha dayanıklıdır.

### 2.3 AI Pipeline — Üç Kademeli Karar Zinciri

Pil ve gecikme bütçesi, tek bir modelin her olayı değerlendirmesine izin vermez. Kademeli filtreleme:

```
  Ham olay akışı (~2.000–15.000 olay/saat)
            │
            ▼
  ┌──────────────────────────────────────┐
  │ L0 — NORMALİZASYON & BASTIRMA        │   Rust/C++ (NDK), <0.1 ms/olay
  │ Gürültü filtresi, dedup, varlık ekle │   %97 olay burada elenir
  └──────────────┬───────────────────────┘
                 ▼ (~300 olay/saat)
  ┌──────────────────────────────────────┐
  │ L1 — DETERMİNİSTİK KURAL MOTORU      │   YARA-L benzeri DSL, <1 ms
  │ "Accessibility + overlay + banka pkg"│   Bilinen saldırı kalıpları
  │ Anında karar verebilir → yaptırım    │   %0 false-negative hedefi
  └──────────────┬───────────────────────┘
                 ▼ (~40 dizi/saat)
  ┌──────────────────────────────────────┐
  │ L2 — ON-DEVICE DAVRANIŞ MODELİ       │   ~8–15M param, INT8 quantized
  │ Girdi: 64 adımlık olay dizisi        │   TFLite / LiteRT + NNAPI/QNN
  │ Mimari: küçük causal transformer     │   Hedef: <30 ms, <40 MB RAM
  │ Çıktı: risk skoru + saldırı sınıfı   │   NPU varsa NPU, yoksa CPU-XNNPACK
  │        + attribution (hangi olaylar) │
  └──────────────┬───────────────────────┘
                 ▼ (belirsiz vakalar, ~2/gün)
  ┌──────────────────────────────────────┐
  │ L3 — BULUT KONSÜLTASYONU (opsiyonel) │   Sadece kullanıcı onayıyla
  │ Gönderilen: APK hash + davranış      │   ASLA: içerik, mesaj, dosya, kimlik
  │ vektörü (anonim, k-anonimite k≥50)   │
  │ Dönen: küresel itibar + YZ analizi   │
  └──────────────────────────────────────┘
```

#### L2 Modelinin Girdi Temsili

Her olay 24 boyutlu bir vektöre gömülür:

```
[ op_type(embed) | hedef_kategori | zaman_delta(log) | foreground? |
  ekran_açık? | uid_yaşı | izin_delta | ağ_hedef_itibar | veri_hacmi_bucket ]
```

Model, **64 olayın dizisini** görür ve bir sonraki olayı tahmin etmeye çalışır (self-supervised ön eğitim). Anomali = yüksek perplexity. Üstüne, etiketli kötü yazılım davranışlarıyla ince ayar (supervised head) yapılır. Bu, sıfırıncı gün tespitinin temelidir: modelin daha önce görmediği malware'i "olağandışı dizi" olarak yakalaması.

#### Federated Learning

- **Ne ayrılır:** Sadece model gradyanları — ham olay hiçbir zaman cihazdan çıkmaz.
- **Nasıl korunur:** Secure aggregation + DP-SGD (ε ≤ 3, δ = 10⁻⁵ bütçesi). Tek bir kullanıcının katkısı matematiksel olarak geri çıkarılamaz.
- **Ne zaman çalışır:** Cihaz şarjda + Wi-Fi + ekran kapalı + pil > %60. Kullanıcı opt-in.
- **Zehirlenme koruması:** Sunucu tarafında Krum/trimmed-mean agregasyonu — kötü niyetli istemcilerin modeli bozması engellenir.

### 2.4 Modül Yapısı (Gradle)

```
ultraguard/
├── app/                        # Compose UI host, navigation
├── core/
│   ├── runtime/                # servis yaşam döngüsü, watchdog, self-protection
│   ├── model/                  # ortak domain tipleri (Event, Verdict, Entity)
│   ├── storage/                # Room + SQLCipher, StrongBox key mgmt
│   └── policy/                 # mod motoru, yaptırım kararları, Action Ledger
├── sensors/
│   ├── package/  appops/  network/  ui/  integrity/  notification/
├── graph/                      # varlık grafiği + korelasyon penceresi
├── ai/
│   ├── rules/                  # L1 DSL + kural paketi (OTA güncellenebilir)
│   ├── inference/              # LiteRT wrapper, NNAPI delegate seçimi
│   └── federated/              # FL istemcisi (opsiyonel modül)
├── feature/
│   ├── dashboard/  timeline/  appdetail/  assistant/  vault/  settings/
├── module-deepscan/            # [R] eBPF/Binder — ayrı APK, opsiyonel
└── native/                     # Rust: olay normalizasyonu, kripto, ETA
```

**Bağımlılık kuralı:** `feature/*` → `core/*` yönünde tek yönlü. `sensors/*` birbirini tanımaz; sadece `graph`'a olay yayınlar. Bu, bir sensörün çökmesinin diğerlerini etkilememesini garanti eder.

---

## 3. UltraGuard Active Modu — Adım Adım Çalışma Mantığı

Somut bir senaryo üzerinden tam akış. **Senaryo:** Kullanıcı, bir mesajlaşma uygulamasından gelen linkle "KargoTakip.apk" dosyasını yan yükledi.

### T+0 sn — Kurulum niyeti

```
PackageInstaller.SessionCallback.onCreated()
  → installerPackage = "com.android.chrome" (Play değil!)
  → L1 kuralı R-INST-002 tetiklenir: "sideload_from_browser"
  → Ön risk: 35/100
```

**UltraGuard eylemi:** Kurulum tamamlanmadan bir *pre-flight* bildirimi:
> *"Bu uygulama Play Store dışından geliyor. Kurulumdan sonra ilk 10 dakika boyunca sıkı gözlem altında tutulacak."*

Kullanıcıyı engellemiyoruz — Paranoid Mode dışında. Engelleme, kullanıcının kurulumu tamamlamasını değiştirmez, sadece güveni azaltır.

### T+4 sn — Kurulum tamamlandı, statik triyaj

```
ACTION_PACKAGE_ADDED
  → Manifest analizi (paralel, ~200 ms):
     • İstenen izinler: BIND_ACCESSIBILITY_SERVICE, SYSTEM_ALERT_WINDOW,
       RECEIVE_SMS, QUERY_ALL_PACKAGES, REQUEST_INSTALL_PACKAGES
     • İmza: self-signed, sertifika yaşı 3 gün
     • targetSdk = 28  ← eski SDK, runtime izin kısıtlarından kaçınma girişimi
     • DEX entropi: 7.94 → paketlenmiş/şifreli payload göstergesi
     • Yerleşik yerel kütüphane: libmsg.so, dinamik yükleme çağrısı içeriyor
  → L1: R-STATIC-011 "accessibility+overlay+sms triadı" → KRİTİK kural
  → Risk: 78/100
```

**UltraGuard eylemi:** **Otonom ön-kısıtlama (pre-emptive containment).**
- Ağ erişimi VpnService seviyesinde **askıya alınır** (paket UID'si drop listesine girer).
- Bildirim: kırmızı, tam ekran değil, ancak öncelikli.

> ⚠️ **KargoTakip erişilebilirlik + ekran üstü çizim + SMS okuma izinlerini birlikte istiyor. Bu, bankacılık trojanlarının ayırt edici imzasıdır. İnternet erişimini geçici olarak kapattım.**
> `[ Uygulamayı kaldır ]` `[ Karantinada tut ]` `[ Ayrıntılar ]` `[ Güveniyorum ]`

Burada **kritik tasarım kararı**: Otonom eylem *geri alınabilir* olanla sınırlıdır. Ağ kesme geri alınabilir; uygulama silme değildir. UltraGuard asla kullanıcı onayı olmadan uygulama silmez.

### T+12 sn — Erişilebilirlik istismarı girişimi

Kullanıcı "Güveniyorum" derse bile gözlem sürer.

```
AccessibilityService.onAccessibilityEvent()
  → yeni servis kaydı: com.kargotakip/.AccService
  → Talep edilen bayraklar: FLAG_RETRIEVE_INTERACTIVE_WINDOWS
                          + canPerformGestures = true
  → L1: R-A11Y-001 "yeni yüklenen paket gesture yeteneği talep ediyor"
  → Risk: 91/100 — KRİTİK
```

**UltraGuard eylemi (Active modda otonom):**
- Ayarlar ekranındaki erişilebilirlik onay diyaloğunun üstüne **kendi uyarı overlay'imiz** çizilir:
  > *"DUR. Bu ekranda 'İzin Ver' derseniz, KargoTakip ekranınızdaki her şeyi okuyabilir ve sizin adınıza dokunabilir. Meşru kargo uygulamaları bunu istemez."*
- Bu, en etkili tek müdahaledir: saldırının %80'i kullanıcının bu diyaloğu okumadan onaylamasına dayanır.

### T+30 sn – T+10 dk — Davranışsal gözlem penceresi

Yan yüklenmiş her uygulama, ilk 10 dakika **yoğun telemetri** modundadır (normalde örneklemeli izleme yapılır — pil için).

```
Olay dizisi L2 modeline gider:
  [pkg_install(sideload), perm_request(a11y), service_start,
   window_query(com.bank.mobile), overlay_draw, clipboard_read,
   net_connect(TLS, JA4=t13d1516h2_..., ASN=bulletproof), data_out(48KB)]

L2 çıktısı:
  risk = 0.96
  sınıf = "banking_overlay_trojan" (conf 0.89)
  attribution = [window_query(banka) 0.34, overlay_draw 0.29,
                 net_connect(kötü ASN) 0.21]
```

**UltraGuard otonom müdahale zinciri (Active mod):**

| Sıra | Eylem | Yetki | Geri alınabilir |
|---|---|---|---|
| 1 | Ağ erişimi kes | [U] VpnService | ✅ |
| 2 | Overlay penceresini gizle | [U] `HIDE_OVERLAY_WINDOWS` (korunan ekranlarımızda) | ✅ |
| 3 | Süreci dondur (öldürme) | [R] cgroup freezer / [E] `setPackagesSuspended` | ✅ |
| 4 | Çalışma zamanı izinlerini geri al | [E] Device Owner / [U] kullanıcıyı yönlendir | ✅ |
| 5 | Kanıt paketi oluştur (APK hash, olay zinciri, ekran görüntüsü değil) | [U] | — |
| 6 | Kullanıcıya kaldırma akışı sun | [U] `ACTION_UNINSTALL_PACKAGE` | ❌ Onay şart |

### T+10 dk — Rapor ve öğrenme

- Zaman çizelgesine olay bloğu işlenir; kullanıcı her adımı görebilir.
- Guardian AI asistanı doğal dilde özet üretir.
- Kullanıcı "Bu yanlış alarmdı" derse: yerel geri bildirim kaydedilir, o paket için eşik ayarlanır ve (opt-in ise) FL turuna anonim negatif örnek olarak katılır.

### 3.1 Active Mod Durum Makinesi

```
       ┌──────────┐  temel profil sapması / yeni paket
       │ BASELINE │──────────────────────────────────────┐
       │ örnekleme│                                      ▼
       │ %5, NPU  │                              ┌───────────────┐
       │ uykuda   │◄──── 30 dk temiz ────────────│  HEIGHTENED   │
       └──────────┘                              │ tam telemetri │
             ▲                                   │ L2 her dizide │
             │                                   └───────┬───────┘
             │                                    risk>0.85│
             │                                           ▼
             │                                   ┌───────────────┐
             └────── kullanıcı onayı / temizlik ──│  CONTAINMENT  │
                                                  │ ağ kesik,     │
                                                  │ süreç dondur, │
                                                  │ kanıt topla   │
                                                  └───────────────┘
```

**Pil bütçesi:** BASELINE durumunda hedef **< %1.5 / 24 saat**. Bu, olay-güdümlü mimarinin (polling yok, `startWatchingMode` callback'leri) ve L0 filtresinin ürünüdür. HEIGHTENED durumu süre sınırlıdır (varsayılan 10 dk) — sürekli yüksek tüketim asla oluşmaz.

---

## 4. Tam Özellik Listesi

### 4.1 Gözlem ve Tespit

| # | Özellik | Teknik açıklama | Yetki |
|---|---|---|---|
| 1.1 | **Yan yükleme triyajı** | Installer paketi + sertifika yaşı + DEX entropisi ile kurulum anı risk skoru | [U] |
| 1.2 | **İzin triadı tespiti** | Accessibility + Overlay + SMS/Notification kombinasyonlarının kural motoru ile yakalanması | [U] |
| 1.3 | **Erişilebilirlik kötüye kullanım koruması** | Yeni a11y servislerinin gesture/window yetkilerinin denetimi + onay ekranı uyarı overlay'i | [U] |
| 1.4 | **Overlay / tapjacking savunması** | Hassas ekran açıkken overlay sayımı; `HIDE_OVERLAY_WINDOWS` ile otomatik gizleme | [U] |
| 1.5 | **Şifreli trafik analizi (ETA)** | JA4 parmak izi, SNI, beacon zamanlama histogramı, ASN itibarı — payload açmadan C2 tespiti | [U] |
| 1.6 | **DNS ve DGA denetimi** | Lokal DNS gözlemi, entropy tabanlı DGA sınıflandırıcı, DoH kaçırma tespiti | [U] |
| 1.7 | **Sensör erişim denetimi** | AppOps ile kamera/mikrofon/konum kullanımının kim-ne-zaman kaydı; arka plan erişimi kırmızı bayrak | [U] |
| 1.8 | **Clipboard koruması** | Android 12+ pano erişim bildirimi + hassas içerik (IBAN, kripto adresi, OTP) desen tespiti ve otomatik temizleme | [U] |
| 1.9 | **Bildirim phishing filtresi** | NotificationListener ile sahte sistem/banka bildirimi tespiti, OTP çalma girişimi engelleme | [U] |
| 1.10 | **Stalkerware avcısı** | Gizli ikon + sürekli konum + a11y + otomatik başlatma paterni; ticari stalkerware imza seti | [U] |
| 1.11 | **Boot & bütünlük izleme** | Play Integrity + KeyMint attestation ile verified boot, bootloader kilidi, dm-verity durumu | [U] |
| 1.12 | **ADB / kablosuz hata ayıklama koruması** | `Settings.Global` izleme; aktifleşirse anında uyarı, Paranoid modda hassas veri kilitleme | [U] |
| 1.13 | **Root / Magisk / Zygisk tespiti** | Çok sinyalli tespit (attestation + dosya + prop + Zygisk davranışı); gizlenmiş root'a karşı dayanıklı | [U] |
| 1.14 | **Syscall davranış izleme** | eBPF ile `execve`/`openat`/`memfd_create` — fileless & LOTL tespiti | [R] |
| 1.15 | **Binder IPC denetimi** | `binder_transaction` tracepoint ile izin atlatma ve sistem servisi istismarı | [R] |
| 1.16 | **Çalıştırma anı engelleme** | `fanotify FAN_OPEN_EXEC_PERM` ile kötü binary'nin çalışmadan durdurulması | [R] |

### 4.2 Karar ve Otonomi

| # | Özellik | Teknik açıklama | Yetki |
|---|---|---|---|
| 2.1 | **Üç kademeli karar zinciri** | L0 filtre → L1 kural → L2 on-device model → L3 opsiyonel bulut | [U] |
| 2.2 | **Öngörücü risk skoru** | Uygulama zarar vermeden önce risk eğrisinin yükselişi; "risk trend" grafiği | [U] |
| 2.3 | **Açıklanabilir karar (XAI)** | Her verdict, kararı üreten 3 olayı attribution ağırlığıyla listeler — kara kutu yok | [U] |
| 2.4 | **Geri alınabilir yaptırım (Action Ledger)** | Her otonom eylem imzalı günlüğe yazılır ve tek dokunuşla geri alınabilir | [U] |
| 2.5 | **Süreç dondurma** | Öldürmek yerine askıya alma — adli kanıt korunur | [R] / [E] |
| 2.6 | **Otonom ağ karantinası** | Paket UID'sinin VpnService seviyesinde drop listesine alınması | [U] |
| 2.7 | **İzin geri alma** | Device Owner ile runtime izinlerinin programatik iptali | [E] |
| 2.8 | **Federated learning** | DP-SGD + secure aggregation ile veri paylaşmadan model iyileştirme | [U] |
| 2.9 | **OTA kural paketi** | L1 kuralları imzalı paket olarak günlük güncellenir; uygulama güncellemesi beklemez | [U] |

### 4.3 Koruma Katmanları

| # | Özellik | Teknik açıklama | Yetki |
|---|---|---|---|
| 3.1 | **Finansal Kalkan** | Banka/ödeme uygulaması ön plandayken: `FLAG_SECURE` zorlaması, overlay bloğu, ekran kaydı ve MediaProjection engeli, pano kilidi, a11y erişim reddi | [U] |
| 3.2 | **Zero Trust ağ katmanı** | Uygulama-başına ağ politikası (allow-list), varsayılan-reddet profili, alan adı/ASN bazlı kurallar | [U] |
| 3.3 | **Phishing & sosyal mühendislik kalkanı** | URL itibarı (lokal Bloom filtresi, sorgu göndermeden), tipo-squatting tespiti, sahte giriş ekranı tanıma | [U] |
| 3.4 | **Deepfake ses/görüntü uyarısı** | Görüntülü arama sırasında cihaz üstü sentetik-medya sınıflandırıcısı (frekans artefaktı + zamansal tutarsızlık); **tespit değil, uyarı** olarak konumlanır | [U] |
| 3.5 | **Sensör kesme** | Şüpheli uygulama kamera/mikrofona eriştiğinde: [E] AppOps ile anlık reddetme, [U] anlık uyarı + kullanıcı yönlendirme. *Sahte veri enjeksiyonu Android'de kullanıcı uygulaması olarak mümkün değildir — bu vaadi vermiyoruz.* | [E] / [U] |
| 3.6 | **Şifreli Kasa (Vault)** | AES-256-GCM, anahtar StrongBox'ta, biyometrik bağlı; gizli dosya + gizli uygulama alanı (work profile tabanlı) | [U] |
| 3.7 | **Kayıp cihaz modülü** | Uzaktan konum, kilitleme, seçici silme. **Gizli kamera/ses kaydı sunmuyoruz** — §6'ya bakınız | [U] |
| 3.8 | **Kendini koruma** | Watchdog süreç çifti, imza doğrulama (kendi APK'sı), anti-hook (frida/xposed tespiti), tamper-evident hash zincirli log | [U] |
| 3.9 | **Post-kuantum hazırlık** | ML-KEM-768 (Kyber) + X25519 hibrit anahtar değişimi; ML-DSA (Dilithium) ile kural paketi imzalama | [U] |

### 4.4 Kullanıcı Deneyimi Özellikleri

| # | Özellik | Teknik açıklama |
|---|---|---|
| 4.1 | **Guardian AI asistanı** | Doğal dilde soru-cevap: *"Bu uygulama ne yapıyor?"* → cihazdaki olay grafiğinden yanıt üretir. Küçük on-device LLM (~1–3B, quantized) + yapılandırılmış olay bağlamı |
| 4.2 | **Zaman çizelgesi (Timeline)** | Her uygulamanın ne zaman ne yaptığının kronolojik, filtrelenebilir kaydı |
| 4.3 | **Risk haritası** | Uygulama × yetenek matrisi, ısı haritası; ağ bağlantı grafiği |
| 4.4 | **"Neden bu izin?" ekranları** | UltraGuard'ın kendi istediği her izin için gerekçe + reddedilirse ne kaybedileceği |
| 4.5 | **Haftalık güvenlik brifingi** | Sade dilde özet: engellenen istekler, riskli uygulamalar, öneriler |
| 4.6 | **Wear OS eşlemesi** | Kritik uyarıların saate düşmesi + tek dokunuşla "durdur" onayı |
| 4.7 | **Şeffaflık merkezi** | UltraGuard'ın kendi ağ trafiğinin, gönderdiği her baytın kullanıcıya gösterilmesi |

### 4.5 Çalışma Modları

| Mod | Davranış |
|---|---|
| **Active** *(varsayılan)* | Tam telemetri, otonom geri-alınabilir müdahale, önemli olaylarda bildirim |
| **Stealth** | Aynı koruma, sessiz. Sadece KRİTİK bildirim çıkar; kalanı brifingde toplanır |
| **Paranoid** | Yan yükleme engelli, varsayılan-reddet ağ politikası, her yeni izin manuel onay, ADB açıksa Vault kilitli |
| **Fleet (Enterprise)** | Device Owner; uzaktan politika, uyumluluk raporu, zorunlu kural setleri, SIEM'e olay aktarımı (CEF/OTLP) |
| **Battery Saver Guard** | Pil %15 altında: L2 modeli askıya alınır, L1 kuralları ve ağ kalkanı çalışmaya devam eder |

---

## 5. Kullanıcı Arayüzü ve Deneyim Tasarımı

### 5.1 Tasarım İlkeleri

1. **Sükûnet varsayılandır.** Yeşil kalkan animasyonu yok, "TEHLİKE!" kırmızısı yok. Güvenlik ürünlerinin kullanıcıyı sürekli endişede tutması bir karanlık kalıptır ve uyarı körlüğü üretir.
2. **Her uyarı bir eylem içerir.** Eylemsiz bildirim gönderilmez.
3. **Otonom eylem önce yapılır, sonra anlatılır** — ama sadece geri alınabilirse.
4. **Teknik derinlik saklıdır, silinmez.** Ana ekran sade; her kartın altında tam olay zinciri var.

### 5.2 Ana Ekran (Dashboard)

```
┌─────────────────────────────────────────┐
│  UltraGuard              ⚙︎   ● Active  │
│                                         │
│         ┌───────────────────┐           │
│         │        94         │           │  ← Cihaz Güven Skoru
│         │   Cihaz Güveni    │           │     (0–100, ince halka)
│         └───────────────────┘           │     Renk: nötr gri-mavi,
│   "Son 24 saatte 3 istek engellendi"    │     düşükse amber
│                                         │
│  ┌───────────────────────────────────┐  │
│  │ ⚠ KargoTakip                      │  │  ← Dikkat kartı (varsa)
│  │   Ağ erişimi durduruldu · 2 dk    │  │
│  │   [ İncele ]      [ Geri al ]     │  │
│  └───────────────────────────────────┘  │
│                                         │
│  Son etkinlik                           │
│  ├ 14:22  Instagram → mikrofon (ön pl.) │
│  ├ 13:58  bilinmeyen.xyz engellendi     │
│  └ 11:04  WhatsApp güncellendi ✓        │
│                                         │
│  ┌────────┬────────┬────────┬────────┐  │
│  │ Zaman  │ Uygul. │  Ağ    │ Kasa   │  │
│  │ çizel. │  47    │ Zero-T │   🔒   │  │
│  └────────┴────────┴────────┴────────┘  │
│                                         │
│  💬 "Bu uygulama ne yapıyor?" diye sor  │  ← Guardian asistan girişi
└─────────────────────────────────────────┘
```

**Cihaz Güven Skoru** hesabı şeffaftır — dokununca kırılım açılır:
`Bütünlük (30) + Yama düzeyi (20) + Uygulama riski (25) + Yapılandırma (15) + Ağ hijyeni (10)`

### 5.3 Tehdit Detay Ekranı

Üç sekmeli yapı:

**① Ne oldu** — Sade dil özeti + otonom eylem listesi + geri alma düğmesi.

**② Kanıt** — Zaman damgalı olay zinciri, her satırda kaynak sensör. Attribution ağırlıkları çubuk olarak:
```
window_query(com.bank.mobile)  ████████░░  0.34
overlay_draw                   ███████░░░  0.29
net_connect(AS-XXXXX, kötü)    █████░░░░░  0.21
```

**③ Ne yapmalıyım** — Kademeli öneri: `Kaldır` (önerilen) / `Karantinada tut` / `Sadece izle` / `Güvenli olarak işaretle` (gerekçe sorulur, yerel modeli eğitir).

### 5.4 Uygulama Detay Ekranı

Her uygulama için "kimlik kartı": kaynak (Play / yan yükleme), imza yaşı, hedef SDK, istenen vs. **fiilen kullanılan** izinler (bu ayrım kritik — "konum izni var ama 90 gündür kullanmadı" güçlü bir sinyaldir), bağlandığı alan adları, risk trend grafiği.

### 5.5 Guardian AI Asistanı

Serbest metin veya sesle. Örnek diyalog:

> **Kullanıcı:** Bu fener uygulaması neden internete bağlanıyor?
>
> **Guardian:** Son 7 günde "Süper Fener" 41 kez `ads-delivery.example.net` adresine bağlandı ve toplam 2.3 MB veri gönderdi. Gönderilen veriler reklam kimliği ve cihaz modeliyle uyumlu boyutta. Ayrıca 3 gün önce yaklaşık konum izni istedi — bir fener uygulaması için gereksiz.
> Öneriyorum: konum iznini kaldırın ve bu uygulamayı Zero Trust listesinde "ağ yok" olarak işaretleyin.
> `[ İzni kaldır ]` `[ Ağını kes ]` `[ Detaylı olay listesi ]`

**Teknik:** Asistan cihazda çalışır ve **sadece yerel olay grafiğine erişir.** Yanıtlar yapılandırılmış verilerden üretilir; model sayı uydurursa gösterilmez — her rakam bir DB sorgusuna bağlanır (grounded generation, doğrulanamayan iddia filtrelenir).

### 5.6 Onboarding ve İzin Talebi

UltraGuard, kullanıcıdan Accessibility ve Notification Listener gibi güçlü izinler ister — yani kötü niyetli uygulamaların istediği izinlerin aynısını. Bu paradoks açıkça ele alınır:

> **Ekran 3/5 — "Neden erişilebilirlik izni istiyorum?"**
> Bankacılık trojanları bu izni ekranınızı okumak için kullanır. Ben aynı izni **başka bir uygulamanın bunu yaptığını görmek için** kullanıyorum. Onsuz, ekran üstü saldırıları tespit edemem.
> **Bu izinle ne yapmıyorum:** ekran içeriğinizi kaydetmiyorum, cihazınızdan dışarı hiçbir ekran verisi göndermiyorum. Bunu Şeffaflık Merkezi'nde kendiniz doğrulayabilirsiniz.
> `[ İzin ver ]` `[ Şimdilik atla — sınırlı korumayla devam et ]`

Atlama her zaman mümkündür ve ürün çalışmaya devam eder. Zorunlu izin yok.

---

## 6. Gizlilik ve Etik Yaklaşım

### 6.1 Temel Taahhüt

> Bir güvenlik uygulaması, koruduğu tehditten daha büyük bir gözetim aracı olmamalıdır.

Piyasadaki mobil AV ürünlerinin çoğu, tarama bahanesiyle kurulu uygulama listesini, gezinti geçmişini ve cihaz kimliğini sunucuya gönderir. UltraGuard'ın ayırt edici duruşu budur.

### 6.2 Veri Sınıflandırması

| Veri | Nerede işlenir | Cihazdan çıkar mı |
|---|---|---|
| Olay akışı (ham telemetri) | Cihaz | **Asla** |
| Ekran içeriği (a11y) | Cihaz, RAM'de, kalıcılaştırılmaz | **Asla** |
| Bildirim içeriği | Cihaz, sadece desen eşleme | **Asla** |
| Ağ metadata (SNI/IP) | Cihaz | **Asla** |
| Kurulu uygulama listesi | Cihaz | **Asla** (bulut sorgusu hash + k-anonimite ile) |
| APK hash (SHA-256) | L3 sorgusunda | Opsiyonel, kullanıcı onaylı |
| Davranış vektörü (anonim, 24-boyut) | L3 sorgusunda | Opsiyonel, kullanıcı onaylı, k≥50 |
| Model gradyanları | FL turunda | Opsiyonel, DP gürültülü |
| Çökme raporu | — | Opsiyonel, PII temizlenmiş |

**L3 bulut konsültasyonu varsayılan olarak KAPALIDIR.** Ürün, bulut olmadan tam işlevseldir.

### 6.3 Doğrulanabilir Şeffaflık

Söylemek yetmez, kanıtlanabilir olmalı:

1. **Şeffaflık Merkezi:** Uygulama içinde, UltraGuard'ın kendi yaptığı her ağ isteği listelenir — hedef, boyut, içerik özeti. Kullanıcı kendi ürününü kendi ürünüyle denetleyebilir.
2. **Yeniden üretilebilir derleme (reproducible builds):** Yayınlanan APK, açık kaynak koddan bit-bit yeniden üretilebilir. Bağımsız araştırmacılar "gönderdiğimiz kod, söylediğimiz kod mu?" sorusunu doğrulayabilir.
3. **Açık kaynak çekirdek:** `core/`, `sensors/`, `ai/inference` modülleri açık kaynak (AGPL). Kapalı kalan: tehdit istihbaratı beslemesi ve kural paketi.
4. **Yıllık bağımsız denetim:** Kod ve gizlilik denetimi, tam rapor kamuya açık.

### 6.4 Etik Sınırlar — Yapmayacaklarımız

Bu bölüm bilinçli olarak bir *yapılacaklar* listesi kadar önemlidir:

| Talep edilen özellik | Kararımız | Gerekçe |
|---|---|---|
| **Gizli kamera/ses kaydı (kayıp cihaz)** | ❌ Uygulanmayacak | Bu özellik, kaybolan telefondan çok **partner takibi** için kullanılır. Stalkerware'e karşı savaşan bir ürün, stalkerware yeteneği taşıyamaz. Bunun yerine: konum, kilitleme, sahibine mesaj gösterme, seçici silme. |
| **Gizli mod / ikon gizleme (aile modu)** | ❌ Uygulanmayacak | İzlenen kişi izlendiğini bilmelidir. Fleet modunda bile cihazda kalıcı, kaldırılamayan bir "yönetiliyor" göstergesi bulunur. |
| **Ebeveyn için mesaj içeriği okuma** | ❌ Uygulanmayacak | Sadece **tehdit sınıflandırması** yapılır (phishing/grooming paterni uyarısı), içerik ebeveyne aktarılmaz. |
| **TLS MITM (sertifika enjeksiyonu)** | ❌ Uygulanmayacak | §2.2.1 — çözdüğünden büyük risk üretir. |
| **Sensöre sahte veri enjeksiyonu** | ❌ Uygulanmayacak | Root'suz teknik olarak imkânsız; root'lu cihazda bile kırılgan ve yanıltıcı bir güvenlik hissi verir. |
| **Rakip AV'yi devre dışı bırakma** | ❌ Uygulanmayacak | Kullanıcının seçimi kullanıcınındır. |

**Fleet (Enterprise) modu için ek ilke:** İşveren cihazın *uyumluluk durumunu* görür (yama düzeyi, root durumu, riskli uygulama sayısı). İşveren **hangi uygulamaların kurulu olduğunu, kişisel kullanım verisini veya konumu göremez** — cihaz BYOD ise. Work profile ayrımı zorunludur.

### 6.5 Yasal Uyum

- **KVKK / GDPR:** Cihaz-üstü işleme ağırlıklı mimari, veri sorumlusu yükünü minimuma indirir. Açık rıza yalnızca L3 ve FL için alınır, ayrı ayrı ve geri alınabilir şekilde.
- **Google Play politikaları:** Accessibility API kullanımı, "güvenlik izleme" gerekçesiyle beyan edilir ve uygulama içi açıklama ekranı Play şartlarını karşılar. `QUERY_ALL_PACKAGES` için AV muafiyeti başvurusu yapılır.
- **Veri saklama:** Yerel olay verisi varsayılan 30 gün (ayarlanabilir 7–90). Kullanıcı tek dokunuşla tümünü siler.

---

## 7. Rekabet Analizi

### 7.1 Karşılaştırma

| Yetenek | Play Protect | Bitdefender | Kaspersky | Norton | Malwarebytes | **UltraGuard** |
|---|---|---|---|---|---|---|
| Tespit temeli | Bulut imza + ML | Bulut imza | Bulut imza + heuristik | Bulut imza | İmza + heuristik | **Cihaz-üstü davranış dizisi** |
| Sıfırıncı gün | Sınırlı | Sınırlı | Orta | Sınırlı | Sınırlı | **Birincil tasarım hedefi** |
| Karar yeri | Bulut | Bulut | Bulut | Bulut | Karma | **%95+ cihaz** |
| Açıklanabilirlik | ✗ | ✗ | ✗ | ✗ | ✗ | **✓ Olay bazlı attribution** |
| Otonom müdahale | Kaldırma önerisi | Uyarı | Uyarı | Uyarı | Uyarı | **✓ Geri alınabilir yaptırım** |
| Ağ katmanı | ✗ | VPN (trafik şirkete) | VPN | VPN | ✗ | **Lokal ZTNA, tünelsiz** |
| A11y kötüye kullanım tespiti | Kısmi | ✗ | ✗ | ✗ | ✗ | **✓ Çekirdek özellik** |
| Stalkerware | Kısmi | Kısmi | ✓ (iyi) | Kısmi | ✓ | **✓ Davranışsal, imzasız da yakalar** |
| Kurulu uygulama listesini gönderir mi | ✓ (Google) | ✓ | ✓ | ✓ | ✓ | **✗ Asla** |
| Root'lu cihazda derin izleme | ✗ | ✗ | ✗ | ✗ | ✗ | **✓ eBPF/Binder [R]** |
| Açık kaynak çekirdek | ✗ | ✗ | ✗ | ✗ | ✗ | **✓ AGPL** |
| Kullanıcı zaman çizelgesi | ✗ | ✗ | Kısmi | ✗ | ✗ | **✓ Tam** |

### 7.2 Dürüst Zayıflıklarımız

Rakiplerin bizden güçlü olduğu noktalar — bunları bilmeden strateji kurulamaz:

- **İmza veri tabanı olgunluğu:** Kaspersky'nin 25 yıllık örnek havuzu var; bizim yok. **Telafi:** Açık kaynak beslemeler (MalwareBazaar, abuse.ch, AndroZoo) + davranışsal tespitin imzaya bağımlı olmaması + partnerlik.
- **Marka güveni ve dağıtım:** Norton'un perakende kanalı ve operatör anlaşmaları var. **Telafi:** Güvenlik araştırmacısı topluluğundan başlayan aşağıdan yukarı büyüme + açık kaynak/denetim güvenilirliği.
- **AV-TEST / AV-Comparatives sertifikaları:** Bu testler statik APK setleriyle imza tespitini ölçer; davranışsal tespit bu metodolojide düşük skor alabilir. **Telafi:** MITRE ATT&CK Mobile matrisi tabanlı ve canlı-örnek testlere katılım; bu değerlendirmeleri sektör standardı haline getirmek için lobi.
- **Yanlış pozitif riski:** Davranışsal tespit, imza tespitinden daha çok yanlış alarm üretme eğilimindedir. **Telafi:** L1'in yüksek kesinlikli kurallarla ön filtreleme yapması, otonom eylemin geri alınabilirle sınırlanması, kullanıcı geri bildiriminin yerel eşiği ayarlaması.

### 7.3 Savunulabilir Fark (Moat)

Kopyalanması en zor üç varlık:

1. **Cihaz-üstü davranış dizi modeli + FL altyapısı.** Rakiplerin bulut-merkezli mimarileri buna geçiş için baştan yeniden yazım gerektirir.
2. **Açıklanabilirlik + geri alınabilir otonomi.** Bu bir özellik değil, bir mimari karardır (Action Ledger her katmana dokunur).
3. **Doğrulanabilir gizlilik duruşu.** Reproducible build + açık kaynak çekirdek, ticari rakiplerin iş modelleri gereği (veri toplama) taklit edemeyeceği bir konumdur.

---

## 8. Geliştirme Yol Haritası

### MVP — 0–3 Ay · "Görüyorum"

**Hedef:** Telemetri ve korelasyon iskeletinin çalıştığını kanıtlamak. Tespit henüz kural tabanlı.

| Alan | Teslim |
|---|---|
| Mimari | Gradle modül iskeleti, `core/runtime` foreground service + watchdog, Room + SQLCipher deposu |
| Sensör | Paket yaşam döngüsü, AppOps izleme, NotificationListener, AccessibilityService (temel) |
| Korelasyon | Olay normalizasyonu, varlık grafiği v1, 64-adım kayan pencere |
| Karar | **Sadece L1 kural motoru** (~60 kural), statik APK triyajı (izin/imza/entropi) |
| UI | Compose dashboard, zaman çizelgesi, uygulama detayı, onboarding izin gerekçe ekranları |
| Kalite | Pil ölçüm harness'ı (Batterystats + Perfetto), 3 referans cihazda 24 saat regresyon |

**Çıkış kriteri:** 24 saatte < %2 pil, 500 örnekli test setinde ≥ %70 tespit, < %3 yanlış pozitif.

---

### v1.0 — 3–9 Ay · "Anlıyorum"

**Hedef:** Ürünü pazara çıkarılabilir hale getirmek. AI ve otonomi devrede.

| Alan | Teslim |
|---|---|
| AI | L2 dizi modeli (INT8, LiteRT + NNAPI), self-supervised ön eğitim + supervised head, attribution çıkışı |
| Ağ | VpnService tabanlı lokal ZTNA, uygulama-başına politika, ETA (JA4 + beacon analizi), DNS/DGA denetimi |
| Otonomi | Action Ledger, geri alınabilir yaptırım zinciri, Active/Stealth/Paranoid modları |
| Koruma | Finansal Kalkan (overlay + ekran kaydı + pano), a11y kötüye kullanım savunması, stalkerware avcısı |
| Güvenlik | Kendini koruma (watchdog + imza doğrulama + anti-hook), StrongBox anahtar yönetimi, PQ hibrit TLS |
| UX | Guardian AI asistanı v1 (kural tabanlı NLU + şablonlu yanıt), Şeffaflık Merkezi |
| Süreç | Reproducible build pipeline, çekirdek modüllerin açık kaynak yayını, bağımsız güvenlik denetimi |

**Çıkış kriteri:** MITRE ATT&CK Mobile teknik kapsamı ≥ %60, canlı örnek setinde ≥ %88 tespit, < %1.5 yanlış pozitif, 24 saatte < %1.5 pil. Play Store yayını.

---

### v1.5 — 9–14 Ay · "Öğreniyorum"

| Alan | Teslim |
|---|---|
| AI | Federated learning üretime alınır (DP-SGD, secure aggregation, Krum agregasyonu) |
| AI | Guardian asistan v2 — gerçek on-device LLM (1–3B quantized), grounded generation |
| Koruma | Deepfake ses/görüntü uyarı modülü, phishing kalkanı v2 (lokal Bloom filtresi) |
| Ürün | Wear OS eşlemesi, tablet uyarlaması, Şifreli Kasa |
| B2B | **UltraGuard Fleet** — Device Owner, uzaktan politika, uyumluluk raporlama, SIEM aktarımı (CEF/OTLP) |

---

### v2.0 — 14–24 Ay · "Derinleşiyorum"

| Alan | Teslim |
|---|---|
| Derin izleme | `module-deepscan` [R]: eBPF syscall izleme, Binder IPC denetimi, `fanotify` çalıştırma engeli, cgroup freezer |
| Kripto | Tam PQ geçişi: ML-KEM-768 hibrit anahtar değişimi, ML-DSA imzalı kural paketleri, PQ-korumalı Vault |
| Ekosistem | Açık tehdit istihbaratı federasyonu (STIX/TAXII), araştırmacı API'si, bug bounty programı |
| Platform | Custom ROM entegrasyonu (GrapheneOS/LineageOS için sistem imzalı varyant), OEM ön yükleme görüşmeleri |

---

### 8.1 Kritik Riskler ve Azaltım

| Risk | Olasılık | Etki | Azaltım |
|---|---|---|---|
| **Play Store, Accessibility gerekçemizi reddeder** | Orta | Kritik | Erken politika ön-başvurusu; a11y'siz de çalışan çekirdek; yedek dağıtım kanalı (doğrudan APK + F-Droid) |
| **NPU/NNAPI parçalanması — modelin bazı cihazlarda yavaş kalması** | Yüksek | Orta | XNNPACK CPU fallback + cihaz yetenek profillemesi + model boyut kademeleri (S/M/L) |
| **Davranışsal tespit yanlış pozitif oranı yüksek çıkar** | Orta | Yüksek | L1 ön filtresi, geri-alınabilir eylem sınırı, agresif beta programı, uygulama-başına kalibrasyon |
| **Pil tüketimi kabul edilemez seviyede** | Orta | Kritik | Her PR'da otomatik pil regresyon testi; bütçe aşımı = merge bloğu |
| **eBPF kernel parçalanması ([R] modülü çalışmaz)** | Yüksek | Düşük | Opsiyonel modül; GKI 5.10+ hedefi; yoksa `procfs`/`inotify` fallback |
| **Rakip veya OEM, VpnService'imizi engeller** | Düşük | Yüksek | Standart API kullanımı, OEM sertifikasyon programlarına katılım |
| **Ürünün kendisi saldırı hedefi olur** | Orta | Kritik | Açık kaynak + bug bounty + bağımsız denetim + minimum saldırı yüzeyi (deepscan ayrı APK) |

---

## 9. Kapanış Değerlendirmesi

UltraGuard OS Shield'in tezinin özü tek cümlede toplanır: **2026–2030 mobil tehditleri imza bırakmaz, davranış bırakır.** AI ile üretilen sosyal mühendislik saldırıları ve otonom casus yazılımlar her kurulumda farklı bir binary üretebilir — ama hepsi aynı eylem dizisini izlemek zorundadır, çünkü Android'in yetenek modeli onlara başka yol bırakmaz. Tespiti bu değişmez katmana kurmak, ürünü tehdidin evrimine karşı dayanıklı kılar.

İkinci ve daha az bariz tez: **gizlilik bir pazarlama argümanı değil, mimari bir avantajdır.** Kararı cihazda vermek yalnızca kullanıcıyı korumaz; gecikmeyi düşürür, çevrimdışı çalışmayı mümkün kılar, bulut maliyetini ortadan kaldırır ve rakiplerin veri toplamaya dayalı iş modelleriyle kopyalayamayacağı bir konum yaratır.

Bu tasarımın gerçekçiliği, ne vaat etmediğinde saklıdır. TLS'i kırmıyoruz, sensöre sahte veri enjekte etmiyoruz, gizli kamera kaydı sunmuyoruz, root'suz cihazda kernel görünürlüğü iddia etmiyoruz. Android'in gerçek kısıtları içinde, o kısıtların izin verdiği en derin görünürlüğü ve en hızlı müdahaleyi kuruyoruz — ve bunun sınırlarını kullanıcıya açıkça söylüyoruz.

> **UltraGuard OS Shield — Taramaz. Anlar.**
