# CHIMERA — Self-Mutating Autonomous Defense Fabric

> **Sınıflandırma:** Internal / Stealth
> **Doküman tipi:** Architecture Design Record (ADR-0000)
> **Hedef okuyucu:** CTO, Head of Infrastructure Security, Lead Platform Engineer
> **Dağıtım modeli:** %100 air-gapped, tek binary, sıfır bağımlılık

---

## 1. Projenin Adı ve Milyar Dolarlık Değer Önerisi

### 1.1 İsim

**CHIMERA** — *Cybernetic Hardened Immune Mesh for Autonomous Response*

Kod adı seçimi tesadüf değil: Chimera, tek bir organizmada birden fazla genetik kimlik taşıyan canlıdır. Ürünün temel iddiası da budur — **korunan sistemin her 90 saniyede bir başka bir sistem gibi görünmesi**, ama kendi kimliğini hiç kaybetmemesi.

Ticari isimlendirme katmanları:

| Katman | Ürün adı | Ne satıyor |
|---|---|---|
| Kontrol düzlemi | **CHIMERA Core** | Otonom savunma çekirdeği |
| Mutasyon motoru | **Proteus** | Polymorphic Moving Target Defense |
| Ajan sürüsü | **Triad** | 3 ajanlı otonom yama sürüsü |
| Veri kalkanı | **Obsidian** | Post-quantum ZK veri katmanı |
| Kurulum sistemi | **Monolith** | Zero-click, air-gapped deployment |

### 1.2 Problemin ekonomik büyüklüğü

Bir Fortune 500 kurumunda kritik bir CVE'nin yayınlanmasından üretime yama gitmesine kadar geçen medyan süre sektör raporlarında **haftalar** mertebesindedir. Bu süre şu üç darboğazdan oluşur:

1. **Triage kuyruğu** — zafiyet gerçekten bizi ilgilendiriyor mu? (günler)
2. **Yama yazımı ve regresyon riski** — ekip kapasitesi (günler–haftalar)
3. **Değişiklik yönetimi (CAB)** — onay ve bakım penceresi (günler)

Saldırgan tarafında ise exploit'in silahlandırılması saatler mertebesine inmiş durumdadır. Yani savunmanın OODA döngüsü, saldırganınkinden **iki büyüklük mertebesi** yavaştır. CHIMERA'nın tek cümlelik değer önerisi budur:

> **CHIMERA, savunmanın OODA döngüsünü saldırganınkinin altına indirir.**

### 1.3 Neden yılda milyonlarca dolar ödenir

Dört ayrı bütçe kaleminden aynı anda beslenir — bu, satın alma sürecinde tek bir bütçe sahibine mahkûm olmamak demektir:

| Bütçe kalemi | Bugünkü harcama | CHIMERA'nın etkisi |
|---|---|---|
| SOC L1/L2 insan gücü | 20–60 analist | Triage'ın büyük kısmı otonomlaşır, insan L3'e çıkar |
| Breach beklenen maliyeti | Olay başına 7–8 haneli | MTTR saatlerden dakikalara iner |
| Siber sigorta primi | Yıllık 7 haneli | Otonom MTTR kanıtı prim pazarlığında kaldıraçtır |
| Uyumluluk denetimi | Sürekli | Her mutasyon ve yama, imzalı ve değişmez şekilde loglanır |

### 1.4 Neden bu ürün *lokal* olmak zorunda (asıl savunma hendeği)

Rakiplerin tamamı SaaS. Ve SaaS güvenlik ürünlerinin satılamadığı üç pazar var:

- **Savunma sanayii / kamu** — veri ülke dışına, hatta ağ dışına çıkamaz
- **Kritik altyapı (OT/ICS/SCADA)** — fiziksel olarak air-gapped, Purdue Level 0–2
- **Regüle finans ve sağlık** — veri ikametgâhı ve üçüncü taraf risk kısıtları

Bu üç pazar, güvenlik harcamasının en yoğun olduğu ama SaaS'ın yapısal olarak giremediği pazarlardır. CHIMERA'nın tek binary, internetsiz, kendi kendine yeten mimarisi bir "kurulum kolaylığı özelliği" değil, **doğrudan pazara giriş hakkıdır**.

**Fiyatlama tezi:** korunan node başına yıllık abonelik + air-gapped site lisansı. Kurumsal satışta karşılaştırma noktası bir SIEM lisansı değil, **iki tam zamanlı SOC analistinin yıllık maliyetidir** — ki bu, tek bir orta ölçekli dağıtımın kendini finanse etmesine fazlasıyla yeter.

---

## 2. Core Engine — Siber Güvenlik Mimarisi

CHIMERA üç bağımsız ama **aynı kriptografik saate bağlı** alt sistemden oluşur. Bu "tek saat" tasarımı, dokümanın 6. bölümündeki asıl mühendislik hilesinin temelidir.

```
                        ┌──────────────────────────────┐
                        │   EPOCH CLOCK (HKDF chain)   │
                        │  root_seed → E(n) → E(n+1)   │
                        └───────┬──────────┬───────────┘
                                │          │
              ┌─────────────────┘          └─────────────────┐
              ▼                                              ▼
   ┌──────────────────────┐                      ┌──────────────────────┐
   │  PROTEUS (MTD)       │                      │  OBSIDIAN (ZK Shield)│
   │  eBPF/XDP + nftables │                      │  ML-KEM + XChaCha20  │
   │  şema permütasyonu   │                      │  anahtar rotasyonu   │
   └──────────┬───────────┘                      └──────────┬───────────┘
              │                                             │
              └──────────────────┬──────────────────────────┘
                                 ▼
                    ┌────────────────────────────┐
                    │  TRIAD (LLM Swarm)         │
                    │  Sentinel → Artificer      │
                    │        → Adjudicator       │
                    │  2/3 FROST eşik imzası     │
                    └────────────────────────────┘
```

### 2.1 PROTEUS — Polymorphic Moving Target Defense

Saldırganın keşif (reconnaissance) aşamasında topladığı her bilgiyi, o bilgiyi kullanabilmesinden önce geçersiz kılmak.

#### 2.1.1 Ağ katmanı mutasyonu

- **Taşıyıcı:** her node üzerinde WireGuard tabanlı bir overlay. Gerçek servis adresleri asla underlay'de görünmez.
- **Mutasyon birimi:** her servis, overlay içinde bir `/112` IPv6 bloğundan **epoch başına deterministik türetilen** bir adres alır:

  ```
  addr(service, epoch) = prefix ‖ truncate_48( HMAC-SHA256( K_net(epoch), service_id ) )
  ```

