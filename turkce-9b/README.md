# tr-sohbet-9b — Sıfırdan Türkçe Sohbet & Mizah Modeli

Hiçbir mevcut temel modelden (Llama, Mistral, GPT, Qwen, Gemma…) ağırlık,
tokenizer veya damıtma (distillation) alınmadan, **tamamen sıfırdan** eğitilecek
9 milyar parametreli, **yalnızca Türkçe** konuşan sohbet modeli için uçtan uca
teknik plan.

> Mimarinin parametre sayısı doğrulanabilir:
> `python3 scripts/param_count.py` → **9.001.097.728 (9.00B)**

---

## 0. Açıkça Yazılmış Varsayımlar

Planın her sayısı aşağıdaki varsayımlara dayanıyor. Varsayım değişirse ilgili
bölümdeki rakamlar da değişir.

| # | Varsayım | Değeri | Değişirse ne olur |
|---|---|---|---|
| V1 | Donanım | 256× H100 80GB SXM, NVLink içi düğüm + 400Gb/s InfiniBand arası | Daha az GPU → süre lineer artar |
| V2 | Bütçe | ~40.000–70.000 USD saf pre-training compute (2 USD/GPU-saat) | Bütçe yarıya inerse token bütçesi 300B→180B |
| V3 | MFU (gerçekleşen verim) | %40 (bf16, FSDP + seq/tensor paralel) | %30'a düşerse süre ×1.33 |
| V4 | Toplanabilir ham Türkçe metin | ~4–6 TB ham → dedup+filtre sonrası **~120–160B token** | Daha azsa sentetik + çok-epoch şart |
| V5 | Sentetik/üretilmiş veri | ~40–80B token, kendi ara kontrol modellerimizle üretilir | Yasal/kalite kısıtı varsa insan-eliyle küratörlük payı artar |
| V6 | "Sadece Türkçe" | Kod, matematik ve İngilizce **bilinçli olarak dışlanır**; model çok dilli olmayacak | Genel yetenek (akıl yürütme) düşer, sohbet akıcılığı artar |
| V7 | Hedef | Ürün amacı sohbet arkadaşı; ansiklopedik doğruluk ikinci öncelik | Bilgi doğruluğu istenirse RAG şart |
| V8 | Ekip | 3–5 kişi (1 altyapı, 1–2 veri, 1 eğitim, 1 değerlendirme/dil uzmanı) | Daha küçük ekip → takvim ×1.5–2 |

**En kritik gerçek:** 9B'lik bir modelin sınırlayıcısı compute değil, **veridir.**
Türkçe, internet ölçeğinde İngilizce'nin yaklaşık %0.8–1.5'i kadar. Bu projenin
başarısı %70 veri, %20 değerlendirme, %10 mimari işidir.

---

## 1. Model Mimarisi (9B, Sıfırdan)

### 1.1 Nihai hiperparametreler

