package com.ultraguard.core.ai.inference

import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent
import kotlin.math.ln
import kotlin.math.min

/**
 * Olay dizisini L2 modelinin bekledigi tensore cevirir.
 *
 * Temsil hakkinda iki karar:
 *  1. **Mutlak zaman yok, delta var.** Model "sabah 3'te" degil "onceki
 *     olaydan 40 ms sonra" bilgisini ogrenir; boylece kullanicinin gunluk
 *     ritmine degil, saldirinin ritmine duyarli olur.
 *  2. **Paket adi yok.** Girdi olarak paket kimligi verilmez. Model bir
 *     uygulamayi adindan taniyamaz, yalnizca ne yaptigindan degerlendirir.
 *     Bu, yeniden paketlemeyi ve marka taklidini etkisiz kilar.
 */
class EventVectorizer {

    fun vectorize(events: List<SecurityEvent>, nowMillis: Long): FloatArray {
        val tensor = FloatArray(SEQUENCE_LENGTH * FEATURE_COUNT)

        // Pencereden en yeni SEQUENCE_LENGTH olay alinir, sona hizalanir.
        // Kisa diziler basta sifir-dolgu ile kalir (causal maskeleme icin).
        val window = events.takeLast(SEQUENCE_LENGTH)
        val offset = SEQUENCE_LENGTH - window.size

        var previousTimestamp = window.firstOrNull()?.timestampMillis ?: nowMillis

        window.forEachIndexed { index, event ->
            val base = (offset + index) * FEATURE_COUNT
            val delta = (event.timestampMillis - previousTimestamp).coerceAtLeast(0L)
            previousTimestamp = event.timestampMillis

            tensor[base + F_EVENT_TYPE] = event.type.ordinal.toFloat() / EventType.CARDINALITY
            tensor[base + F_DOMAIN] = event.type.domain.ordinal.toFloat() / DOMAIN_CARDINALITY
            tensor[base + F_SOURCE] = event.source.ordinal.toFloat() / SOURCE_CARDINALITY
            tensor[base + F_TIME_DELTA] = normalizeLog(delta.toFloat(), maxLog = LOG_ONE_HOUR)
            tensor[base + F_FOREGROUND] = event.attrBool(EventAttributes.FOREGROUND).toFloat()
            tensor[base + F_SCREEN_ON] = event.attrBool(EventAttributes.SCREEN_ON).toFloat()
            tensor[base + F_BYTES_OUT] =
                normalizeLog((event.attrLong(EventAttributes.BYTES_OUT) ?: 0L).toFloat(), LOG_TEN_MB)
            tensor[base + F_BYTES_IN] =
                normalizeLog((event.attrLong(EventAttributes.BYTES_IN) ?: 0L).toFloat(), LOG_TEN_MB)
            tensor[base + F_DOMAIN_ENTROPY] =
                (event.attr(EventAttributes.DOMAIN_ENTROPY)?.toFloatOrNull() ?: 0f) / MAX_ENTROPY
            tensor[base + F_DEX_ENTROPY] =
                (event.attr(EventAttributes.DEX_ENTROPY)?.toFloatOrNull() ?: 0f) / MAX_ENTROPY
            tensor[base + F_REPUTATION] = reputationScore(event.attr(EventAttributes.REPUTATION_VERDICT))
            tensor[base + F_HAS_TARGET] = (event.attr(EventAttributes.TARGET_PACKAGE) != null).toFloat()
            tensor[base + F_PROTECTED_TARGET] =
                (event.attr(EventAttributes.PROTECTED_CATEGORY) == "financial").toFloat()
            tensor[base + F_GESTURE_CAPABLE] = event.attrBool(EventAttributes.CAN_PERFORM_GESTURES).toFloat()
            tensor[base + F_PRESENT] = 1f // dolgu adimlarini gercek olaylardan ayirir
        }
        return tensor
    }

    private fun normalizeLog(value: Float, maxLog: Float): Float =
        min(ln(1f + value.coerceAtLeast(0f)) / maxLog, 1f)

    private fun reputationScore(verdict: String?): Float = when (verdict) {
        "malicious" -> 1.0f
        "suspicious" -> 0.6f
        "unknown" -> 0.3f
        "clean" -> 0.0f
        else -> 0.3f
    }

    private fun Boolean.toFloat(): Float = if (this) 1f else 0f

    companion object {
        /** Modelin gordugu baglam uzunlugu. Egitimle birlikte degistirilmelidir. */
        const val SEQUENCE_LENGTH = 64
        const val FEATURE_COUNT = 15

        private const val F_EVENT_TYPE = 0
        private const val F_DOMAIN = 1
        private const val F_SOURCE = 2
        private const val F_TIME_DELTA = 3
        private const val F_FOREGROUND = 4
        private const val F_SCREEN_ON = 5
        private const val F_BYTES_OUT = 6
        private const val F_BYTES_IN = 7
        private const val F_DOMAIN_ENTROPY = 8
        private const val F_DEX_ENTROPY = 9
        private const val F_REPUTATION = 10
        private const val F_HAS_TARGET = 11
        private const val F_PROTECTED_TARGET = 12
        private const val F_GESTURE_CAPABLE = 13
        private const val F_PRESENT = 14

        private const val DOMAIN_CARDINALITY = 7f
        private const val SOURCE_CARDINALITY = 12f
        private const val MAX_ENTROPY = 8f
        private val LOG_ONE_HOUR = ln(1f + 3_600_000f)
        private val LOG_TEN_MB = ln(1f + 10_485_760f)
    }
}
