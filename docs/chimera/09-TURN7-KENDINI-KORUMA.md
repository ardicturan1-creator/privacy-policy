# CHIMERA EDR — Turn 7 / Faz 4: Kendini Koruma

Faz 1–3 boyunca CHIMERA hep **başka** şeylere baktı: şifreleyen süreçler,
saldıran IP'ler, yedek imha komutları. Faz 4 soruyu tersine çevirir:
*CHIMERA'nın kendi süreci ne kadar dayanıklı, ve makinenin kendisinden
gerçek zamanlı telemetri alabiliyor mu?*

## A) Önceki Seviye (Faz 3 sonu)

| Yetenek | Faz 3 durumu |
|---|---|
| Kendi adres alanını koruma | **Yok** — hiçbir azaltma politikası uygulanmıyor |
| Kernel-kaynaklı gerçek zamanlı telemetri | **Yok** — tespit `notify` ve olay günlüğü gecikmelerine bağlı |
| Windows Servisi olarak çalışma | **Yok** — düz konsol exe'si (`05-*.md` §6'da belgelenmiş açık) |
| Test sayısı | 161 |

## B) Yapılan Değişiklikler

| # | Değişiklik | Dosya |
|---|---|---|
| 1 | **`SetProcessMitigationPolicy`** ile 4 azaltma politikası | `mitigation.rs` (**yeni**) |
| 2 | **ETW tüketicisi** — Kernel-File/Kernel-Process sağlayıcıları | `etw.rs` (**yeni**) |
| 3 | ETW yazma hızı gözlemcisi + bulgu üretimi | `etw.rs`, `scanner.rs` |
| 4 | **Windows Servisi** kurulum/kaldırma + `SERVICE_CONTROL_STOP` | `service.rs` (**yeni**) |
| 5 | `serve`, SCM'den başlatıldıysa otomatik servis moduna geçiyor | `main.rs` |
| 6 | `static mut` KALDIRILDI → `OnceLock`/atomik (Rust 2024 UB riski) | `etw.rs`, `service.rs` |
| 7 | ETW durum raporu **dürüstleştirildi** (aşağıda §G.2) | `etw.rs` |

### B.1 ETW: bu bir kernel driver DEĞİLDİR

Hiçbir `.sys` yazılmamıştır, hiçbir kernel geri çağrısı kurulmamıştır ve
öyle bir iddia **yoktur**. Windows çekirdeği dosya ve süreç olaylarını
**zaten** ETW üzerinden yayınlar; bu modül o yayının resmî kullanıcı-modu
tüketicisidir (`StartTraceW` → `EnableTraceEx2` → `OpenTraceW` →
`ProcessTrace`). `logman`, `xperf` ve pek çok ticari EDR'ın kullanıcı-modu
bileşeni de aynı API'yi kullanır.

Telemetri **kernel kaynaklıdır**, onu toplayan kod Ring-3'te sıradan bir
süreçtir — `firewall.rs`'in Windows'un kendi kernel filtreleme motorunu
(WFP) kullanıcı-modu COM arayüzünden yönetmesiyle **aynı desen**.

### B.2 Bilinçli olarak yapılmayan: ETW yük (payload) çözümü

Bir ETW olayının gövdesini (dosya adı, komut satırı) çözmek
`TdhGetEventInformation`/`TdhFormatProperty` ile ayrı ve büyük bir
şema-çözme katmanı gerektirir. Bu turda **uygulanmadı** ve uygulandığı
iddia **edilmez**. Bunun yerine her olayın **başlığından**
(`EVENT_HEADER`, şema gerektirmez) okunan alanlar kullanılır: sağlayıcı
GUID'i, olay kimliği, **süreç kimliği**, zaman damgası.

Bu, "hangi dosya" sorusunu yanıtlamaz ama **"hangi PID, ne hızda, ne tür
kernel olayı üretiyor"** sorusunu yanıtlar.

### B.3 ETW yüksek hız gözlemi neden devre kesiciyi TETİKLEMEZ