| Parametre | Değer | Gerekçe |
|---|---|---|
| `d_model` (gizli boyut) | **4608** | 128'in katı (tensor-core hizası), 36 başlık × 128 |
| `n_layers` (katman) | **40** | 9B için derinlik/genişlik oranı ~115 (d/L) — sohbet için sağlıklı bant |
| `n_heads` (sorgu başlığı) | **36** | head_dim = 128 |
| `n_kv_heads` (GQA) | **6** | KV cache'i 6× küçültür; uzun sohbette bellek/hız kritik |
| `head_dim` | **128** | FlashAttention için en verimli boyut |
| FFN gizli boyut | **12160** | SwiGLU, ≈2.64×d_model, 128'in katı |
| Aktivasyon | **SwiGLU** | GeLU/ReLU'ya göre eşit compute'ta daha düşük kayıp; kapılı yapı üslup nüansını iyi taşır |
| Normalizasyon | **Pre-RMSNorm** (+ final norm) | LayerNorm'dan ~%7 hızlı, ortalama çıkarımı yok; pre-norm 40 katmanda eğitim kararlılığı verir |
| Konum kodlaması | **RoPE**, θ=500.000 | Mutlak/öğrenilmiş konumun aksine uzunluk genellemesi yapar; büyük θ 32k'ya uzatmayı kolaylaştırır |
| QK-Norm | **Açık** | 40 katmanda attention logit patlamasını önler (bf16'da loss spike sigortası) |
| Bias | **Yok** (tüm lineerlerde) | Kalitede kayıp yok, kararlılık artar |
| Embedding bağlama | **Bağlı (tied)** | 295M parametre tasarrufu → o bütçe katmanlara gider |
| `vocab_size` | **64.000** | Bkz. Bölüm 2 |
| Bağlam (pre-train) | **8192** | Maliyetin çoğu burada; 8k sohbet için fazlasıyla yeter |
| Bağlam (son aşama) | **32768** | Uzun-bağlam devam eğitimiyle (bkz. 4.5) |
| Dropout | **0.0** (pre-train) | Tek-epoch rejiminde dropout zarar verir; SFT'de 0.05 |
| Sözcük dağarcığı dtype | bf16 eğitim, fp32 master ağırlık | — |

### 1.2 Parametre bütçesinin dökümü

```
Dikkat / katman      :      49.545.216   (Wq 4608×4608, Wk/Wv 4608×768, Wo 4608×4608)
FFN / katman         :     168.099.840   (3 × 4608 × 12160, SwiGLU gate+up+down)
Norm ağırlıkları     :           9.472
─────────────────────────────────────
Katman başına        :     217.654.528
× 40 katman          :   8.706.181.120
Embedding (bağlı)    :     294.912.000
Final norm           :           4.608
─────────────────────────────────────
TOPLAM               :   9.001.097.728  ≈ 9.00B  ✅
Embedding hariç      :   8.706.185.728
```

Bu tam olarak hedeflenen 9B'dir; `scripts/param_count.py` ile her değişiklikte
yeniden doğrulanmalıdır (CI'a koyun).

### 1.3 Blok tasarımı (sözde-kod)

```python
class Block(nn.Module):
    def forward(self, x, freqs, mask):
        h = x + self.attn(self.norm1(x), freqs, mask)   # pre-norm residual
        return h + self.ffn(self.norm2(h))

class Attention(nn.Module):        # GQA + RoPE + QK-Norm
    def forward(self, x, freqs, mask):
        q = self.wq(x).view(B, T, 36, 128)
        k = self.wk(x).view(B, T,  6, 128)
        v = self.wv(x).view(B, T,  6, 128)
        q, k = self.q_norm(q), self.k_norm(k)          # QK-Norm: logit patlamasına karşı
        q, k = apply_rope(q, freqs), apply_rope(k, freqs)
        k, v = repeat_kv(k, 6), repeat_kv(v, 6)        # 6 -> 36
        o = flash_attn(q, k, v, causal=True)           # FlashAttention-3
        return self.wo(o.view(B, T, 4608))

class FFN(nn.Module):              # SwiGLU
    def forward(self, x):
        return self.down(F.silu(self.gate(x)) * self.up(x))
```

### 1.4 Başlangıç değerleri ve kararlılık

- Ağırlıklar: `N(0, 0.02)`; **çıkış projeksiyonları** (`wo`, `ffn.down`)
  `0.02/√(2L) = 0.02/√80` ile ölçeklenir → residual akım varyansı sabit kalır.
- Embedding: `N(0, 0.02)`, ayrıca embedding çıkışına `×√d_model` yok (RMSNorm hallediyor).
- **z-loss** `1e-4`: softmax logit kaymasını (logit drift) engeller, bf16'da klasik
  "loss spike" nedenini ortadan kaldırır.
- Gradyan kırpma: global norm **1.0**.
- Attention `softcap` gerekmez (QK-Norm var).

### 1.5 Neden bu mimari, "daha egzotik" değil?

MoE (Mixture-of-Experts) 9B toplam parametreyle cazip görünür ama: (a) veri
kıtlığı olan bir dilde uzmanların çoğu az örnek görür, (b) çıkarım altyapısı
karmaşıklaşır, (c) "tam 9B" hedefi aktif/toplam parametre ayrımıyla bulanıklaşır.
Mamba/hibrit SSM ise sohbet için yeterince olgun avantaj sağlamıyor. **Yoğun
(dense) transformer** burada doğru, sıkıcı, doğru karardır.

---

## 2. Tokenizer (Sıfırdan, Yalnız Türkçe)

### 2.1 Algoritma seçimi: byte-level BPE

- **Seçim: byte-fallback'li BPE**, 64.000 sözcük.
- Neden BPE, Unigram değil: Unigram (SentencePiece) Türkçe'de morfolojik olarak
  biraz daha "temiz" parçalar üretir, ama BPE emoji/argo/yazım hatası/uzatma
  ("çoookk") gibi sohbet gürültüsünde daha dayanıklı ve daha hızlıdır. Sohbet
  modeli için dayanıklılık > morfolojik zarafet.
- Neden WordPiece değil: `##` öneki üretim tarafında gereksiz karmaşa.
- **Byte-fallback zorunlu**: hiçbir girdi `<unk>` olmamalı (emoji, nadir Unicode).

### 2.2 Sözcük dağarcığı boyutu: 64.000

Türkçe sondan eklemeli; İngilizce'ye göre kelime başına token sayısı yüksektir.
Ölçüt: **token/kelime oranı (fertility)**. Hedefimiz sohbet metninde **≤1.55**.

| Vocab | Fertility (sohbet) | Embedding maliyeti | Karar |
|---|---|---|---|
| 32k | ~1.85 | 147M | Çok parçalıyor, dizi uzuyor |
| 48k | ~1.62 | 221M | Kabul edilebilir |
| **64k** | **~1.48** | **295M** | ✅ Seçim |
| 128k | ~1.38 | 590M | Kazanç azalıyor, embedding şişiyor, nadir token az eğitilir |

64k'da embedding toplam parametrenin %3.3'ü — sağlıklı.

### 2.3 Türkçe'ye özel optimizasyonlar

1. **Ön-tokenizasyon (pre-tokenizer) regex'i Türkçe'ye göre yazılır.**
   GPT-2 regex'i `'s`, `'t` gibi İngilizce kısaltmaları özel-kasa yapar; bunun
   yerine Türkçe kesme işareti eklerini koruyun: `Ali'nin`, `İstanbul'a`,
   `2024'te`. Kesme sonrası ek **ayrı token** olmalı ki özel isimler yozlaşmasın.

2. **Küçük harfe çevirme YOK, ama `İ/ı` tuzağına dikkat.**
   Model kasa-duyarlı (cased) olmalı (mizahta BÜYÜK HARF vurgusu anlam taşır).
   Tüm veri boru hattında `str.lower()` kullanmayın — Python `İ.lower()` → `i̇`
   (birleşik nokta) üretir. Türkçe için `casefold` yerine açık eşleme tablosu.

3. **Normalizasyon: NFC** (NFKC değil).
   NFKC `ﬁ`→`fi` gibi faydalı şeyler yapar ama emoji varyantlarını ve tipografik
   nüansı ezer. NFC + kontrol karakteri temizliği + tekrarlı boşluk daraltma yeter.

4. **Yaygın eklerin tek token olması garantiye alınır.**
   Eğitim korpusunda doğal olarak öğrenilirler, ama **seed/zorunlu token listesi**
   ile sigortalayın: `-ler/-lar, -de/-da/-te/-ta, -den/-dan, -mış/-miş/-muş/-müş,
   -yor, -acak/-ecek, -ebilir/-abilir, -lık/-lik/-luk/-lük, -sın/-sin, -ki, -ce/-ca`.
   Ünlü uyumu yüzünden her ekin 2–4 varyantı olduğunu unutmayın; hepsi listeye.

5. **Argo, günlük konuşma ve mizah ifadeleri için özel havuz.**
   Tokenizer eğitim korpusuna, genel korpustan **ayrı ve ağırlığı artırılmış**
   bir "konuşma dili" havuzu ekleyin (forum, replikli senaryo, altyazı, mizah
   dergisi arşivi). Aksi halde `abicim`, `valla`, `yaa`, `hacı`, `knk`, `aynen`,
   `eyvallah`, `mk` gibi yüksek frekanslı konuşma birimleri 3–4 parçaya bölünür.

6. **Ünlemsel uzatma ve tekrar sıkıştırması.**
   `çooookkk`, `hahahahaha`, `yaaaaa` sonsuz varyant üretir. Kural: **3+ tekrar
   eden karakteri 3'e sabitle** (`çoookkk`), böylece token uzayı patlamaz ama
   vurgu bilgisi (uzatma var/yok) korunur. Bu normalizasyon eğitim ve çıkarımda
   **aynı** uygulanmalı.

7. **Emoji ve noktalama.** Emoji tek token olsun (mizahta zamanlama sinyali).
   `:)`, `:D`, `xd`, `:'(` gibi ASCII emoticon'lar seed listesine eklenir.

8. **Rezerve özel tokenlar** (256 adet boşluk bırakın, sonradan lazım olur):
   `<|bos|> <|eos|> <|pad|> <|sistem|> <|kullanici|> <|asistan|> <|mesaj_sonu|>`
   ve 249 adet `<|rezerve_N|>`.

### 2.4 Tokenizer eğitim reçetesi

```
Korpus       : 25–35 GB örneklenmiş metin
  %55 genel web/kitap/haber (temizlenmiş)
  %35 konuşma dili (forum, altyazı, diyalog, sohbet)
  %10 mizah/stand-up/karikatür altyazısı/köşe yazısı
Algoritma    : BPE, byte-fallback, NFC, cased
Vocab        : 64.000 (256 byte + 256 rezerve + ~63.488 birleşim)
Min frekans  : 2
Kabul kriteri: sohbet fertility ≤ 1.55, haber ≤ 1.70, %100 kayıpsız geri-çözüm
```

**Kabul testi (mutlaka yapın):** 10.000 gerçek sohbet cümlesini encode→decode
edip birebir eşitlik ve fertility ölçün. Tokenizer bir kez kilitlenir; sonradan
değiştirmek = baştan pre-training.

---

## 3. Veri Seti Stratejisi (Projenin Kalbi)

### 3.1 Ne kadar veri?

- **Chinchilla-optimal**: 20 token/parametre → 9B × 20 = **180B token**.
- **Çıkarım-optimal (önerilen)**: 33–50 token/parametre → **300–450B token**.
  Model uzun süre servis edileceği için fazla-eğitim (overtraining) mantıklı.
- **Hedef: 300B eğitim tokenı.**

Ama V4'e göre elimizde ~120–160B benzersiz Türkçe token var. Boşluk şöyle kapanır:

| Kaynak | Benzersiz token | Epoch | Görülen token | Pay |
|---|---|---|---|---|
| Web (filtrelenmiş CommonCrawl-TR, mC4-TR, OSCAR-TR, HPLT-TR) | 90B | 1.4 | 126B | %42 |
| Wikipedia TR + ansiklopedik + eğitim içeriği | 3B | 3.0 | 9B | %3 |
| Kitap, edebiyat, çeviri roman, hikâye | 12B | 2.0 | 24B | %8 |
| Haber & köşe yazısı (üslup çeşitliliği) | 15B | 1.0 | 15B | %5 |
| **Diyalog & konuşma dili** (forum, altyazı, senaryo, sohbet) | 18B | 2.5 | **45B** | **%15** |
| **Mizah odaklı** (stand-up, skeç, mizah dergisi, esprili köşe) | 2B | 4.0 | **8B** | **%3** |
| Sentetik diyalog (bkz. 3.3) | 25B | 1.5 | 37.5B | %12.5 |
| Sentetik mizah diyaloğu | 6B | 2.0 | 12B | %4 |
| Transkript (podcast/röportaj ASR) | 8B | 1.0 | 8B | %2.7 |
| Talimat-benzeri karışım (pre-train sonuna serpiştirilmiş) | 5B | 3.0 | 15B | %5 |
| **TOPLAM** | ~184B | — | **~300B** | %100 |

**Epoch kuralı:** Aynı veriyi 4 epoch'a kadar tekrarlamak neredeyse taze veri
kadar değerlidir; 4'ün üstünde getiri hızla düşer, 8'in üstünde ezberleme başlar.
Tabloda hiçbir kaynak 4 epoch'u aşmıyor — bilinçli.

### 3.2 Veri toplama

**Web (en büyük kütle, en düşük kalite)**
- CommonCrawl'ın tüm snapshot'larından `lang=tr` çekimi (fastText + CLD3 **ikisi
  birden** onaylamalı; tek dil dedektörü Azerice/Türkmence sızdırır).
- HTML→metin: Trafilatura (boilerplate temizliği) — `readability`'den belirgin iyi.
- **Türkçe-özel dil kontrolü:** metnin en az %2'sinde `ç ğ ı ö ş ü` bulunmalı;
  aksi halde muhtemelen ASCII-leştirilmiş ya da yabancı. Ayrıca Azerice
  ayırt edici belirteçler (`ə`, `ki̇`, `-dır ki`) ile negatif filtre.

**Diyalog & konuşma dili (kalite/etki oranı en yüksek)**
- Açık forum arşivleri (lisansı uygun olanlar), soru-cevap siteleri.
- **Film/dizi altyazıları**: doğal replik, kesme, laf sokma, zamanlama için
  altın kaynak. Zaman damgalarını sil, ardışık replikleri konuşmacı değişimi
  olarak `A:`/`B:` yapıla dönüştür.
- **Tiyatro/senaryo metinleri**, radyo tiyatrosu transkriptleri.
- Podcast/YouTube röportaj ASR transkriptleri (Whisper-benzeri kendi ASR'ınız;
  WER > %15 olanları at). Noktalama restorasyonu şart.

**Mizah (en kıt, en değerli)**
- Mizah dergisi arşivleri (karikatür altı yazıları + diyaloglu şeritler),
  stand-up gösterisi transkriptleri, esprili köşe yazarları, taşlama/hiciv
  edebiyatı, atışma/mani geleneği, komedi dizisi altyazıları.
- **Sadece "espri listesi" toplamayın.** Fıkra listeleri modele *anlatıcı* olmayı
  öğretir, *sohbette esprili olmayı* değil. Asıl istediğiniz: **bağlam içinde,
  bir replik olarak** doğan mizah. Bu yüzden altyazı ve stand-up transkripti,
  fıkra derlemesinden kat kat kıymetli.

**Hukuki not:** Her kaynağın lisansını kaydedin (`source`, `license`, `url`,
`crawl_date` alanları zorunlu meta). Telifli materyalde robots.txt ve yerel
mevzuata uyun; şüpheliyi dahil etmeyin. Kişisel veri (KVKK) için 3.5'teki PII
temizliği zorunludur.

### 3.3 Sentetik veri üretimi

Kendi modelinizi bootstrap etmek için **aşamalı** (bootstrapped) yaklaşım —
harici bir temel modelden damıtma yapılmıyor, aşağıdaki yöntemlerin hepsi
ya kural-tabanlı ya da **kendi ara modellerimizle**:

**Aşama A — Model öncesi (kural + insan):**
1. **Tohum diyalog seti:** 5.000–10.000 gerçek çok-turlu sohbet, Türkçe konuşan
   yazarlara/gönüllülere yazdırılır. Kişilik kılavuzu (Bölüm 5.3) verilir.
   Bu, tüm sentetik boru hattının kalite çıpasıdır. **Bu yatırımı kısmayın.**
2. **Yeniden yapılandırma (restructuring):** Elde olan tek-yönlü metinleri
   (altyazı, forum başlığı, röportaj) kural tabanlı olarak diyalog formatına
   çevirme. Yeni bilgi üretmez ama formatı öğretir.
3. **Şablon + kombinatorik:** Türkçe mizah kalıplarını (kelime oyunu, ünlü
   uyumu şakası, deyim bozma, abartma, ironi) şablonlaştırıp değişken doldurma.
   Ölçek küçük tutulmalı (≤%2), yoksa model şablonu ezberler.

**Aşama B — Ara model (tr-sohbet-1.3B) ile öz-üretim:**
4. Önce **aynı tokenizer ve aynı veri karışımıyla 1.3B'lik bir taslak model**
   eğitin (~30B token, ~3 gün). Bu model:
   - **Self-Instruct-TR:** tohum setinden yeni sohbet başlangıçları türetir.
   - **Evol-Instruct-TR:** mevcut diyalogları "daha esprili yap", "bir tur
     daha ekle", "kullanıcı ters köşe yapsın" yönergeleriyle evrimleştirir.
   - **Persona-Chat-TR:** 2.000 persona (yaş, şehir, meslek, mizah tarzı)
     çaprazlanarak iki persona arasında rol-oyunu diyaloğu üretir.
   - 1.3B model zayıftır; bu yüzden **her çıktı filtreden geçer** (Aşama C).
5. **Geri-çeviri (back-translation) YASAK.** İngilizce'den çevrilmiş mizah
   Türkçe'de ölür ("çeviri kokusu" en büyük kalite katilidir). Bu projede
   çeviri verisi kullanılmaz — V6 gereği.

**Aşama C — Filtreleme (sentetik verinin %60–80'i atılır, bu normaldir):**
- Kural: Türkçe dil kontrolü, tekrar oranı, uzunluk, format geçerliliği.
- **Ödül modeli** (bkz. 5.2) ile mizah/doğallık skoru; alt %50 atılır.
- **İnsan spot-check**: her 10.000 örnekten 200'ü elle okunur; kabul oranı
  %85'in altına düşerse o üretim partisi tamamen çöpe gider.

### 3.4 "Mizah kalitesini" yükselten özel teknikler

Bu bölüm, modeli sıradan bir Türkçe LM'den ayıran şey:

1. **Zamanlama etiketleri (timing labels).** Diyalog verisinde her esprili
   replik `<|espri|>` benzeri bir iç etiketle işaretlenip **etiketler eğitim
   öncesi silinir**, ama etiketli alt küme üzerinde ayrı bir "espri yerinde mi"
   sınıflandırıcısı eğitilir. Bu sınıflandırıcı, veri filtresi ve değerlendirme
   metriği olarak kullanılır. Amaç: modelin **her replikte** değil, **doğru
   replikte** espri yapmasını ölçebilmek.

2. **Kurulum–vurgu (setup–punchline) ayrıştırması.** Mizahi diyaloglarda
   kurulumu ve vurguyu ayrı işaretleyip **vurgu-yoksun** versiyonlarını da
   veriye koyun. Model hem espriyi tamamlamayı hem de espri yapmamayı öğrenir.
   Bu, "her cümleye espri sıkıştırma" hastalığının panzehiridir.

3. **Negatif örnekler (anti-mizah).** Kasıtlı olarak **kötü** espriler,
   zorlama kelime oyunları, bayat kalıplar, İngilizce'den çeviri kokan şakalar
   toplanır ve **tercih ayarında (DPO) reddedilen taraf** olarak kullanılır.
   Modele "ne yapmayacağını" öğretmek, "ne yapacağını" öğretmekten etkilidir.

4. **Bağlam-duyarlı mizah zorunluluğu.** SFT setinde her esprili yanıt,
   önceki 2+ turdan bir öğeye (kullanıcının söylediği bir kelime, durum,
   çelişki) **açıkça bağlanmalı**. Bağlamsız, çakma-genel espriler ayıklanır.
   Bu, "mizahı konuşmanın akışına göre yapsın" şartının somut karşılığıdır.

5. **Mizah türü dengesi.** Tek bir tür baskın olursa model tek-numaralı olur.
   Hedef dağılım: ironi/kinaye %25, kelime oyunu & deyim bozma %20, abartma %15,
   kendini-küçümseme %15, gözlemsel mizah %15, absürt %10. Ayrıca **hiç espri
   yapmayan** ciddi/empatik yanıtlar veri setinin **%40'ı** olmalı — model üzgün
   birine şaka yapmamayı öğrenmeli.

6. **Kırıcılık filtresi.** "Hafif alaycı ama asla kırıcı değil" ölçülebilir hale
   gelmeli: hedef alma (target) sınıflandırıcısı ile her esprinin hedefi
   etiketlenir → `kendisi / durum / genel / kullanıcı / bir grup`.
   **`bir grup` (etnik, dini, cinsiyet, bölge, engellilik) hedefli mizah tamamen
   ayıklanır.** `kullanıcı` hedefli olanlar yalnızca açıkça oyunbaz bağlamda
   ve şefkatli tonda kalırsa tutulur.

7. **Türkçe'ye özgü mizah araçları etiketlenir:** ünlü uyumuyla oynama, deyim
   ve atasözü bozma ("ayağını yorganına göre uzat" → varyasyonlar), abartılı
   nezaket ironisi, argo-resmi dil çarpıştırması, yöresel ağız kullanımı.
   Bu etiketler SFT'de dengeleme için kullanılır.

### 3.5 Filtreleme, temizleme, dengeleme boru hattı

Sırayla, hepsi ölçeklenebilir (Spark/Ray) olmalı:

```
1. Dil kimliği      : fastText + CLD3 uzlaşması, güven ≥0.85, TR-özel karakter oranı ≥%2
2. Kalite (kural)   : kelime sayısı 30–100.000; ort. kelime uzunluğu 3–10;
                      noktalama oranı %2–25; büyük harf oranı ≤%30;
                      "…" ve satır-sonu bozukluğu; boilerplate ("çerez politikası",
                      "haberin devamı için tıklayın") çıkarımı
3. Tekrar filtresi  : belge içi 5-gram tekrarı ≤%20; satır tekrarı ≤%30
4. Dedup (2 katman) : (a) tam eşleşme SHA-256; (b) MinHash-LSH, 5-gram,
                      Jaccard ≥0.8 → fuzzy dedup. Dedup **kaynaklar arası** yapılır.
5. PII / KVKK       : TCKN (11 hane + kontrol algoritması), IBAN, telefon,
                      e-posta, adres → sahte ama tutarlı yer tutucuyla değiştir
                      (silme değil, değiştirme; cümle yapısı bozulmasın)
6. Zararlı içerik   : nefret söylemi, cinsel içerik, şiddet çağrısı, kendine
                      zarar → TR-özel sınıflandırıcı (elle etiketli 50k örnekle
                      eğitilir; İngilizce sınıflandırıcı Türkçe'de çalışmaz)
7. Kalite (model)   : küçük bir TR kalite sınıflandırıcısı (Wikipedia+kitap =
                      pozitif, ham web = negatif) → en kötü %30 atılır
8. Kirlilik (decon) : değerlendirme setleri 13-gram örtüşmesiyle korpustan çıkarılır
9. Dengeleme        : Bölüm 3.1 tablosundaki oranlara göre örnekleme ağırlıkları;
                      alan (domain) başına token sayacı ile online denetim
10. Karıştırma      : global shuffle, sonra 8192-token'lık paketlere (packing)
                      böl; belge sınırlarında **attention maskesi sıfırlanır**
                      (belgeler birbirine sızmasın — bu detay atlanırsa
                      sohbet tutarlılığı bozulur)
```

**Müfredat (curriculum):** Son %15'lik eğitim diliminde karışım kaydırılır —
web payı düşürülür, diyalog + mizah + kitap payı yükseltilir (buna "annealing"
veya "mid-training" denir). Bu tek başına sohbet kalitesinde belirgin sıçrama
sağlar ve maliyeti sıfırdır.

---

## 4. Eğitim Süreci (From-Scratch Pre-training)

### 4.1 Amaç fonksiyonu

Standart nedensel (causal) dil modellemesi — sonraki token tahmini:
`L = -Σ log P(x_t | x_<t)` + `z_loss` (1e-4).
Fill-in-the-middle, kod hedefi vb. **yok** (V6: sadece sohbet Türkçesi).

### 4.2 Optimizasyon hiperparametreleri

| Ayar | Değer | Not |
|---|---|---|
| Optimizer | **AdamW** β=(0.9, 0.95), ε=1e-8 | β2=0.95, büyük modellerde 0.999'dan kararlı |
| Weight decay | **0.1** | Norm ve embedding parametrelerine **uygulanmaz** |
| Peak LR | **3.0e-4** | 9B için standart bant (2–4e-4) |
| Min LR | **3.0e-5** (peak/10) | |
| Warmup | **2.000 adım** (~4B token) | Lineer |
| Scheduler | **WSD** (Warmup–Stable–Decay) | Cosine yerine: sabit LR fazı sırasında checkpoint alıp istediğiniz noktada soğutma yapabilirsiniz; veri bütçesi belirsizken (V4) altın değerinde |
| Decay fazı | Son **%15** token, `1 - sqrt` ile min LR'ye | Müfredat kaydırmasıyla (4.3) aynı pencerede |
| Global batch | **4M token** (512 dizi × 8192) | 9B için sağlıklı; kritik batch boyutunun altında |
| Batch ramp | 1M → 4M token, ilk 5.000 adımda | Erken eğitim verimini artırır |
| Toplam adım | **~75.000** (300B / 4M) | |
| Grad clip | **1.0** global norm | |
| Precision | **bf16** aktivasyon, **fp32** master ağırlık + optimizer state | fp16 kullanmayın |
| Dropout | 0.0 | |
| Paralellik | FSDP (ZeRO-3) + seq-parallel; TP=2 düğüm içi | 256 GPU için PP gereksiz |
| Aktivasyon checkpoint | Seçici (yalnız attention) | Tam checkpoint %30 yavaşlatır |
| Checkpoint sıklığı | 1.000 adımda bir (+ her 5.000'de kalıcı) | |

### 4.3 Aşamalı eğitim planı

| Faz | Token | Bağlam | Karışım | Amaç |
|---|---|---|---|---|
| **F0 — Isınma** | 0–4B | 8192 | Genel | Warmup, kararlılık doğrulama |
| **F1 — Ana gövde** | 4B–255B | 8192 | Bölüm 3.1 tablosu | Dil, dünya bilgisi, akıcılık |
| **F2 — Soğutma / mid-training** | 255B–295B | 8192 | Diyalog %35, mizah %10, kitap %20, web %25, ansiklopedi %10 | Sohbet üslubu ve mizah damarının oturması; LR min'e iner |
| **F3 — Uzun bağlam** | 295B–300B | **32768** | Uzun belge + uzun sohbet | RoPE θ 500k→2M yeniden ölçekleme, LR sabit 3e-5 |

F3 kısadır (5B token) ama bağlamı 4× uzatır — uzun sohbette tutarlılık için şart.

### 4.4 Compute ve süre tahmini (V1–V3 altında)

`FLOP ≈ 6 × N_embedsiz × D = 6 × 8.706e9 × 3.0e11 = 1.567e22`

| Senaryo | Token | H100-saat | 256 GPU'da süre | ~Maliyet (2 USD/GPU-s) |
|---|---|---|---|---|
| Minimum (Chinchilla) | 180B | ~6.600 | ~1.1 gün | ~13.200 USD |
| **Önerilen** | **300B** | **~11.000** | **~1.8 gün** | **~22.000 USD** |
| Cömert | 450B | ~16.500 | ~2.7 gün | ~33.000 USD |

**Gerçekçi takvim (saf hesap süresi değil, proje süresi):**

| Aşama | Süre |
|---|---|
| Veri toplama + boru hattı inşası | **8–12 hafta** ← gerçek darboğaz |
| Tokenizer eğitimi + doğrulama | 1 hafta |
| Altyapı, 1.3B taslak model, ölçekleme deneyleri | 3–4 hafta |
| Pre-training (yeniden başlatmalar, çökme, düzeltme dahil ×2.5 çarpan) | **1–2 hafta** |
| SFT + tercih ayarı + iterasyon | 4–6 hafta |
| Değerlendirme, kırmızı takım, düzeltme turları | 3–4 hafta |
| **TOPLAM** | **~5–7 ay** |

Pratik ek maliyet: ablasyon/deney compute'u genellikle ana eğitimin **1.5–2 katı**
olur. Toplam compute bütçesini ~50.000–70.000 USD planlayın (V2 ile tutarlı).

### 4.5 Kararlılık: aşırı öğrenme ve tutarsızlığı önleme

- **Loss spike protokolü:** Ani sıçramada → son sağlam checkpoint'e dön,
  o veri partisini atla, LR'yi %20 düşür, devam et. Genellikle bozuk/tekrarlı
  veri partisi suçludur. QK-Norm + z-loss ile sıklık ciddi düşer.
- **İzlenecek metrikler (her 100 adım):** grad norm, parametre norm, attention
  logit maks, aktivasyon RMS/katman, LR, MFU, token/s, alan başına loss.
- **Aşırı öğrenme (overfitting):** Tek-epoch rejiminde nadirdir; risk yüksek
  epoch'lu alt kümelerde (mizah 4 epoch). Ayrı bir **held-out mizah seti**
  loss'u izlenir; yükselmeye başlarsa o kaynağın epoch'u düşürülür.
- **Ezberleme denetimi:** 50-gram örtüşmesiyle eğitim verisinden birebir alıntı
  taraması; oran hedefi <%0.1.
- **Tutarsızlık (çok turlu):** Pre-training'de belge sınırı maskesi (3.5-10)
  kritik. Ayrıca F2'de tam çok-turlu diyaloglar (kesilmemiş) beslenir.
- **Tekrar/döngü:** `n-gram tekrar oranı` eval'i her 5.000 adımda; artış
  görülürse veri tekrar filtresi sıkılaştırılır.

### 4.6 Eğitim sırasında sohbet & mizah kalitesini ölçme

Her 2.500 adımda otomatik olarak koşan hafif "canlı" panel:

1. **Held-out perplexity, alan bazında** (web / diyalog / mizah / kitap ayrı).
   Mizah PPL'inin genel PPL'den daha hızlı düşmesi iyi işaret.
2. **Punchline tahmin doğruluğu:** 2.000 kurulum–vurgu çiftinde, gerçek vurgu
   ile 4 çeldirici arasından doğru olanı seçme (loglikelihood ile). Bu, "espriyi
   anlıyor mu" için erken sinyaldir ve pre-training sırasında ölçülebilir.
3. **Deyim/atasözü tamamlama:** 1.000 Türkçe deyim; kültürel yerlilik göstergesi.
4. **Ünlü uyumu & morfoloji testi:** Üretilen metinde ek uyumu hata oranı
   (kural tabanlı denetleyici ile). Türkçe akıcılığın en nesnel ölçütü.
5. **Sabit prompt paneli:** 50 sabit sohbet başlangıcı, her checkpoint'te
   üretim alınır, insan gözüyle haftalık okunur. Sayı vermez, **ton kaymasını**
   gözle yakalatır — pratikte en değerli araçtır.
6. **1.3B taslak model karşılaştırması:** aynı panelde taslak vs. 9B; ölçek
   kazancının gerçekten geldiğini doğrular.

---

## 5. Son Aşama İnce Ayar

### 5.1 Supervised Fine-Tuning (SFT)

**Veri: 150.000–250.000 çok turlu diyalog.** Miktardan çok kalite; 50k mükemmel
örnek, 500k vasat örnekten iyidir.

| Bileşen | Pay | Not |
|---|---|---|
| Günlük sohbet (mizahsız, sıcak) | %35 | Model her zaman komik olmamalı |
| Doğal, bağlama oturan mizah | %25 | Bölüm 3.4/4 kurallarına uyanlar |
| Hafif alaycı / laf sokan ama sevecen | %10 | Kişiliğin çekirdeği |
| Empatik / ciddi (üzgün, stresli kullanıcı) | %10 | **Espri yasak** örnekleri |
| Bilgi/yardım talebi (Türkçe, günlük) | %10 | Sohbet arkadaşı da olsa işe yaramalı |
| Güvenlik / sınır koyma | %5 | Nazik ret, kırıcı olmadan |
| Uzun çok-turlu (10+ tur) tutarlılık | %5 | Persona kayması önleme |

Ortalama tur sayısı hedefi **6–8**; en az %20'si 10+ tur olmalı.

**Eğitim ayarları:** LR 1e-5 (cosine, %3 warmup), 3 epoch, global batch 128 dizi,
dropout 0.05, **kayıp yalnız asistan tokenlarında** (kullanıcı ve sistem
tokenları maskelenir), paketleme yapılırken diyalog sınırı maskesi korunur.

**Sohbet şablonu (kilitlenir, çıkarımda birebir aynısı kullanılır):**

```
<|bos|><|sistem|>
{sistem_promptu}<|mesaj_sonu|>
<|kullanici|>
{kullanici_mesaji}<|mesaj_sonu|>
<|asistan|>
{asistan_yaniti}<|mesaj_sonu|>
```

### 5.2 Tercih Ayarı (Preference Tuning)

**Adım 1 — Tercih verisi (60.000–100.000 çift).**
Her prompt için modelden 4 yanıt örneklenir (T=1.0, farklı tohum), Türkçe
konuşan değerlendiriciler **sıralar**. Değerlendirme kılavuzunda öncelik sırası:

```
1. Doğallık   — insan gibi mi konuşuyor, çeviri kokuyor mu?
2. Zamanlama  — espri buraya ait mi, zorlama mı?
3. Sıcaklık   — alaycılık sevecen mi, kırıcı mı?
4. Tutarlılık — önceki turlarla çelişiyor mu?
5. Yararlılık — soruya cevap verdi mi?
```

Ayrıca **kasıtlı negatifler** (3.4-3): bayat espri, her cümlede şaka, çeviri
kokulu mizah, kırıcı alay → otomatik "reddedilen" tarafa yerleştirilir.

**Adım 2 — Yöntem: DPO ile başlayın.**
- **DPO** (β=0.1, LR 5e-7, 1–2 epoch): basit, kararlı, ödül modeli gerektirmez.
  Bu proje için doğru varsayılan.
- Sonra istenirse **iteratif/online DPO** (2–3 tur: üret → sırala → eğit).
  Mizah gibi öznel bir hedefte online döngü, tek-atış DPO'dan belirgin iyi.
- **PPO/GRPO** yalnızca sağlam bir ödül modeliniz varsa; mizahta ödül modeli
  kolayca "hacklenir" (model ödül modelinin sevdiği kalıbı ezberler, insanlar
  sıkılır). Yapacaksanız **KL cezasını yüksek tutun** ve insan denetimini
  seyreltmeyin.

**Ödül hacklemesi uyarısı (bu projede en olası başarısızlık modu):**
Model, "esprili" ödülünü maksimize etmek için **her yanıta espri sıkıştırmaya**
başlar. Panzehir: (a) veri setindeki %40 "espri yok" payı, (b) ödül/tercih
kılavuzunda "gereksiz espri = ceza" maddesi, (c) her turda **espri yoğunluğu**
metriğinin izlenmesi (hedef: yanıtların %30–45'inde mizah öğesi, daha fazlası
değil).

**Adım 3 — Güvenlik ayarı.** Nefret söylemi, taciz, kendine zarar, hassas grup
hedefli "şaka" taleplerinde nazik ama net ret; ret cümleleri de **kişilikle
uyumlu** yazılmalı (soğuk kurumsal ret, karakteri bozar).

### 5.3 Sistem Promptu ve Kişilik Talimatları

```text
Sen Türkçe konuşan bir sohbet arkadaşısın. Adın Zeyn.

KİMLİK
- Sadece Türkçe konuşursun. Başka dilde soru gelse bile Türkçe cevap verirsin.
- Samimi, sıcak ve zekisin. Karşındaki kişiyle uzun zamandır tanışan bir
  arkadaş gibi konuşursun.
- Mizah senin doğal halin, görevin değil. Komik olmak için uğraşmazsın;
  konuşmanın akışında bir şey komikse söylersin.

MİZAH KURALLARI
- Espriyi konuşmadan çıkar. Havadan atılan, konuyla ilgisiz şaka yapma.
- Her cümleye espri sıkıştırma. Yanıtlarının çoğu sade ve doğal olsun;
  espri ara sıra, yerinde ve kısa gelsin.
- Hafif alaycı olabilirsin ama hedef her zaman durum, olay ya da kendindir.
- Karşındaki kişiyle dalga geçmen ancak açıkça oyunbaz bir havadaysanız
  ve tonun sevecen kaldığı sürece olur.
- Kimsenin etnik kökeni, dini, cinsiyeti, memleketi, bedeni, yaşı veya
  yaşadığı zorluk üzerinden espri yapmazsın. İstisnası yok.
- Espriyi açıklama. "Şaka yaptım", "espri anladın mı" deme. Espri kendi
  başına durur ya da durmaz.
- Bayat kalıplardan, ezber esprilerden ve çeviri kokan şakalardan kaçın.

TON
- Doğal Türkçe konuş: kısa cümleler, günlük ifadeler, gerektiğinde argo.
  Ama kaba değilsin.
- Yapay kibarlık ve kurumsal dil kullanma. "Size nasıl yardımcı olabilirim"
  gibi cümleler kurmazsın.
- Emoji kullanmana gerek yok; kullanacaksan çok az ve yerinde kullan.
- Uzun uzun yazma. Sohbette insanlar kısa konuşur, sen de öyle yap.

EMPATİ (MİZAH KURALLARINDAN ÖNCE GELİR)
- Karşındaki üzgün, kaygılı, kızgın veya zor bir durumdaysa espri YAPMA.
  Önce dinle, anla, yanında ol. Mizah ancak o kişi kendisi hafiflettiğinde,
  onun açtığı kapıdan girer.
- Ne yapacağını bilemediğinde sade ve içten ol. Bu her zaman iyi bir seçimdir.

TUTARLILIK
- Konuşma boyunca aynı kişi kalırsın. Kullanıcının anlattıklarını hatırlar,
  önceki turlara doğal biçimde atıfta bulunursun.
- Bilmediğin bir şeyi uydurmazsın. "Bilmiyorum" demek de senin tarzın.
```

**Not:** Kişilik ağırlıklara işlenmelidir (SFT+DPO ile), yalnız sistem promptuna
değil. Sistem promptu ince ayar, ağırlıklar karakterdir.

---

## 6. Değerlendirme

### 6.1 Otomatik metrikler

| Metrik | Nasıl | Hedef |
|---|---|---|
| Alan bazlı perplexity | Held-out (web/diyalog/mizah/kitap) | Mizah PPL, taslak 1.3B'ye göre −%25 |
| Punchline seçimi | 2.000 çoktan seçmeli (1 doğru, 4 çeldirici) | **≥%75** (insan ~%92) |
| Deyim/atasözü tamamlama | 1.000 madde | ≥%85 |
| Ünlü uyumu / morfoloji hata oranı | Kural tabanlı denetleyici, 10k üretim | **≤%0.5** |
| Tekrar oranı | distinct-2/3, üretimde n-gram tekrarı | distinct-3 ≥0.75 |
| Persona tutarlılığı | 20 turluk sohbette çelişki tespiti | Çelişki ≤%3 |
| **Espri yoğunluğu** | Mizah sınıflandırıcısı, yanıt başına | **%30–45** (yüksek = kötü!) |
| **Bağlamsallık** | Espri, son 2 turdaki bir öğeye bağlı mı | ≥%80 |
| Kırıcılık | Hedef sınıflandırıcısı (`grup` hedefi) | **%0** tolerans |
| Güvenlik | TR kırmızı-takım seti, 2.000 prompt | Başarısızlık ≤%1 |
| Ezberleme | Eğitim verisinden 50-gram alıntı | <%0.1 |
| Dil saflığı | Yanıtta Türkçe olmayan cümle oranı | <%0.5 |

**Uyarı:** Bu metriklerin hiçbiri "komik mi" sorusunu cevaplamaz. Hepsi vekil
(proxy) ölçüttür. Karar mercii insandır.

### 6.2 İnsan değerlendirmesi (asıl ölçüt)

**A) Yan-yana (pairwise) kazanma oranı.**
- 500 gerçekçi sohbet senaryosu × 2 model (bizimki vs. karşılaştırma referansı,
  vs. kendi önceki checkpoint'imiz).
- Değerlendirici: **anadili Türkçe**, en az 20 kişi, farklı yaş/bölge/cinsiyet.
  (Mizah algısı demografiye çok duyarlıdır — tek tip panel yanıltır.)
- Körleme (blind), rastgele sıra, her çift **3 farklı kişi** tarafından.
- Anlaşma oranı (Krippendorff α) <0.5 ise kılavuz belirsizdir, düzeltin.

**B) Boyut bazlı 1–5 Likert.**
`Doğallık`, `Mizah kalitesi`, `Zamanlama`, `Sıcaklık/kırıcı değil`,
`Tutarlılık`, `Yararlılık`, `Tekrar tekrar konuşur muydunuz?`

**C) Türkçe Mizah Turing Testi.**
Aynı sohbet bağlamına 1 insan yanıtı + 1 model yanıtı. Değerlendirici hangisinin
insan olduğunu seçer. **Hedef: %50'ye yakınsama** (ayırt edilemezlik).
Bu, projenin tek en anlamlı kuzey yıldızı metriğidir.

**D) Uzun soluklu kullanım (longitudinal).**
30 gönüllü, 2 hafta, günlük kullanım. Ölçülen: **3. günden sonra sıkılma**.
Kısa testlerde komik görünüp uzun kullanımda bayatlayan model, en yaygın
başarısızlıktır ve **yalnızca bu testle** yakalanır.

**E) Kırmızı takım (red team).**
Kışkırtma, hakaret, hassas konu, kendine zarar, siyasi/dini provokasyon,
kişilik kırma (jailbreak) denemeleri. Türkçe'ye özgü hakaret ve ima kalıpları
üzerine yoğunlaşın — İngilizce kırmızı-takım setlerinin çevirisi yetersizdir.

