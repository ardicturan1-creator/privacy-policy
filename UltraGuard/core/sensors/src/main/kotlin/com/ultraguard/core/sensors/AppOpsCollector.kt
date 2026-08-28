package com.ultraguard.core.sensors

import android.app.AppOpsManager
import android.content.Context
import android.os.PowerManager
import com.ultraguard.core.common.log.UgLog
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
 * Sensor erisim telemetrisi -- kamera, mikrofon, konum, pano.
 *
 * `AppOpsManager.startWatchingMode`, hangi uygulamanin **hangi anda** bir
 * yetenegi kullandigini olay olarak bildirir. Bu, izin listesine bakmaktan
 * temelde farklidir: izin "yapabilir" der, AppOps "yapti" der.
 *
 * Kritik ayirt edici, olayin baglamidir: ekran kapaliyken ve uygulama arka
 * plandayken alinan bir mikrofon erisimi ile kullanicinin acikca baslattigi
 * bir ses kaydi, ayni izin altinda gerceklesir ama tamamen farkli seylerdir.
 */
@Singleton
class AppOpsCollector @Inject constructor(
    @ApplicationContext private val context: Context,
    private val eventBus: EventBus,
    private val clock: Clock,
) {
    private val appOpsManager = context.getSystemService(AppOpsManager::class.java)
    private val powerManager = context.getSystemService(PowerManager::class.java)
    private val watchers = mutableListOf<AppOpsManager.OnOpChangedListener>()

    @Synchronized
    fun start() {
        if (watchers.isNotEmpty()) return

        WATCHED_OPS.forEach { (op, eventType) ->
            val listener = AppOpsManager.OnOpChangedListener { changedOp, packageName ->
                if (packageName.isNullOrEmpty()) return@OnOpChangedListener
                onOpChanged(changedOp, packageName, eventType)
            }
            runCatching {
                appOpsManager.startWatchingMode(op, null, listener)
                watchers += listener
            }.onFailure { error ->
                // Bazi op'lar sistem izni ister. Basarisiz olan tek bir op
                // digerlerini etkilemez; eksik sinyali kaydeder ve devam ederiz.
                UgLog.w(TAG, "AppOps izlenemedi: $op", error)
            }
        }
    }

    @Synchronized
    fun stop() {
        watchers.forEach { runCatching { appOpsManager.stopWatchingMode(it) } }
        watchers.clear()
    }

    private fun onOpChanged(op: String, packageName: String, eventType: EventType) {
        val now = clock.nowMillis()
        val screenOn = powerManager?.isInteractive == true
        val foreground = isForeground(packageName)

        eventBus.publish(
            SecurityEvent(
                timestampMillis = now,
                type = eventType,
                subject = Subject.App(packageName, uidOf(packageName)),
                source = SensorSource.APP_OPS,
                attributes = mapOf(
                    EventAttributes.SCREEN_ON to screenOn.toString(),
                    EventAttributes.FOREGROUND to foreground.toString(),
                    EventAttributes.PERMISSION to op,
                ),
            ),
        )

        // Arka planda ve ekran kapaliyken gerceklesen sensor erisimi ayrica
        // isaretlenir; kural motoru bunu dogrudan sorgular.
        if (!screenOn && !foreground) {
            eventBus.publish(
                SecurityEvent(
                    timestampMillis = now,
                    type = EventType.SENSOR_BACKGROUND_ACCESS,
                    subject = Subject.App(packageName, uidOf(packageName)),
                    source = SensorSource.APP_OPS,
                    attributes = mapOf(
                        EventAttributes.PERMISSION to op,
                        EventAttributes.SCREEN_ON to "false",
                        EventAttributes.FOREGROUND to "false",
                    ),
                ),
            )
        }
    }

    private fun isForeground(packageName: String): Boolean = runCatching {
        val activityManager = context.getSystemService(android.app.ActivityManager::class.java)
        activityManager.runningAppProcesses?.any { process ->
            process.processName == packageName &&
                process.importance <= android.app.ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND
        } == true
    }.getOrDefault(false)

    private fun uidOf(packageName: String): Int = runCatching {
        context.packageManager.getApplicationInfo(packageName, 0).uid
    }.getOrDefault(-1)

    private companion object {
        const val TAG = "AppOpsCollector"

        /**
         * Pano ve ekran yakalama op'lari AOSP'de tanimlidir ancak sabitleri
         * `@hide` isaretlidir; SDK'da gorunmezler. Yansima ile erismek
         * kirilgan oldugu icin (adlar surumler arasi degisebilir) op
         * dizeleri dogrudan yazilir. Bu adlar AOSP'de Android 10'dan beri
         * degismemistir; `startWatchingMode` taninmayan bir op icin sessizce
         * basarisiz olur ve ilgili sinyal yalnizca eksik kalir -- calisma
         * zamaninda cokme uretmez.
         */
        const val OPSTR_READ_CLIPBOARD = "android:read_clipboard"
        const val OPSTR_PROJECT_MEDIA = "android:project_media"

        val WATCHED_OPS: List<Pair<String, EventType>> = listOf(
            AppOpsManager.OPSTR_CAMERA to EventType.SENSOR_CAMERA_ACCESS,
            AppOpsManager.OPSTR_RECORD_AUDIO to EventType.SENSOR_MICROPHONE_ACCESS,
            AppOpsManager.OPSTR_FINE_LOCATION to EventType.SENSOR_LOCATION_ACCESS,
            AppOpsManager.OPSTR_COARSE_LOCATION to EventType.SENSOR_LOCATION_ACCESS,
            AppOpsManager.OPSTR_SYSTEM_ALERT_WINDOW to EventType.OVERLAY_DRAWN,
            OPSTR_READ_CLIPBOARD to EventType.CLIPBOARD_READ,
            OPSTR_PROJECT_MEDIA to EventType.MEDIA_PROJECTION_STARTED,
        )
    }
}
