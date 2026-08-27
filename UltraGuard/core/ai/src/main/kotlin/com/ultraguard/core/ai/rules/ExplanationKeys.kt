package com.ultraguard.core.ai.rules

import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent

/**
 * Olay turunu, kullaniciya gosterilecek sade dil aciklamasinin string
 * kaynak anahtarina eslestirir.
 *
 * Aciklama metinleri `:core:designsystem` icindeki `strings.xml`'de yasar;
 * bu katman **hicbir sekilde** cihaz diline veya Android Context'e bagli
 * degildir, boylece kural motoru saf JVM testinde calisir.
 */
internal object ExplanationKeys {

    fun of(type: EventType): String = when (type) {
        EventType.PACKAGE_SIDELOAD_DETECTED -> "expl_sideload"
        EventType.PACKAGE_INSTALLED -> "expl_installed"
        EventType.STATIC_DANGEROUS_PERMISSION_SET -> "expl_dangerous_perm_set"
        EventType.STATIC_HIGH_ENTROPY_DEX -> "expl_packed_dex"
        EventType.STATIC_LEGACY_TARGET_SDK -> "expl_legacy_sdk"
        EventType.STATIC_SELF_SIGNED_YOUNG_CERT -> "expl_young_cert"
        EventType.STATIC_NATIVE_LOADER_PRESENT -> "expl_native_loader"
        EventType.ACCESSIBILITY_SERVICE_ENABLED -> "expl_a11y_enabled"
        EventType.ACCESSIBILITY_GESTURE_CAPABILITY -> "expl_a11y_gesture"
        EventType.ACCESSIBILITY_WINDOW_QUERY -> "expl_a11y_window_query"
        EventType.OVERLAY_DRAWN -> "expl_overlay"
        EventType.OVERLAY_ON_PROTECTED_SCREEN -> "expl_overlay_protected"
        EventType.MEDIA_PROJECTION_STARTED -> "expl_screen_capture"
        EventType.SENSOR_CAMERA_ACCESS -> "expl_camera"
        EventType.SENSOR_MICROPHONE_ACCESS -> "expl_microphone"
        EventType.SENSOR_LOCATION_ACCESS -> "expl_location"
        EventType.SENSOR_BACKGROUND_ACCESS -> "expl_background_sensor"
        EventType.CLIPBOARD_READ -> "expl_clipboard"
        EventType.CLIPBOARD_SENSITIVE_CONTENT -> "expl_clipboard_sensitive"
        EventType.NOTIFICATION_PHISHING_PATTERN -> "expl_notification_phishing"
        EventType.NETWORK_BEACON_PATTERN -> "expl_beacon"
        EventType.NETWORK_DGA_DOMAIN -> "expl_dga"
        EventType.NETWORK_REPUTATION_HIT -> "expl_bad_reputation"
        EventType.NETWORK_BULK_UPLOAD -> "expl_bulk_upload"
        EventType.NETWORK_TLS_HANDSHAKE -> "expl_tls"
        EventType.NETWORK_CONNECTION_OPENED -> "expl_connection"
        EventType.ROOT_INDICATOR_FOUND -> "expl_root_indicator"
        EventType.HOOKING_FRAMEWORK_DETECTED -> "expl_hooking"
        EventType.ADB_ENABLED -> "expl_adb"
        EventType.WIRELESS_DEBUGGING_ENABLED -> "expl_wireless_debug"
        EventType.KERNEL_MEMFD_CREATE -> "expl_memfd"
        EventType.KERNEL_EXEC -> "expl_exec"
        EventType.KERNEL_PTRACE_ATTACH -> "expl_ptrace"
        EventType.BINDER_SENSITIVE_TRANSACTION -> "expl_binder"
        else -> "expl_generic_${type.name.lowercase()}"
    }

    /** Aciklamaya gomulecek somut degerler — "…com.bank.mobile penceresini sorguladi". */
    fun argsOf(event: SecurityEvent): List<String> = buildList {
        event.attr(EventAttributes.TARGET_PACKAGE)?.let { add(it) }
        event.attr(EventAttributes.WINDOW_OWNER)?.let { add(it) }
        event.attr(EventAttributes.REMOTE_HOST)?.let { add(it) }
        event.attr(EventAttributes.PERMISSION)?.let { add(it) }
        event.attr(EventAttributes.INSTALLER_PACKAGE)?.let { add(it) }
        event.attr(EventAttributes.INDICATOR)?.let { add(it) }
    }
}
