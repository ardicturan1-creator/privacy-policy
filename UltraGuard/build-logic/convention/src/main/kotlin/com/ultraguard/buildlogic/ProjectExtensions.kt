package com.ultraguard.buildlogic

import com.android.build.api.dsl.CommonExtension
import org.gradle.api.JavaVersion
import org.gradle.api.Project
import org.gradle.api.artifacts.VersionCatalog
import org.gradle.api.artifacts.VersionCatalogsExtension
import org.gradle.kotlin.dsl.dependencies
import org.gradle.kotlin.dsl.getByType
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.dsl.KotlinAndroidProjectExtension

internal object UltraGuardConfig {
    const val COMPILE_SDK = 35

    /**
     * Android 10 (API 29) tabani bilincli bir karardir:
     *  - Scoped storage zorunlu, boylece dosya sistemi telemetrisi tutarli.
     *  - `AppOpsManager.startWatchingMode` sensor op'lari icin guvenilir.
     *  - `PackageInstaller.SessionCallback` yan yukleme kaynagini raporlar.
     * API 29 altinda bu sinyallerin yarisi yok; eksik telemetriyle calisan
     * bir guvenlik urunu, korumadigi seyi korudugunu iddia eder.
     */
    const val MIN_SDK = 29
    const val TARGET_SDK = 35

    val JAVA_VERSION = JavaVersion.VERSION_17
    val JVM_TARGET = JvmTarget.JVM_17
}

internal val Project.libs: VersionCatalog
    get() = extensions.getByType<VersionCatalogsExtension>().named("libs")

internal fun Project.configureKotlinAndroid(
    commonExtension: CommonExtension<*, *, *, *, *, *>,
) {
    commonExtension.apply {
        compileSdk = UltraGuardConfig.COMPILE_SDK

        defaultConfig {
            minSdk = UltraGuardConfig.MIN_SDK
        }

        compileOptions {
            sourceCompatibility = UltraGuardConfig.JAVA_VERSION
            targetCompatibility = UltraGuardConfig.JAVA_VERSION
            isCoreLibraryDesugaringEnabled = true
        }

        packaging {
            resources.excludes += setOf(
                "/META-INF/{AL2.0,LGPL2.1}",
                "/META-INF/DEPENDENCIES",
            )
        }
    }

    extensions.configure(KotlinAndroidProjectExtension::class.java) {
        compilerOptions {
            jvmTarget.set(UltraGuardConfig.JVM_TARGET)
            freeCompilerArgs.addAll(
                "-opt-in=kotlin.RequiresOptIn",
                "-opt-in=kotlinx.coroutines.ExperimentalCoroutinesApi",
                "-Xconsistent-data-class-copy-visibility",
            )
        }
    }

    dependencies {
        add("coreLibraryDesugaring", libs.findLibrary("desugar-jdk-libs").get())
    }
}
