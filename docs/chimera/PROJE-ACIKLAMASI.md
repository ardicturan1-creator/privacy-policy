# CHIMERA EDR — Proje Açıklaması

## Bu proje nedir?

CHIMERA, Rust ile yazılmış, **üç ayrı yürütülebilir dosyadan** oluşan,
son-kullanıcı iş istasyonuna kurulan bir **Uç Nokta Tespit ve Müdahale
(EDR) ajanı** iskeletidir. Post-kuantum kriptografi (ML-KEM-1024,
ML-DSA-87 — NIST FIPS 203/204) üzerine kurulu, kimlik doğrulamalı ve
şifreli bir yerel IPC protokolüyle birbirine bağlı üç bileşenden oluşur:

| Dosya | Rol |
|---|---|
| `chimera-core.exe` | Arka planda sürekli çalışan ana servis. Kimlik doğrulama, IPC sunucusu, kendi kendini onaran watchdog, siber-yanıltma (decoy dosyalar + tarpit), kanıta-dayanıklı denetim kaydı. |
| `chimera-sentinel.exe` | Core'un karşılıklı watchdog eşi. Core'u periyodik olarak "yokluyor" (heartbeat); yanıt gelmezse yeniden başlatıyor. Core da aynı şekilde sentinel'i izliyor — **çift yönlü** bir kaza-sonrası-toparlanma mekanizması. |
| `chimera-admin.exe` | Sıfır Güven (Zero-Trust) komut satırı kontrol paneli. Ayrıcalıklı işlemler (günlükleri okuma, modu değiştirme, denetim zincirini doğrulama) yalnızca Shamir(2,3) paylarının en az ikisi verildiğinde çalışır. |

## Neden üç ayrı dosya?

Gerçek EDR ürünlerinde olduğu gibi: **hiçbir tek sürecin çökmesi/durması
tüm korumayı devre dışı bırakmamalı.** GUI/kontrol paneli (`chimera-admin`)
kapansa, hatta hiç açılmasa bile `chimera-core` bağımsız çalışmaya devam
eder. `chimera-core` beklenmedik şekilde çökerse (kaza, panik), eşi
`chimera-sentinel` bunu saniyeler içinde fark edip yeniden başlatır — ve
tersi de geçerlidir.