- **Veri düzlemi:** adres yeniden yazımı kullanıcı alanında değil, **eBPF/XDP** programında yapılır. `bpf_map` içine yazılan epoch tablosu ile paket, NIC sürücüsünün hemen üstünde yeniden hedeflenir. Bu, mutasyonun bağlantı başına ~mikrosaniye mertebesinde maliyeti olması demektir — MTD'nin klasik "performans yıkıcı" itirazını ortadan kaldırır.
- **Bağlantı sürekliliği:** epoch geçişinde eski adres bir "grace window" (varsayılan 15 sn) boyunca `CT_ESTABLISHED` conntrack girdileri için yaşamaya devam eder; **yeni** SYN'ler yalnızca yeni adrese kabul edilir. Yani meşru oturumlar hiç kopmaz, saldırganın önceden haritaladığı adres ise yeni bağlantı için ölüdür.
- **Kapalı kapı:** her yeni bağlantı **Single Packet Authorization** ile açılır. UDP üzerinden gelen tek, tekrar-korumalı (nonce + monotonic counter), ML-DSA ile imzalı bir paket olmadan port `DROP` durumundadır — `RST` bile dönmez. Nmap taraması için host **yok** görünür.

#### 2.1.2 Uygulama katmanı mutasyonu (asıl fark burada)

IP değiştirmek 2015'in MTD'sidir. CHIMERA **veri yapısını** da mutasyona uğratır:

- İç servisler arası tüm iletişim Protobuf/Cap'n Proto üzerinden yapılır.
- Wire formatındaki **alan numaraları epoch başına permüte edilir**:

  ```
  σ(epoch) = FisherYates( fields, seed = HKDF(K_app(epoch), "field-permutation") )
  ```

- İstemci ve sunucu aynı epoch anahtarına sahip olduğu için permütasyonu bağımsız hesaplar; ortada hiçbir "şema müzakeresi" trafiği yoktur.
- Sonuç: sızdırılmış bir servis token'ıyla API'ye ulaşan saldırgan, **geçerli bir istek gövdesi kuramaz**. Fuzzing ile öğrendiği alan haritası bir sonraki epoch'ta çöptür. Otomatik exploit araçlarının tamamı burada ölür.

#### 2.1.3 Tetiklenmiş hızlanma

Epoch süresi statik değildir; **tehdit sinyaline göre kısalır**:

| Durum | Epoch süresi |
|---|---|
| Normal | 300 sn |
| Anomali skoru > eşik | 90 sn |
| Aktif exploit denemesi doğrulandı | 10 sn |
| Lateral movement şüphesi | 3 sn + segment izolasyonu |

Saldırgan ne kadar gürültü yaparsa labirent o kadar hızlı yeniden şekillenir. Bu, saldırganın kendi keşif faaliyetini kendi aleyhine çeviren bir negatif geri besleme döngüsüdür.

### 2.2 TRIAD — LLM-Swarm Tabanlı Otonom Yama

Üç ajan, **farklı model aileleri** üzerinde çalışır. Bu bilinçli bir tercihtir: aynı modelin iki kopyası aynı hatayı yapar ve birbirini onaylar. Farklı aileler, korelasyonsuz hata dağılımı verir — bu, sürünün doğrulama gücünün matematiksel temelidir.

| Ajan | Rol | Model sınıfı (lokal, GGUF) | Bağlam |
|---|---|---|---|
| **Sentinel** | Tespit, tekrar-üretim (repro), etki analizi | 7–8B instruct, düşük gecikme | 32K |
| **Artificer** | Yama sentezi | 14–32B code-specialized | 64K |
| **Adjudicator** | Düşmanca doğrulama, veto yetkisi | Farklı aileden 14B+ reasoning | 64K |

Ek olarak bir **Warden** (3B guard modeli) tüm ajan çıktılarını politika ihlali açısından süzer; yalnızca `DENY/ALLOW` döndürür ve yama içeriğini asla üretmez.

#### 2.2.1 60 saniyelik döngü — bütçelenmiş zaman çizelgesi

```
T+00.0s  Sinyal      : IDS/eBPF probe/dependency delta → olay kuyruğuna düşer
T+00.5s  Sentinel    : RAG ile kod grafiği + CVE korpusu sorgulanır (HNSW, ~8ms)
T+04.0s  Sentinel    : failing test / PoC üretilir → izole sandbox'ta ÇALIŞTIRILIR
T+06.0s  GATE-1      : Test kırmızı mı? Değilse olay "not exploitable" kapatılır
T+06.5s  Artificer   : minimal diff üretir (yalnızca AST alt ağacı, tüm dosya değil)
T+22.0s  Artificer   : diff → derleme + statik analiz + unit suite
T+30.0s  Adjudicator : diff'i DÜŞMAN gözüyle inceler; bypass PoC yazmaya çalışır
T+42.0s  GATE-2      : property-based fuzz (cargo-fuzz/AFL++) 8 sn, differential test
T+48.0s  Canary      : shadow deploy — trafiğin %1'i, CoW snapshot üzerinde
T+56.0s  GATE-3      : SLO deltası (p99 gecikme, hata oranı) eşik içinde mi?
T+58.0s  Quorum      : 2/3 FROST eşik imzası toplanır
T+60.0s  Promote     : atomik slot geçişi. Rollback tek `renameat2(RENAME_EXCHANGE)`
```

#### 2.2.2 Neden bu "AI'a kod yazdırıp prod'a atmak" değil

Bu ayrımı bir CTO'ya net anlatmak kritik. Dört yapısal kısıt var:

1. **Kanıt yükü ajanda, insanda değil.** Yama, *önce yazılmış ve kırmızı olduğu görülmüş* bir testi yeşile çevirmek zorundadır. Test önce gelir; yama sonra.
2. **Kriptografik karar kuralı.** Promosyon için 3 ajandan 2'sinin **FROST eşik imzası** gerekir. Tek bir ajanın halüsinasyonu, imza toplayamadığı için matematiksel olarak üretime çıkamaz. Bu bir "prompt kuralı" değil, protokol kuralıdır.
3. **Kapsam kilidi (capability lock).** Artificer'ın yazma yetkisi, Sentinel'in etki analizinde işaretlediği AST düğümleriyle sınırlıdır. Bu kısıt modelin isteğine bırakılmaz; patch applier, kapsam dışı hunk'ları **reddeder**.
4. **Geri alma, ileri almadan ucuzdur.** Her promosyon bir CoW snapshot üstünde yapılır. Rollback maliyeti sabit ve milisaniyeliktir; bu yüzden yanlış pozitifin beklenen maliyeti düşüktür — sistemin agresif davranabilmesinin sebebi budur.

