# Siber-Guard — Kişisel Ağ Güvenliği Sınıflandırıcısı

Kendi eğitilmiş, hafif bir makine öğrenmesi sınıflandırıcısıyla **hangi
bağlantının güvenli, hangisinin şüpheli/zararlı olduğunu** tespit eden ve
isteğe bağlı olarak engelleyen bir DNS filtreleme sistemi. Artı, bir
dosyanın (programın) virüs olup olmadığını **gerçek antivirüs
veritabanlarına** sorarak kontrol eden yardımcı araç.

> **v2 notu:** İlk sürümden sonra proje ciddi şekilde genişletildi — çok
> parçalı Türkçe uzantı (`.gov.tr`/`.com.tr`) ayrıştırma hatası düzeltildi,
> combosquat/punycode tespiti eklendi, değerlendirme bağımsız bir "zor test
> seti" ve çapraz doğrulama ile dürüstleştirildi, DNS proxy'ye önbellek +
> izin/red listesi + asla-asılı-kalmama garantisi eklendi, CLI araçlarına
> toplu/JSON mod geldi, ve 26 testlik bir birim test paketi eklendi. Hepsi
> **eski komutlarla birebir uyumlu** — mevcut kullanım şekli bozulmadı.
> Ayrıntılar için aşağıdaki "v2 — Neler değişti" bölümüne bakın.

## Önce en önemli düzeltme: barındırma (hosting)

İlk istekte "Anthropic sunucularında çalışsın, tüm trafiğim oradan geçsin"
denmişti. Bu **yapılamaz ve yapılmamalı**:

- Bu proje Claude Code'un geçici bir kodlama oturumunda geliştirildi. O
  oturum hareketsizlik sonrası kapanıyor, dışarıdan gelen bağlantı kabul
  etmiyor ve kalıcı bir ağ hizmeti barındırmak için tasarlanmadı.
- Kişisel internet trafiğini üçüncü bir tarafın (burada: bu oturumun
  çalıştığı altyapının) sürekli izlemesi hem teknik olarak buraya uygun
  değil hem de gizlilik açısından **kendi trafiğini kendi kontrolündeki bir
  yerde tutman** çok daha doğru bir tasarımdır.

**Doğru kurulum:** Bu kod, senin kendi bilgisayarında veya kendi
kiraladığın bir VPS'te (DigitalOcean, Hetzner, kendi ev sunucun vb.)
çalışır. Trafiğin böylece yalnızca **senin kontrolündeki** bir makineden
geçer. Kurulum adımları aşağıda (`Dağıtım` bölümü).

## Bu ne yapar, ne yapmaz

| Yapar | Yapmaz |
|---|---|
| Sorgulanan **domain adının** güvenli/şüpheli olma ihtimalini tahmin eder | HTTPS içeriğini deşifre edip okumaz (MITM yok) |
| Şüpheli bulduğu domain'lere DNS seviyesinde erişimi engeller | Sıfır-gün (zero-day) zararlı yazılımı garanti tespit etmez |
| Bir dosyanın SHA-256 özetini gerçek antivirüs veritabanına (VirusTotal) sorar | Dosya içeriğinden kendi başına "virüs mü değil mi" **uydurmaz** |
| Kendi makinende/sunucunda, senin kontrolünde çalışır | Başkasının trafiğini senin izninle olmadan izlemez/engellemez |

### Neden tam HTTPS trafiğini incelemiyoruz (bilinçli tasarım kararı)

"Hangi bağlantı güvenli" sorusunu tam çözmek için TLS trafiğini araya girip
(MITM) şifresini çözmek gerekir. Bunun için her cihaza özel bir kök
sertifika kurulması şart — bu, o cihazın **tüm** şifreli trafiğini (bankacılık,
sağlık, özel mesajlar dahil) okunabilir hale getirir. Bu proje bunu
**kasıtlı olarak yapmıyor**: DNS seviyesinde filtreleme (hangi domain'e
bağlanılmak istendiği), içeriği hiç görmeden neredeyse aynı korumayı
(zararlı sunucuya bağlanmayı engelleme) çok daha düşük risk ve karmaşıklıkla
sağlar. Bu, Pi-hole gibi kanıtlanmış araçların da kullandığı yöntemdir.

## Mimari