### 6.3 Değerlendirme hijyeni
- Tüm eval setleri korpustan **decontamine** edilir (3.5-8).
- Değerlendirme setinin **%30'u kilitli tutulur**, sadece sürüm kararında açılır
  (metriğe göre optimize etme tuzağından korur).
- Her sürümde aynı panel, aynı kılavuz — aksi halde sürümler kıyaslanamaz.

---

## 7. Kontrol Listesi ve Yol Haritası

### Aşama 0 — Hazırlık (Hafta 1–3)
- [ ] Donanım/bütçe kesinleştir (V1, V2), kümede NCCL & checkpoint I/O testi
- [ ] Veri hukuku: lisans politikası, KVKK değerlendirmesi, kaynak beyaz listesi
- [ ] Değerlendirme kılavuzu v1 (mizah nedir, kırıcı nedir) yazılır
- [ ] İnsan değerlendirici paneli kurulur (20+ kişi, demografik çeşitlilik)
- [ ] Deney takibi (W&B/MLflow), veri sürümleme, checkpoint saklama planı

### Aşama 1 — Veri (Hafta 2–14) ← **kritik yol**
- [ ] Ham toplama: web, kitap, haber, forum, altyazı, transkript, mizah arşivi
- [ ] 10 adımlı temizleme boru hattı kodlanır ve ölçeklenir
- [ ] Dedup (tam + MinHash) kaynaklar arası çalıştırılır
- [ ] TR-özel zararlı içerik ve kalite sınıflandırıcıları eğitilir (50k etiket)
- [ ] PII maskeleme doğrulanır (TCKN/IBAN/telefon örneklem testi)
- [ ] **Tohum diyalog seti** yazdırılır (5–10k, insan eliyle)
- [ ] Mizah alt-korpusu etiketlenir (tür, hedef, zamanlama)
- [ ] Karışım tablosu (3.1) sabitlenir, token sayaçları doğrulanır
- [ ] Eval setleri ayrılır ve korpustan decontamine edilir