Yüksek dosya yazma hızı fidye yazılımına **özgü değildir**: bir derleyici,
veritabanı, video dönüştürücü veya yedekleme aracı da aynı hızı üretir.
Bu yüzden bu sayaç yalnızca bir **bulgu** üretir (`remediation: None`,
insan incelemesine düşer) ve bulgu metninin kendisi bu uyarıyı taşır.
Otomatik askıya alma, `heuristic.rs`'in **entropi + hız** birleşimine ya
da tuzağa dokunmaya bağlı kalır. Bu, bilinçli bir yanlış-pozitif
tercihidir ve testlidir.

### B.4 Azaltma politikaları ve riskleri

| Politika | Neyi engeller | Varsayılan |
|---|---|---|
| `ExtensionPointDisable` | Eski DLL enjeksiyonu (AppInit_DLLs, global hook'lar) | **Açık** (düşük risk) |
| `StrictHandleCheck` | Tanıtıcı karıştırma saldırılarını gürültülü yapar | **Açık** (düşük risk) |
| `DynamicCode` | Çalışma zamanında çalıştırılabilir bellek ayırma (kabuk kodu) | **Açık** (orta risk) |
| `SignatureMicrosoftSignedOnly` | İmzasız DLL yüklenmesi | **KAPALI** — `CHIMERA_MITIGATION_SIGNED_ONLY=1` ile açılır |

**Bu politikalar GERİ ALINAMAZ** (Windows bunu bilerek böyle
tasarlamıştır — kapatılabilseydi saldırgan kapatırdı). Bu yüzden en
riskli olan varsayılan olarak kapalıdır.

**Hiçbiri `taskkill /F`'e karşı koruma DEĞİLDİR.** Bunlar süreci
öldürülemez yapmaz; adres alanına **kod sokulmasını** zorlaştırır. Farklı
şeylerdir. Projenin kurucu kuralı değişmedi.

### B.5 Windows Servisi — `05-*.md` §6'daki açığın kapatılması

Turn 6 dürüstçe şunu yazmıştı: *"bir Windows Servisi olarak kurulsaydı,
`SERVICE_CONTROL_STOP` farklı bir mekanizma gerektirirdi."* Bu tam olarak
kapatıldı: `SERVICE_CONTROL_STOP`/`SHUTDOWN`, mevcut temiz durdurma
mekanizmasının **aynısını** tetikler (`runtime/stop.flag` + kendi
soketimize uyandırma bağlantısı) — mantık **çoğaltılmadı**, yeniden
kullanıldı.

`serve` artık önce `StartServiceCtrlDispatcherW` dener; bu yalnızca süreç
**gerçekten** SCM tarafından başlatıldıysa başarılı olur, aksi halde
normal konsol yoluna düşülür. Yani tek bir ikili her iki modda da çalışır
ve mod tahmin edilmez.

### B.6 `static mut` kaldırıldı

İlk uygulama, C ABI geri çağrılarından erişilen durum için `static mut`
kullanıyordu. Rust 2024 bunu `static_mut_refs` uyarısıyla işaretler ve
**tanımsız davranış riski** taşır. Bir güvenlik ürününde bunu kabul etmek
doğru olmazdı; `OnceLock` ("bir kez yaz, çok kez oku") ve `AtomicIsize`
ile değiştirildi. Derleme her iki hedefte de **0 uyarı**.

## C) Gerçekten Kazanılan Direnç

| Senaryo | Faz 3 | Faz 4 |
|---|---|---|
| Saldırgan CHIMERA'ya DLL enjekte etmeye çalışıyor | Engel yok | 3 azaltma politikası (uzantı noktası, dinamik kod, tanıtıcı) |
| Oturum kapatma / makine yeniden başlatma | CHIMERA durur | Servis olarak **otomatik başlar**, oturumdan bağımsız çalışır |
| `net stop chimera-core` | Görülmezdi | `SERVICE_CONTROL_STOP` → temiz durdurma |
| Toplu dosya yazma dalgası | Yalnızca tuzak dizininde görülür | ETW ile **makine genelinde**, PID bazında (telemetri akıyorsa) |

## D) Kalan Zayıflıklar (Dürüst)

- **ETW yönetici hakkı gerektirir.** `StartTraceW`, ayrıcalıksız bir
  hesapta başarısız olur. Bu durumda `etw.status` denetim kaydına
  "KURULAMADI" yazılır ve gerçek zamanlı telemetri **yoktur**.
- **ETW yük çözümü yok** (bkz. §B.2) — "hangi dosya şifrelendi"
  sorusunu ETW üzerinden yanıtlayamayız.
- **Azaltma politikaları geri alınamaz** (bkz. §B.4); yanlış bir seçim
  ancak süreç yeniden başlatılarak düzeltilebilir.
- **`DynamicCode` politikası**, adres alanına DLL enjekte eden **meşru**
  yazılımları (bazı EDR/APM ajanları, erişilebilirlik araçları) kırabilir.
  Rust JIT kullanmadığı için CHIMERA'nın kendi kodu etkilenmez.
- **Servis kurulumu yönetici hakkı gerektirir** ve `sc delete` ile
  silinebilir. Servis olmak öldürülemezlik değildir.
- **Servis hesabı LocalSystem'dir.** Bu, ETW ve olay günlüğü erişimi için
  gereklidir ama aynı zamanda **en yüksek ayrıcalıktır**; ele geçirilmiş
  bir CHIMERA süreci makinenin tamamına erişir. Üretimde daha dar bir
  hizmet hesabı + gerekli ayrıcalıkların tek tek verilmesi
  değerlendirilmelidir. Bu sürümde yapılmadı.
- **`is_protected_pid` hâlâ yalnızca 4 PID'i korur** (Faz 1'den kalan
  bilinçli eksik).