```
Cihazların DNS ayarı → Siber-Guard DNS proxy (senin sunucun, port 53)
                              │
                              ├─ 1) denylist'te mi? ──── evet ──→ NXDOMAIN
                              ├─ 2) allowlist'te mi? ─── evet ──→ upstream'e yönlendir
                              ├─ 3) önbellekte mi? (TTL'li) ── varsa risk skorunu kullan
                              ├─ 4) yoksa: domain → özellik çıkarımı (features.py)
                              │              → kalibre edilmiş sınıflandırıcı (RF/GBM/LR, CV ile seçilir)
                              │
                    risk ≥ eşik? ──── evet ──→ NXDOMAIN (engellendi, log'a + istatistiğe yazılır)
                              │
                             hayır
                              │
                              ↓
                    gerçek DNS sunucusuna yönlendir (sınırlı deneme; hepsi
                    başarısız olursa SERVFAIL döner — istemci ASLA asılı kalmaz)
```

## Klasör yapısı

```
siber-guard/
  data/
    benign_domains.txt   — bilinen, gerçek, güvenilir domain listesi (~228 adet)
    dataset.csv           — generate_dataset.py çıktısı (eğitim verisi, ~880 satır)
    hard_test_set.csv      — eğitim üretecinden BAĞIMSIZ, dürüst değerlendirme seti
    allowlist.txt            — her zaman izin verilecek domainler (boş, doldurman için)
    denylist.txt              — her zaman engellenecek domainler (boş, doldurman için)
  src/
    config.py               — merkezi ayar yönetimi (config.json, geriye dönük uyumlu)
    features.py              — domain adından leksik/istatistiksel özellik çıkarımı
    generate_dataset.py       — etiketli eğitim verisi + zor test seti üretir
    train_classifier.py        — CV ile model seçimi, kalibrasyon, dürüst değerlendirme
    check.py                    — domain/URL kontrol aracı (tekli/toplu/JSON)
    dns_proxy.py                 — DNS filtreleme proxy'si (önbellek + izin/red listesi)
    file_reputation.py            — dosya/URL → VirusTotal itibar sorgusu (önbellekli)
    stats.py                       — log dosyasından özet rapor
  models/
    domain_classifier.joblib  — eğitilmiş model + metadata (~3.7 MB)
    evaluation_report.txt      — CV + zor-test-seti sonuçları (her eğitimde yenilenir)
  tests/
    test_features.py, test_generate_dataset.py, test_dns_proxy.py — 26 pytest testi
  deploy/
    siber-guard-dns.service   — systemd servis örneği (Linux sunucu için)
  config.example.json
  requirements.txt
  requirements-dev.txt         — + pytest (testleri çalıştırmak için)
```

## ⚠️ Eğitim verisi hakkında dürüst uyarı

Bu geliştirme ortamının ağ politikası, canlı tehdit istihbaratı
feed'lerine (URLhaus, OpenPhish, PhishTank, Tranco) erişimi engelliyor
(sadece paket kayıtları gibi belirli adresler açık). Bu yüzden:

- **Güvenilir domain listesi** (`data/benign_domains.txt`): gerçek, iyi
  bilinen ~225 popüler ve kurumsal domain (Google, bankalar, .gov.tr
  siteleri, üniversiteler vb.) — bunlar gerçek.
- **Zararlı domain örnekleri**: canlı bir feed'den değil, güvenlik
  literatüründe belgelenmiş tekniklerle **algoritmik olarak üretildi**
  (`generate_dataset.py`):
  1. **Typosquatting**: bilinen markaların adında karakter oynaması +
     şüpheli TLD veya "login/verify/güvenlik" gibi kelimeler
  2. **Combosquatting**: marka adı bozulmadan, başka bir kelimeyle
     birleştirilir (ör. `trendyolindirimleri.com`) — temiz `.com` kullanır,
     typosquat'tan farklı ve tespiti daha zor bir aile
  3. **Leetspeak**: harflerin görsel olarak benzer rakamlarla değişimi
     (`payp4l`, `amaz0n` gibi)
  4. **Punycode/IDN homograf**: `xn--` önekli, Unicode görsel benzerlik
     saldırılarının temsili biçimi
  5. **DGA-tarzı** (Domain Generation Algorithm): kötü amaçlı yazılımların
     C2 sunucularında yaygın görülen yüksek-entropili rastgele string'ler
     + ucuz/anonim TLD'ler (.top, .xyz, .tk vb.)
  6. **Türkçe ccTLD hedefli**: yukarıdaki tekniklerin `.com.tr`/`.gov.tr`
     varyantları — hedef kitle Türkçe konuşanlar olduğu için

### Dürüst değerlendirme: neden "%100 doğruluk" yanıltıcıydı ve nasıl düzeltildi

İlk sürümde model, kendi ürettiği ayrıştırılabilir sentetik veri üzerinde
test edilip **%100 doğruluk** elde etmişti — bu, modelin kendi üretecinin
kalıbını ezberlediğini gösterir, gerçek dünya performansını göstermez.

