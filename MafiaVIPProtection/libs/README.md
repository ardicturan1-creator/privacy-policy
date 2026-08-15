# libs klasoru

Derleme icin gerekli referans DLL'lerini buraya kopyalayin:

| Dosya | Nereden |
|---|---|
| `ScriptHookVDotNet3.dll` | ScriptHookVDotNet surumunun icinden (GTA V ana klasorunde de bulunur) |
| `LemonUI.SHVDN3.dll` | LemonUI release paketinden (`LemonUI.SHVDN3.dll`) |

Bu iki dosya buradaysa proje internet olmadan derlenir.
Klasor bossa `MafiaVIPProtection.csproj` ayni paketleri NuGet'ten cekmeye calisir.

> Not: Bu DLL'ler baska projelerin telifli dosyalari oldugu icin depoya dahil edilmemistir.