## E) Performans / Maliyet

- **ETW geri çağrısı sıcak yoldur:** saniyede on binlerce kez
  çağrılabilir. Bu yüzden içinde **tahsis, biçimlendirme veya disk
  erişimi YOKTUR** — yalnızca bir sayaç artırımı, ve kilit `try_lock`
  ile alınır. Kilit alınamazsa olay **düşürülür**: sıcak yolda beklemek
  ETW tamponlarını taşırır ve daha fazla olay kaybettirirdi.
- **Sayaç haritası sınırsız büyümez:** `evict_stale`, penceresinin 4
  katından eski PID kayıtlarını temizler (testlidir).
- **Hız gözlemcisi 30 saniyede bir uyanır** — boru hattının 30 dakikalık
  turuna bırakılsaydı gözlemlerin neredeyse tamamı kaçırılırdı.
- **Azaltma politikaları:** süreç başına bir kez, dört sistem çağrısı.
  Çalışma zamanı maliyeti yok.

## F) Yanlış-Pozitif / Kararlılık Riskleri

- **ETW yüksek hız bulgusu, en yüksek yanlış-pozitif oranına sahip
  sinyaldir** — bu yüzden otomatik aksiyon taşımaz (bkz. §B.3). Eşik
  (30 sn'de 2000 yazma) ortama göre ayarlanmalıdır.
- **`DynamicCode` politikası** bir üçüncü taraf ajanı kırarsa, bu
  **süreç başlangıcında** ortaya çıkar ve geri alınamaz — dağıtımdan
  önce hedef ortamda denenmelidir.
- **Servis modu**, konsol modundan farklı bir çalışma dizini ve
  ayrıcalık bağlamı kullanır; `--root` mutlak bir yol olmalıdır
  (kurulum bunu zaten mutlak yazar).

## G) Canlı Doğrulama — GERÇEK Sonuçlar

### G.1 Doğrulanan (kanıtlı)

- **`cargo test --workspace` (native Linux): 179/179** (Faz 3 sonu: 161 —
  **18 yeni test**: `etw` 11, `mitigation` 4, `service` 2, `scanner` 1).
- **Çapraz derleme temiz, HER İKİ hedefte de 0 uyarı** — yeni ETW,
  Services ve SystemServices FFI dahil, ve `static mut` kaldırıldıktan
  sonra `static_mut_refs` uyarısı da yok.
- **`SetProcessMitigationPolicy` CANLI çalıştı:** Wine altında
  `serve` başlangıcında 3/4 politika `Ok` döndürdü ve dördüncüsü
  ("istenmedi, varsayılan kapalı") doğru şekilde raporlandı.
- **Windows Servisi kurulumu/kaldırması CANLI çalıştı:** `CreateServiceW`
  ve `DeleteService` Wine'ın SCM'inde başarılı oldu; komut satırı,
  başlangıç tipi (OTOMATİK) ve hesap (LocalSystem) doğru yazıldı.
- **`release-check.py` sertleştirme geçidi geçti** (3 exe: ASLR/DEP/
  stripped/yol temizliği).
- **Temiz durdurma bozulmadı:** yeni ETW ve hız gözlemcisi thread'leri
  `pipeline_running` bayrağını paylaşıyor; SIGINT'te süreç temiz çıktı,
  0 zombie, denetim zinciri SAĞLAM.

### G.2 Bu turda CANLI ÖLÇÜMLE YAKALANAN ve DÜZELTİLEN bir AŞIRI İDDİA

Bu, Faz 4'ün en önemli bulgusudur ve projenin dürüstlük kuralının
gerçekten işlediğini gösterir.

İlk uygulama, `StartTraceW` ve `EnableTraceEx2` başarılı olur olmaz
`EtwStatus::Running` dönüyordu ve `serve` konsola şunu yazıyordu:

```
ETW: 'CHIMERA-ETW' oturumu calisiyor (kernel dosya/surec telemetrisi)
```

**Bu YANLIŞTI.** Yalnızca ETW API'sini kullanan minimal bir sonda ile
ölçüldü:

```
StartTraceW -> 0
EnableTraceEx2(Kernel-File) -> 0
EnableTraceEx2(Kernel-Process) -> 0
OpenTraceW -> GECERSIZ
--- 200 dosya YAZILIYOR (ETW olay uretmeli) ---
TOPLAM ETW olayi: 0
SONUC: olay akisi YOK (oturum kuruldu ama olay TESLIM EDILMIYOR)
```

Yani Wine'da oturum **kuruluyor** ama `OpenTraceW` geçersiz tanıtıcı
döndürüyor ve **tek bir olay bile teslim edilmiyor**. Operatöre "gerçek
zamanlı kernel telemetrim var" demek, bir güvenlik ürününde
verilebilecek en kötü yanlış bilgidir.

**Düzeltme:** `spawn()` artık tüketici thread'inin `OpenTraceW` sonucunu
bir kanal üzerinden geri bildirmesini **bekler** (5 sn zaman aşımı) ve
başarısızsa yarım kalmış oturumu durdurup `Failed` döner. Aynı ortamda
şimdi şunu yazıyor:

```
ETW: oturum KURULAMADI (OpenTraceW GECERSIZ tanitici dondu -- oturum
kuruldu ama olay TESLIM EDILEMIYOR) -- gercek zamanli kernel telemetrisi YOK
```

### G.3 Bu turda yakalanan İKİNCİ gerçek hata (platform farkı)

Faz 3'ün yedek testleri Linux'ta geçiyor ama **Wine altında başarısız
oluyordu**. Sebep gerçek bir işletim sistemi farkıdır: **Windows'ta
`remove_dir_all`, salt-okunur bir dosyada BAŞARISIZ olur**; Linux'ta
yazılabilir bir dizindeki salt-okunur dosya silinebilir. `backup.rs`
yedek dosyalarını bilerek salt-okunur işaretlediği için, testlerin
temizliği Windows'ta sessizce başarısız oluyor, eski (başka bir anahtarla
imzalanmış) anlık görüntü kalıyor ve bir sonraki çalıştırma
`IMZA GECERSIZ` alıyordu. Düzeltme: `clear_read_only_recursive`
`pub(crate)` yapıldı ve testler silmeden önce onu çağırıyor — tek doğru
kaynak, her testte yeniden uygulanmıyor.

