package com.ultraguard.shield.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import com.ultraguard.core.common.di.ApplicationScope
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.datastore.SettingsStore
import com.ultraguard.core.engine.ThreatPipeline
import com.ultraguard.core.model.ProtectionMode
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * Pil seviyesine gore koruma modunu ayarlar.
 *
 * Esik altina inildiginde [ProtectionMode.BATTERY_GUARD] devreye girer:
 * pahali L2 dizi modeli askiya alinir, kural motoru ve ag kalkani calismaya
 * devam eder. **Koruma azalir ama asla sifirlanmaz** -- ve bu durum ana
 * ekranda acikca gosterilir.
 *
 * Kullanicinin sectigi mod hatirlanir: pil toparlandiginda eski moda geri
 * donulur, kullanicinin tercihi kalici olarak ezilmez.
 */
@Singleton
class BatteryModeMonitor @Inject constructor(
    @ApplicationContext private val context: Context,
    private val settingsStore: SettingsStore,
    private val pipeline: ThreatPipeline,
    @ApplicationScope private val scope: CoroutineScope,
) {
    /** Pil koruma moduna gecmeden onceki kullanici tercihi. */
    @Volatile
    private var modeBeforeBatteryGuard: ProtectionMode? = null

    @Volatile
    private var registered = false

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(receiverContext: Context?, intent: Intent?) {
            val level = intent?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: return
            val scale = intent.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
            if (level < 0 || scale <= 0) return

            val percent = level * 100 / scale
            val charging = intent.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
                .let { it == BatteryManager.BATTERY_STATUS_CHARGING || it == BatteryManager.BATTERY_STATUS_FULL }

            scope.launch { evaluate(percent, charging) }
        }
    }

    @Synchronized
    fun start() {
        if (registered) return
        context.registerReceiver(
            receiver,
            IntentFilter(Intent.ACTION_BATTERY_CHANGED),
            Context.RECEIVER_NOT_EXPORTED,
        )
        registered = true
    }

    @Synchronized
    fun stop() {
        if (!registered) return
        runCatching { context.unregisterReceiver(receiver) }
        registered = false
    }

    private suspend fun evaluate(percent: Int, charging: Boolean) {
        val current = settingsStore.settings.first().mode

        // Sarjdayken pil koruma moduna gerek yok.
        val shouldGuard = !charging && percent <= LOW_BATTERY_THRESHOLD

        when {
            shouldGuard && current != ProtectionMode.BATTERY_GUARD -> {
                modeBeforeBatteryGuard = current
                settingsStore.setMode(ProtectionMode.BATTERY_GUARD)
                pipeline.setMode(ProtectionMode.BATTERY_GUARD)
                UgLog.i(TAG, "Pil %$percent: koruma modu BATTERY_GUARD")
            }

            !shouldGuard && current == ProtectionMode.BATTERY_GUARD &&
                percent >= RECOVERY_THRESHOLD -> {
                // Histerezis: %15'te girip %15'te cikmak, pil seviyesi esik
                // civarinda salinirken mod degisimini surekli tetikler.
                val restored = modeBeforeBatteryGuard ?: ProtectionMode.ACTIVE
                modeBeforeBatteryGuard = null
                settingsStore.setMode(restored)
                pipeline.setMode(restored)
                UgLog.i(TAG, "Pil %$percent: koruma modu $restored olarak geri yuklendi")
            }
        }
    }

    private companion object {
        const val TAG = "BatteryModeMonitor"
        const val LOW_BATTERY_THRESHOLD = 15
        const val RECOVERY_THRESHOLD = 25
    }
}
