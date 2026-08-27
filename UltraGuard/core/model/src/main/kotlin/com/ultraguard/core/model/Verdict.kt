package com.ultraguard.core.model

import kotlinx.serialization.Serializable

/**
 * Bir karar katmaninin (L1 kural, L2 model veya L3 konsultasyon) urettigi hukum.
 *
 * Her hukum **aciklanabilir olmak zorundadir**: [attributions] bos birakilamaz.
 * Kara kutu bir skorun kullaniciya gosterilmesi UltraGuard'da mimari olarak
 * engellenmistir — bkz. [RiskFusion] ve `VerdictValidator`.
 */
@Serializable
data class Verdict(
    val id: Long = 0L,
    val packageName: String,
    val createdAtMillis: Long,
    val tier: DecisionTier,
    val score: RiskScore,
    val threatClass: ThreatClass,
    val confidence: Float,
    val attributions: List<Attribution>,
    /** L1 icin tetikleyen kural kimligi, L2 icin model surumu. */
    val originId: String,
    val correlationWindowEventIds: List<Long> = emptyList(),
) {
    init {
        require(confidence in 0f..1f) { "confidence 0..1 araliginda olmali: $confidence" }
        require(attributions.isNotEmpty()) {
            "Aciklanamayan hukum uretilemez (paket=$packageName, origin=$originId)"
        }
    }

    /** Kullaniciya gosterilecek en agirlikli uc kanit. */
    fun topEvidence(limit: Int = 3): List<Attribution> =
        attributions.sortedByDescending { it.weight }.take(limit)
}

/** Karar zincirindeki kademe. Maliyet ve gecikme yukari dogru artar. */
@Serializable
enum class DecisionTier {
    /** L1: deterministik kural motoru. <1 ms, sifir yanlis-negatif hedefi. */
    RULE_ENGINE,

    /** L2: cihaz uzeri dizi modeli. ~30 ms, sifirinci gun tespiti. */
    ON_DEVICE_MODEL,

    /** L3: bulut konsultasyonu. Varsayilan KAPALI, kullanici onayina bagli. */
    CLOUD_CONSULTATION,

    /** Kullanicinin acik karari — her seyin ustundedir. */
    USER_OVERRIDE,
}

/**
 * 0..100 arasi risk skoru. Deger tipidir; aritmetigi [RiskFusion] yapar,
 * cagri yerlerinde serbestce toplanip cikarilmaz.
 */
@JvmInline
@Serializable
value class RiskScore(val value: Int) : Comparable<RiskScore> {
    init {
        require(value in 0..100) { "Risk skoru 0..100 olmali: $value" }
    }

    val band: RiskBand
        get() = when (value) {
            in 0..24 -> RiskBand.MINIMAL
            in 25..49 -> RiskBand.LOW
            in 50..74 -> RiskBand.ELEVATED
            in 75..89 -> RiskBand.HIGH
            else -> RiskBand.CRITICAL
        }

    override fun compareTo(other: RiskScore): Int = value.compareTo(other.value)

    companion object {
        val ZERO = RiskScore(0)
        fun fromProbability(p: Float): RiskScore = RiskScore((p.coerceIn(0f, 1f) * 100).toInt())
    }
}

@Serializable
enum class RiskBand { MINIMAL, LOW, ELEVATED, HIGH, CRITICAL }

/**
 * Tehdit sinifi. L1 kurallari bunu dogrudan atar; L2 modeli bir siniflandirma
 * basligi olarak uretir.
 */
@Serializable
enum class ThreatClass {
    NONE,
    BANKING_OVERLAY_TROJAN,
    ACCESSIBILITY_ABUSE,
    STALKERWARE,
    SPYWARE_GENERIC,
    ADWARE_AGGRESSIVE,
    DROPPER,
    FILELESS_LOADER,
    CREDENTIAL_PHISHING,
    SMS_FRAUD,
    CRYPTO_CLIPPER,
    C2_BEACON,
    DATA_EXFILTRATION,
    ROOT_EXPLOIT_ATTEMPT,
    POLICY_VIOLATION,
    ANOMALOUS_BEHAVIOR,
}

/**
 * Bir hukmun neden verildigini olay duzeyinde aciklayan kanit parcasi.
 * [weight] degerlerinin toplami 1.0'a normalize edilir.
 */
@Serializable
data class Attribution(
    val eventId: Long,
    val eventType: EventType,
    val weight: Float,
    /** Kullaniciya gosterilecek sade dil aciklamasi anahtar/argumanlari. */
    val explanationKey: String,
    val explanationArgs: List<String> = emptyList(),
) {
    init {
        require(weight in 0f..1f) { "attribution agirligi 0..1 olmali: $weight" }
    }
}
