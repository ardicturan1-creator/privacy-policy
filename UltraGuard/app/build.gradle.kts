plugins {
    id("ultraguard.android.application")
    id("ultraguard.android.compose")
    id("ultraguard.android.hilt")
}

android {
    namespace = "com.ultraguard.shield"

    defaultConfig {
        applicationId = "com.ultraguard.shield"
        versionCode = 1
        versionName = "0.1.0-mvp"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    /**
     * Imzalama yapilandirmasi Gradle ozelliklerinden okunur; anahtar veya
     * parola hicbir zaman depoya girmez. CI bunlari GitHub Secrets'tan
     * gecici dosya ve ortam degiskeni olarak saglar.
     *
     * Yerel yayin derlemesi icin `~/.gradle/gradle.properties` icine:
     *   ultraguard.storeFile=/mutlak/yol/ultraguard.jks
     *   ultraguard.storePassword=...
     *   ultraguard.keyAlias=...
     *   ultraguard.keyPassword=...
     */
    val releaseStoreFile = providers.gradleProperty("ultraguard.storeFile").orNull

    signingConfigs {
        if (releaseStoreFile != null) {
            create("release") {
                storeFile = file(releaseStoreFile)
                storePassword = providers.gradleProperty("ultraguard.storePassword").orNull
                keyAlias = providers.gradleProperty("ultraguard.keyAlias").orNull
                keyPassword = providers.gradleProperty("ultraguard.keyPassword").orNull
                enableV1Signing = false
                // v2 + v3: Android 7+ tum cihazlarda dogrulanir, v4 ise
                // artimli kurulum icin. v1 (JAR imzasi) kapali birakilir:
                // minSdk 29 oldugu icin gereksizdir ve Janus benzeri
                // imza-atlatma siniflarina yuzey acar.
                enableV2Signing = true
                enableV3Signing = true
            }
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
            isMinifyEnabled = false
            // Debug derlemesinde imza allow-list'i bostur; SelfProtection
            // bunu gorup dogrulamayi atlar.
            buildConfigField("String", "SIGNING_CERT_SHA256", "\"\"")
        }

        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            signingConfig = signingConfigs.findByName("release")

            // Kendini koruma icin beklenen imza ozeti. Bos birakilirsa
            // `verifyReleaseSigningConfigured` gorevi derlemeyi durdurur:
            // imza dogrulamasi olmadan yayinlanan bir guvenlik urunu,
            // yeniden paketlenmeye acik demektir.
            val expectedSignature = providers
                .gradleProperty("ultraguard.signingCertSha256")
                .getOrElse("")
            buildConfigField("String", "SIGNING_CERT_SHA256", "\"$expectedSignature\"")
        }
    }

    buildFeatures {
        buildConfig = true
    }
}

/**
 * Release derlemesinin kendini koruma yapilandirmasi olmadan cikmasini
 * engeller. Bu kontrol bilincli olarak sert basarisizlik uretir.
 */
val verifyReleaseSigningConfigured by tasks.registering {
    doLast {
        val signature = providers
            .gradleProperty("ultraguard.signingCertSha256")
            .getOrElse("")
        check(signature.length == 64) {
            "Release derlemesi icin `ultraguard.signingCertSha256` (64 haneli SHA-256) " +
                "gradle ozelligi tanimlanmalidir. Imza dogrulamasi olmadan yayin yapilamaz."
        }
    }
}

tasks.matching { it.name == "assembleRelease" || it.name == "bundleRelease" }.configureEach {
    dependsOn(verifyReleaseSigningConfigured)
}

dependencies {
    implementation(projects.core.model)
    implementation(projects.core.common)
    implementation(projects.core.designsystem)
    implementation(projects.core.database)
    implementation(projects.core.datastore)
    implementation(projects.core.security)
    implementation(projects.core.engine)
    implementation(projects.core.policy)
    implementation(projects.core.ai)
    implementation(projects.core.sensors)
    implementation(projects.core.network)
    implementation(projects.module.deepscan)

    implementation(projects.feature.dashboard)
    implementation(projects.feature.timeline)
    implementation(projects.feature.appdetail)
    implementation(projects.feature.assistant)
    implementation(projects.feature.settings)

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.hilt.work)
    implementation(libs.hilt.navigation.compose)

    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.androidx.espresso.core)
}
