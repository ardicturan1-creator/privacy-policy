package com.ultraguard.core.model

import kotlinx.serialization.Serializable

/**
 * Bir uygulamanin UltraGuard'daki "kimlik karti" — statik gercekler ve
 * zaman icinde biriken davranissal ozet birlikte.
 */
@Serializable
data class AppProfile(
    val packageName: String,
    val uid: Int,
    val label: String,
    val versionName: String?,
    val versionCode: Long,
    val installSource: InstallSource,
    val installerPackage: String?,
    val firstInstallMillis: Long,
    val lastUpdateMillis: Long,
    val targetSdk: Int,
    val signatureSha256: String,
    val certificateAgeDays: Int,
    val isSystemApp: Boolean,
    val requestedPermissions: Set<String>,
    /**
     * Fiilen kullanilan izinler. `requestedPermissions` ile arasindaki fark
     * guclu bir sinyaldir: "konum izni var ama 90 gundur kullanilmadi"
     * cogu zaman gereksiz bir izin talebine isaret eder.
     */
    val exercisedPermissions: Map<String, Long> = emptyMap(),
    val currentRisk: RiskScore = RiskScore.ZERO,
    val riskTrend: List<RiskSample> = emptyList(),
    val networkPolicy: AppNetworkPolicy = AppNetworkPolicy.INHERIT,
    val userTrustOverride: TrustOverride = TrustOverride.NONE,
) {
    /** Kullanici tarafindan istenmemis, kullanilmayan tehlikeli izinler. */
    fun dormantPermissions(nowMillis: Long, thresholdDays: Int = 30): Set<String> {
        val cutoff = nowMillis - thresholdDays * 86_400_000L
        return requestedPermissions.filter { permission ->
            val lastUse = exercisedPermissions[permission]
            lastUse == null || lastUse < cutoff
        }.toSet()
    }
}

@Serializable
enum class InstallSource {
    PLAY_STORE,
    OTHER_APP_STORE,
    /** Tarayici veya mesajlasma uygulamasindan yan yukleme — ilk risk sinyali. */
    SIDELOADED,
    ADB,
    PREINSTALLED,
    UNKNOWN,
}

@Serializable
data class RiskSample(val timestampMillis: Long, val score: RiskScore)

@Serializable
enum class AppNetworkPolicy { INHERIT, ALLOW, BLOCK_ALL, ALLOW_LIST_ONLY }

@Serializable
enum class TrustOverride {
    NONE,
    /** Kullanici "guveniyorum" dedi. Izleme surer, yaptirim esigi yukselir. */
    USER_TRUSTED,
    /** Kullanici yanlis pozitif bildirdi; yerel esik kalibre edilir. */
    FALSE_POSITIVE_REPORTED,
}

/**
 * Cihaz Guven Skoru. Kirilimi kullaniciya acikca gosterilir — tek bir
 * anlasilmaz sayi degil, bes bilesenin toplami.
 */
@Serializable
data class DeviceTrustScore(
    val integrity: Int,      // 0..30  verified boot, bootloader, attestation
    val patchLevel: Int,     // 0..20  guvenlik yamasi guncelligi
    val appRisk: Int,        // 0..25  kurulu uygulamalarin risk dagilimi
    val configuration: Int,  // 0..15  ADB, bilinmeyen kaynaklar, ekran kilidi
    val networkHygiene: Int, // 0..10  engellenen baglanti orani, DNS hijyeni
) {
    val total: Int get() = integrity + patchLevel + appRisk + configuration + networkHygiene

    init {
        require(integrity in 0..30 && patchLevel in 0..20 && appRisk in 0..25 &&
            configuration in 0..15 && networkHygiene in 0..10) {
            "Guven skoru bileseni kendi araliginin disinda"
        }
    }

    companion object {
        val MAX = DeviceTrustScore(30, 20, 25, 15, 10)
    }
}