**Önemli ve bilinçli bir sınır:** Bu "karşılıklı izleme", işletim
sisteminin veya bir yöneticinin **bilinçli** sonlandırmasına (Görev
Yöneticisi'nde "Son İşlem", `taskkill /F`, SIGKILL) karşı DİRENMEZ ve
direnmemelidir — böyle bir direnç, meşru güvenlik araştırmacılarının ve
olay müdahale ekiplerinin sistemi analiz etmesini de aynı ölçüde engeller
ve savunma yazılımını zararlı yazılım/rootkit davranışına yaklaştırır.
Sadece **kaza sonucu** (çökme, panik, beklenmeyen çıkış) duran bir bileşeni
otomatik olarak toparlar. Bilinçli bir `stop` komutu HER ZAMAN saygı görür.

## Öne çıkan güvenlik özellikleri

- **Post-kuantum kimlik doğrulama:** Her bileşenin kalıcı bir ML-DSA-87
  imza kimliği vardır; el sıkışmalar ML-KEM-1024 ile şifrelenir,
  XChaCha20-Poly1305 ile korunan bir oturum anahtarı türetilir.
- **Sıfır Güven yönetim paneli:** Master anahtar hiçbir zaman tek bir
  yerde bütün halde saklanmaz — Shamir(2,3) ile 3 parçaya bölünür (TPM/
  donanım tokenı, yönetici parolası, offline zarf). Herhangi 2'si yeterli,
  tek başına hiçbiri yetmez.
- **Karşılıklı ikili doğrulama (attestation):** Her iki taraf da
  birbirinin ÇALIŞAN ikili dosyasının BLAKE3 özetini el sıkışma sırasında
  değiş tokuş eder ve operatörün önceden sabitlediği değerle karşılaştırır
  — çalınmış bir kimlik anahtarıyla bile değiştirilmiş bir ikili dosya
  bağlanamaz.
- **Kanıta-dayanıklı (tamper-evident) denetim kaydı:** Tüm güvenlik
  olayları (`core.start`, yetkisiz erişim denemeleri, mod değişiklikleri,
  kalp atışları) BLAKE3 ile bir öncekine zincirlenmiş JSON satırları olarak
  yazılır. Diske erişimi olan bir saldırgan geçmiş bir satırı sessizce
  değiştirirse/silerse, zincirdeki kopma `chimera-core verify-audit`
  (yerel) veya `chimera-admin verify-audit` (uzaktan, Sıfır Güven ile)
  komutuyla YAKALANIR.
- **Siber yanıltma:** Gerçekçi görünen tuzak (decoy) dosyalar ve saldırgan
  araçlarını sınırlı süreyle oyalayan bir bağlantı tarpiti.
- **Hız sınırlama:** Yerel IPC bağlantı kabul döngüsü, pahalı kriptografik
  el sıkışma işlemlerinden ÖNCE standart bir GCRA hız sınırlayıcıdan
  geçer — yetkisiz bir yerel sürecin CPU tüketerek hizmet dışı bırakma
  denemesini engeller.
- **Windows ikili sertleştirmesi:** ASLR, DEP açık; sembol tabloları
  temizlenmiş (stripped); yapı makinesine ait dosya yolları ikili dosyadan
  kaldırılmış (`--remap-path-prefix`); panik mesajları konsola değil
  yalnızca yerel denetim kaydına yazılır.

## Bu sürüm neyi KAPSAMAZ (bilinçli ve belgelenmiş sınırlar)

Bu, "hackerlar hiçbir şekilde giremez" iddiasında bir ürün DEĞİLDİR — böyle
bir iddia gerçekçi değildir ve genellikle zararlı yazılım davranışına
yaklaşmadan sağlanamaz. Bunun yerine amaç: **ikili dosya incelense/tersine
mühendislik yapılsa bile kritik sırların, özel anahtarların ve güven
zincirinin ele geçirilememesi; ikili değiştirildiğinde veya sahte bir
bileşen sisteme sokulduğunda bunun tespit edilebilmesidir.**

Bilinen, belgelenmiş sınırlar (ayrıntılar için `docs/chimera/03-HARDENING.md`
ve `04-TURN5-DERINLEMESINE-IYILESTIRME.md`):
- Kernel-seviyesi (Ring-0) bir sürücü YOKTUR — tüm koruma Ring-3'te çalışır.
- Windows Control Flow Guard (CFG), bu ortamda kullanılan MinGW/GNU linker
  ile üretilemedi (gerçek Windows + MSVC araç zinciri gerektirir).
- Gerçek bir arka uç/tehdit istihbaratı entegrasyonu yoktur — bu, tek
  başına çalışan bir uç nokta ajanı iskeletidir.
- İmzalı otomatik güncelleme mekanizması bu sürümde YOKTUR (bilinen bir
  boşluk olarak belgelenmiştir).

## Nasıl denenir (hızlı başlangıç)

```
chimera-core.exe identity --root C:\ChimeraData
chimera-admin.exe identity --root C:\ChimeraData
chimera-core.exe trust --root C:\ChimeraData --pubkey <admin-hex>
chimera-admin.exe trust-core --root C:\ChimeraData --pubkey <core-hex>
chimera-core.exe provision --root C:\ChimeraData
chimera-core.exe serve --root C:\ChimeraData
:: (ayri bir terminalde)
chimera-admin.exe status --root C:\ChimeraData
chimera-admin.exe verify-audit --root C:\ChimeraData --share <A> --share <B>
```

## Nasıl doğrulandı?

Bu paketteki 3 `.exe`, bu Linux tabanlı geliştirme ortamında
`x86_64-pc-windows-gnu` hedefine GERÇEKTEN çapraz derlenmiş, ardından
**Wine 9.0 altında gerçek Windows ikili yürütmesi olarak** test edilmiştir
(salt statik çapraz-derleme doğrulaması değil): kimlik üretimi, güven
kurulumu, provizyon, IPC üzerinden gerçek el sıkışma + kalp atışı trafiği,
ayrıcalıklı komutlar, ve kasıtlı olarak kurcalanmış bir denetim kaydının
hem yerel hem uzaktan doğrulama yollarıyla YAKALANMASI dahil. Ayrıca 58
otomatik birim/entegrasyon testi (`cargo test --workspace`) ve bir PE
başlık/sertleştirme denetim betiği (`scripts/release-check.py`) ile
desteklenmektedir.

Bu belge ve pakette teknik olarak doğrulanamamış hiçbir iddia
bulunmamaktadır: her özellik ya gerçek bir testle ya da gerçek bir canlı
süreç doğrulamasıyla desteklenmiştir.
