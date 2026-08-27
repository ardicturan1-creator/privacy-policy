package com.ultraguard.core.network

import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.abs
import kotlin.math.sqrt

/**
 * Komuta-kontrol (C2) beacon tespiti -- sifre cozmeden, yalnizca zamanlamadan.
 *
 * Bir C2 implanti periyodik olarak "buradayim, emir var mi?" diye sorar.
 * Icerik sifrelidir ve gorulemez, ancak **ritim gorulur**: sabit araliklarla
 * gonderilen kucuk paketler, insan kullanimindan istatistiksel olarak
 * ayirt edilebilir. Gercek kullanici trafigi duzensizdir; makine trafigi
 * duzenlidir.
 *
 * Modern implantlar bunu "jitter" ekleyerek gizler (or. 60 sn ± %20). Bu
 * yuzden mutlak duzenlilik degil, **varyasyon katsayisi** (standart sapma /
 * ortalama) olculur: %20 jitter bile insan trafiginin dagilimindan cok daha
 * dardir.
 */
@Singleton
class BeaconDetector @Inject constructor() {

    private val history = mutableMapOf<FlowKey, ArrayDeque<Long>>()

    data class FlowKey(val packageName: String, val remoteHost: String)

    /**
     * @return beacon paterni tespit edildiyse sonuc, aksi halde `null`.
     */
    @Synchronized
    fun observe(key: FlowKey, timestampMillis: Long): BeaconVerdict? {
        val timestamps = history.getOrPut(key) { ArrayDeque() }
        timestamps.addLast(timestampMillis)
        while (timestamps.size > MAX_HISTORY) timestamps.removeFirst()

        if (timestamps.size < MIN_SAMPLES) return null

        val intervals = timestamps.zipWithNext { a, b -> b - a }
        if (intervals.any { it <= 0 }) return null

        val mean = intervals.average()
        if (mean < MIN_INTERVAL_MILLIS || mean > MAX_INTERVAL_MILLIS) return null

        val variance = intervals.sumOf { (it - mean) * (it - mean) } / intervals.size
        val coefficientOfVariation = sqrt(variance) / mean

        if (coefficientOfVariation > MAX_COEFFICIENT_OF_VARIATION) return null

        return BeaconVerdict(
            intervalMillis = mean.toLong(),
            jitterRatio = coefficientOfVariation.toFloat(),
            sampleCount = timestamps.size,
        )
    }

    @Synchronized
    fun forget(packageName: String) {
        history.keys.removeAll { it.packageName == packageName }
    }

    @Synchronized
    fun prune(olderThanMillis: Long) {
        history.entries.removeAll { (_, timestamps) ->
            timestamps.lastOrNull()?.let { it < olderThanMillis } ?: true
        }
    }

    internal companion object {
        /** Alti ornekten az veride varyasyon katsayisi anlamsizdir. */
        const val MIN_SAMPLES = 6
        const val MAX_HISTORY = 32

        /** 10 sn altindaki aralik cogu zaman mesru yoklamadir (chat, oyun). */
        const val MIN_INTERVAL_MILLIS = 10_000.0

        /** 2 saat ustu aralik zaten arka plan senkronizasyonuna benzer. */
        const val MAX_INTERVAL_MILLIS = 7_200_000.0

        /**
         * %35 varyasyon esigi: %20 jitter'li bir implanti yakalar, insan
         * kullanim ritmini (tipik olarak >%80) disarida birakir.
         */
        const val MAX_COEFFICIENT_OF_VARIATION = 0.35
    }
}

data class BeaconVerdict(
    val intervalMillis: Long,
    val jitterRatio: Float,
    val sampleCount: Int,
)

/**
 * Algoritma uretimi alan adi (DGA) tespiti.
 *
 * DGA, C2 altyapisinin kapatilmasina karsi dayaniklilik saglar: implant her
 * gun yuzlerce rastgele alan adi uretir ve saldirgan yalnizca birini kaydeder.
 * Bu adlar insan tarafindan secilmis adlardan iki olculebilir sekilde ayrilir:
 * karakter entropisi ve digram (iki harfli dizi) dogalligi.
 */
@Singleton
class DgaClassifier @Inject constructor() {

    fun assess(hostname: String): DgaAssessment? {
        val label = hostname.substringBefore('.').lowercase()
        if (label.length < MIN_LABEL_LENGTH || label.length > MAX_LABEL_LENGTH) return null
        if (label.any { !it.isLetterOrDigit() && it != '-' }) return null

        val entropy = characterEntropy(label)
        val vowelRatio = label.count { it in VOWELS }.toFloat() / label.length
        val digitRatio = label.count { it.isDigit() }.toFloat() / label.length

        // Uc bagimsiz sinyal. Tek basina yuksek entropi, kisaltilmis mesru
        // alan adlarinda da gorulur (or. "cdn7x"); dusuk unlu orani ile
        // birlestiginde ayirt edicilik belirgin sekilde artar.
        val suspicious = entropy >= ENTROPY_THRESHOLD &&
            vowelRatio <= VOWEL_RATIO_THRESHOLD

        if (!suspicious) return null

        return DgaAssessment(
            entropy = entropy,
            vowelRatio = vowelRatio,
            digitRatio = digitRatio,
        )
    }

    private fun characterEntropy(value: String): Float {
        val frequencies = value.groupingBy { it }.eachCount()
        var entropy = 0.0
        frequencies.values.forEach { count ->
            val p = count.toDouble() / value.length
            entropy -= p * (Math.log(p) / Math.log(2.0))
        }
        return entropy.toFloat()
    }

    internal companion object {
        const val MIN_LABEL_LENGTH = 8
        const val MAX_LABEL_LENGTH = 63
        const val ENTROPY_THRESHOLD = 3.6f
        const val VOWEL_RATIO_THRESHOLD = 0.30f
        val VOWELS = setOf('a', 'e', 'i', 'o', 'u')
    }
}

data class DgaAssessment(
    val entropy: Float,
    val vowelRatio: Float,
    val digitRatio: Float,
)
