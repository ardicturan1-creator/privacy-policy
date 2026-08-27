pluginManagement {
    includeBuild("build-logic")
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "UltraGuard"

enableFeaturePreview("TYPESAFE_PROJECT_ACCESSORS")

include(":app")

// --- Cekirdek katmanlar -------------------------------------------------
include(":core:model")        // saf domain tipleri (JVM)
include(":core:common")       // dispatcher'lar, saat, loglama, sonuc tipleri
include(":core:designsystem") // Compose tema, token'lar, ortak bilesenler
include(":core:database")     // Room + SQLCipher kalici depo
include(":core:datastore")    // ayarlar / mod tercihleri
include(":core:security")     // keystore, butunluk, root tespiti, kendini koruma
include(":core:engine")       // olay veriyolu, korelasyon grafigi, durum makinesi
include(":core:policy")       // mod motoru, yaptirim karari, Action Ledger
include(":core:ai")           // L1 kural motoru + L2 cikarim + risk fuzyonu
include(":core:sensors")      // telemetri toplayicilar
include(":core:network")      // VpnService ZTNA + sifreli trafik analizi

// --- Ozellik modulleri --------------------------------------------------
include(":feature:dashboard")
include(":feature:timeline")
include(":feature:appdetail")
include(":feature:assistant")
include(":feature:settings")

// --- Opsiyonel derin izleme (root/KernelSU) -----------------------------
// Ayri APK olarak dagitilir; ana uygulama bu modul olmadan tam islevseldir.
include(":module:deepscan")