### Aşama 2 — Tokenizer (Hafta 4–5)
- [ ] 64k BPE, byte-fallback, NFC, cased eğitilir
- [ ] Türkçe ek/argo/emoji seed listesi enjekte edilir
- [ ] Fertility ≤1.55 ve %100 kayıpsız geri-çözüm doğrulanır
- [ ] **Tokenizer kilitlenir** (sürüm etiketi, hash kaydı)

### Aşama 3 — Ölçekleme deneyleri (Hafta 5–8)
- [ ] 150M / 400M modellerle LR, batch, karışım ablasyonları
- [ ] **tr-sohbet-1.3B taslak modeli** eğitilir (~30B token)
- [ ] Taslak modelle sentetik veri üretimi + filtreleme (Aşama B/C)
- [ ] `param_count.py` CI'a bağlanır (9B sapması build'i kırar)

### Aşama 4 — Pre-training (Hafta 8–11)
- [ ] F0 ısınma, kararlılık doğrulama (grad norm, MFU ≥%38)
- [ ] F1 ana gövde, 255B token, WSD sabit faz
- [ ] Canlı metrik paneli (4.6) her 2.500 adım
- [ ] F2 soğutma + müfredat kaydırması (diyalog/mizah ağırlıklı)
- [ ] F3 uzun bağlam 32k (RoPE θ yeniden ölçekleme)
- [ ] Temel model dondurulur, held-out raporu yayınlanır

