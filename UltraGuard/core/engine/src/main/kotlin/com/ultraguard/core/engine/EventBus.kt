package com.ultraguard.core.engine

import com.ultraguard.core.common.di.ApplicationScope
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.model.SecurityEvent
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/**
 * Sensorlerden korelasyon katmanina tek yonlu olay akisi.
 *
 * Tasarim kararlari:
 *  - **Sensorler birbirini tanimaz.** Hepsi yalnizca buraya yazar. Bir sensor
 *    cokerse veya izni geri alinirsa digerleri hicbir sey hissetmez.
 *  - **Geri basinc dusurmeyle cozulur, bloklamayla degil.** Bir sensor
 *    (ozellikle ag katmani) tuketiciden hizli uretebilir. Olay kaybetmek,
 *    sensoru bloklayip sistemi yavaslatmaktan iyidir; kayip sayilir ve
 *    orneklem oraninin dusurulmesi icin sinyal olarak kullanilir.
 */
@Singleton
class EventBus @Inject constructor(
    @ApplicationScope private val scope: CoroutineScope,
) {
    private val _events = MutableSharedFlow<SecurityEvent>(
        replay = 0,
        extraBufferCapacity = BUFFER_CAPACITY,
        onBufferOverflow = BufferOverflow.DROP_OLDEST,
    )

    val events: SharedFlow<SecurityEvent> = _events.asSharedFlow()

    @Volatile
    var droppedEventCount: Long = 0L
        private set

    /**
     * Sensorlerden cagrilir. Askiya alinmaz -- sensor geri cagrimlarinin cogu
     * (AccessibilityService, AppOpsManager) ana is parcaciginda calisir ve
     * orada bloklamak ANR uretir.
     */
    fun publish(event: SecurityEvent) {
        if (!_events.tryEmit(event)) {
            droppedEventCount++
            if (droppedEventCount % DROP_LOG_INTERVAL == 0L) {
                UgLog.w(TAG, "Olay veriyolu doygun; toplam dusen olay: $droppedEventCount")
            }
        }
    }

    fun publishAll(events: List<SecurityEvent>) = events.forEach(::publish)

    private companion object {
        const val TAG = "EventBus"
        const val BUFFER_CAPACITY = 512
        const val DROP_LOG_INTERVAL = 100L
    }
}