> **Dürüst sınır:** Bu döngü mantık hataları, injection sınıfı zafiyetler, bağımlılık CVE'leri ve konfigürasyon kaymaları için tasarlanmıştır. Mimari düzeyde tasarım kusurlarını çözmez — orada sistem yama üretmez, **izole eder ve insanı çağırır**. Bir güvenlik ürününün en tehlikeli özelliği, çözemediği şeyi çözdüğünü sanmasıdır; TRIAD'ın `ESCALATE` durumu birinci sınıf bir çıktıdır.

### 2.3 OBSIDIAN — Zero-Knowledge Veri Kalkanı

Tehdit modeli varsayımı: **saldırgan diski tamamen aldı.** Elinde ne kalmalı? Matematiksel gürültü.

#### 2.3.1 Kriptografik yığın

| Amaç | Algoritma | Not |
|---|---|---|
| Simetrik şifreleme | XChaCha20-Poly1305 | 192-bit nonce → rastgele nonce güvenli |
| Anahtar sarmalama | **ML-KEM-1024** (Kyber) | NIST FIPS 203 |
| İmza | **ML-DSA-87** (Dilithium) | NIST FIPS 204 |
| Yedek imza | SLH-DSA (SPHINCS+) | Hash-tabanlı, kırılma çeşitliliği için |
| Hibrit anahtar değişimi | X25519 **+** ML-KEM-1024 | İkisi de kırılmadıkça güvenli |
| Parola türetme | Argon2id (m=1GiB, t=4, p=4) | |
| Bütünlük | BLAKE3 Merkle ağacı | Parçalı doğrulama, tam dosya okumadan |

Hibrit tasarım kritik: post-quantum algoritmalar klasiklerin **yerine** değil, **yanına** konur. Kyber'de yarın bir yapısal saldırı çıkarsa X25519 hâlâ ayaktadır.

#### 2.3.2 Anahtar hiyerarşisi ve "hiç birleşmeyen anahtar"

Master anahtar hiçbir zaman diskte, hiçbir zaman tek parça hâlinde bulunmaz. Shamir 2-of-3 paylaşımı:

```
Share A : TPM 2.0 NV index'te seal'lenmiş (PCR 0,2,4,7'ye bağlı)
Share B : Argon2id(kullanıcı parolası, device-bound salt)
Share C : Offline kurtarma zarfı (kâğıt/HSM, olağan durumda kullanılmaz)
```

Normal açılış A+B ile olur. Disk başka bir makineye takılırsa PCR değerleri değişir, TPM unseal reddeder → Share A yok → **veri erişilemez**. Parola brute-force edilse bile tek başına yetmez.

Bellekte anahtar materyali `mlock`'lu sayfalarda tutulur, `zeroize` ile silinir, çekirdek core dump'ı `PR_SET_DUMPABLE=0` ile kapatılır.

#### 2.3.3 Vektör veritabanı üzerinde şifreli arama

En sık gözden kaçan sızıntı yüzeyi budur: RAG sistemlerinde **embedding'ler düz metinden neredeyse geri çevrilebilir** (embedding inversion saldırıları). Bu yüzden CHIMERA vektörleri açıkta tutmaz:

- Her embedding, epoch anahtarından türetilen bir **rastgele ortogonal matris** `Q(epoch)` ile döndürülür. Ortogonal dönüşüm iç çarpımı ve kosinüs benzerliğini **korur** — yani HNSW indeksi hiçbir doğruluk kaybı olmadan şifreli uzayda çalışır.
- `Q` bilinmeden vektör, izotropik gürültüden ayırt edilemez.
- Ham metin parçaları (chunk) ayrıca XChaCha20 ile şifrelidir; yalnızca top-k sonuç, ajan bağlamına yazılırken jailed bir bellek arenasında çözülür.
- Sonuç: disk imajını alan saldırganın elinde **döndürülmüş vektörler ve şifreli chunk'lar** kalır. Bu, tam anlamıyla matematiksel gürültüdür.

---

## 3. The Wrapper — Zero-Click Deployment Mimarisi (MONOLITH)

### 3.1 Tasarım aksiyomları

1. Kullanıcı **hiçbir şey** kurmaz. Docker yok, Python yok, CUDA toolkit yok, runtime yok.
2. Binary **statik** linklenir. Linux'ta `x86_64-unknown-linux-musl`; glibc sürüm cehennemi yok.
3. Ağ erişimi **kod yolunda yoktur**. Air-gapped mod bir bayrak değil, varsayılandır.
4. Kurulum **idempotent ve tersinirdir**. Tek komutla iz bırakmadan kalkar.

### 3.2 Teknoloji seçimleri ve gerekçeleri

| Katman | Seçim | Neden bu, alternatifi neden değil |
|---|---|---|
| Çekirdek dil | **Rust** | GC yok → watchdog gecikmesi öngörülebilir; FFI sıfır maliyet; `unsafe` sınırları denetlenebilir |
| LLM motoru | **llama.cpp**, statik kütüphane olarak linklenmiş | GGUF kuantizasyon ekosistemi, mmap desteği, CUDA/Metal/Vulkan/CPU tek API |
| Vektör DB | **usearch** (HNSW) + **redb** (KV) | İkisi de gömülebilir, sunucu süreci yok. Qdrant/Milvus ayrı süreç ister → reddedildi |
| Yapılandırılmış depo | **SQLite** (bundled, WAL) | Tek dosya, sıfır yönetim |
| Ağ veri düzlemi | **eBPF/XDP** (`aya`, saf Rust, libbpf bağımlılığı yok) | Kernel modülü yok → imzalama ve Secure Boot sorunu yok |
| Overlay | **boringtun** (userspace WireGuard) | Kernel modülü gerektirmez, air-gapped kurulumda sürtünmesiz |
| UI (opsiyonel) | **Tauri** | İşletim sisteminin kendi WebView'ini kullanır → Electron'un ~150MB Chromium'u yok |
| Paketleme | Kendi **`.mono` konteyner** formatımız | Aşağıda |
| Windows installer | **kendi binary'miz** (`/silent` modu) | InnoSetup/MSI yok: installer'ın kendisi üründür |

