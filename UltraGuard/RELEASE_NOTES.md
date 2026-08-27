## Kurulum

APK dosyasını indirin ve cihazınızda açın. Play Store dışından kurulum için
**Ayarlar → Güvenlik → Bilinmeyen kaynaklar** iznini vermeniz gerekir.

## Uyumluluk

| | |
|---|---|
| **Android sürümü** | 10 (API 29) ve üzeri |
| **İşlemci mimarisi** | arm64-v8a · armeabi-v7a · x86 · x86_64 |
| **Paket tipi** | Tek universal APK — ABI bölünmesi yok |

Android 10 tabanı bir kısıt değil, gereklilik: paket→UID eşlemesi
(`getConnectionOwnerUid`) ve güvenilir sensör telemetrisi
(`AppOpsManager.startWatchingMode`) bu sürümle geliyor. Bunlar olmadan ağ ve
sensör izleme anonim bir akışa indirgenir.

## Doğrulama

İndirdiğiniz dosyanın bozulmadığından emin olmak için:

```bash
sha256sum UltraGuard-*.apk
```

Çıkan özeti `SHA256SUMS.txt` içindekiyle karşılaştırın.

## İlk çalıştırma

Uygulama izinleri **tek tek ve gerekçeleriyle** ister. Hiçbiri zorunlu
değildir; vermediğiniz her izin yalnızca ilgili koruma katmanını kapatır ve
bu, ana ekranda açıkça gösterilir.

Erişilebilirlik izni ekranında duraksarsanız haklısınız — bankacılık
trojanları da tam olarak bu izni ister. UltraGuard aynı izni, başka bir
uygulamanın bunu yaptığını görmek için kullanır ve ekran içeriğini okuma
yeteneği yapılandırma dosyasında platform düzeyinde kapatılmıştır.

## Bilinen sınırlar

- Cihaz-üstü davranış modeli (L2) bu sürümde yok; kural motoru (L1) tek
  başına çalışıyor.
- Tehdit istihbaratı beslemesi henüz bağlı değil; itibar alanları boş.
- Root gerektiren derin izleme modülü (eBPF/Binder) ayrı bir pakettir ve
  bu sürüme dahil değildir.
