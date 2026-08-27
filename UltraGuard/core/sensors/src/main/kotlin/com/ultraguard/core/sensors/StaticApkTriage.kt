package com.ultraguard.core.sensors

import android.content.Context
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import com.ultraguard.core.common.di.IoDispatcher
import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.InstallSource
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.Subject
import dagger.hilt.android.qualifiers.ApplicationContext
import java.security.MessageDigest
import java.util.concurrent.TimeUnit
import java.util.zip.ZipFile
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.withContext

/**
 * Kurulum anindaki statik triyaj.
 *
 * Bu, karar zincirinin **en ucuz ve en erken** noktasidir: uygulama henuz
 * hicbir sey yapmadan once elimizdeki tek veri manifest ve APK yapisidir.
 * Buradan cikan sinyaller tek basina hukum vermez, ancak korelasyon
 * penceresine girerek sonraki davranissal kanitlarin agirligini belirler.
 */
@Singleton
class StaticApkTriage @Inject constructor(
    @ApplicationContext private val context: Context,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) {

    suspend fun analyze(packageName: String, nowMillis: Long): List<SecurityEvent> =
        withContext(ioDispatcher) {
            val info = runCatching { packageInfo(packageName) }.getOrNull()
                ?: return@withContext emptyList()
            val uid = info.applicationInfo?.uid ?: return@withContext emptyList()
            val subject = Subject.App(packageName, uid)

            buildList {
                installSourceEvent(info, subject, nowMillis)?.let(::add)
                permissionTriadEvent(info, subject, nowMillis)?.let(::add)
                legacyTargetSdkEvent(info, subject, nowMillis)?.let(::add)
                entropyEvent(info, subject, nowMillis)?.let(::add)
                nativeLoaderEvent(info, subject, nowMillis)?.let(::add)
                certificateEvent(info, subject, nowMillis)?.let(::add)
            }
        }

    private fun packageInfo(packageName: String): PackageInfo {
        val flags = PackageManager.GET_PERMISSIONS or PackageManager.GET_SIGNING_CERTIFICATES
        return context.packageManager.getPackageInfo(
            packageName,
            PackageManager.PackageInfoFlags.of(flags.toLong()),
        )
    }

    /**
     * Kurulum kaynagi.
     *
     * `getInstallSourceInfo` Android 11+ (API 30) itibariyla installer paketini
     * verir. Tarayici veya mesajlasma uygulamasindan gelen bir kurulum, tek
     * basina zararli degildir ama korelasyonda en degerli baglangic
     * sinyalidir: kotu amacli paketlerin ezici cogunlugu bu yoldan girer.
     */
    private fun installSourceEvent(
        info: PackageInfo,
        subject: Subject.App,
        nowMillis: Long,
    ): SecurityEvent? {
        val installer = runCatching {
            context.packageManager.getInstallSourceInfo(info.packageName).installingPackageName
        }.getOrNull()

        val source = classifyInstaller(installer)
        if (source != InstallSource.SIDELOADED && source != InstallSource.ADB) return null

        return event(
            type = EventType.PACKAGE_SIDELOAD_DETECTED,
            subject = subject,
            nowMillis = nowMillis,
            EventAttributes.INSTALLER_PACKAGE to (installer ?: "unknown"),
        )
    }

    private fun classifyInstaller(installer: String?): InstallSource = when (installer) {
        null -> InstallSource.SIDELOADED
        "com.android.vending" -> InstallSource.PLAY_STORE
        "com.android.shell" -> InstallSource.ADB
        in KNOWN_APP_STORES -> InstallSource.OTHER_APP_STORE
        in BROWSER_AND_MESSAGING -> InstallSource.SIDELOADED
        else -> InstallSource.OTHER_APP_STORE
    }

    /**
     * Tehlikeli izin kombinasyonlari.
     *
     * Tek tek her izin mesru olabilir; **birlikte** istenmeleri niyeti ele
     * verir. Erisilebilirlik (ekrani oku) + ekran ustu cizim (uzerine bindir)
     * + SMS (dogrulama kodunu al) uclusu, bankacilik trojaninin tam is
     * tanimidir.
     */
    private fun permissionTriadEvent(
        info: PackageInfo,
        subject: Subject.App,
        nowMillis: Long,
    ): SecurityEvent? {
        val requested = info.requestedPermissions?.toSet().orEmpty()
        val pattern = PERMISSION_PATTERNS.firstOrNull { (_, permissions) ->
            permissions.all { it in requested }
        } ?: return null

        return event(
            type = EventType.STATIC_DANGEROUS_PERMISSION_SET,
            subject = subject,
            nowMillis = nowMillis,
            EventAttributes.MATCHED_PATTERN to pattern.first,
        )
    }

    /**
     * Eski `targetSdk`.
     *
     * Dusuk hedef SDK, calisma zamani izin modelinden ve scoped storage'dan
     * kacinmanin bilinen yoludur. Play Store bunu kisitlar; yan yuklenmis
     * paketlerde hala yaygindir.
     */
    private fun legacyTargetSdkEvent(
        info: PackageInfo,
        subject: Subject.App,
        nowMillis: Long,
    ): SecurityEvent? {
        val targetSdk = info.applicationInfo?.targetSdkVersion ?: return null
        if (targetSdk >= LEGACY_SDK_THRESHOLD) return null

        return event(
            type = EventType.STATIC_LEGACY_TARGET_SDK,
            subject = subject,
            nowMillis = nowMillis,
            EventAttributes.TARGET_SDK to targetSdk.toString(),
        )
    }

    /**
     * DEX entropisi.
     *
     * Yuksek entropi = sikistirilmis veya sifrelenmis icerik. Yasal
     * obfuscation da entropi yukseltir, bu yuzden bu sinyal **tek basina
     * kullanilmaz**; kural motorunda yalnizca yerel yukleyiciyle birlikte
     * anlam kazanir (bkz. R-STATIC-011).
     */
    private fun entropyEvent(
        info: PackageInfo,
        subject: Subject.App,
        nowMillis: Long,
    ): SecurityEvent? {
        val apkPath = info.applicationInfo?.sourceDir ?: return null
        val entropy = runCatching { maxDexEntropy(apkPath) }.getOrNull() ?: return null
        if (entropy < HIGH_ENTROPY_THRESHOLD) return null

        return event(
            type = EventType.STATIC_HIGH_ENTROPY_DEX,
            subject = subject,
            nowMillis = nowMillis,
            EventAttributes.DEX_ENTROPY to "%.2f".format(entropy),
        )
    }

    /**
     * APK icindeki DEX dosyalarinin en yuksek Shannon entropisi.
     *
     * Tam dosyayi degil, ilk [ENTROPY_SAMPLE_BYTES] baytini orneller: DEX
     * basliginin hemen ardindaki bolge, paketlenmis bir payload varsa zaten
     * yuksek entropilidir ve 20 MB'lik bir APK'yi tam okumak kurulum aninda
     * kabul edilemez bir gecikme yaratir.
     */
    internal fun maxDexEntropy(apkPath: String): Double? = ZipFile(apkPath).use { zip ->
        zip.entries().asSequence()
            .filter { it.name.endsWith(".dex") }
            .mapNotNull { entry ->
                zip.getInputStream(entry).use { stream ->
                    val buffer = ByteArray(ENTROPY_SAMPLE_BYTES)
                    val read = stream.read(buffer)
                    if (read <= 0) null else shannonEntropy(buffer, read)
                }
            }
            .maxOrNull()
    }

    /** Shannon entropisi, bayt basina bit cinsinden (0..8). */
    internal fun shannonEntropy(data: ByteArray, length: Int): Double {
        if (length <= 0) return 0.0
        val frequencies = IntArray(256)
        for (index in 0 until length) {
            frequencies[data[index].toInt() and 0xFF]++
        }
        var entropy = 0.0
        for (count in frequencies) {
            if (count == 0) continue
            val probability = count.toDouble() / length
            entropy -= probability * (Math.log(probability) / Math.log(2.0))
        }
        return entropy
    }

    /** `System.loadLibrary` cagrisi iceren yerel kutuphane varligi. */
    private fun nativeLoaderEvent(
        info: PackageInfo,
        subject: Subject.App,
        nowMillis: Long,
    ): SecurityEvent? {
        val hasNativeLibs = info.applicationInfo?.nativeLibraryDir
            ?.let { java.io.File(it).listFiles()?.isNotEmpty() == true }
            ?: false
        if (!hasNativeLibs) return null

        return event(EventType.STATIC_NATIVE_LOADER_PRESENT, subject, nowMillis)
    }

    /**
     * Sertifika yasi.
     *
     * Gunler once uretilmis kendinden imzali bir sertifika, tek kullanimlik
     * kampanya altyapisinin isaretidir. Yerlesik gelistiricilerin
     * sertifikalari yillar oncesine dayanir.
     */
    private fun certificateEvent(
        info: PackageInfo,
        subject: Subject.App,
        nowMillis: Long,
    ): SecurityEvent? {
        val signingInfo = info.signingInfo ?: return null
        val certificates = if (signingInfo.hasMultipleSigners()) {
            signingInfo.apkContentsSigners
        } else {
            signingInfo.signingCertificateHistory
        } ?: return null

        val certificate = certificates.firstOrNull() ?: return null
        val parsed = runCatching {
            val factory = java.security.cert.CertificateFactory.getInstance("X.509")
            factory.generateCertificate(certificate.toByteArray().inputStream())
                as java.security.cert.X509Certificate
        }.getOrNull() ?: return null

        val ageDays = TimeUnit.MILLISECONDS.toDays(nowMillis - parsed.notBefore.time)
        val selfSigned = parsed.issuerX500Principal == parsed.subjectX500Principal
        if (!selfSigned || ageDays > YOUNG_CERT_DAYS) return null

        return event(
            type = EventType.STATIC_SELF_SIGNED_YOUNG_CERT,
            subject = subject,
            nowMillis = nowMillis,
            EventAttributes.CERT_AGE_DAYS to ageDays.toString(),
        )
    }

    private fun event(
        type: EventType,
        subject: Subject.App,
        nowMillis: Long,
        vararg attributes: Pair<String, String>,
    ) = SecurityEvent(
        timestampMillis = nowMillis,
        type = type,
        subject = subject,
        source = SensorSource.STATIC_TRIAGE,
        attributes = attributes.toMap(),
    )

    internal companion object {
        const val HIGH_ENTROPY_THRESHOLD = 7.8
        const val LEGACY_SDK_THRESHOLD = 29
        const val YOUNG_CERT_DAYS = 30L
        const val ENTROPY_SAMPLE_BYTES = 256 * 1024

        /**
         * Tehlikeli izin kombinasyonlari. Sira onemlidir: en spesifik ve en
         * yuksek guvenli kalip once gelir, ilk eslesme kazanir.
         */
        val PERMISSION_PATTERNS: List<Pair<String, Set<String>>> = listOf(
            "a11y_overlay_sms" to setOf(
                "android.permission.BIND_ACCESSIBILITY_SERVICE",
                "android.permission.SYSTEM_ALERT_WINDOW",
                "android.permission.RECEIVE_SMS",
            ),
            "a11y_overlay_install" to setOf(
                "android.permission.BIND_ACCESSIBILITY_SERVICE",
                "android.permission.SYSTEM_ALERT_WINDOW",
                "android.permission.REQUEST_INSTALL_PACKAGES",
            ),
            "covert_surveillance" to setOf(
                "android.permission.ACCESS_FINE_LOCATION",
                "android.permission.RECORD_AUDIO",
                "android.permission.RECEIVE_BOOT_COMPLETED",
            ),
            "sms_fraud" to setOf(
                "android.permission.RECEIVE_SMS",
                "android.permission.SEND_SMS",
                "android.permission.READ_PHONE_STATE",
            ),
        )

        val KNOWN_APP_STORES = setOf(
            "com.amazon.venezia",
            "com.sec.android.app.samsungapps",
            "com.huawei.appmarket",
            "org.fdroid.fdroid",
        )

        val BROWSER_AND_MESSAGING = setOf(
            "com.android.chrome",
            "org.mozilla.firefox",
            "com.whatsapp",
            "org.telegram.messenger",
            "com.android.browser",
        )
    }
}
