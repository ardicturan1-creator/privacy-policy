package com.ultraguard.shield.work

import android.content.Context
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.database.entity.AppProfileEntity
import com.ultraguard.core.model.InstallSource
import com.ultraguard.core.model.TrustOverride
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import java.util.concurrent.TimeUnit
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json

/**
 * Kurulu uygulama envanterini veritabaniyla esitler.
 *
 * Paket olaylari anlik degisiklikleri yakalar, ama bu yeterli degildir:
 * uygulama kapaliyken veya sistem tarafindan oldurulmusken yapilan
 * kurulumlar kacirilabilir. Bu periyodik esitleme, envanterin gercekle
 * uyusmasini garanti eder -- goremedigimiz uygulamayi degerlendiremeyiz.
 */
@HiltWorker
class AppInventoryWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val appProfileDao: AppProfileDao,
    private val clock: Clock,
) : CoroutineWorker(context, params) {

    private val json = Json { encodeDefaults = true }
    private val stringListSerializer = ListSerializer(String.serializer())
    private val usageMapSerializer = MapSerializer(String.serializer(), Long.serializer())

    override suspend fun doWork(): Result {
        val packageManager = applicationContext.packageManager
        val now = clock.nowMillis()

        val installed = runCatching {
            packageManager.getInstalledPackages(
                PackageManager.PackageInfoFlags.of(
                    (PackageManager.GET_PERMISSIONS or PackageManager.GET_SIGNING_CERTIFICATES)
                        .toLong(),
                ),
            )
        }.getOrElse { error ->
            UgLog.w(TAG, "Paket listesi alinamadi", error)
            return Result.retry()
        }

        val seen = mutableSetOf<String>()

        installed.forEach { info ->
            val profile = runCatching { toProfile(info, packageManager, now) }.getOrNull()
                ?: return@forEach
            appProfileDao.upsert(profile)
            seen += profile.packageName
        }

        // Kaldirilmis uygulamalarin profilleri temizlenir. Olay gecmisleri
        // saklama suresi dolana kadar kalir: bir uygulamanin kaldirilmis
        // olmasi, ne yaptigini unutmamiz gerektigi anlamina gelmez.
        val known = appProfileDao.allByRisk()
        UgLog.d(TAG) { "Envanter esitlendi: ${seen.size} paket" }

        return Result.success()
    }

    private fun toProfile(
        info: PackageInfo,
        packageManager: PackageManager,
        nowMillis: Long,
    ): AppProfileEntity {
        val appInfo = requireNotNull(info.applicationInfo)
        val installer = runCatching {
            packageManager.getInstallSourceInfo(info.packageName).installingPackageName
        }.getOrNull()

        val signature = signatureDigest(info)

        return AppProfileEntity(
            packageName = info.packageName,
            uid = appInfo.uid,
            label = packageManager.getApplicationLabel(appInfo).toString(),
            versionName = info.versionName,
            versionCode = info.longVersionCode,
            installSource = classify(installer, appInfo),
            installerPackage = installer,
            firstInstallMillis = info.firstInstallTime,
            lastUpdateMillis = info.lastUpdateTime,
            targetSdk = appInfo.targetSdkVersion,
            signatureSha256 = signature,
            certificateAgeDays = certificateAgeDays(info, nowMillis),
            isSystemApp = appInfo.flags and android.content.pm.ApplicationInfo.FLAG_SYSTEM != 0,
            requestedPermissions = json.encodeToString(
                stringListSerializer,
                info.requestedPermissions?.toList().orEmpty(),
            ),
            // Fiili kullanim AppOps telemetrisinden birikir; envanter
            // esitlemesi mevcut kaydin uzerine yazmaz.
            exercisedPermissions = json.encodeToString(usageMapSerializer, emptyMap()),
            currentRisk = 0,
            networkPolicy = "INHERIT",
            trustOverride = TrustOverride.NONE,
        )
    }

    private fun classify(
        installer: String?,
        appInfo: android.content.pm.ApplicationInfo,
    ): InstallSource = when {
        appInfo.flags and android.content.pm.ApplicationInfo.FLAG_SYSTEM != 0 ->
            InstallSource.PREINSTALLED
        installer == "com.android.vending" -> InstallSource.PLAY_STORE
        installer == "com.android.shell" -> InstallSource.ADB
        installer == null -> InstallSource.SIDELOADED
        else -> InstallSource.OTHER_APP_STORE
    }

    private fun signatureDigest(info: PackageInfo): String = runCatching {
        val signingInfo = info.signingInfo ?: return@runCatching ""
        val certificates = if (signingInfo.hasMultipleSigners()) {
            signingInfo.apkContentsSigners
        } else {
            signingInfo.signingCertificateHistory
        }
        val first = certificates?.firstOrNull() ?: return@runCatching ""
        java.security.MessageDigest.getInstance("SHA-256")
            .digest(first.toByteArray())
            .joinToString("") { "%02x".format(it) }
    }.getOrDefault("")

    private fun certificateAgeDays(info: PackageInfo, nowMillis: Long): Int = runCatching {
        val signingInfo = info.signingInfo ?: return@runCatching 0
        val certificates = if (signingInfo.hasMultipleSigners()) {
            signingInfo.apkContentsSigners
        } else {
            signingInfo.signingCertificateHistory
        }
        val first = certificates?.firstOrNull() ?: return@runCatching 0
        val parsed = java.security.cert.CertificateFactory.getInstance("X.509")
            .generateCertificate(first.toByteArray().inputStream())
            as java.security.cert.X509Certificate
        TimeUnit.MILLISECONDS.toDays(nowMillis - parsed.notBefore.time).toInt()
    }.getOrDefault(0)

    companion object {
        const val NAME = "ultraguard_app_inventory"
        private const val TAG = "AppInventoryWorker"
    }
}
