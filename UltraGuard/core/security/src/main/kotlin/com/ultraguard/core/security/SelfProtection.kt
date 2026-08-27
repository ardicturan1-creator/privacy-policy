package com.ultraguard.core.security

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import com.ultraguard.core.common.log.UgLog
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import java.security.MessageDigest
import javax.inject.Inject
import javax.inject.Singleton

/**
 * UltraGuard'in kendini koruma katmani.
 *
 * Bir guvenlik urunu, korudugu cihazdaki en degerli hedeftir: onu susturan
 * saldirgan geri kalan her seyi serbestce yapabilir. Bu sinif "kandirilmis
 * bir motorun karar vermesindense hic karar vermemesi yegdir" ilkesini
 * uygular -- bulgu varsa kritik yaptirim yollari devre disi kalir ve durum
 * kullaniciya gosterilir.
 */
@Singleton
class SelfProtection @Inject constructor(
    @ApplicationContext private val context: Context,
    private val expectedSignatures: ExpectedSignatures,
) {

    fun assess(): SelfProtectionStatus {
        val findings = buildList {
            if (hasHookingFramework()) add(SelfProtectionFinding.HOOKING_FRAMEWORK)
            if (isDebuggerAttached()) add(SelfProtectionFinding.DEBUGGER_ATTACHED)
            if (!isSignatureExpected()) add(SelfProtectionFinding.SIGNATURE_MISMATCH)
            if (isRunningUnderEmulator()) add(SelfProtectionFinding.EMULATED_ENVIRONMENT)
        }
        return SelfProtectionStatus(findings)
    }

    /**
     * Frida ve Xposed/LSPosed tespiti.
     *
     * Kendi surecimizin bellek haritasini okumak Android'de her zaman
     * serbesttir (`/proc/self/maps`); baska bir surecinkini okumak degildir.
     * Bu, root'suz cihazda elimizdeki en guclu enjeksiyon sinyalidir.
     */
    private fun hasHookingFramework(): Boolean {
        val mapsHit = runCatching {
            File("/proc/self/maps").useLines { lines ->
                lines.any { line -> HOOK_LIBRARY_MARKERS.any { line.contains(it, ignoreCase = true) } }
            }
        }.getOrDefault(false)
        if (mapsHit) return true

        return XPOSED_CLASSES.any { className -> runCatching { Class.forName(className) }.isSuccess }
    }

    private fun isDebuggerAttached(): Boolean =
        android.os.Debug.isDebuggerConnected() || android.os.Debug.waitingForDebugger()

    /**
     * APK imzasinin beklenen degerde olup olmadigini dogrular. Yeniden
     * paketlenmis bir UltraGuard -- ornegin yaptirim kodu cikarilmis bir
     * surum -- burada yakalanir.
     */
    private fun isSignatureExpected(): Boolean {
        val expected = expectedSignatures.sha256Digests
        if (expected.isEmpty()) {
            // Debug derlemesinde imza sabiti bos birakilir. Release'de bos
            // olmasi derleme zamaninda engellenir (bkz. app/build.gradle.kts).
            UgLog.i(TAG, "Imza allow-list bos; dogrulama atlandi (debug derlemesi)")
            return true
        }
        return runCatching {
            val flags = PackageManager.PackageInfoFlags
                .of(PackageManager.GET_SIGNING_CERTIFICATES.toLong())
            val info = context.packageManager.getPackageInfo(context.packageName, flags)
            val signingInfo = info.signingInfo ?: return false
            val certificates = if (signingInfo.hasMultipleSigners()) {
                signingInfo.apkContentsSigners
            } else {
                signingInfo.signingCertificateHistory
            }
            certificates.orEmpty().any { signature ->
                val digest = MessageDigest.getInstance("SHA-256").digest(signature.toByteArray())
                digest.joinToString("") { "%02x".format(it) } in expected
            }
        }.getOrElse {
            UgLog.w(TAG, "Imza dogrulanamadi", it)
            false
        }
    }

    private fun isRunningUnderEmulator(): Boolean =
        Build.FINGERPRINT.startsWith("generic") ||
            Build.MODEL.contains("Emulator") ||
            Build.HARDWARE in setOf("goldfish", "ranchu", "vbox86")

    private companion object {
        const val TAG = "SelfProtection"

        val HOOK_LIBRARY_MARKERS = listOf(
            "frida", "gadget", "gum-js-loop", "xposed", "lsposed", "substrate",
        )

        val XPOSED_CLASSES = listOf(
            "de.robv.android.xposed.XposedBridge",
            "de.robv.android.xposed.XposedHelpers",
        )
    }
}

/**
 * Yayin imzalarinin SHA-256 ozetleri. `:app` modulu bunu `BuildConfig`
 * uzerinden saglar; boylece `:core:security` derleme yapilandirmasindan
 * bagimsiz kalir ve saf birim testinde calisir.
 */
data class ExpectedSignatures(val sha256Digests: Set<String>)

data class SelfProtectionStatus(val findings: List<SelfProtectionFinding>) {
    val isCompromised: Boolean get() = findings.any { it.blocksEnforcement }

    /** Butunlugumuzden emin degilsek Kasa acilmaz. */
    val shouldLockVault: Boolean get() = isCompromised
}

enum class SelfProtectionFinding(val blocksEnforcement: Boolean) {
    HOOKING_FRAMEWORK(blocksEnforcement = true),
    SIGNATURE_MISMATCH(blocksEnforcement = true),
    DEBUGGER_ATTACHED(blocksEnforcement = false),
    EMULATED_ENVIRONMENT(blocksEnforcement = false),
}
