# CHIMERA EDR — Turn 6: Gerçek Ağ Savunması + Detector/Validator/Executor

Bu tur, kullanıcının "sadece izleyen değil, ağ saldırılarını ve yazılım
tehditlerini GERÇEKTEN engelleyen, düzenli tarayan, bulduğu açığı
otomatik yamalayan, 3 ajanlı (biri yamalar/biri test eder/biri
tamamlar) bir yapı" talebine karşılık verir. Talebin İKİ kısmı, literal
haliyle uygulanamaz/güvensiz olduğu için BİLİNÇLİ olarak farklı,
gerçekten çalışan bir biçimde karşılandı — aşağıda hem NEDEN hem de
YERİNE NE YAPILDIĞI açıklanıyor.

## 0) İki bilinçli kapsam kararı

### 0.1 Neden özel bir kernel driver (`.sys`) YOK

Modern Windows'ta (Secure Boot + driver imza zorunluluğu açıkken) imzasız
bir kernel driver **yüklenmez**: Microsoft'un EV kod imzalama sertifikası
+ Microsoft'a gönderilip alınan attestation imzası olmadan `SERVICE_START`
`ERROR_INVALID_IMAGE_HASH`/benzer bir hata ile reddedilir. Bu ortamda
(Linux tabanlı, WDK/MSVC olmayan bir çapraz-derleme zinciri) böyle bir
imzayı üretmenin teknik imkânı da yok. Bu nedenle "işte kernel driver'ın"
diyip yüklenmeyecek bir `.sys` üretmek, projenin baştan beri bağlı olduğu
"sahte/çalışmayan bileşen YOK" kuralını ihlal eder.

