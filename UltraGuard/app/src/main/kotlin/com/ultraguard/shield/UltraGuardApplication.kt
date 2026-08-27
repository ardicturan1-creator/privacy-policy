package com.ultraguard.shield

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import com.ultraguard.core.common.log.UgLog
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class UltraGuardApplication : Application(), Configuration.Provider {

    @Inject lateinit var workerFactory: HiltWorkerFactory

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder()
            .setWorkerFactory(workerFactory)
            .setMinimumLoggingLevel(android.util.Log.INFO)
            .build()

    override fun onCreate() {
        super.onCreate()
        UgLog.verboseEnabled = BuildConfig.DEBUG
        createNotificationChannels()
    }

    /**
     * Bildirim kanallari.
     *
     * Kanal ayrimi bilinclidir: koruma durumu bildirimi sessiz ve dusuk
     * onceliklidir (kullanici onu gormek zorunda degil), tehdit uyarilari
     * yuksek onceliklidir. Ikisini ayni kanalda toplamak, kullanicinin
     * ikisini birden susturmasina yol acar -- ve o an gercek uyariyi da
     * kaybederiz.
     */
    private fun createNotificationChannels() {
        val manager = getSystemService(NotificationManager::class.java)

        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_PROTECTION,
                getString(R.string.protection_service_channel_name),
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = getString(R.string.protection_service_channel_description)
                setShowBadge(false)
            },
        )

        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_THREATS,
                getString(R.string.threat_channel_name),
                NotificationManager.IMPORTANCE_HIGH,
            ).apply {
                description = getString(R.string.threat_channel_description)
                enableVibration(true)
            },
        )
    }

    companion object {
        const val CHANNEL_PROTECTION = "ultraguard_protection"
        const val CHANNEL_THREATS = "ultraguard_threats"
    }
}
