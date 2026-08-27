package com.ultraguard.shield.service

import android.app.Notification
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import androidx.core.app.NotificationCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import com.ultraguard.core.datastore.SettingsStore
import com.ultraguard.core.engine.MonitoringStateMachine
import com.ultraguard.core.engine.ThreatPipeline
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.sensors.AppOpsCollector
import com.ultraguard.core.sensors.PackageEventCollector
import com.ultraguard.core.sensors.ClipboardMonitor
import com.ultraguard.core.sensors.SystemSettingsCollector
import com.ultraguard.shield.response.ThreatResponder
import com.ultraguard.shield.work.WorkScheduler
import com.ultraguard.shield.MainActivity
import com.ultraguard.shield.R
import com.ultraguard.shield.UltraGuardApplication
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

/**
 * Koruma motorunun tasiyicisi.
 *
 * Neden on plan servisi: Android'de arka plan kisitlari, uzun sureli
 * dinleyicilerin oldurulmesine yol acar. Bir guvenlik urunu icin "bazen
 * calisiyor" kabul edilemez -- ya surekli calisir ya da kullanici korunmadigini
 * bilmelidir. Kalici bildirim bu yuzden gizlenemez ve gizlenmemelidir:
 * kullanici, izlendigini her zaman gorebilmelidir.
 */
@AndroidEntryPoint
class ProtectionService : LifecycleService() {

    @Inject lateinit var pipeline: ThreatPipeline
    @Inject lateinit var appOpsCollector: AppOpsCollector
    @Inject lateinit var packageCollector: PackageEventCollector
    @Inject lateinit var settingsCollector: SystemSettingsCollector
    @Inject lateinit var stateMachine: MonitoringStateMachine
    @Inject lateinit var settingsStore: SettingsStore
    @Inject lateinit var threatResponder: ThreatResponder
    @Inject lateinit var clipboardMonitor: ClipboardMonitor
    @Inject lateinit var batteryModeMonitor: BatteryModeMonitor
    @Inject lateinit var workScheduler: WorkScheduler

    override fun onCreate() {
        super.onCreate()
        startForeground(
            NOTIFICATION_ID,
            buildNotification(ProtectionMode.ACTIVE, watchedCount = 0),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE,
        )

        // Sira onemlidir: once karar zinciri ve yanit halkasi ayaga kalkar,
        // sonra sensorler olay uretmeye baslar. Tersi olursa acilistaki ilk
        // olaylar dinleyicisiz kalir ve sessizce kaybolur.
        pipeline.start()
        threatResponder.start()

        appOpsCollector.start()
        packageCollector.start()
        settingsCollector.start()
        clipboardMonitor.start()
        batteryModeMonitor.start()

        workScheduler.scheduleAll()
        observeMode()
    }

    private fun observeMode() {
        lifecycleScope.launch {
            settingsStore.settings.collectLatest { settings ->
                pipeline.setMode(settings.mode)
                updateNotification(
                    mode = settings.mode,
                    watchedCount = stateMachine.packagesUnderContainment().size,
                )
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        // Sistem servisi oldururse yeniden baslatilir: koruma bosluklu olamaz.
        return START_STICKY
    }

    override fun onDestroy() {
        appOpsCollector.stop()
        packageCollector.stop()
        settingsCollector.stop()
        clipboardMonitor.stop()
        batteryModeMonitor.stop()
        super.onDestroy()
    }

    private fun updateNotification(mode: ProtectionMode, watchedCount: Int) {
        val manager = getSystemService(android.app.NotificationManager::class.java)
        manager.notify(NOTIFICATION_ID, buildNotification(mode, watchedCount))
    }

    private fun buildNotification(mode: ProtectionMode, watchedCount: Int): Notification {
        val openApp = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        return NotificationCompat.Builder(this, UltraGuardApplication.CHANNEL_PROTECTION)
            .setContentTitle(getString(R.string.protection_active))
            .setContentText(
                getString(R.string.protection_active_detail, mode.name, watchedCount),
            )
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setContentIntent(openApp)
            .setOngoing(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .build()
    }

    companion object {
        private const val NOTIFICATION_ID = 1001

        fun start(context: android.content.Context) {
            context.startForegroundService(Intent(context, ProtectionService::class.java))
        }
    }
}
