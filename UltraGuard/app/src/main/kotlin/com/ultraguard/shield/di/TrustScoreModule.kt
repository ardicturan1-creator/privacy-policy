package com.ultraguard.shield.di

import android.content.Context
import android.os.Build
import android.provider.Settings
import com.ultraguard.core.common.di.IoDispatcher
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.database.dao.NetworkFlowDao
import com.ultraguard.core.model.DeviceTrustScore
import com.ultraguard.core.security.DatabaseKeyProvider
import com.ultraguard.core.security.KeyStorageLevel
import com.ultraguard.core.security.RootDetector
import com.ultraguard.core.security.SelfProtection
import com.ultraguard.feature.dashboard.TrustScoreCalculator
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map

@Module
@InstallIn(SingletonComponent::class)
abstract class TrustScoreModule {

    @Binds
    @Singleton
    abstract fun bindTrustScoreCalculator(impl: DeviceTrustScoreCalculator): TrustScoreCalculator
}

/**
 * Cihaz Guven Skoru hesabi.
 *
 * Skorun tasarim ilkesi: **her puan kaybinin kullaniciya gosterilebilir bir
 * nedeni olmalidir.** Anlasilmaz tek bir sayi, kullaniciyi bilgilendirmez;
 * yalnizca kaygilandirir. Bu yuzden bes bilesenin her biri ayri hesaplanir
 * ve arayuzde kirilim olarak acilabilir.
 *
 * Agirliklar:
 *   Butunluk 30 · Yama 20 · Uygulama riski 25 · Yapilandirma 15 · Ag 10
 */
