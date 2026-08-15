# Mafia VIP Protection — GTA V (ScriptHookVDotNet v3)

Mafya tarzi, yuksek profilli bir suc orgutunun **VIP koruma birimi**. Uc katman:

1. **Yakin Koruma (Close Protection Team)** — oyuncunun etrafinda dogal formasyon, akilli tepki
2. **Konvoy / Motorcade** — lead + destek araclari, gercekci eskort AI, duruşta 360° savunma
3. **Hava Savunma ve Destek** — Buzzard / Savage / Valkyrie / Annihilator + Cargobob tahliye & birlik indirme

Ek olarak: kara yoluyla **takviye (backup)** sistemi, tam `.ini` yapilandirmasi, LemonUI menusu, blip'ler.

---

## 1. Kurulum Rehberi

### 1.1 Gereksinimler

| Bilesen | Not |
|---|---|
| GTA V (Legacy / Enhanced fark etmez, SHVDN destekliyorsa) | Single-player |
| [ScriptHookV](http://www.dev-c.com/gtav/scripthookv/) | `ScriptHookV.dll` + `dinput8.dll` |
| [ScriptHookVDotNet v3](https://github.com/scripthookvdotnet/scripthookvdotnet/releases) | `ScriptHookVDotNet.asi`, `ScriptHookVDotNet3.dll` |
| [LemonUI (SHVDN3)](https://github.com/LemonUIbyLemon/LemonUI/releases) | `LemonUI.SHVDN3.dll` |
| .NET Framework 4.8 | Windows'ta zaten kurulu |

### 1.2 Derleme

```
MafiaVIPProtection/
├─ MafiaVIPProtection.csproj
├─ MafiaVIPProtection.ini
├─ libs/                     <-- ScriptHookVDotNet3.dll + LemonUI.SHVDN3.dll buraya
└─ src/*.cs
```

1. `libs/` klasorune `ScriptHookVDotNet3.dll` ve `LemonUI.SHVDN3.dll` dosyalarini kopyalayin.
   (Bos birakirsaniz proje ayni paketleri NuGet'ten cekmeye calisir.)
2. Derleyin:
   - **Visual Studio:** `MafiaVIPProtection.csproj` -> Release / x64 -> Build
   - **Komut satiri:** `dotnet build -c Release`
     (veya `msbuild MafiaVIPProtection.csproj /p:Configuration=Release`)
3. Cikti: `bin\Release\MafiaVIPProtection.dll`

### 1.3 Oyuna kurulum

GTA V ana klasorunuzdeki `scripts\` klasorune su iki dosyayi atin:

```
GTA V\
├─ ScriptHookV.dll
├─ dinput8.dll
├─ ScriptHookVDotNet.asi
├─ ScriptHookVDotNet3.dll
└─ scripts\
   ├─ LemonUI.SHVDN3.dll            <-- LemonUI (zorunlu)
   ├─ MafiaVIPProtection.dll        <-- derlenen mod
   └─ MafiaVIPProtection.ini        <-- ayarlar (yoksa ilk calistirmada otomatik olusur)
```

> `.ini` dosyasini kopyalamayi unutursaniz sorun degil: mod ilk calistigi anda
> varsayilan degerlerle `scripts\MafiaVIPProtection.ini` dosyasini kendisi olusturur.

Hata ayiklama gunlugu: `scripts\MafiaVIPProtection.log` (her oyun oturumunda sifirlanir).

---

## 2. Kullanim — Tuslar ve Menu

### 2.1 Varsayilan tuslar

| Tus | Islev |
|---|---|
| **F7** | Ana menuyu ac / kapa |
| **F8** | Yakin koruma ekibini cagir / dagit |
| **F9** | Konvoyu olustur / dagit |
| **F10** | Hava destegini cagir / geri gonder |
| **F11** | Kara yoluyla takviye birlik cagir |
| (ayarlanabilir) | Formasyon degistir — `FormationCycleKey` |
| (ayarlanabilir) | Tum ekipleri temizle — `ClearAllKey` |

Tuslar `.ini` icinden degistirilebilir. Baska modlarla cakisma yasarsaniz
`[Keys] ModifierKey = LControlKey` yaparak hizli tuslari `Ctrl + F8` gibi
kombinasyona cevirebilirsiniz (menu tusu modifier istemez).

### 2.2 Menu yapisi

```
MAFIA VIP
├─ Yakin Koruma
│  ├─ Ekibi Cagir / Ekibi Dagit
│  ├─ Koruma Sayisi        (1 .. MaxGuards)
│  ├─ Silah                (Rastgele + .ini'deki silah listesi)
│  ├─ Davranis             (Agresif / Defansif / Pasif)
│  ├─ Formasyon            (Elmas / Kutu / Kolon / Kama / Cember)
│  └─ Otomatik Yedek       (olen korumanin yerine yenisi gelsin mi)
├─ Konvoy
│  ├─ Konvoy Olustur / Konvoyu Dagit
│  ├─ Hiz Modu             (Normal / Agresif / Sessiz)
│  ├─ Dizilim              (Elmas / Kutu / Kolon / Kama / Cember)
│  ├─ VIP Araci            (secip ENTER'a basinca yeni VIP araci spawn olur)
│  ├─ Ekibi Indir (360 Savunma)
│  └─ Ekibi Topla
├─ Hava Destegi
│  ├─ Hava Destegi Cagir / Geri Gonder
│  ├─ Hava Araci           (buzzard / savage / valkyrie / annihilator ...)
│  ├─ Yukseklik (m)        (20 - 200)
│  ├─ Mesafe (m)           (30 - 200, devriye yaricapi)
│  ├─ Otomatik Engaje
│  ├─ Cargobob - Tahliye
│  └─ Cargobob - Birlik Indir
└─ Genel
   ├─ Yedek Birlik Cagir (Kara)
   ├─ Tum Ekipleri Temizle
   ├─ Blipler / Bildirimler / Polisi Tehdit Say
   ├─ Ayarlari Yeniden Yukle
   └─ Yardim (tus atamalari)
```

### 2.3 Davranis mantigi

| Mod | Ne yapar |
|---|---|
| **Agresif** | Menzildeki silahli/ates eden herkesi proaktif engaje eder |
| **Defansif** (varsayilan) | Yalnizca oyuncuya veya ekibe saldiran hedeflere karsilik verir |
| **Pasif** | Ates etmez, sadece formasyonda kalir |

Korumalar **oyuncu ile ayni dostluk grubundadir**; sivillere ve polise **notr**
davranirlar, kendiliginden saldirmazlar. Tehdit motoru bir hedefi ancak dusmanca
bir eylemden sonra (ates etme, hasar verme, catismaya girme) isaretler ve hedefi
20 saniye "aggro" hafizasinda tutar — boylece dusman siper alinca takip birakilmaz.
Polisle catismak isterseniz `[General] EngagePolice = true` yapin (aranma varken devreye girer).

### 2.4 Blip renkleri

| Renk | Birim |
|---|---|
| Yesil | Korumalar |
| Mavi | VIP araci ve konvoy araclari |
| Sari | Hava destegi / Cargobob |

Renk ve ikonlar `.ini` icinde sayisal olarak degistirilebilir (`BlipColor`, `BlipSprite`).

---

## 3. Ayar Dosyasi (`MafiaVIPProtection.ini`)

Tum bolumler ve anahtarlar dosyanin icinde aciklamalidir. Ozet:

| Bolum | Ne ayarlanir |
|---|---|
| `[Keys]` | Tum tus atamalari + opsiyonel modifier |
| `[General]` | Blip/bildirim, tehdit yaricapi, polis davranisi, ceset temizligi, oyuncu olunce ekip davranisi, iliski grubu adi |
| `[CloseProtection]` | Koruma sayisi/limiti, ped modelleri, silahlar, can/zirh/isabet, savas parametreleri, otomatik yedek, formasyon, kiyafet setleri, blip |
| `[Convoy]` | VIP araci, lead/destek arac modelleri ve sayilari, arac basina ekip, hiz ve surus stilleri, durusta inme, zirhlandirma, renk/cam filmi, blip |
| `[AirSupport]` | Hava araci listesi, pilot modeli, nisanci sayisi, yukseklik/yaricap/hiz, otomatik engaje, cargobob modeli, blip |
| `[Backup]` | Takviye araci, ekip buyuklugu, spawn mesafesi, bekleme suresi |

Ornek — daha sert bir ekip:

```ini
[CloseProtection]
GuardCount = 8
Health = 800
Armor = 100
Accuracy = 95
CombatAbility = 2
Weapons = SpecialCarbine, CombatMG
DefaultBehavior = Aggressive
```

Ornek — add-on ped/arac kullanimi:

```ini
[CloseProtection]
GuardModels = my_addon_guard_01, my_addon_guard_02

[Convoy]
VipVehicleModel = my_addon_limo
```

Gecersiz bir model adi girilirse mod cokmez: modeli atlar, `.log` dosyasina
`Gecersiz model: ...` satirini yazar ve listede calisan bir model varsa onu kullanir.

Ornek — kiyafet setleri (`component:drawable:texture`, parcalar `|` ile, setler `,` ile):

```ini
OutfitSets = 3:1:0|4:1:0|8:1:0|11:1:0, 3:2:0|4:2:0|8:2:0|11:2:0
```

`.ini` degistirdikten sonra **Genel -> Ayarlari Yeniden Yukle** ile oyunu kapatmadan
uygulayabilirsiniz (tum aktif birimler temizlenir, ayarlar bastan okunur).

---

## 4. Teknik Notlar

### Mimari

| Dosya | Sorumluluk |
|---|---|
| `Main.cs` | `Script` girisi, tick zamanlayici, tus yonetimi, yasam dongusu |
| `Config.cs` | Tum ayarlar + varsayilan `.ini` uretimi + deger sinirlama |
| `IniFile.cs` | Bagimsiz, hafif `.ini` okuyucu (tus/enum/liste/ondalik destegi) |
| `Natives.cs` | Native cagrilari — **ham hash** ile, SHVDN surum farklarindan bagimsiz |
| `Utils.cs` | Model yukleme, blip, silme, konum ve geometri yardimcilari |
| `Logger.cs` | Dosya log'u |
| `Relationships.cs` | Iliski gruplari (oyuncu ile dost, sivil/polise notr) |
| `GuardFactory.cs` | Ped ve arac uretimi + "profesyonel koruma" yapilandirmasi |
| `ThreatScanner.cs` | Tehdit tespiti ve aggro hafizasi |
| `CloseProtection.cs` | Yakin koruma ekibi, formasyon, arac binis/inis, yedekleme |
| `ConvoyManager.cs` | Konvoy, eskort rolleri, hiz modlari, 360° mevzilenme |
| `AirSupport.cs` | Helikopter eskortu/saldirisi, Cargobob tahliye & birlik indirme |
| `BackupManager.cs` | Kara yoluyla takviye birlik |
| `MenuSystem.cs` | LemonUI menuleri |

### Performans

Tick her karede calisir (menu icin gerekli), ancak agir isler zamanlayiciya bolunmustur:

| Is | Aralik |
|---|---|
| Tehdit taramasi | 750 ms |
| Yakin koruma | 400 ms |
| Konvoy | 900 ms |
| Hava destegi | 1200 ms |
| Takviye | 1000 ms |

Gorevler (task) yalnizca durum degistiginde veya "bayatladiginda" yeniden verilir;
her tick'te task spam'i yapilmaz. Olen/imha olan varliklar listelerden temizlenir,
cesetler ayarlanan sureden sonra silinir, script kapatilirken (`Aborted`) her sey temizlenir.

### Saglamlik

- Tum public giris noktalari `try/catch` ile korunur; hata `.log` dosyasina yazilir, oyun akisi bozulmaz.
- Her entity kullanimindan once `Exists()` / `IsAlive` kontrolu yapilir (`Utils.Valid`, `Utils.AlivePed`).
- `.ini` degerleri `Clamp()` ile guvenli araliklara cekilir.
- Native'ler ham hash ile cagrildigi icin SHVDN v3'un farkli surumlerinde enum ismi degisse bile kod derlenir/calisir.

---

## 5. Bilinen Sinirlamalar

1. **Konvoy AI oyunun surus motoruna baglidir.** `TASK_VEHICLE_ESCORT` dar sokaklarda
   veya yogun trafikte zaman zaman takilabilir; "Sessiz" modda daha duzgun, "Agresif"
   modda daha kaotik surerler. Cok dar alanlarda `LeadVehicleCount = 0` daha temiz sonuc verir.
2. **Helikopter gorev kodlari** (`TASK_HELI_MISSION`) oyunun ic mantigina gore calisir;
   silahsiz bir helikopter (orn. `valkyrie` yolcu koltuklari bos) hedefe roket atmaz,
   engaje isini nisancilar yapar. Silahli hava araci icin `savage` / `annihilator` / `hunter` tercih edin.
3. **Cargobob tahliyesi** oyuncunun manuel binmesini bekler; 60 saniye icinde binmezseniz ayrilir.
4. **Add-on araclarda koltuk sayisi** farkli olabilir; ekip `GetVehicleMaxNumberOfPassengers`
   ile sinirlandirilir, bu yuzden `GuardsPerVehicle` degerinden az koruma binebilir.
5. **Oyuncunun aracina binis**: arac hareket halindeyken veya koruma 25 m'den uzaktayken
   takilma yasanmamasi icin koltuga isinlanarak bindirilir (bilincli tercih).
6. **Cok fazla birim** (8 koruma + 4 arac + helikopter) dusuk sistemlerde FPS dusurebilir;
   ped/arac limitine takilirsaniz spawn basarisiz olur ve `.log` dosyasina yazilir.
7. Mod yalnizca **single-player** icindir. FiveM / GTA Online ile kullanilamaz ve kullanilmamalidir.

## 6. Gelistirme Onerileri (yol haritasi)

- Formasyonlarin dar alanlarda / kapi gecislerinde daralmasi (dinamik offset sikistirma)
- Konvoy icin rota bazli sahne: hedef nokta secip konvoyu oraya yonlendirme
- Suikast senaryolari: rastgele pusu olaylari, VIP tehdit seviyesi
- Korumalarin sesli tepkileri (`PLAY_PED_AMBIENT_SPEECH_NATIVE` ile "Contact!", "Move!")
- Kaydedilebilir profil sistemi (birden fazla ekip sablonu)
- Menu icinde canli ekip listesi (her korumanin can/zirh durumu)