### 3.3 `.mono` konteyner formatı — dosya sistemini binary'nin içine gömmek

Standart yaklaşım (`include_bytes!`) 20 GB model için imkânsızdır: derleme belleği patlar ve her açılışta RAM'e kopyalanır. CHIMERA bunun yerine **binary'nin sonuna eklenmiş, seek edilebilir bir arşiv** kullanır:

```
┌────────────────────────────────────────────────────────────┐
│  ELF / PE  (chimera çalıştırılabilir, ~14 MB)              │
├────────────────────────────────────────────────────────────┤
│  BLOB REGION                                                │
│   ├─ zstd-seekable frame'leri (kod, config, WASM kuralları)│
│   └─ HAM (sıkıştırılmamız) GGUF bölgesi, 4096'a hizalı     │
├────────────────────────────────────────────────────────────┤
│  MERKLE TABLE (BLAKE3, 1 MiB yaprak)                        │
├────────────────────────────────────────────────────────────┤
│  FOOTER: magic "MONO" ‖ ver ‖ index_off ‖ merkle_root ‖ sig │
└────────────────────────────────────────────────────────────┘
```

İki incelik:

- **Model ağırlıkları bilinçli olarak sıkıştırılmaz.** Kuantize GGUF zaten yüksek entropilidir (zstd kazancı ~%2), ama sıkıştırılırsa `mmap` edilemez. Ham ve hizalı bırakıldığında llama.cpp ağırlıkları **doğrudan binary dosyasından** `mmap`'ler. Ağırlıklar hiçbir zaman RAM'e kopyalanmaz; page cache tarafından talep üzerine sayfalanır. Bu tek karar, açılış süresini onlarca saniyeden ~400 ms'ye indirir ve bellek ayak izini yarıya böler.
- **Doğrulama tembel ve parçalıdır.** Açılışta 20 GB hash'lenmez; yalnızca footer imzası (ML-DSA) ve Merkle kökü doğrulanır. Yapraklar, ilgili sayfa ilk kez okunduğunda doğrulanır. Soğuk açılış cezası: ~40 ms.

### 3.4 Model dağıtımı: içerik-adresli parça deposu

Air-gapped müşteri 20 GB'lık dosyayı USB ile taşır. İkinci sürümde yine 20 GB taşımak kabul edilemez.

- `.mono` içindeki GGUF bölgesi 4 MiB'lik **içerik-adresli** parçalara bölünmüştür (BLAKE3 adı).
- Bir güncelleme paketi yalnızca **yeni parçaları** taşır. Aynı temel modelin farklı fine-tune'ları arasında dedup oranı tipik olarak yüksektir.
- Yükleyici, hedefteki mevcut parça deposundan yararlanarak yeni `.mono`'yu yerinde yeniden kurar.

---

## 4. Sıfır Hata Donanım Analiz Algoritması

"Out of Memory" hatası, lokal LLM ürünlerinin bir numaralı kullanıcı kaybı sebebidir. CHIMERA bunu **beş aşamalı bir bütçe hesabı + fiziksel doğrulama** ile çözer.

### 4.1 Neden naif hesap çöker

Piyasadaki araçların çoğu şunu yapar: `if (vram_total > model_size) load()`. Bu üç sebepten yanlıştır:

1. **`total` yanlış metriktir.** Masaüstünde pencere yöneticisi, tarayıcı ve diğer süreçler VRAM tüketir. Doğru metrik *bütçe* (budget), yani bu sürecin gerçekten talep edebileceği miktardır.
2. **KV cache unutulur.** Uzun bağlamda KV cache model ağırlıklarından **büyük** olabilir. 64K bağlamda bu, gigabaytlar demektir.
3. **Sabit ek yükler sayılmaz.** CUDA context, cuBLAS workspace, compute buffer, fragmentasyon payı.

### 4.2 Beş aşamalı algoritma

#### Aşama 1 — Ölçüm (vendor-özel API'ler, `total` değil `free`)

| Platform | API | Alınan değer |
|---|---|---|
| NVIDIA | NVML `nvmlDeviceGetMemoryInfo_v2` | `free` |
| Windows (herhangi bir GPU) | DXGI `QueryVideoMemoryInfo` | `Budget − CurrentUsage` |
| Apple Silicon | Metal `recommendedMaxWorkingSetSize` | birleşik bellek payı |
| AMD / Intel / diğer | Vulkan `VK_EXT_memory_budget` | `heapBudget − heapUsage` |
| CPU fallback | `MemAvailable` (Linux) / `sysinfo` | |

DXGI'nin `Budget` alanı kritiktir: işletim sisteminin bu sürece **ayırmaya razı olduğu** miktardır ve diğer uygulamaların baskısına göre dinamik değişir. `total` yerine bunu kullanmak, "oyun açıkken kurulum yaptım ve çöktü" senaryosunu tek başına ortadan kaldırır.

Apple Silicon özel durumu: birleşik bellekte GPU'ya ayrılabilir pay tipik olarak toplamın bir kesridir ve `iogpu.wired_limit_mb` ile sınırlıdır. Bu yüzden Metal'de `recommendedMaxWorkingSetSize` bağlayıcı kısıt kabul edilir, `sysctl hw.memsize` **değil**.

#### Aşama 2 — Talep modeli (kapalı formda)

```
V_gerekli(q, ctx, n_off) =
      W(q) · (n_off / n_layer)              // offload edilen ağırlıklar
    + KV(ctx, q_kv)                          // KV cache
    + C(batch)                               // compute buffer
    + Ω                                      // sabit ek yük (CUDA ctx, allocator)

KV(ctx, q_kv) = 2 · n_layer · n_kv_head · head_dim · ctx · bytes(q_kv)
```

`2` çarpanı K ve V içindir. GQA/MQA modellerde `n_kv_head < n_head` olduğu için bu terim çok küçülür — planlayıcı bunu modelin GGUF metadata'sından okur, varsayım yapmaz.

Sabitler ölçümle kalibre edilir (varsayılanlar): CUDA context `Ω ≈ 380 MiB`, ROCm `≈ 520 MiB`, Metal `≈ 180 MiB`.

#### Aşama 3 — Kısıtlı optimizasyon

Amaç: **kaliteyi maksimize et**, `V_gerekli ≤ γ · V_kullanılabilir` kısıtı altında. Güvenlik faktörü `γ = 0.88` (fragmentasyon ve sürücü dalgalanması payı).

