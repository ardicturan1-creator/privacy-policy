package com.ultraguard.core.engine

import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent
import javax.inject.Inject
import javax.inject.Singleton

/**
 * L0 -- gurultu bastirma katmani.
 *
 * Ham telemetri saatte binlerce olay uretir ve bunun buyuk cogunlugu
 * tekrardir: ayni uygulama ayni saniye icinde konumu bes kez sorar, bir
 * bildirim uc kez guncellenir. Bu katman **olaylarin ~%97'sini eler** ve
 * geri kalan pahali kademelerin gercekten calisabilmesini saglar.
 *
 * Iki mekanizma:
 *  1. **Dedup penceresi** -- ayni (paket, tur, hedef) uclusu icin kisa sure
 *     icindeki tekrarlar tek olaya indirilir.
 *  2. **Sistem paketi filtresi** -- OEM sistem uygulamalarinin rutin
 *     davranisi izlenmez; onlar zaten platformun guven tabanindadir ve
 *     izlenmeleri yalnizca gurultu uretir.
 */
@Singleton
class EventNormalizer @Inject constructor(
    private val clock: Clock,
    private val systemPackageOracle: SystemPackageOracle,
) {
    private data class DedupKey(
        val packageName: String,
        val type: EventType,
        val target: String?,
    )

    private val lastSeen = LinkedHashMap<DedupKey, Long>(
        /* initialCapacity = */ 256,
        /* loadFactor = */ 0.75f,
        /* accessOrder = */ true,
    )

    @Synchronized
    fun normalize(event: SecurityEvent): SecurityEvent? {
        val packageName = event.packageName ?: return event // sistem olaylari her zaman gecer

        // Kendi olaylarimizi islemek sonsuz donguye yol acar.
        if (packageName == OWN_PACKAGE_PREFIX) return null

        if (systemPackageOracle.isTrustedSystemPackage(packageName) &&
            event.type !in ALWAYS_INSPECT
        ) {
            return null
        }

        val key = DedupKey(packageName, event.type, dedupTarget(event))
        val now = clock.elapsedRealtimeMillis()
        val previous = lastSeen[key]
        val window = dedupWindowFor(event.type)

        if (previous != null && now - previous < window) return null

        lastSeen[key] = now
        evictIfNeeded()
        return event
    }

    private fun dedupTarget(event: SecurityEvent): String? =
        event.attr(com.ultraguard.core.model.EventAttributes.TARGET_PACKAGE)
            ?: event.attr(com.ultraguard.core.model.EventAttributes.REMOTE_HOST)
            ?: event.attr(com.ultraguard.core.model.EventAttributes.PERMISSION)

    /**
     * Dedup penceresi olay turune gore degisir.
     *
     * Yuksek frekansli, dusuk bilgili olaylar (sensor yoklamalari) genis
     * pencerede toplanir; nadir ve yuksek bilgili olaylar (kurulum, imza
     * degisimi) **hic dedup edilmez** -- birini kacirmak kabul edilemez.
     */
    private fun dedupWindowFor(type: EventType): Long = when (type) {
        EventType.PACKAGE_INSTALLED,
        EventType.PACKAGE_REMOVED,
        EventType.PACKAGE_SIGNATURE_CHANGED,
        EventType.ACCESSIBILITY_SERVICE_ENABLED,
        EventType.ACCESSIBILITY_GESTURE_CAPABILITY,
        EventType.MEDIA_PROJECTION_STARTED,
        -> 0L

        EventType.SENSOR_LOCATION_ACCESS,
        EventType.SENSOR_CAMERA_ACCESS,
        EventType.SENSOR_MICROPHONE_ACCESS,
        -> 30_000L

        EventType.NETWORK_CONNECTION_OPENED,
        EventType.NETWORK_DNS_QUERY,
        -> 10_000L

        EventType.FOREGROUND_APP_CHANGED,
        EventType.NOTIFICATION_POSTED,
        -> 5_000L

        else -> 2_000L
    }

    private fun evictIfNeeded() {
        while (lastSeen.size > MAX_DEDUP_ENTRIES) {
            val oldest = lastSeen.keys.firstOrNull() ?: break
            lastSeen.remove(oldest)
        }
    }

    private companion object {
        const val MAX_DEDUP_ENTRIES = 512
        const val OWN_PACKAGE_PREFIX = "com.ultraguard.shield"

        /** Sistem paketi olsa bile her zaman incelenen olaylar. */
        val ALWAYS_INSPECT = setOf(
            EventType.PACKAGE_SIGNATURE_CHANGED,
            EventType.MEDIA_PROJECTION_STARTED,
            EventType.SELF_TAMPER_SUSPECTED,
        )
    }
}

/** Paketin platformun guven tabaninda olup olmadigini soyler. */
interface SystemPackageOracle {
    fun isTrustedSystemPackage(packageName: String): Boolean
}
