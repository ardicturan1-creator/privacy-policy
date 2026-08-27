package com.ultraguard.core.sensors

import android.content.ClipboardManager
import android.content.Context
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
 * Pano izleme -- kripto adresi degistirme (clipper) saldirilarina karsi.
 *
 * **Gizlilik sozlesmesi:** pano icerigi hicbir zaman kaydedilmez veya
 * gonderilmez. Metin [SensitivePatternMatcher] tarafindan RAM'de
 * siniflandirilir ve olaya yalnizca `crypto_address` gibi turetilmis bir
 * etiket yazilir.
 *
 * **Android 10+ kisiti:** arka plandaki bir uygulama panoyu okuyamaz --
 * bu, platformun kullaniciyi koruyan dogru bir kararidir ve UltraGuard da
 * bundan muaf degildir. Dolayisiyla bu izleyici yalnizca UltraGuard on
 * plandayken calisir. Asil clipper tespiti, kotucul paketin pano erisimini
 * AppOps uzerinden gormeye dayanir (bkz. R-CLIP-001); bu sinif o sinyale
 * "panoda hassas bir sey vardi" baglamini ekler.
 */
@Singleton
class ClipboardMonitor @Inject constructor(
    @ApplicationContext private val context: Context,
    private val eventBus: EventBus,
    private val patternMatcher: SensitivePatternMatcher,
    private val clock: Clock,
) {
    private val clipboardManager = context.getSystemService(ClipboardManager::class.java)

    private val listener = ClipboardManager.OnPrimaryClipChangedListener {
        runCatching { inspectClipboard() }.onFailure { error ->
            UgLog.d(TAG) { "Pano okunamadi (beklenen: arka planda kisitli): ${error.message}" }
        }
    }

    @Volatile
    private var registered = false

    @Synchronized
    fun start() {
        if (registered) return
        clipboardManager?.addPrimaryClipChangedListener(listener)
        registered = true
    }

    @Synchronized
    fun stop() {
        if (!registered) return
        runCatching { clipboardManager?.removePrimaryClipChangedListener(listener) }
        registered = false
    }

    private fun inspectClipboard() {
        val clip = clipboardManager?.primaryClip ?: return
        if (clip.itemCount == 0) return

        // Metin buradan sonra hicbir yere yazilmaz.
        val classification = (0 until clip.itemCount)
            .asSequence()
            .mapNotNull { index -> clip.getItemAt(index)?.text }
            .firstNotNullOfOrNull(patternMatcher::classify)
            ?: return

        eventBus.publish(
            SecurityEvent(
                timestampMillis = clock.nowMillis(),
                type = EventType.CLIPBOARD_SENSITIVE_CONTENT,
                subject = Subject.System,
                source = SensorSource.APP_OPS,
                attributes = mapOf(EventAttributes.MATCHED_PATTERN to classification),
            ),
        )
    }

    private companion object {
        const val TAG = "ClipboardMonitor"
    }
}
