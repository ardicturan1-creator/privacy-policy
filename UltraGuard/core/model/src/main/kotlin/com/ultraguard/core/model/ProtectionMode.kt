package com.ultraguard.core.model

import kotlinx.serialization.Serializable

/**
 * Calisma modu. Mod, **ne izlendigini degil**, izlenenin sonucunda ne
 * yapildigini ve kullaniciya nasil anlatildigini belirler. Telemetri
 * her modda ayni kalir; degisen yaptirim esikleri ve bildirim politikasidir.
 */
@Serializable
enum class ProtectionMode(
    /** Otonom yaptirimin devreye girdigi risk esigi. */
    val autonomousThreshold: RiskScore,
    /** Kullaniciya bildirim gonderilen en dusuk risk bandi. */
    val notifyFrom: RiskBand,
    /** Yan yuklenen paketler kurulum aninda engellenir mi? */
    val blockSideload: Boolean,
    /** Ag politikasi varsayilani. */
    val networkDefault: NetworkStance,
    /** Her yeni tehlikeli izin icin manuel onay istenir mi? */
    val requireManualPermissionApproval: Boolean,
) {
    /** Varsayilan. Tam telemetri, geri alinabilir otonom mudahale. */
    ACTIVE(
        autonomousThreshold = RiskScore(75),
        notifyFrom = RiskBand.ELEVATED,
        blockSideload = false,
        networkDefault = NetworkStance.ALLOW_WITH_INSPECTION,
        requireManualPermissionApproval = false,
    ),

    /** Ayni koruma, sessiz. Yalnizca KRITIK bildirim cikar. */
    STEALTH(
        autonomousThreshold = RiskScore(75),
        notifyFrom = RiskBand.CRITICAL,
        blockSideload = false,
        networkDefault = NetworkStance.ALLOW_WITH_INSPECTION,
        requireManualPermissionApproval = false,
    ),

    /** En agresif. Varsayilan-reddet ag, yan yukleme engeli, manuel onaylar. */
    PARANOID(
        autonomousThreshold = RiskScore(50),
        notifyFrom = RiskBand.LOW,
        blockSideload = true,
        networkDefault = NetworkStance.DENY_BY_DEFAULT,
        requireManualPermissionApproval = true,
    ),

    /** Kurumsal. Esikler uzaktan politika ile gelir; burada guvenli varsayilan. */
    FLEET(
        autonomousThreshold = RiskScore(60),
        notifyFrom = RiskBand.ELEVATED,
        blockSideload = true,
        networkDefault = NetworkStance.DENY_BY_DEFAULT,
        requireManualPermissionApproval = false,
    ),

    /**
     * Pil %15 altinda otomatik devreye girer. L2 modeli askiya alinir;
     * L1 kurallari ve ag kalkani calismaya devam eder. Koruma azalir ama
     * asla sifirlanmaz — ve bu durum kullaniciya acikca gosterilir.
     */
    BATTERY_GUARD(
        autonomousThreshold = RiskScore(80),
        notifyFrom = RiskBand.HIGH,
        blockSideload = false,
        networkDefault = NetworkStance.ALLOW_WITH_INSPECTION,
        requireManualPermissionApproval = false,
    ),
    ;

    /** [BATTERY_GUARD] disinda L2 dizi modeli calisir. */
    val runsOnDeviceModel: Boolean get() = this != BATTERY_GUARD
}

@Serializable
enum class NetworkStance { ALLOW_WITH_INSPECTION, DENY_BY_DEFAULT }

/**
 * Izleme yogunlugu durum makinesi. Pil butcesinin tamami bu uc durumun
 * dogru yonetilmesine baglidir: BASELINE'da orneklemeli, HEIGHTENED'da tam
 * telemetri toplanir ve HEIGHTENED **her zaman sure sinirlidir**.
 */
@Serializable
enum class MonitoringState(val samplingRatio: Float) {
    /** Sakin durum. Olaylarin %5'i L2'ye kadar cikar. Hedef: <%1.5 pil/24s. */
    BASELINE(0.05f),

    /** Yeni paket veya temel profil sapmasi sonrasi, varsayilan 10 dakika. */
    HEIGHTENED(1.0f),

    /** Aktif yaptirim altinda. Tam telemetri + kanit toplama. */
    CONTAINMENT(1.0f),
}