v2'de bunu düzeltmek için `generate_dataset.py`, eğitim üretecinden
**tamamen bağımsız**, elle yazılmış küçük bir `hard_test_set.csv` de
üretiyor (14 zararlı + 25 güvenilir, farklı bir rastgele tohum ve farklı
kombinasyon mantığıyla). `train_classifier.py` artık modeli **iki ayrı
sette** raporluyor:

| Değerlendirme | ROC-AUC | Doğruluk | Not |
|---|---|---|---|
| Test seti (aynı üretecin %25'i) | 0.9999 | %99 | İyimser — üretecin kendi kalıbı |
| **Zor test seti (bağımsız)** | **0.9557** | **%95** | Gerçekçi tahmin |

Zor test setindeki 2 hata da anlamlı ve dürüstçe raporlanıyor
(`models/evaluation_report.txt`): `hepsiburada-kurumsal.com` (gerçekten
belirsiz bir örnek) ve `istanbul-altyapi.gov.tr` (tireli ama resmi bir
domain — eğitim setinde bu şekli yeterince temsil edilmemiş). Bu, "mükemmel
görün" yerine "gerçekte ne kadar iyi" sorusuna dürüst bir cevap.

Bu iyileştirme sürecinde iki somut hata da bulunup düzeltildi:
- **Çok parçalı TLD hatası**: eski kod `e-devlet.gov.tr` için "gov"u marka
  sanıyor, asıl adı (`e-devlet`) görmezden geliyordu. Türkçe'de `.com.tr`/
  `.gov.tr`/`.edu.tr` çok yaygın olduğu için ciddi bir hataydı — gömülü bir
  "public suffix" alt kümesiyle düzeltildi (`features.py`).
- **Combosquat sinyali kullanılmıyordu**: ilk üretici, combosquat
  örneklerini çoğunlukla şüpheli TLD/tire ile birlikte üretiyordu, bu yüzden
  model `is_combosquat` özelliğini hiç öğrenmeden de "yeterince başarılı"
  oluyordu — temiz `.com` combosquat'ları (`trendyolindirimleri.com` gibi)
  kaçırıyordu. Üretici, gerçek dünyadaki combosquat davranışına (temiz TLD
  tercih edilir) uyacak şekilde yeniden dengelendi; zor test setindeki bu
  tür örneklerin yakalanma oranı %57'den %100'e çıktı.

### Gerçek veriyle değiştirme (kendi sunucunda, tam internet erişimiyle)

Kendi VPS'inde bu kısıtlama olmayacak. Gerçek veri için:

```bash
# data/benign_domains.txt yerine Tranco top-1M listesi:
curl -o data/tranco.csv.zip https://tranco-list.eu/top-1m.csv.zip

# data/malicious_domains.txt için gerçek feed'ler:
curl -o data/urlhaus.txt https://urlhaus.abuse.ch/downloads/text_online/
curl -o data/openphish.txt https://openphish.com/feed.txt
# (PhishTank icin ucretsiz API anahtari: https://phishtank.org/api_info.php)
```

`generate_dataset.py` içindeki `typosquat_variants`/`dga_variants`
çağrılarını, bu gerçek listelerden okuma ile değiştirin — kod yapısı
(özellik çıkarımı, eğitim, DNS proxy) aynı kalır.

## Kurulum ve test (bu makinede / geliştirme)

```bash
cd siber-guard
pip install -r requirements.txt
# testleri de çalıştıracaksan: pip install -r requirements-dev.txt

# 1) Eğitim verisini + bağımsız zor test setini üret
python3 src/generate_dataset.py

# 2) Modeli eğit (CV ile model seçimi + kalibrasyon + iki ayrı raporda değerlendirme)
python3 src/train_classifier.py

# 3) Tek domain kontrol et (eskisiyle birebir uyumlu)
python3 src/check.py trendyol.com
python3 src/check.py giris-trendyol-guvenlik.top

# 3b) Toplu kontrol + JSON çıktı (yeni)
python3 src/check.py --batch domain_listesi.txt
python3 src/check.py --json trendyol.com

# 4) Bir dosyanın/URL'nin gerçek virüs durumunu kontrol et (ücretsiz VT API anahtarıyla)
export VT_API_KEY=senin_anahtarin
python3 src/file_reputation.py /path/to/dosya.exe
python3 src/file_reputation.py --batch dosya1.exe dosya2.zip   # yeni
python3 src/file_reputation.py --url https://supheli-site.top  # yeni

# 5) Testleri çalıştır (yeni, 26 test)
python3 -m pytest tests/ -v

# 6) DNS proxy loglarından özet rapor (yeni)
python3 src/stats.py
```

Bu ortamda gerçekten çalıştırıp doğruladım — çıktı örnekleri:

```
$ python3 src/check.py giris-trendyol-guvenlik.top
Risk skoru : 1.00 (0=guvenli, 1=zararli)
Karar      : YUKSEK RISK - engellenmesi onerilir
Gerekceler :
  - supheli/ucuz uzanti (TLD) kullaniliyor
  - bilinen bir marka adi, baska bir kelimeyle birlestirilmis (combosquat)
  - 'login/verify/guvenlik' gibi supheli kelime iceriyor
  - rastgele/anlamsiz karakter dizisine benziyor (yuksek entropi)

$ python3 src/check.py trendyolindirimleri.com   # temiz .com combosquat, TLD/tire ipucu yok
Risk skoru : 0.93 (0=guvenli, 1=zararli)
Karar      : YUKSEK RISK - engellenmesi onerilir
Gerekceler :
  - bilinen bir marka adi, baska bir kelimeyle birlestirilmis (combosquat)

$ python3 src/check.py e-devlet.gov.tr
Risk skoru : 0.12 (0=guvenli, 1=zararli)
Karar      : GUVENILIR gorunuyor
Gerekceler :
  - resmi/denetimli bir uzanti kullaniyor (.gov.tr, .edu.tr vb.) -- guven artirici
```

```
$ python3 -m pytest tests/ -v
============================== 26 passed in 0.23s ==============================
```

## Dağıtım (kendi bilgisayarın / kendi VPS'in)

### Seçenek A — Kendi bilgisayarında (ev kullanımı)

```bash
python3 src/dns_proxy.py --listen 127.0.0.1 --port 5300 --upstream 1.1.1.1
```

İşletim sisteminin DNS ayarını `127.0.0.1:5300`'e yönlendir (port 53 için
yönetici yetkisi gerekir — `sudo` ile `--port 53` kullanabilirsin).

