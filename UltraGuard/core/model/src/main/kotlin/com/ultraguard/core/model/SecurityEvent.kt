package com.ultraguard.core.model

import kotlinx.serialization.Serializable

/**
 * UltraGuard'in tum telemetri kaynaklarinin ortak para birimi.
 *
 * Tasarim kararlari:
 *  - **Icerik tasimaz.** Bir olay "hangi uygulama, ne zaman, hangi yetenegi
 *    kullandi" bilgisini tasir; ekran metni, bildirim govdesi veya dosya
 *    icerigi asla bu yapiya girmez. Sensorler icerigi RAM'de degerlendirip
 *    yalnizca turetilmis bir sinyal (or. `matched_pattern=otp_exfil`) yayar.
 *  - **Duz ve serilestirilebilir.** Ayni yapi hem Room'a yazilir, hem L2
 *    modeline vektorlestirilir, hem de zaman cizelgesinde gosterilir.
 *  - **`attributes` serbest ama sozlesmelidir.** Anahtarlar [EventAttributes]
 *    icinde sabittir; keyfi anahtar eklemek modeli sessizce bozar.
 */
@Serializable
data class SecurityEvent(
    val id: Long = 0L,
    val timestampMillis: Long,
    val type: EventType,
    val subject: Subject,
    val source: SensorSource,
    val attributes: Map<String, String> = emptyMap(),
) {
    val packageName: String? get() = subject.packageName
    val uid: Int? get() = subject.uid

    fun attr(key: String): String? = attributes[key]
    fun attrLong(key: String): Long? = attributes[key]?.toLongOrNull()
    fun attrBool(key: String): Boolean = attributes[key]?.toBooleanStrictOrNull() ?: false
}

/**
 * Olayin oznesi. Sistem kaynakli olaylarda ([Subject.System]) paket yoktur —
 * or. "ADB acildi", "verified boot durumu degisti".
 */
@Serializable
sealed interface Subject {
    val packageName: String?
    val uid: Int?

    @Serializable
    data class App(
        override val packageName: String,
        override val uid: Int,
    ) : Subject

    @Serializable
    data object System : Subject {
        override val packageName: String? get() = null
        override val uid: Int? get() = null
    }
}

/** Olayi ureten telemetri toplayicisi. Bir sensor cokerse digerleri etkilenmez. */
@Serializable
enum class SensorSource {
    PACKAGE_LIFECYCLE,
    STATIC_TRIAGE,
    APP_OPS,
    ACCESSIBILITY,
    NOTIFICATION_LISTENER,
    NETWORK_FLOW,
    INTEGRITY,
    SYSTEM_SETTINGS,
    USAGE_STATS,
    SELF_PROTECTION,

    /** Yalnizca root/KernelSU cihazlarda, `:module:deepscan` tarafindan. */
    KERNEL_EBPF,
    BINDER_IPC,
}

/**
 * Olay taksonomisi. Sira **asla degistirilmemelidir**: ordinal degeri hem
 * veritabaninda hem de L2 modelinin gomme tablosunda indeks olarak kullanilir.
 * Yeni tur her zaman sona eklenir.
 */
@Serializable
enum class EventType(val domain: EventDomain) {
    // --- Paket yasam dongusu ---
    PACKAGE_INSTALLED(EventDomain.PACKAGE),
    PACKAGE_UPDATED(EventDomain.PACKAGE),
    PACKAGE_REMOVED(EventDomain.PACKAGE),
    PACKAGE_INSTALL_SESSION_STARTED(EventDomain.PACKAGE),
    PACKAGE_SIDELOAD_DETECTED(EventDomain.PACKAGE),
    PACKAGE_SIGNATURE_CHANGED(EventDomain.PACKAGE),

    // --- Statik triyaj bulgulari ---
    STATIC_DANGEROUS_PERMISSION_SET(EventDomain.PACKAGE),
    STATIC_HIGH_ENTROPY_DEX(EventDomain.PACKAGE),
    STATIC_LEGACY_TARGET_SDK(EventDomain.PACKAGE),
    STATIC_NATIVE_LOADER_PRESENT(EventDomain.PACKAGE),
    STATIC_SELF_SIGNED_YOUNG_CERT(EventDomain.PACKAGE),