**Bu hata yalnızca Windows ikililerini Wine altında GERÇEKTEN çalıştırdığımız
için bulundu.** Yalnızca Linux'ta test edilseydi sessizce kalırdı.

### G.4 DOĞRULANAMAYAN — Wine sınırları (dürüstçe)

- **ETW olay akışı doğrulanamadı** (bkz. §G.2). `StartTraceW`/
  `EnableTraceEx2` çağrı dizisi ve `EVENT_TRACE_PROPERTIES` ikili düzeni
  gerçek Windows bağlarına karşı derlendi ve Wine'da hata vermeden
  çalıştı; ama "gerçek bir Windows'ta kernel olayları GERÇEKTEN akıyor
  mu" sorusunun cevabı yalnızca **gerçek bir Windows makinesinde**
  alınabilir. Sayaç mantığı 11 birim testiyle doğrulandı.
- **Azaltma politikalarının GERÇEKTEN uygulandığı doğrulanamadı.**
  Wine `SetProcessMitigationPolicy` çağrılarına `Ok` döndü, ama Wine'ın
  bunları gerçekten zorunlu kılıp kılmadığı (yani bir DLL enjeksiyonunun
  gerçekten engellenip engellenmediği) test edilemedi. Çağrı dizisi ve
  yapı boyutu doğrulandı.
