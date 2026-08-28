# Siber-Guard — Kişisel Ağ Güvenliği Sınıflandırıcısı

Kendi eğitilmiş, hafif bir makine öğrenmesi sınıflandırıcısıyla **hangi
bağlantının güvenli, hangisinin şüpheli/zararlı olduğunu** tespit eden ve
isteğe bağlı olarak engelleyen bir DNS filtreleme sistemi. Artı, bir
dosyanın (programın) virüs olup olmadığını **gerçek antivirüs
veritabanlarına** sorarak kontrol eden yardımcı araç.

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
                              ├─ domain → özellik çıkarımı (features.py)
                              ├─ özellikler → RandomForest sınıflandırıcı
                              │
                    risk ≥ eşik? ──── evet ──→ NXDOMAIN (engellendi, log'a yazılır)
                              │
                             hayır
                              │
                              ↓
                    gerçek DNS sunucusuna (1.1.1.1) yönlendir → gerçek cevabı dön
```

## Klasör yapısı

```
siber-guard/
  data/
    benign_domains.txt   — bilinen, gerçek, güvenilir domain listesi (~225 adet)
    dataset.csv           — generate_dataset.py çıktısı (eğitim verisi)
  src/
    features.py            — domain adından leksik/istatistiksel özellik çıkarımı
    generate_dataset.py    — etiketli eğitim verisi üretir (bkz. aşağıdaki uyarı)
    train_classifier.py    — RandomForest sınıflandırıcıyı eğitir ve değerlendirir
    check.py                — tek domain/URL için CLI kontrol aracı
    dns_proxy.py             — DNS filtreleme proxy sunucusu
    file_reputation.py        — dosya SHA-256 → VirusTotal itibar sorgusu
  models/
    domain_classifier.joblib  — eğitilmiş model (~480 KB, hafif)
  deploy/
    siber-guard-dns.service   — systemd servis örneği (Linux sunucu için)
  requirements.txt
```

## ⚠️ Eğitim verisi hakkında dürüst uyarı

Bu geliştirme ortamının ağ politikası, canlı tehdit istihbaratı
feed'lerine (URLhaus, OpenPhish, PhishTank, Tranco) erişimi engelliyor
(sadece paket kayıtları gibi belirli adresler açık). Bu yüzden:

- **Güvenilir domain listesi** (`data/benign_domains.txt`): gerçek, iyi
  bilinen ~225 popüler ve kurumsal domain (Google, bankalar, .gov.tr
  siteleri, üniversiteler vb.) — bunlar gerçek.
- **Zararlı domain örnekleri**: canlı bir feed'den değil, güvenlik
  literatüründe belgelenmiş iki teknikle **algoritmik olarak üretildi**
  (`generate_dataset.py`):
  1. **Typosquatting**: bilinen markaların adında karakter oynaması +
     şüpheli TLD veya "login/verify/güvenlik" gibi kelimeler
  2. **DGA-tarzı** (Domain Generation Algorithm): kötü amaçlı yazılımların
     C2 sunucularında yaygın görülen yüksek-entropili rastgele string'ler
     + ucuz/anonim TLD'ler (.top, .xyz, .tk vb.)

Bu sentetik veri, modelin **çalıştığını göstermek** için yeterli (aşağıdaki
test sonuçlarına bakın) ama gerçek dünyadaki tüm phishing çeşitliliğini
kapsamaz. **Test sonucundaki %100 doğruluk bu yüzden yanıltıcıdır** —
model, kendi ürettiğim ayrıştırılabilir sentetik veriyi mükemmel öğrenmiş
demektir, gerçek dünyada bu kadar yüksek olmayacaktır.

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

# 1) Eğitim verisini üret
python3 src/generate_dataset.py

# 2) Modeli eğit (birkaç saniye sürer, CPU yeterli)
python3 src/train_classifier.py

# 3) Tek domain kontrol et
python3 src/check.py trendyol.com
python3 src/check.py giris-trendyol-guvenlik.top

# 4) Bir dosyanın gerçek virüs durumunu kontrol et (ücretsiz VT API anahtarıyla)
export VT_API_KEY=senin_anahtarin
python3 src/file_reputation.py /path/to/dosya.exe
```

Bu ortamda gerçekten çalıştırıp doğruladım — çıktı örnekleri:

```
$ python3 src/check.py giris-trendyol-guvenlik.top
Risk skoru : 0.99 (0=guvenli, 1=zararli)
Karar      : YUKSEK RISK - engellenmesi onerilir
Gerekceler :
  - supheli/ucuz uzanti (TLD) kullaniliyor
  - 'login/verify/guvenlik' gibi supheli kelime iceriyor
  - rastgele/anlamsiz karakter dizisine benziyor (yuksek entropi)

$ python3 src/check.py trendyoll.com     # gercekci .com typosquat, TLD ipucu yok
Risk skoru : 0.50 (0=guvenli, 1=zararli)
Karar      : SUPHELI - dikkatli olun
Gerekceler :
  - bilinen bir markanin yazilisina typo ile benziyor
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
   değiştirilmeden production'da güvenilmemeli.
2. **Sadece domain adına bakar** — bir domain adı temiz görünüp arkasında
   zararlı içerik barındırabilir (ele geçirilmiş meşru sitede barındırılan
   phishing sayfası gibi). Bu senaryoyu DNS seviyesi filtre yakalayamaz.
3. **Dosya/virüs kontrolü** kendi eğittiğimiz modelden DEĞİL, gerçek
   VirusTotal API'sinden geliyor — bu bilinçli bir tercih (bkz.
   `file_reputation.py` içindeki not): ikili dosya analizi güvenilir
   şekilde sıfırdan yapılamayacak kadar zor ve riskli bir problemdir.
4. **False positive/negative olur** — bu bir güvenlik katmanıdır, tek
   başına yeterli koruma değildir. Gerçek antivirüs yazılımı, güncel
   işletim sistemi ve dikkatli tıklama alışkanlığının yerini tutmaz.