    // --- Izin ve sensor kullanimi ---
    PERMISSION_GRANTED(EventDomain.PERMISSION),
    PERMISSION_REVOKED(EventDomain.PERMISSION),
    SENSOR_CAMERA_ACCESS(EventDomain.SENSOR),
    SENSOR_MICROPHONE_ACCESS(EventDomain.SENSOR),
    SENSOR_LOCATION_ACCESS(EventDomain.SENSOR),
    SENSOR_BACKGROUND_ACCESS(EventDomain.SENSOR),
    CLIPBOARD_READ(EventDomain.SENSOR),
    CLIPBOARD_SENSITIVE_CONTENT(EventDomain.SENSOR),
    MEDIA_PROJECTION_STARTED(EventDomain.SENSOR),

    // --- Arayuz katmani ---
    ACCESSIBILITY_SERVICE_ENABLED(EventDomain.UI),
    ACCESSIBILITY_GESTURE_CAPABILITY(EventDomain.UI),
    ACCESSIBILITY_WINDOW_QUERY(EventDomain.UI),
    OVERLAY_DRAWN(EventDomain.UI),
    OVERLAY_ON_PROTECTED_SCREEN(EventDomain.UI),
    FOREGROUND_APP_CHANGED(EventDomain.UI),
    NOTIFICATION_POSTED(EventDomain.UI),
    NOTIFICATION_PHISHING_PATTERN(EventDomain.UI),

    // --- Ag ---
    NETWORK_CONNECTION_OPENED(EventDomain.NETWORK),
    NETWORK_DNS_QUERY(EventDomain.NETWORK),
    NETWORK_TLS_HANDSHAKE(EventDomain.NETWORK),
    NETWORK_BEACON_PATTERN(EventDomain.NETWORK),
    NETWORK_DGA_DOMAIN(EventDomain.NETWORK),
    NETWORK_REPUTATION_HIT(EventDomain.NETWORK),
    NETWORK_BULK_UPLOAD(EventDomain.NETWORK),
    NETWORK_BLOCKED_BY_POLICY(EventDomain.NETWORK),

    // --- Butunluk ve sistem ---
    INTEGRITY_VERDICT_CHANGED(EventDomain.INTEGRITY),
    BOOTLOADER_UNLOCKED(EventDomain.INTEGRITY),
    ROOT_INDICATOR_FOUND(EventDomain.INTEGRITY),
    ADB_ENABLED(EventDomain.INTEGRITY),
    WIRELESS_DEBUGGING_ENABLED(EventDomain.INTEGRITY),
    UNKNOWN_SOURCES_ENABLED(EventDomain.INTEGRITY),
    SELF_TAMPER_SUSPECTED(EventDomain.INTEGRITY),
    HOOKING_FRAMEWORK_DETECTED(EventDomain.INTEGRITY),

    // --- Kernel yuzeyi (yalnizca [R]) ---
    KERNEL_EXEC(EventDomain.KERNEL),
    KERNEL_MEMFD_CREATE(EventDomain.KERNEL),
    KERNEL_PTRACE_ATTACH(EventDomain.KERNEL),
    BINDER_SENSITIVE_TRANSACTION(EventDomain.KERNEL),
    ;

    companion object {
        /** L2 gomme tablosunun boyutu bu degere baglidir. */
        val CARDINALITY: Int = entries.size
    }
}

@Serializable
enum class EventDomain { PACKAGE, PERMISSION, SENSOR, UI, NETWORK, INTEGRITY, KERNEL }

/** [SecurityEvent.attributes] icin sozlesmeli anahtarlar. */
object EventAttributes {
    const val INSTALLER_PACKAGE = "installer_package"
    const val PERMISSION = "permission"
    const val TARGET_PACKAGE = "target_package"
    const val DEX_ENTROPY = "dex_entropy"
    const val TARGET_SDK = "target_sdk"
    const val CERT_AGE_DAYS = "cert_age_days"
    const val FOREGROUND = "foreground"
    const val SCREEN_ON = "screen_on"
    const val REMOTE_HOST = "remote_host"
    const val REMOTE_IP = "remote_ip"
    const val REMOTE_PORT = "remote_port"
    const val REMOTE_ASN = "remote_asn"
    const val TLS_FINGERPRINT = "tls_ja4"
    const val BYTES_OUT = "bytes_out"
    const val BYTES_IN = "bytes_in"
    const val BEACON_INTERVAL_MS = "beacon_interval_ms"
    const val DOMAIN_ENTROPY = "domain_entropy"
    const val REPUTATION_VERDICT = "reputation_verdict"
    const val MATCHED_PATTERN = "matched_pattern"
    const val WINDOW_OWNER = "window_owner"
    const val CAN_PERFORM_GESTURES = "can_perform_gestures"
    const val INDICATOR = "indicator"
    const val PROTECTED_CATEGORY = "protected_category"
}