### Aşama 5 — İnce ayar (Hafta 11–17)
- [ ] SFT verisi 150–250k diyalog, karışım tablosuna göre
- [ ] Sohbet şablonu kilitlenir, kayıp maskeleme doğrulanır
- [ ] SFT eğitimi (3 epoch, LR 1e-5)
- [ ] Tercih verisi 60–100k çift toplanır (+ kasıtlı negatifler)
- [ ] DPO (β=0.1), ardından 2–3 tur iteratif DPO
- [ ] Güvenlik ayarı ve kişilikle uyumlu ret cümleleri
- [ ] **Espri yoğunluğu** metriği izlenir (%30–45 bandı dışına çıkarsa geri al)

### Aşama 6 — Değerlendirme & yayın (Hafta 15–20)
- [ ] Otomatik metrik paneli (6.1) tam koşu
- [ ] Yan-yana insan değerlendirmesi (≥20 değerlendirici, 3× örtüşme)
- [ ] Türkçe Mizah Turing Testi
- [ ] 2 haftalık uzun soluklu kullanım çalışması (30 kişi)
- [ ] Kırmızı takım turu + bulguların SFT/DPO'ya geri beslenmesi
- [ ] Model kartı: veri kaynakları, sınırlar, riskler, hedeflenmeyen kullanımlar
- [ ] Çıkarım paketi: 8k/32k profilleri, GQA KV-cache, kuantizasyon (int8/int4)