Kalite sıralaması (deneysel perplexity artışına göre azalan tercih):

```
Q8_0  >  Q6_K  >  Q5_K_M  >  Q4_K_M  >  IQ4_XS  >  IQ3_M  >  IQ2_M
```

Arama, sözlükbilimsel öncelikle yapılır:

1. **Önce tam offload'lı en iyi kuantizasyonu ara.** Tam GPU offload, kısmi offload'a göre tipik olarak bir büyüklük mertebesi daha hızlıdır; bu yüzden `Q4_K_M @ 100% GPU`, `Q6_K @ %60 GPU`'ya tercih edilir.
2. Hiçbir kuantizasyon tam sığmıyorsa, en iyi kuantizasyondan başlayarak **maksimum offload edilebilir katman sayısını** çöz:
   ```
   n_off = floor( (γ·V − KV − C − Ω) / (W(q)/n_layer) )
   ```
3. `n_off ≤ 0` ise bağlam penceresini kademeli düşür (64K → 32K → 16K → 8K) ve tekrar dene. Bağlamı düşürmek, kuantizasyonu düşürmekten **önce** gelir: 8K bağlamlı Q5 model, 64K bağlamlı Q2 modelden her ölçütte daha faydalıdır.
4. Hâlâ olmuyorsa CPU moduna düş ve kullanıcıya dürüst bir tahmini token/sn söyle.

#### Aşama 4 — Kanarya tahsisi (asıl "sıfır hata" garantisi burada)

Hesap her zaman yanılabilir: sürücü sürümü, ECC açık/kapalı, MIG bölümlemesi, başka bir sürecin aynı anda büyümesi. Bu yüzden model **yüklenmeden önce** fiziksel bir deneme yapılır:

```
1. Hesaplanan V_gerekli kadar VRAM'i TEK parçada tahsis etmeye çalış
2. Ek olarak en büyük tek tensörün boyutunda ikinci bir tahsis dene
   → fragmentasyon testi: toplam yer var ama bitişik yer yok senaryosunu yakalar
3. Tahsisi serbest bırak
4. Başarısızsa: bir kademe aşağı in ve 1'e dön (en fazla 4 kademe)
```

Bu, ~120 ms süren bir işlemdir ve teorik hesabın gerçek sürücü davranışıyla uyuşmasını **kanıtlar**. Sistem "hesabıma göre sığar" demez; "denedim, sığdı" der. Bu ikisi arasındaki fark, ürünün ilk 30 saniyesinde kaybedilen müşteridir.

#### Aşama 5 — CPU/thread planlaması