### Seçenek B — Kendi VPS'in (ev ağının tamamını korumak için)

1. VPS kirala (senin sahip olduğun/yönettiğin bir hesapla), Python 3.10+ kur.
2. Bu klasörü `/opt/siber-guard` altına kopyala, `pip install -r requirements.txt`.
3. `python3 src/generate_dataset.py && python3 src/train_classifier.py`
   (ya da yukarıdaki "gerçek veriyle değiştirme" adımını uygula).
4. `deploy/siber-guard-dns.service` dosyasını `/etc/systemd/system/`'e
   kopyala, yolları kendi kurulumuna göre düzelt:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now siber-guard-dns
   ```
5. Ev router'ının DNS ayarında bu VPS'in IP'sini birincil DNS olarak gir.
   Artık router'a bağlı **tüm cihazların** DNS sorguları bu filtreden geçer.

### Ayarları özelleştirme (config.json)

```bash
cp config.example.json config.json
# risk_threshold, upstream_dns, cache_ttl_seconds vb. duzenle
```

`config.json` yoksa (varsayılan), önceki sürümdeki sabit değerler kullanılır
— hiçbir mevcut kurulum bozulmaz. Komut satırı argümanları her zaman
`config.json`'daki değeri ezer.

**İzin/red listesi ile yanlış-pozitif/negatifi düzeltme:**

```bash
echo "guvendigim-ozel-site.com" >> data/allowlist.txt
echo "bilinen-kotu-site.top"    >> data/denylist.txt
```

Bu listelerdeki domainler, modelin kararından **bağımsız olarak** her zaman
izin verilir/engellenir — yanlış bir sınıflandırmayı model yeniden
eğitmeden anında düzeltmenin yolu budur.

### Güvenlik notları (mutlaka oku)

- Bu DNS sunucusunu **herkese açık (0.0.0.0)** dinletirsen, herkes onu
  kullanabilir/kötüye kullanabilir (DNS amplification saldırısına araç
  olabilir). Güvenlik duvarında yalnızca kendi IP aralığından gelen
  sorgulara izin ver.
- Yalnızca **kendi cihazların/ağın** için kullan. Başkasının trafiğini onun
  bilgisi/izni olmadan bu sisteme yönlendirmek yasa dışı olabilir.
- `risk-threshold` değerini düşük tutarsan (örn. 0.3) daha agresif engeller
  ama yanlış-pozitif (gerçekte güvenli bir siteyi engelleme) artar.
  Varsayılan 0.6 dengelidir.

## Sınırlamalar (dürüstçe)

1. **Sentetik eğitim verisi** — yukarıda açıklandığı gibi, gerçek feed'lerle
   değiştirilmeden production'da güvenilmemeli. Zor test setindeki %95
   doğruluk gerçekçi bir tahmindir ama gerçek feed'lerle eğitilmiş bir
   modelin yerini tutmaz.
2. **Sadece domain adına bakar** — bir domain adı temiz görünüp arkasında
   zararlı içerik barındırabilir (ele geçirilmiş meşru sitede barındırılan
   phishing sayfası gibi). Bu senaryoyu DNS seviyesi filtre yakalayamaz.
3. **Dosya/virüs kontrolü** kendi eğittiğimiz modelden DEĞİL, gerçek
   VirusTotal API'sinden geliyor — bu bilinçli bir tercih (bkz.
   `file_reputation.py` içindeki not): ikili dosya analizi güvenilir
   şekilde sıfırdan yapılamayacak kadar zor ve riskli bir problemdir. Dosya
   adı kontrolü (çift uzantı gibi) ise **açıkça kural tabanlı bir
   heuristiktir, ML değildir** — öyle etiketlenip sunulur.
4. **False positive/negative olur** — bu bir güvenlik katmanıdır, tek
   başına yeterli koruma değildir. Gerçek antivirüs yazılımı, güncel
   işletim sistemi ve dikkatli tıklama alışkanlığının yerini tutmaz.
   Yanlış sınıflandırmaları `data/allowlist.txt`/`denylist.txt` ile anında
   düzeltebilirsin.
5. **Gömülü "public suffix" listesi tamdeğil** — `features.py` içindeki
   `MULTI_PART_SUFFIXES`, en yaygın Türkçe ve uluslararası çok parçalı
   uzantıları içerir ama publicsuffix.org'daki binlerce kuralın tamamını
   kapsamaz. Kendi VPS'inde tam internet erişimin varsa, `tldextract`
   kütüphanesiyle değiştirmen daha sağlam olur.
6. **Bu geliştirme ortamına özgü bir ağ garipliği gözlemlendi**: bazen DNS
   proxy başladıktan sonra gelen ilk UDP paketi sunucuya hiç ulaşmıyor
   (muhtemelen bu sandbox'ın sanal ağ altyapısının "ilk paket" davranışı).
   Bunun etkisini sınırlamak için `dns_proxy.py`'ye sınırlı yeniden deneme
   ve zaman aşımında SERVFAIL dönme eklendi — istemci artık hiçbir durumda
   süresiz beklemiyor. Gerçek bir VPS/ev ağında bu özel davranışı
   beklemiyoruz (standart ağ, iç içe sanallaştırma yok).

## v2 — Neler değişti (özet)

| Alan | v1 | v2 |
|---|---|---|
| `.gov.tr`/`.com.tr` ayrıştırma | Hatalı (marka adı kayboluyordu) | Gömülü public-suffix alt kümesiyle düzeltildi |
| Zararlı üretim teknikleri | 2 (typosquat, DGA) | 6 (+ combosquat, leetspeak, punycode, TR ccTLD hedefli) |
| Özellik sayısı | 14 | 18 (+combosquat, punycode, regulated-suffix, port, dot-count) |
| Model seçimi | Tek RandomForest | 3 aday, 5-kat CV ile en iyisi seçilir + kalibrasyon |
| Değerlendirme | Tek test seti, %100 (yanıltıcı) | Test seti + bağımsız zor test seti (%95, dürüst) |
| DNS proxy | Önbelleksiz, izin/red listesi yok | TTL önbellek, allowlist/denylist, istatistik, ısınma çağrısı |
| DNS proxy dayanıklılığı | Upstream başarısız olursa istemci asılı kalabilir | Sınırlı deneme + SERVFAIL garantisi |
| CLI (`check.py`) | Tek domain | + toplu mod, + JSON çıktı, + gerekçe genişletildi |
| `file_reputation.py` | Tek dosya, önbelleksiz | + toplu mod, + URL kontrolü, + önbellek, + çift-uzantı heuristiği |
| Ayarlar | Sadece CLI argümanı | + `config.json` (geriye dönük uyumlu) |
| Testler | Yok | 26 pytest testi (`tests/`) |
| Log/istatistik | Sadece ham log | + `stats.py` özet raporu |

Tüm eski komutlar (`check.py <domain>`, `file_reputation.py <dosya>`,
`dns_proxy.py --listen ... --port ...`) **birebir aynı şekilde** çalışmaya
devam ediyor — yeni özelliklerin hepsi opsiyonel ekler.
