package com.ultraguard.core.sensors

import android.content.Context
import android.database.ContentObserver
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.engine.EventBus
import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.Subject
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Cihaz yapilandirmasi telemetrisi: ADB, kablosuz hata ayiklama, bilinmeyen
 * kaynaklar.
 *
 * Bunlar "kotu amacli uygulama" degildir -- cihazin saldiri yuzeyini
 * genisleten ayarlardir. Kablosuz hata ayiklama acik bir cihaza, ayni agdaki
 * biri fiziksel erisim olmadan baglanabilir; bu, kilit ekraninin buyuk olcude
 * anlamsizlastigi bir durumdur.
 */
@Singleton
class SystemSettingsCollector @Inject constructor(
    @ApplicationContext private val context: Context,
    private val eventBus: EventBus,
    private val clock: Clock,
) {
    private val handler = Handler(Looper.getMainLooper())

    private val observer = object : ContentObserver(handler) {
        override fun onChange(selfChange: Boolean, uri: Uri?) {
            evaluate()
        }
    }

    fun start() {
        WATCHED_SETTINGS.forEach { setting ->
            runCatching {
                context.contentResolver.registerContentObserver(
                    Settings.Global.getUriFor(setting),
                    false,
                    observer,
                )
            }
        }
        evaluate()
    }

    fun stop() {
        runCatching { context.contentResolver.unregisterContentObserver(observer) }
    }

    /** Mevcut yapilandirmayi degerlendirir ve bulgulari olay olarak yayar. */
    fun evaluate() {
        val now = clock.nowMillis()

        if (globalFlag(Settings.Global.ADB_ENABLED)) {
            publish(EventType.ADB_ENABLED, now, "adb")
        }
        if (globalFlag(SETTING_ADB_WIFI_ENABLED)) {
            publish(EventType.WIRELESS_DEBUGGING_ENABLED, now, "adb_wifi")
        }
        if (secureFlag(Settings.Secure.INSTALL_NON_MARKET_APPS)) {
            publish(EventType.UNKNOWN_SOURCES_ENABLED, now, "unknown_sources")
        }
    }

    private fun globalFlag(key: String): Boolean = runCatching {
        Settings.Global.getInt(context.contentResolver, key, 0) == 1
    }.getOrDefault(false)

    @Suppress("DEPRECATION")
    private fun secureFlag(key: String): Boolean = runCatching {
        Settings.Secure.getInt(context.contentResolver, key, 0) == 1
    }.getOrDefault(false)

    private fun publish(type: EventType, nowMillis: Long, indicator: String) {
        eventBus.publish(
            SecurityEvent(
                timestampMillis = nowMillis,
                type = type,
                subject = Subject.System,
                source = SensorSource.SYSTEM_SETTINGS,
                attributes = mapOf(EventAttributes.INDICATOR to indicator),
            ),
        )
    }

    private companion object {
        /** AOSP'de `Settings.Global.ADB_WIFI_ENABLED` gizli bir sabittir. */
        const val SETTING_ADB_WIFI_ENABLED = "adb_wifi_enabled"

        val WATCHED_SETTINGS = listOf(
            Settings.Global.ADB_ENABLED,
            SETTING_ADB_WIFI_ENABLED,
        )
    }
}
