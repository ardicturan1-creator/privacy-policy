# Nebula Drift 3D — APK Nasıl Derlenir?

Bu klasörde tam bir 3D mobil oyun (Three.js) + onu native Android uygulamasına
saran bir Capacitor projesi (`android/`) hazır durumda. Oyunun kodu, ikonları,
PWA manifest'i ve Android proje iskeleti bu oturumda oluşturuldu ve test edildi.

## Neden APK dosyası burada yok?

Bu proje, ağ erişimi kısıtlı bir bulut sandbox'ında hazırlandı. Android APK
derlemek için Gradle'ın **Google'ın Maven deposundan** (`dl.google.com`) Android
Gradle Plugin ve destek kütüphanelerini indirmesi gerekiyor; bu adres bu
sandbox'ın ağ politikası tarafından engelleniyor (`403 Forbidden`). Ayrıca
ortamda kurulu bir Android SDK da yok. Bu yüzden gerçek, imzalı bir `.apk`
dosyasını burada üretemedim — ama her şeyi senin bilgisayarında **tek komutla**
derleyebileceğin şekilde hazırladım.

## Gereksinimler (kendi bilgisayarında)

1. **Node.js 18+** — https://nodejs.org
2. **JDK 17** (Android Gradle Plugin 8.x için gerekli)
3. **Android SDK** — en kolay yol [Android Studio](https://developer.android.com/studio)
   kurmak; kurulumda "Android SDK" ve "Android SDK Platform-Tools" seçili
   olsun. Studio ilk açılışta gerekli SDK bileşenlerini indirir.
4. İnternet bağlantısı (Gradle ve Android bağımlılıklarını indirmek için).

## En hızlı yol: tek komut

```bash
cd game
./build-apk.sh
```

Bu script sırasıyla:
1. `npm install` — bağımlılıkları kurar
2. `npm run build` — Three.js oyununu üretim paketine derler (`dist/`)
3. `npx cap sync android` — web paketini Android projesine kopyalar
4. `android/gradlew assembleDebug` — imzasız/debug APK'yı üretir

Bitince APK şurada olur:

```
game/android/app/build/outputs/apk/debug/app-debug.apk
```

Bu debug APK'yı telefonuna kopyalayıp doğrudan kurabilirsin (Ayarlar >
Güvenlik > "Bilinmeyen kaynaklara izin ver" gerekebilir).

## Android Studio ile (alternatif / önerilen)

```bash
cd game
npm install
npm run build
npx cap open android
```

Bu, projeyi Android Studio'da açar. Oradan:
- **Run ▶** ile bağlı bir telefonda / emülatörde direkt çalıştırabilirsin,
- **Build → Build Bundle(s) / APK(s) → Build APK(s)** ile APK üretebilirsin,
- **Build → Generate Signed Bundle / APK** ile Play Store'a yüklenebilir,
  imzalı bir **release APK/AAB** oluşturabilirsin (kendi keystore'unu
  oluşturman istenecek).

## Oyunda değişiklik yaptıktan sonra

Kaynak kodu `src/` altında JavaScript/Three.js. Her değişiklikten sonra
Android tarafına yansıtmak için:

```bash
npm run build
npx cap sync android
```

sonra tekrar `./build-apk.sh` çalıştır ya da Android Studio'dan Run'a bas.

## Web'de test etmek (APK'ya hiç gerek kalmadan)

Oyun aynı zamanda bir PWA (Progressive Web App). Sunucuya deploy edip
telefon tarayıcısından "Ana ekrana ekle" diyerek tam ekran, uygulama gibi
çalışan bir sürümünü de kullanabilirsin — Android SDK gerekmez:

```bash
npm run build
npm run preview -- --host
```

## Proje yapısı

```
game/
  src/
    main.js          - oyun döngüsü, giriş noktası
    game/
      scene.js        - Three.js sahne, kamera, yıldız/nebula arkaplanı
      player.js        - oyuncu gemisi, hareket, ateş etme
      world.js         - prosedürel asteroit alanı
      enemies.js        - drone düşmanlar + dalga bossları
      combat.js         - mermi havuzları, çarpışma
      powerups.js        - kalkan/tamir/çoklu atış/hızlı atış/boost eşyaları
      particles.js        - cannon-es fizikli patlama enkazı + kıvılcımlar
      audio.js             - tamamen prosedürel WebAudio ses efektleri/müzik
      state.js              - skor, kombo, dalga, en iyi skor (localStorage)
      ui.js                  - HUD ve menü DOM mantığı
  public/                     - ikonlar, manifest, service worker
  android/                     - Capacitor native Android projesi
  build-apk.sh                  - tek komutla APK derleme scripti
```

## Oynanış

- **Sürükle** (parmak/mouse) — gemiyi sürükle, ekranın her yerinde çalışır
- **Ateş** butonu (veya Space) — asteroitleri ve düşmanları vur
- **Boost** butonu (veya Shift) — geçici hız artışı, boost barını tüketir
- Her 3 dalgada bir **sektör muhafızı** (boss) çıkar
- Güç arttırıcılar: Kalkan, Tamir, Çoklu Atış, Hızlı Atış, Tam Boost
- Skor, dalga sayısı ve en iyi skor `localStorage`'da saklanır
