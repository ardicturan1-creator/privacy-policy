package com.ultraguard.core.sensors

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import com.ultraguard.core.common.di.ApplicationScope
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.engine.EventBus
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.Subject
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Paket yasam dongusu telemetrisi.
 *
 * Kurulum, guvenlik acisindan cihazin en kirilgan anidir: yeni kod, henuz
 * hakkinda hicbir davranissal gecmis olmadan sisteme girer. Bu toplayici
 * o ani yakalar ve hemen statik triyaji tetikler.
 */
@Singleton
class PackageEventCollector @Inject constructor(
    @ApplicationContext private val context: Context,
    private val eventBus: EventBus,
    private val staticTriage: StaticApkTriage,
    private val clock: Clock,
    @ApplicationScope private val scope: CoroutineScope,
) {
    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(receiverContext: Context?, intent: Intent?) {
            val action = intent?.action ?: return
            val packageName = intent.data?.schemeSpecificPart ?: return
            val replacing = intent.getBooleanExtra(Intent.EXTRA_REPLACING, false)
            val uid = intent.getIntExtra(Intent.EXTRA_UID, -1)
            val now = clock.nowMillis()

            val type = when {
                action == Intent.ACTION_PACKAGE_ADDED && replacing -> EventType.PACKAGE_UPDATED
                action == Intent.ACTION_PACKAGE_ADDED -> EventType.PACKAGE_INSTALLED
                action == Intent.ACTION_PACKAGE_FULLY_REMOVED -> EventType.PACKAGE_REMOVED
                else -> return
            }

            eventBus.publish(
                SecurityEvent(
                    timestampMillis = now,
                    type = type,
                    subject = Subject.App(packageName, uid),
                    source = SensorSource.PACKAGE_LIFECYCLE,
                ),
            )

            // Kaldirilan paket icin triyaj yapilamaz (APK artik yok).
            if (type == EventType.PACKAGE_REMOVED) return

            // Statik triyaj IO yapar; yayin alicisinin 10 saniyelik zaman
            // siniri icinde bitmeyebilir, bu yuzden uygulama scope'una tasinir.
            scope.launch {
                eventBus.publishAll(staticTriage.analyze(packageName, now))
            }
        }
    }

    @Volatile
    private var registered = false

    @Synchronized
    fun start() {
        if (registered) return
        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_PACKAGE_ADDED)
            addAction(Intent.ACTION_PACKAGE_FULLY_REMOVED)
            addDataScheme("package")
        }
        // Paket yayinlari sistemden gelir; `RECEIVER_NOT_EXPORTED` Android 13+
        // uzerinde zorunludur ve kayitli aliciyi diger uygulamalara kapatir.
        context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        registered = true
    }

    @Synchronized
    fun stop() {
        if (!registered) return
        runCatching { context.unregisterReceiver(receiver) }
        registered = false
    }
}