- **Servis YAŞAM DÖNGÜSÜ doğrulanamadı.** Kurulum/kaldırma çalıştı, ama
  `sc start chimera-core` → `service_main` → `SERVICE_CONTROL_STOP`
  zinciri Wine'ın kısmi SCM'inde denenemedi. Bu, `05-*.md` §6'da
  belgelenen açığın **kodda** kapatıldığı ama **canlı olarak** yalnızca
  gerçek Windows'ta kanıtlanabileceği anlamına gelir.
- **`chimera-ipc` handshake testinin durumu KESİNLEŞTİRİLDİ.**
  Faz 1/2'de "Wine'da başarısız", Faz 3'te "geçti" diye yazmıştık; her
  ikisi de tek gözleme dayanıyordu. Ölçüldü:
  - **Tek başına çalıştırıldığında: 8/8 geçti** (Wine).
  - **Tam `chimera-ipc` paketi içinde: ~3 koşumda 1 başarısız**, hem
    `--test-threads=1` hem paralel modda.
  - **Native Linux: 5/5 geçti.**
  Yani bu, aynı süreçte önceki testlerden etkilenen, **Wine'a özgü,
  aralıklı bir soket durumu duyarlılığıdır** — protokolde bir hata
  değildir. Bu test Turn 5/6 kodudur ve Turn 7'de **değiştirilmemiştir**.

## H) Test Özeti

| Modül | Yeni test | Neyi kanıtlıyor |
|---|---|---|
| `etw.rs` | 11 | Sağlayıcı GUID'lerinin doğruluğu, eşik altı sessizlik, pencere dönüşünde sayacın sıfırlanması, PID bağımsızlığı, bellek büyümesinin sınırlanması, bayat gözlemin "şu an" sayılmaması, durum metninin başarısızlığı gizlememesi |
| `mitigation.rs` | 4 | Desteklenmeyen platformun dürüst raporlanması, uygulanan/uygulanamayan ayrımı, imzasız-DLL yasağının **opt-in** olması |
| `service.rs` | 2 | Servis kimliğinin kararlılığı, platform dışı açık ret |
| `scanner.rs` | 1 | ETW hız bulgusunun rapor edilmesi ama **otomatik aksiyon taşımaması** |
| **Toplam** | **18** | 161 → **179** |

---

*Faz 1: `06-TURN7-AKTIF-SAVUNMA.md`, Faz 2: `07-TURN7-AG-SERTLESTIRME.md`,
Faz 3: `08-TURN7-KURTARMA-DAYANIKLILIGI.md`.*