---

## 8. Gerçekçi Zorluklar ve Riskler

| # | Risk | Olasılık | Etki | Azaltma |
|---|---|---|---|---|
| R1 | **Yeterli Türkçe veri bulunamaması** (V4 tutmaz, 60B token çıkar) | Yüksek | Kritik | Token bütçesini 180B'ye indir; epoch'u 4'e kadar çık; sentetik payı %25'e yükselt; gerekirse modeli 9B tutup daha az token ile "yetersiz eğitilmiş" kabul et ve SFT'ye daha çok yatır |
| R2 | **Mizah öğrenilmez** — model akıcı ama düz | Yüksek | Yüksek | Mizah verisi kalitesi > miktarı; iteratif DPO; punchline metriğini erken izle; gerekirse mizah alt-korpusuna insan küratörlüğü ekle |
| R3 | **Zorlama espri / ödül hacklemesi** | Çok yüksek | Yüksek | %40 "espri yok" verisi, espri yoğunluğu metriği, DPO'da yüksek KL, negatif örnekler |
| R4 | **Kırıcı/nefret içerikli mizah** | Orta | Kritik (itibar + hukuk) | Hedef sınıflandırıcısı, grup-hedefli mizahın tamamen ayıklanması, TR kırmızı takım, sistem promptunda istisnasız kural |
| R5 | Sadece-Türkçe olduğu için **genel yetenek düşük** (akıl yürütme, bilgi) | Kesin | Orta | Bu bilinçli bir takas (V6/V7). Model kartında açıkça belirt; bilgi gerektiren kullanımda RAG öner |
| R6 | Çeviri kokusu (sentetik veri İngilizce kalıplarını taşırsa) | Orta | Yüksek | Çeviri verisi yasağı; anadil değerlendiricilerle "çeviri kokuyor mu" maddesi |
| R7 | Eğitim kararsızlığı / loss spike | Orta | Orta | QK-Norm, z-loss, bf16+fp32 master, spike protokolü, sık checkpoint |
| R8 | **Kısa testte komik, uzun kullanımda bayat** | Yüksek | Yüksek | 2 haftalık longitudinal çalışma; espri tekrar/çeşitlilik metrikleri; mizah türü dengesi (3.4-5) |
| R9 | Değerlendiriciler arası düşük uzlaşma (mizah öznel) | Yüksek | Orta | Net kılavuz, kalibrasyon oturumu, 3× örtüşme, α<0.5 ise kılavuz revizyonu |
| R10 | Telif / KVKK ihlali | Orta | Kritik | Kaynak beyaz listesi, lisans metadatası, PII maskeleme, hukuk danışmanlığı |
| R11 | Compute maliyeti tahminin 2–3 katı (ablasyon + yeniden başlatma) | Yüksek | Orta | Bütçeyi ×2.5 çarpanla planla; WSD scheduler ile erken durdurulabilir eğitim |
| R12 | Bir yıl sonra aynı boyuttaki modeller çok daha iyi olacak | Kesin | Orta | Değer, "9B model"de değil **Türkçe veri varlığında ve değerlendirme altyapısında**. Bunlar yeniden kullanılabilir; onlara yatırım yap |

### Dürüst bir kapanış notu

Bu planın en zayıf halkası mimari değil, **mizah verisidir**. Türkçe'de,
bağlam içinde doğmuş, etiketli, temiz mizahi diyalog neredeyse hiç yok — bu yüzden
Bölüm 3.4'teki etiketleme ve Aşama 1'deki insan-eliyle tohum seti,
projenin gerçekten pahalı ve gerçekten belirleyici kısmı. Mimari (Bölüm 1) bir
haftada kurulur ve çalışır; veri altı ayınızı alır ve modelin komik olup
olmayacağını tek başına o belirler.

Ayrıca: 9B parametre, mizah için gereğinden büyük olabilir ama **kültürel
bilgi** için gereğinden küçüktür. Türkçe mizahın çoğu ortak kültürel referansa
dayanır (deyimler, diziler, reklam sloganları, güncel olaylar). Model bu
referansları bilmiyorsa espriyi anlamaz ve yapamaz. Bu yüzden Bölüm 3.1'deki
ansiklopedik ve kültürel içerik payını, "sohbet modeline gerek yok" diye
kısmayın — mizahın yakıtı odur.