@Singleton
class DeviceTrustScoreCalculator @Inject constructor(
    @ApplicationContext private val context: Context,
    private val appProfileDao: AppProfileDao,
    private val networkFlowDao: NetworkFlowDao,
    private val rootDetector: RootDetector,
    private val selfProtection: SelfProtection,
    private val keyProvider: DatabaseKeyProvider,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) : TrustScoreCalculator {

    override fun scoreStream(): Flow<DeviceTrustScore> =
        appProfileDao.allByRisk()
            .map { profiles ->
                DeviceTrustScore(
                    integrity = integrityScore(),
                    patchLevel = patchLevelScore(),
                    appRisk = appRiskScore(profiles.map { it.currentRisk }),
                    configuration = configurationScore(),
                    networkHygiene = networkHygieneScore(),
                )
            }
            .flowOn(ioDispatcher)

    /**
     * Butunluk (0-30).
     *
     * Root tek basina puan sifirlamaz: bilincli olarak root'lanmis bir cihaz
     * "ele gecirilmis" degildir. Ancak uygulama sandbox'i zayifladigi icin
     * tehdit modeli degisir ve bu skora yansir. Buna karsilik hooking
     * cercevesi veya imza uyusmazligi ciddi kayiptir -- bunlar UltraGuard'in
     * kendisinin kandirildigi anlamina gelir.
     */
    private fun integrityScore(): Int {
        var score = MAX_INTEGRITY

        val rootAssessment = rootDetector.detect()
        if (rootAssessment.isRooted) score -= ROOT_PENALTY

        val selfStatus = selfProtection.assess()
        if (selfStatus.isCompromised) score -= COMPROMISED_PENALTY

        // Yazilim destekli anahtar saklama, donanim destekliden zayiftir.
        when (keyProvider.attainedSecurityLevel) {
            KeyStorageLevel.STRONGBOX -> Unit
            KeyStorageLevel.TEE -> score -= TEE_PENALTY
            KeyStorageLevel.SOFTWARE, KeyStorageLevel.UNKNOWN -> score -= SOFTWARE_KEY_PENALTY
        }

        return score.coerceIn(0, MAX_INTEGRITY)
    }

    /**
     * Yama duzeyi (0-20).
     *
     * Guvenlik yamasi tarihi ne kadar eskiyse, bilinen ve kamuya acik
     * aciklarin sayisi o kadar fazladir. Bu, kullanicinin dogrudan kontrol
     * edemedigi bir bilesendir (uretici gunceleme yayinlamiyor olabilir),
     * bu yuzden arayuzde suclayici degil bilgilendirici dille sunulur.
     */
    private fun patchLevelScore(): Int {
        val patchDate = runCatching {
            java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US)
                .parse(Build.VERSION.SECURITY_PATCH)
        }.getOrNull() ?: return MAX_PATCH / 2

        val ageDays = TimeUnit.MILLISECONDS.toDays(System.currentTimeMillis() - patchDate.time)

        return when {
            ageDays <= 60 -> MAX_PATCH
            ageDays <= 120 -> 16
            ageDays <= 240 -> 11
            ageDays <= 365 -> 6
            else -> 0
        }
    }

    /**
     * Uygulama riski (0-25).
     *
     * En riskli tek uygulama, ortalamadan daha anlamlidir: 50 temiz uygulama
     * arasindaki bir bankacilik trojani, ortalamayi alirsak gorunmez olur.
     * Bu yuzden tepe deger ve yuksek riskli uygulama sayisi birlikte
     * kullanilir.
     */
    private fun appRiskScore(risks: List<Int>): Int {
        if (risks.isEmpty()) return MAX_APP_RISK

        val peak = risks.max()
        val highRiskCount = risks.count { it >= HIGH_RISK_THRESHOLD }

        val peakPenalty = (peak * MAX_APP_RISK) / 100
        val countPenalty = (highRiskCount * PER_HIGH_RISK_PENALTY).coerceAtMost(MAX_APP_RISK)

        return (MAX_APP_RISK - maxOf(peakPenalty, countPenalty)).coerceIn(0, MAX_APP_RISK)
    }

    /** Yapilandirma (0-15): hata ayiklama yuzeyi ve bilinmeyen kaynaklar. */
    private fun configurationScore(): Int {
        var score = MAX_CONFIGURATION

        if (globalFlag(Settings.Global.ADB_ENABLED)) score -= ADB_PENALTY
        if (globalFlag(SETTING_ADB_WIFI)) score -= WIRELESS_DEBUG_PENALTY

        val hasScreenLock = runCatching {
            context.getSystemService(android.app.KeyguardManager::class.java).isDeviceSecure
        }.getOrDefault(true)
        if (!hasScreenLock) score -= NO_SCREEN_LOCK_PENALTY

        return score.coerceIn(0, MAX_CONFIGURATION)
    }

    /**
     * Ag hijyeni (0-10).
     *
     * Engellenen baglanti sayisi **kotu bir isaret degildir** -- korumanin
     * calistigini gosterir. Puan kaybi, engellenen baglantilarin toplam
     * icindeki payi anormal derecede yuksek oldugunda olusur: bu, cihazda
     * israrla disari ulasmaya calisan bir sey oldugu anlamina gelir.
     */
    private suspend fun networkHygieneScore(): Int {
        val sinceDay = (System.currentTimeMillis() - WEEK_MILLIS) / DAY_MILLIS
        val summary = runCatching { networkFlowDao.deviceWideSummary(sinceDay) }.getOrNull()
            ?: return MAX_NETWORK
        if (summary.totalConnections == 0) return MAX_NETWORK

        val blockedRatio = summary.blockedConnections.toFloat() / summary.totalConnections
        return when {
            blockedRatio < 0.01f -> MAX_NETWORK
            blockedRatio < 0.05f -> 8
            blockedRatio < 0.15f -> 5
            else -> 2
        }
    }

    private fun globalFlag(key: String): Boolean = runCatching {
        Settings.Global.getInt(context.contentResolver, key, 0) == 1
    }.getOrDefault(false)

    private companion object {
        const val MAX_INTEGRITY = 30
        const val MAX_PATCH = 20
        const val MAX_APP_RISK = 25
        const val MAX_CONFIGURATION = 15
        const val MAX_NETWORK = 10

        const val ROOT_PENALTY = 8
        const val COMPROMISED_PENALTY = 20
        const val TEE_PENALTY = 2
        const val SOFTWARE_KEY_PENALTY = 6

        const val HIGH_RISK_THRESHOLD = 75
        const val PER_HIGH_RISK_PENALTY = 9

        const val ADB_PENALTY = 5
        const val WIRELESS_DEBUG_PENALTY = 7
        const val NO_SCREEN_LOCK_PENALTY = 8

        const val SETTING_ADB_WIFI = "adb_wifi_enabled"
        const val DAY_MILLIS = 86_400_000L
        const val WEEK_MILLIS = 7 * DAY_MILLIS
    }
}
