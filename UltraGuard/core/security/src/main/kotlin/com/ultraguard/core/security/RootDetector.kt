package com.ultraguard.core.security

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import com.ultraguard.core.model.Capability
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Cok sinyalli root / degistirme tespiti.
 *
 * Onemli duruş: **root bir tehdit degildir.** Kullanicilarin bir bolumu
 * cihazlarini bilincli olarak root'lar. UltraGuard bunu bir suc olarak
 * isaretlemez; yalnizca (a) tehdit modelini gunceller -- root'lu cihazda
 * uygulama sandbox'i zayiflar -- ve (b) [Capability.ROOTED] gerektiren derin
 * izleme kurallarini acar.
 *
 * Tespit, tek bir kontrole degil sinyal toplamina dayanir. Magisk DenyList
 * gibi gizleme mekanizmalari bireysel kontrolleri atlatir; birbirinden
 * bagimsiz alti sinyalin tamamini tutarli sekilde gizlemek cok daha zordur.
 */
@Singleton
class RootDetector @Inject constructor(
    @ApplicationContext private val context: Context,
) {

    fun detect(): RootAssessment {
        val indicators = buildList {
            if (hasSuBinary()) add(RootIndicator.SU_BINARY)
            if (hasManagementApp()) add(RootIndicator.MANAGEMENT_APP)
            if (hasTestKeysBuild()) add(RootIndicator.TEST_KEYS_BUILD)
            if (hasWritableSystemPartition()) add(RootIndicator.WRITABLE_SYSTEM)
            if (hasDangerousProperties()) add(RootIndicator.DEBUGGABLE_PROPS)
            if (hasZygiskArtifacts()) add(RootIndicator.ZYGISK_ARTIFACT)
        }

        return RootAssessment(
            indicators = indicators,
            // Tek bir zayif sinyal (or. yalnizca test-keys) root demek degildir;
            // iki bagimsiz sinyal esigi yanlis pozitifi belirgin sekilde dusurur.
            isRooted = indicators.size >= ROOT_CONFIDENCE_THRESHOLD,
        )
    }

    private fun hasSuBinary(): Boolean = SU_PATHS.any { path ->
        runCatching { File(path).exists() }.getOrDefault(false)
    }

    private fun hasManagementApp(): Boolean = MANAGEMENT_PACKAGES.any { pkg ->
        runCatching {
            context.packageManager.getPackageInfo(pkg, PackageManager.PackageInfoFlags.of(0))
            true
        }.getOrDefault(false)
    }

    private fun hasTestKeysBuild(): Boolean = Build.TAGS?.contains("test-keys") == true

    /**
     * `/system` normalde salt-okunur monte edilir. Yazilabilir olmasi,
     * bootloader'in acilmis ve dm-verity'nin devre disi birakilmis olmasina
     * isaret eder.
     */
    private fun hasWritableSystemPartition(): Boolean = WRITABLE_PATHS.any { path ->
        runCatching { File(path).canWrite() }.getOrDefault(false)
    }

    private fun hasDangerousProperties(): Boolean = runCatching {
        systemProperty("ro.secure") == "0" || systemProperty("ro.debuggable") == "1"
    }.getOrDefault(false)

    /** Zygisk, Zygote surecine enjekte olur ve karakteristik izler birakir. */
    private fun hasZygiskArtifacts(): Boolean = runCatching {
        File("/proc/self/maps").useLines { lines ->
            lines.any { it.contains("zygisk") || it.contains("magisk") }
        }
    }.getOrDefault(false)

    private fun systemProperty(key: String): String? = runCatching {
        Class.forName("android.os.SystemProperties")
            .getMethod("get", String::class.java)
            .invoke(null, key) as? String
    }.getOrNull()

    private companion object {
        const val ROOT_CONFIDENCE_THRESHOLD = 2

        val SU_PATHS = listOf(
            "/system/bin/su", "/system/xbin/su", "/sbin/su",
            "/system/sd/xbin/su", "/vendor/bin/su", "/data/adb/magisk",
            "/data/adb/ksu", "/data/adb/ap",
        )

        val MANAGEMENT_PACKAGES = listOf(
            "com.topjohnwu.magisk",
            "me.weishu.kernelsu",
            "io.github.huskydg.magisk",
            "eu.chainfire.supersu",
        )

        val WRITABLE_PATHS = listOf("/system", "/system/bin", "/vendor", "/etc")
    }
}

data class RootAssessment(
    val indicators: List<RootIndicator>,
    val isRooted: Boolean,
) {
    /** Root'lu cihaz derin izleme yeteneklerini acar. */
    fun grantedCapabilities(isDeviceOwner: Boolean): Set<Capability> = buildSet {
        add(Capability.UNROOTED)
        if (isRooted) add(Capability.ROOTED)
        if (isDeviceOwner) add(Capability.ENTERPRISE)
    }
}

enum class RootIndicator {
    SU_BINARY,
    MANAGEMENT_APP,
    TEST_KEYS_BUILD,
    WRITABLE_SYSTEM,
    DEBUGGABLE_PROPS,
    ZYGISK_ARTIFACT,
}