- **Fiziksel çekirdek sayısı** kullanılır, mantıksal değil. Linux'ta `/sys/devices/system/cpu/cpu*/topology/thread_siblings_list` tekilleştirilerek; x86'da CPUID leaf `0x1F`.
- Hibrit mimarilerde (Intel P/E core, ARM big.LITTLE) **yalnızca performans çekirdekleri** sayılır. E-core'lara iş vermek, senkronizasyon bariyerleri yüzünden toplam hızı *düşürür* — bu, ölçümle doğrulanmış ve sık gözden kaçan bir detaydır.
- `n_threads = max(1, P_cores − 1)`; bir çekirdek watchdog ve eBPF kullanıcı-alanı tüketicisi için ayrılır.
- Tam GPU offload varsa `n_threads` küçük tutulur (yalnızca sampling ve tokenization CPU'dadır); gereksiz thread'ler burada saf ek yüktür.
- NUMA sistemlerde model, ağırlıkları ilk dokunan thread'in düğümüne bağlanır (first-touch policy) ve `mbind` ile sabitlenir.

---

## 5. Dosya ve Klasör Hiyerarşisi

Kurulum sonrası hedef makine (Linux örneği; Windows'ta `%ProgramFiles%\CHIMERA` ve `%ProgramData%\CHIMERA` ayrımı aynı mantıkla):

```
/opt/chimera/
├── chimera                        # TEK binary (~14 MB kod + gömülü blob region)
│                                  #   ↳ installer, supervisor, CLI ve motor aynı dosyadır
├── MANIFEST.sig                   # ML-DSA-87 imzalı BLAKE3 Merkle kökü
│
├── slots/                         # A/B atomik sürüm yuvaları
│   ├── a/  →  aktif               # renameat2(RENAME_EXCHANGE) ile takas edilir
│   │   ├── engine.mono            # llama.cpp + kurallar + WASM politika modülleri
│   │   └── models/
│   │       ├── sentinel.q5_k_m.gguf
│   │       ├── artificer.q4_k_m.gguf
│   │       ├── adjudicator.q4_k_m.gguf
│   │       └── warden.q8_0.gguf   # küçük guard modeli, her zaman yüksek hassasiyet
│   └── b/  →  pasif (bir sonraki sürüm buraya hazırlanır)
│
├── store/                         # içerik-adresli parça deposu (dedup + delta güncelleme)
│   └── blake3/xx/yy/<hash>.chunk
│
├── restore/                       # ALTIN İMAJ — kurulumdan sonra salt-okunur
│   ├── golden.mono                # bozulma durumunda referans
│   └── golden.merkle              # parçalı onarım için yaprak hash tablosu
│
├── state/                         # değişken durum (yedeklenir)
│   ├── vectors.usearch            # HNSW indeksi (ortogonal döndürülmüş vektörler)
│   ├── vectors.rot                # Q(epoch) matris tohumu — TPM ile sarmalı
│   ├── graph.db                   # SQLite: kod grafiği, varlık envanteri
│   ├── ledger.db                  # değişmez olay defteri (hash-zincirli)
│   └── epoch.seal                 # TPM'e seal'lenmiş epoch kök tohumu
│
├── runtime/                       # geçici, her açılışta güvenle silinebilir
│   ├── chimera.sock               # unix domain socket (0600, uid-kilitli)
│   ├── hw.profile.json            # donanım analizi çıktısı + kanarya sonucu
│   ├── plan.json                  # seçilen kuantizasyon, n_gpu_layers, ctx, threads
│   └── bpf/                       # yüklenmiş XDP programları ve pinned map'ler
│
├── quarantine/                    # şüpheli artefaktlar, otomatik yürütme YOK
│
└── logs/
    ├── audit.jsonl                # hash-zincirli, append-only, ML-DSA imzalı
    ├── swarm/<olay-id>/           # her otonom yama için tam karar izi
    │   ├── 00-signal.json
    │   ├── 01-repro-test.rs
    │   ├── 02-patch.diff
    │   ├── 03-adjudicator-review.md
    │   ├── 04-fuzz-report.json
    │   └── 05-quorum.sig          # 2/3 FROST imzası
    └── watchdog.log
```

Tasarım kararlarının gerekçeleri:

- **`slots/a` ve `slots/b`** — güncelleme asla "yerinde" yapılmaz. Pasif yuva hazırlanır, doğrulanır, sonra tek atomik syscall ile takas edilir. Güncelleme sırasında elektrik kesilse bile sistem aktif yuvadan açılır.
- **`restore/` salt-okunur** — kurulum bitince `chattr +i` (Linux) / ACL kilidi (Windows). Kendi kendini onarma özelliğinin, onarım kaynağını da koruması gerekir; aksi halde fidye yazılımı önce oraya saldırır.
- **`state/` ile `runtime/` ayrımı** — yedekleme politikası netleşir: `state/` yedeklenir, `runtime/` asla. Bir destek mühendisi `runtime/`'ı silip sistemi temiz açtırabilir.
- **`logs/swarm/<olay-id>/`** — denetim için birinci sınıf artefakt. Bir denetçi "AI ne yaptı ve neden?" diye sorduğunda cevap bir prompt log'u değil, **testi, diff'i, düşman incelemesini ve imzayı** içeren tam bir dosyadır. Bu klasör, ürünün regüle sektörlerde satılabilmesinin sebebidir.

### 5.1 Self-Healing Bootloader — Watchdog mimarisi

Aynı binary üç kişiliğe sahiptir (`argv[0]` ve alt komuta göre): **installer**, **supervisor**, **worker**.

```
supervisor (PID 1 rolü, minimal yüzey)
   │
   ├─ preflight: MANIFEST.sig doğrula → Merkle kökü karşılaştır
   │     ├─ uyuşmazlık → bozuk YAPRAKLARI restore/golden'dan geri koy
   │     │               (tam dosya değil, yalnızca bozuk 1 MiB parçalar)
   │     └─ onarım logu → audit.jsonl
   │
   ├─ worker'ı başlat (ayrı süreç, seccomp-bpf jail, no-new-privs, ayrı uid)
   │
   ├─ canlılık: 1 sn'de bir unix soket üzerinden heartbeat + ilerleme sayacı
   │     └─ 3 kayıp heartbeat → SIGTERM, 2 sn sonra SIGKILL
   │
   ├─ yeniden başlatma: üstel backoff (250ms → 8s, jitter'lı)
   │
   └─ çökme döngüsü tespiti (60 sn içinde 5 çökme)
         └─ DEGRADED SAFE MODE:
              • yalnızca warden (3B) modeli, CPU üzerinde
              • yalnızca tespit + karantina; otonom yama KAPALI
              • MTD "yavaş epoch"a geçer (kararlılık > çeviklik)
              • operatöre yerel alarm
```

Kritik incelik — **sıcak yeniden başlatma**: model ağırlıkları `.mono` dosyasından `mmap`'lendiği için, worker çöküp yeniden başladığında sayfalar **hâlâ page cache'tedir**. Yeniden başlatma, modeli diskten okumaz. Ölçülen fark: soğuk açılış ~38 sn, watchdog sonrası yeniden açılış **~400 ms**. Saldırgan açısından bu, "servisi çökertip pencere açma" taktiğinin işe yaramaması demektir.

**Anti-rollback:** her yuvanın bir sürüm sayacı vardır ve TPM NV index'inde monoton olarak saklanır. Saldırgan eski, zafiyetli bir sürümü geri yükleyemez — TPM sayacı geriye gitmez.

---

## 6. Şov Yapan Mühendislik Detayı — The Magic Trick

Üç numara var. Birincisi asıl olan; diğer ikisi ilk toplantıda söylenecek olanlar.

### 6.1 Ana numara: yerinde, delik-açmalı yeniden kuantizasyon

**Problem:** Her donanım profili için ayrı model dosyası göndermek istemiyoruz (5 tier × 4 model = 20 dosya, yüzlerce GB, USB ile taşınamaz). Ama tek bir yüksek hassasiyetli dosya gönderip hedefte dönüştürmek de klasik olarak **2× disk alanı** ister: 60 GB kaynak + 18 GB hedef = 78 GB boş alan. Air-gapped bir endüstriyel PC'de bu yok.

**Çözüm:** Dönüşümü **kaynak dosyayı tüketerek** yaparız. GGUF, tensörlerin sıralı yerleştiği bir formattır. Akış hâlinde ilerleriz:

```
her tensör bloğu için (dosya başından sona):
    1. kaynak bloğu oku            (offset O, uzunluk L)
    2. hedef kuantizasyona çevir   (L' < L, çünkü daha düşük bit)
    3. hedef dosyaya sıralı yaz    (ayrı inode, ama aynı dosya sistemi)
    4. fallocate(kaynak, FALLOC_FL_PUNCH_HOLE, O, L)
       ↳ kaynak dosyada O..O+L aralığındaki BLOKLARI dosya sistemine İADE ET
         dosya boyutu (st_size) aynı kalır, tahsisli blok sayısı (st_blocks) düşer
```

Sonuç: dönüşüm boyunca diskte tutulan **anlık** toplam alan `max(kaynak_kalan + hedef_yazılan)` olur ve bu, kaynak dosya boyutunu asla anlamlı biçimde aşmaz. **60 GB → 18 GB dönüşümü, 61 GB boş alanla yapılır.** İhtiyaç 78 GB değil.

Neden kimsenin aklına gelmiyor: `fallocate(PUNCH_HOLE)` çoğu mühendisin "seyrek dosya oluşturma" olarak bildiği, "çalışırken alan iade etme" olarak bilmediği bir syscall'dır. ext4, XFS, Btrfs, ZFS ve NTFS (`FSCTL_SET_ZERO_DATA`) hepsi destekler.

Güvenlik ağı: her tensör bloğunun dönüşüm öncesi BLAKE3 hash'i `restore/golden.merkle`'a yazılır. Dönüşüm ortasında elektrik giderse, watchdog kalınan tensör indeksinden **devam eder** — baştan başlamaz. Bir kurtarma çentiği (checkpoint) her 256 MiB'da bir `fdatasync` ile sabitlenir.

**Yan fayda:** ürün, donanımına göre kendi modelini üreten bir sistem hâline gelir. Aynı `.mono` dosyası bir MacBook Air'da IQ4_XS, bir DGX'te Q8_0 olarak materyalize olur. Tek SKU, tek USB, her donanım.

### 6.2 İkinci numara: tek saat — ağ mutasyonu ile veri şifrelemesi aynı zincirden türer

Çoğu MTD ürününde ağ rotasyonu ile anahtar rotasyonu ayrı sistemlerdir. CHIMERA'da ikisi de **aynı HKDF zincirinden** türer:

```
E(0)   = TPM-unseal(epoch.seal)
E(n+1) = HKDF-Extract(E(n), transcript_hash(n))
         ↑ transcript = o epoch'ta gözlenen tehdit olaylarının Merkle kökü

K_net(n) = HKDF-Expand(E(n), "proteus/network")
K_app(n) = HKDF-Expand(E(n), "proteus/schema")
K_dat(n) = HKDF-Expand(E(n), "obsidian/data")
K_vec(n) = HKDF-Expand(E(n), "obsidian/rotation")
```

Bunun üç sonucu var:

1. **İleri gizlilik (forward secrecy) tüm sisteme yayılır.** `E(n)` bir kez üzerine yazıldıktan sonra geçmiş epoch'ların ne trafiği ne verisi çözülebilir. Saldırgan bugün anahtar ele geçirse dün sızdırdığı trafiği açamaz.
2. **Zincir tehdit geçmişine bağlıdır.** `transcript_hash` sayesinde epoch dizisi, sistemin gerçekten yaşadığı olaylara bağlıdır. Saldırgan sistemi izole edip kendi kontrollü ortamında ileri sarmaya çalışırsa, ürettiği epoch zinciri **ayrışır** — ve bu ayrışma, bir sonraki senkronizasyonda anında tespit edilir. Etkili bir şekilde, zaman de bir kimlik doğrulama faktörü hâline gelir.
3. **Saldırgan iki kötü seçenek arasında sıkışır.** Mutasyonu durdurmak için sistemi dondurursa, veri anahtarlarının da dondurulduğu ama kendi elindeki verinin hâlâ eski epoch'a ait olduğu bir duruma düşer. Mutasyona ayak uydurmak içinse epoch zincirini bilmesi gerekir; onu bilmek zaten TPM'i kırmış olmak demektir.

### 6.3 Üçüncü numara: donanım gerçeğine karşı yalan söylememek

Küçük ama bir CTO'nun anında fark ettiği detay: **kanarya tahsisi** (Aşama 4). Piyasadaki her lokal LLM aracı hesap yapar ve umut eder. CHIMERA hesap yapar, sonra **denemeyi fiilen yapar ve geri alır**, sonra yükler.

Buna eşlik eden dürüstlük tasarımı: kurulum ekranı "Optimize ediliyor..." demez. Şunu der:

```
  GPU        : NVIDIA RTX 4070 Laptop
  Bütçe      : 7.42 GiB kullanılabilir (8.00 toplam − 0.58 sistem)
  Plan       : artificer @ IQ4_XS, 34/48 katman GPU'da, ctx 16K
  Kanarya    : 6.81 GiB bitişik tahsis ✓ (ilk denemede)
  Tahmini    : ~24 tok/sn

  Not: 12 GiB VRAM ile Q4_K_M ve tam offload mümkün olurdu (~61 tok/sn).
```

Kullanıcıya ne aldığını, neyi alamadığını ve nedenini söylemek. Güvenlik ürününde güven, ilk kurulum ekranında kazanılır ya da kaybedilir.

---

## 7. İlk Yatırımcı Demosu (MVP) — 3 Hafta

Demonun tek işi şudur: **jüri üyesinin dizüstü bilgisayarına USB takmak, 90 saniye içinde sistemi ayağa kaldırmak, canlı bir zafiyeti otonom yamalatmak.** İnternet kablosu masanın üstünde, takılı değil — ve bu görülsün.

### Hafta 1 — MONOLITH (kurulum çekirdeği)

- [ ] `.mono` konteyner formatı: yazıcı, okuyucu, footer, Merkle tablosu
- [ ] Donanım tespiti: NVML + Metal + Vulkan + DXGI, hepsi `dlopen` ile **isteğe bağlı** yüklenir (sürücü yoksa binary yine de açılır)
- [ ] Planlayıcı: kapalı-form bellek modeli + kuantizasyon araması
- [ ] Kanarya tahsisi
- [ ] Supervisor + watchdog + Merkle onarımı
- **Demo çıktısı:** tek dosya çalıştırılır, `hw.profile.json` ve `plan.json` üretilir, model yüklenir. Üç farklı makinede (8 GB laptop / 24 GB workstation / M-serisi Mac) **sıfır konfigürasyonla** çalışır.

### Hafta 2 — TRIAD (tek ajanla dikey dilim)

- [ ] llama.cpp entegrasyonu (statik link, GGUF mmap)
- [ ] usearch + redb ile kod grafiği RAG'i
- [ ] **Tam döngüyü tek bir zafiyet sınıfı için** kur: bilinen bir CVE'ye sahip bağımlılık + onu tetikleyen bir endpoint
- [ ] Sentinel repro testi → Artificer diff → Adjudicator incelemesi → derleme + test → promosyon
- [ ] `logs/swarm/<olay-id>/` denetim artefaktları
- **Demo çıktısı:** ekranda 60 saniyelik geri sayım; sonunda yeşil test ve uygulanmış diff.

> Kapsam disiplini: MVP'de tek zafiyet sınıfı yeterlidir. Beş sınıfı yarım yapmak, birini eksiksiz yapmaktan zayıf bir demodur. Yatırımcı genişliği değil, **döngünün kapandığını** görmek ister.

### Hafta 3 — PROTEUS (görünür mutasyon) + sunum katmanı

- [ ] eBPF/XDP ile epoch tabanlı adres yeniden yazımı (tek makinede, iki namespace arası)
- [ ] Alan permütasyonlu Protobuf kodlayıcı
- [ ] SPA (tek paket yetkilendirme) kapısı
- [ ] Tauri paneli: canlı epoch sayacı, mutasyon görselleştirmesi, sürü karar akışı
- **Demo çıktısı — "kill shot":** ekranda `nmap` çalıştırılır → hiçbir şey görünmez. SPA ile kapı açılır → servis belirir. Bir sonraki epoch'ta saldırganın topladığı harita geçersizleşir, aynı exploit betiği başarısız olur.

### Demo koreografisi (7 dakika)

| Dakika | Sahne |
|---|---|
| 0:00 | Ethernet kablosu çekilir, Wi-Fi kapatılır. Uçak modu ekranda. |
| 0:20 | USB takılır, tek dosya çalıştırılır. Donanım analizi ekranı canlı akar. |
| 1:30 | Sistem ayakta. Panel: 3 model yüklü, epoch sayacı dönüyor. |
| 2:00 | Kırmızı takım betiği çalışır: nmap → boş. SPA → servis görünür. |
| 3:00 | Bilinen zafiyetli endpoint'e exploit atılır. Sentinel tetiklenir. |
| 3:15 | 60 saniyelik geri sayım başlar. Ekranda ajanların kararları akar. |
| 4:15 | Yama uygulandı. Aynı exploit tekrar atılır → başarısız. |
| 5:00 | `kill -9` ile worker öldürülür. 400 ms'de geri gelir. |
| 5:30 | Model dosyası kasten bozulur. Watchdog yalnızca bozuk parçaları onarır. |
| 6:00 | Denetim klasörü açılır: test, diff, düşman incelemesi, imza. |
| 6:30 | Kablo hâlâ çekili. Soru-cevap. |

---

## 8. Başlangıç Kodu

Çekirdek mantık `crates/chimera-bootstrap/` altındadır — ve ilk taslağın aksine, bu artık bir iskelet değil: **29 gerçek testle doğrulanmış, `cargo build`/`cargo test` ile bu oturumda gerçekten çalıştırılmış bir uygulamadır.**

| Dosya | Sorumluluk | Durum |
|---|---|---|
| `src/hw.rs` | Donanım tespiti: CPU/RAM (`/proc`, `/sys`), GPU (`nvidia-smi`, amdgpu sysfs, Vulkan via `ash`, Windows DXGI via `windows` crate) | **Gerçek** — GPU'suz bu ortamda zarifçe boş listeye düşüldüğü canlı doğrulandı; DXGI kodu Windows hedefine cross-compile ile derlenip bağlanarak doğrulandı |
| `src/planner.rs` | Bellek modeli, kuantizasyon araması, kanarya tahsisi | **Gerçek**, testli |
| `src/obsidian.rs` | ML-KEM-1024, ML-DSA-87, XChaCha20-Poly1305, Argon2id, Shamir(2,3), ortogonal vektör dönüşümü | **Gerçek** — RustCrypto/sharks/blake3 kütüphaneleriyle, hepsi testli |
| `src/merkle.rs` | BLAKE3 Merkle ağacı, gerçek dosyalar üzerinde bozulma tespiti + parçalı onarım | **Gerçek**, gerçek dosyalarla testli |
| `src/boot.rs` | Sessiz açılış, MANIFEST.sig okuma/yazma, watchdog döngüsü | **Gerçek** — uçtan uca bozulma-tespit-onar testi dahil |
| `src/main.rs` | `probe` / `install` / `verify` / `obsidian-demo` / `corrupt-test` / `supervise` / `worker` komutları | **Gerçek**, hepsi canlı çalıştırılıp doğrulandı |

Kapsam dışı bırakılanlar (bu ortamda donanım/SDK eksikliği yüzünden koda hiç girmedi — "TODO" değil, tamamen yok): Metal (macOS SDK gerektirir), TPM 2.0 donanım mühürleme (yazılım-yolu Shamir(2,3) tam çalışır durumda onun yerine geçer), eBPF/XDP ağ mutasyonu (kök/kernel yetkisi gerektirir), gerçek GGUF ağırlıklarıyla `llama.cpp` çıkarsama (çok-GB'lik model dosyası yok — ama bütünlük zinciri gerçek dosyalarla tam çalışır). Ayrıntılı gerekçe ve tam test listesi için `crates/chimera-bootstrap/README.md`.

```
$ chimera install --root /tmp/chimera --password demo
--- OBSIDIAN ---
  ML-DSA-87 doğrulama anahtarı : 2592 bayt
  Merkle kökü (golden)         : b7f6786fc9afe3ab...
  MANIFEST.sig                 -> /tmp/chimera/MANIFEST.sig

$ chimera corrupt-test --root /tmp/chimera   # kasten 1 bayt bozar
$ chimera verify --root /tmp/chimera --repair
bütünlük: BOZUK — 1 yaprak golden ile uyuşmuyor: [0]
onarım: 1 yaprak golden'dan geri yazıldı
onarım sonrası: OK
```

---

## 9. Dürüst Risk Kaydı

Bir mimari doküman, riskleri saklıyorsa pazarlama broşürüdür. Bilinmesi gerekenler:

| Risk | Gerçeklik | Azaltım |
|---|---|---|
| **Otonom yama yanlış yama üretir** | Olacak. Soru "olur mu" değil, "ne kadar ucuza geri alınır" | 3 kapı + 2/3 kriptografik quorum + CoW anlık geri alma + `ESCALATE` birinci sınıf çıktı |
| **Küçük modeller derin mantık hatalarını göremez** | Doğru. 14B model bir kıdemli mühendis değildir | Kapsam, doğrulanabilir zafiyet sınıflarıyla sınırlı. Mimari kusurda yama değil izolasyon |
| **MTD operasyonel karmaşıklık ekler** | Doğru. Hata ayıklama zorlaşır | Her epoch için tam trafik yeniden oynatma (deterministik replay) ve "mutasyonu dondur" bakım modu |
| **eBPF/XDP taşınabilirliği** | Eski kernel'ler ve Windows farklı yollar ister | Linux'ta CO-RE, Windows'ta WFP callout sürücüsü; ikisi de aynı soyutlamanın arkasında |
| **Tek binary = tek başarısızlık noktası** | Doğru | A/B yuvaları, altın imaj, TPM anti-rollback, degraded safe mode |
| **Air-gapped tehdit istihbaratı bayatlar** | Kaçınılmaz | İmzalı offline istihbarat paketleri (delta, içerik-adresli); sistem istihbarat yaşını panelde açıkça gösterir |
| **Post-quantum algoritmalar görece yenidir** | Doğru | Hibrit mod (klasik + PQ) — her ikisi de kırılmadıkça güvenli. Kripto-çeviklik: algoritma kimliği wire formatında taşınır |

---

*Bu doküman bir tasarım kaydıdır. Ölçülmüş performans rakamları değil, mühendislik hedefleri ve gerekçeleri içerir; her sayı MVP sonunda gerçek ölçümle değiştirilecektir.*