**Yerine ne yapıldı:** `firewall.rs`, Windows'un **kendi** kernel-seviyeli
filtreleme motorunu (WFP), o motorun resmî kullanıcı-modu COM arayüzü
(`INetFwPolicy2`/`INetFwRule`, `HNetCfg.FwPolicy2`/`HNetCfg.FWRule`
ProgID'leri üzerinden `CLSIDFromProgID`+`CoCreateInstance`) ile yönetir —
Windows Güvenlik Duvarı GUI'sinin, `netsh advfirewall`'ın ve PowerShell
`NetSecurity` modülünün ARKA PLANDA kullandığı, Vista'dan beri değişmeyen
aynı kararlı genel API. Engelleme gerçekten kernel seviyesinde uygulanır;
bunu uygulayan kod bizim yazıp imzalamamız gereken bir sürücü değil,
işletim sisteminin kendisidir.

### 0.2 Neden "her açığı otomatik bulup kendi kendine patch'lesin" YOK (literal haliyle)

Hiçbir ciddi EDR/güvenlik ürünü (CrowdStrike, Microsoft Defender ATP
dahil), insan onayı olmadan sistem/kernel durumunu değiştiren, açık uçlu
bir "tespit et → yama yaz → dağıt" otomasyonu ÇALIŞTIRMAZ — yanlış bir
tespit veya kötü bir yama, sistemi bizzat kendisi bozabilir/yeni bir açık
açabilir. Bunun riski, çözdüğü sorunla kıyaslanamayacak kadar büyüktür.

**Yerine ne yapıldı:** `pipeline.rs`, kullanıcının istediği "biri
yamalar/biri test eder/biri tamamlar" fikrinin GERÇEK ve GÜVENLİ
karşılığı olan üç aşamalı bir mimari uygular (aşağıda §2).

## 1) Yeni: `firewall.rs` — gerçek ağ saldırısı engelleme

`crates/chimera-core/src/firewall.rs`. Windows'ta (`#[cfg(windows)]`)
`windows` crate'inin GERÇEK, üretilmiş Win32/COM bağlarını kullanır
(kaynak kodu doğrudan `~/.cargo/registry`'deki üretilmiş bindings
dosyasından okunarak yazıldı — API imzaları tahmin edilmedi):

- `block_ip(root, ip)`: verilen adres için HEM gelen HEM giden yönde birer
  `INetFwRule` oluşturup `INetFwPolicy2::Rules().Add()` ile ekler (tüm
  profiller: Domain/Private/Public). Yalnızca gelen yönü engellemek, ele
  geçirilmiş bir sürecin o adrese GİDEN bağlantısını (C2'ye "eve telefon")
  engellemezdi — bu yüzden iki kural.
- `unblock_ip(root, ip)`: her iki kuralı `Rules().Remove()` ile kaldırır.
- `list_blocked_ips(root)`: yerel bir aday listesini (`state/blocked_ips.list`)
  **GERÇEK** güvenlik duvarı durumuna karşı `Rules().Item()` ile doğrular —
  dışarıdan silinmiş kurallar listeden düşer, yalnızca kendi defterine
  körü körüne güvenmez.
- `is_enabled()`: aktif ağ profili için Windows Firewall'ın açık olup
  olmadığını `INetFwPolicy2::get_FirewallEnabled()` ile sorgular.
- Girdi doğrulama: `validate_ip()` gerçek bir IPv4/IPv6 olmayan hiçbir
  şeyin COM API'sine "adres" olarak geçmesine izin vermez.
- Linux'ta (`#[cfg(not(windows))]`) her fonksiyon açık bir "desteklenmiyor"
  hatası döner — `cargo test --workspace` bu yüzden native Linux'ta da
  derlenir/çalışır, sahte bir "başarılı" davranışı SİMÜLE ETMEZ.

## 2) Yeni: `pipeline.rs` — Detector → Validator → Executor

Kullanıcının "3 ajan" isteğinin gerçek karşılığı:

1. **Detector** — `scanner::scan()` çalıştırır, ham bulgu listesi üretir.
2. **Validator** — her bulgunun `remediation` alanını SABİT bir
   whitelist'e (`is_whitelisted()`) karşı kontrol eder. Whitelist DIŞINDA
   hiçbir aksiyon ASLA çalıştırılmaz; yalnızca "İNSAN İNCELEMESİ GEREKEN"
   listesine düşer.
3. **Executor** — SADECE whitelist'i geçen aksiyonları `remediate.rs`
   üzerinden uygular. HER deneme (başarılı/başarısız) `auditlog::append`
   ile kanıta-dayanıklı denetim kaydına yazılır.

Whitelist bilinçli olarak KÜÇÜKTÜR (`Remediation::EnableFirewall`,
`Remediation::DisableSmb1`) — her ikisi de `remediate.rs`'te belgelenen
dört şartı karşılar: (a) tek/dar bir ayarı değiştirir, (b) geri
alınabilir, (c) veri kaybına yol açmaz, (d) reboot/servis yeniden
başlatma gibi kesinti yaratan adımları OTOMATİK TETİKLEMEZ (SMB1
kapatma, servis yeniden başlatılana kadar tam etkin olmadığını açıkça
söyler, ama yeniden başlatmayı kendisi yapmaz).

`chimera-core serve` artık bu boru hattını 30 dakikada bir arka planda
otomatik çalıştıran bir thread başlatır (`pipeline::spawn_background_loop`)
— var olan `sentinel-watchdog` thread'iyle AYNI temiz-durdurma desenini
izler (bkz. §5).

## 3) Yeni: `scanner.rs` — gerçek sertleştirme kontrolleri

Beş somut, doğrulanabilir kontrol (geniş bir CVE veritabanı/tehdit
istihbaratı entegrasyonu YOKTUR — bkz. proje sınırları):

| Kontrol | API | Otomatik düzeltme |
|---|---|---|
| Güvenlik duvarı kapalı mı | `firewall::is_enabled()` | VAR (whitelist) |
| SMBv1 açık mı | Registry `LanmanServer\Parameters\SMB1` | VAR (whitelist) |
| RDP açık ama NLA zorunlu değil | Registry `WinStations\RDP-Tcp\UserAuthentication` | YOK (insan onayı gerekir — erişim yöntemini etkileyebilir) |
| Dinleyen TCP portları | `GetExtendedTcpTable` (gerçek Win32 API) | YOK (yalnızca rapor) |
| Autorun (`Run`) kayıtları | `RegEnumValueW` ile numaralandırma | YOK (imza veritabanı olmadan "kötü niyetli" yargısı verilemez) |

## 4) Yeni: `remediate.rs` — Executor'ın çalıştırdığı dar düzeltmeler

`enable_firewall()`: `INetFwPolicy2::put_FirewallEnabled()` ile üç
profilde de (Domain/Private/Public) açar. `disable_smb1()`:
`RegSetValueExW` ile `SMB1=0` yazar (anahtar zaten LanmanServer'ın
kendi kurduğu, her Windows'ta var olan bir anahtar olduğu için
`RegCreateKeyExW`/`Win32_Security` bağımlılığına gerek kalmadı).

## 5) IPC protokolü ve `chimera-admin` genişlemesi

`chimera-ipc::protocol`'e 4 yeni istek (`ScanNow`, `BlockIp`, `UnblockIp`,
`ListBlockedIps`) ve 2 yeni yanıt (`ScanReportOk`, `BlockIpOk`) eklendi —
hepsi diğer ayrıcalıklı komutlarla AYNI Shamir(2,3) `unlock` kapısına
tabidir. `chimera-admin`'e karşılık gelen 4 alt komut eklendi:

```
chimera-admin scan         --root R --share HEX --share HEX
chimera-admin block-ip     --root R --share HEX --share HEX --ip <IP>
chimera-admin unblock-ip   --root R --share HEX --share HEX --ip <IP>
chimera-admin list-blocked --root R --share HEX --share HEX
```

`chimera-core serve`'ün istek döngüsüne 4 yeni `match` kolu eklendi;
her biri `unlocked()` kontrolünden geçmeden `Denied` döner.

## 6) Canlı Wine testi — GERÇEK sonuçlar (iyi VE kötü haber dahil)

Taze bir test kökünde tam yaşam döngüsü çalıştırıldı: identity → trust →
provision → serve → scan → block-ip → list-blocked → unblock-ip →
verify-audit → temiz durdurma.

**Çalıştığı doğrulanan:**
- `scan`, gerçek IPC üzerinden çalıştı; `GetExtendedTcpTable` Wine'ın
  KENDİ dinleyen TCP portlarını (5 adet, gerçek PID'lerle) GERÇEKTEN
  döndürdü — bu simüle edilmiş bir veri değil, canlı bir Win32 API
  çağrısının sonucu.
- `block-ip`/`unblock-ip`, geçersiz bir IP'yi (`not-an-ip`) COM'a hiç
  gitmeden temiz bir hatayla reddetti.
- Her scan/block/unblock denemesi, hash-zincirli denetim kaydına yazıldı;
  `verify-audit` zincirin bastan sona SAĞLAM olduğunu (29+ kayıt)
  doğruladı.
- Core'un otomatik başlattığı `chimera-sentinel` eşiyle karşılıklı
  heartbeat trafiği kesintisiz devam etti — yeni thread'ler mevcut
  watchdog mimarisini BOZMADI.
- Temiz durdurma sinyali (bkz. aşağıdaki dürüst not) geldiğinde yeni
  `pipeline_running` bayrağı da doğru şekilde `false`'a çekildi;
  arka plan tarama thread'i asılı kalmadı, 0 zombie süreç.

**Dürüstçe belgelenmesi gereken bir Wine SINIRI (yeni bir kod hatası
DEĞİL — bir test-ortamı gerçeği):** Wine'ın `HNetCfg.FwPolicy2`/`FWRule`
COM uygulaması KISMİdir: `Rules().Add()`/`Rules().Remove()` çağrıları
S_OK ile "başarılı" dönüyor (muhtemelen gerçek bir kural deposu
OLMADAN), ama `Rules().Item()` (var olup olmadığını GERÇEKTEN sorgulayan
çağrı) dürüstçe "bulunamadı" diyor. Bu yüzden `list-blocked`, `block-ip`
"başarılı" dedikten hemen sonra bile BOŞ liste döndürdü — kodun kendi iç
tutarlılık kontrolü (Item ile doğrulama) bu durumu YANLIŞ bir "hâlâ
blokta" iddiasına dönüştürmek yerine doğru şekilde boş listeye düşürdü.
**Sonuç:** COM çağrı DİZİSİ ve tip imzaları gerçek, üretilmiş Windows
bindings'e karşı doğrulandı (derleyici hatası yok) ve Wine altında
çökmeden çalıştı; ama "gerçek bir Windows makinesinde paket GERÇEKTEN
bloklanıyor mu" sorusunun nihai kanıtı, Wine'ın kısmi güvenlik duvarı
desteği nedeniyle yalnızca GERÇEK bir Windows makinesinde
tamamlanabilir.

**Dürüstçe belgelenmesi gereken İKİNCİ bir bulgu (mevcut, Turn-5'ten
kalma temiz-durdurma mekanizmasıyla ilgili):** `kill -TERM` (SIGTERM),
Wine altında çalışan bu ikili dosyanın kayıtlı `SetConsoleCtrlHandler`
tabanlı işleyicisini TETİKLEMEDİ — `stop.flag` hiç yazılmadı. `kill -INT`
(SIGINT) ise GÜVENİLİR şekilde tetikledi: `stop.flag` yazıldı, `core.stop`
denetim kaydına düştü, süreç 2 saniye içinde temiz çıktı, 0 zombie.
Bunun nedeni muhtemelen Wine'ın SIGINT'i Windows'un `CTRL_C_EVENT`'ine
eşlemesi (belgelenen bir Wine davranışı) ama SIGTERM için böyle bir
eşleme sunmaması. **Gerçek Windows'ta bunun anlamı:** `ctrlc` crate'i
Windows'ta `CTRL_C_EVENT`, `CTRL_CLOSE_EVENT`, `CTRL_LOGOFF_EVENT` ve
`CTRL_SHUTDOWN_EVENT`'in HEPSİNİ AYNI işleyiciye bağlar — yani gerçek bir
Windows'ta Ctrl-C, konsol penceresini kapatma veya oturum kapatma/sistem
kapatma HEPSİ bu turda doğrulanan AYNI kod yolunu tetikler. Doğrulanamayan
tek şey, bu ikilinin şu anda bir Windows SERVİSİ olarak KURULU
olmadığıdır (düz bir konsol/arka plan exe'sidir) — bir Windows Servisi
olarak kurulsaydı, `SERVICE_CONTROL_STOP` farklı bir mekanizma
(`RegisterServiceCtrlHandlerEx`) gerektirirdi; bu, projenin baştan beri
belgelenen kapsamının DIŞINDADIR, Turn 6'da YENİ ortaya çıkan bir sınır
değildir.

## 7) Test özeti

- `cargo test --workspace` (native Linux): **67/67 test geçti** (Turn 5
  sonu: 58 — 9 yeni test: `firewall.rs` 4, `scanner.rs` 2, `pipeline.rs` 3).
- `x86_64-pc-windows-gnu` çapraz derlemesi: ilk denemede İKİ EKSİKSİZ
  derleme (tüm COM/registry/IP-helper FFI çağrıları dahil) — API
  imzaları tahmin edilmeden, üretilmiş `windows` crate kaynağından
  doğrudan okunarak yazıldığı için.
- `scripts/release-check.py`: 3 exe de PE sertleştirme geçidini geçti
  (ASLR/DEP/stripped/yol-temizliği, yeni `ole32.dll`/`advapi32.dll`/
  `iphlpapi.dll` bağlantılarıyla birlikte).
- Canlı Wine 9.0 testi: yukarıda §6.

## Özet: Bu turda eklenenler

| # | Bileşen | Durum |
|---|---|---|
| `firewall.rs` (gerçek ağ engelleme) | chimera-core | Eklendi, cross-compile + Wine'da canlı test (Wine'ın COM sınırı belgelendi) |
| `scanner.rs` (5 sertleştirme kontrolü) | chimera-core | Eklendi, `GetExtendedTcpTable` canlı Wine'da GERÇEK veriyle doğrulandı |
| `pipeline.rs` (Detector/Validator/Executor) | chimera-core | Eklendi, whitelist mantığı birim test + canlı IPC round-trip |
| `remediate.rs` (2 whitelisted düzeltme) | chimera-core | Eklendi |
| 4 yeni IPC mesajı | chimera-ipc | Eklendi, round-trip testli |
| 4 yeni `chimera-admin` alt komutu | chimera-admin | Eklendi, canlı Wine'da test edildi |

Hiçbir mevcut işlevsellik bozulmadı (67/67 test yeşil); hiçbir özel
kriptografik/güvenlik ilkeli icat edilmedi (Windows'un kendi Firewall/
Registry/IP-Helper API'leri kullanıldı); hiçbir kernel driver iddiası
YOKTUR; hiçbir "otonom AI kernel patch'i" iddiası YOKTUR — her ikisi de
yukarıda §0'da açıklanan somut, teknik/güvenlik gerekçeleriyle bilinçli
olarak kapsam dışı bırakıldı.
