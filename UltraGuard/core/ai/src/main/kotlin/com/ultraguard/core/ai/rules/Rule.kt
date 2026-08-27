package com.ultraguard.core.ai.rules

import com.ultraguard.core.model.Attribution
import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.ThreatClass

/**
 * L1 deterministik kurali.
 *
 * L1'in isi hizli ve **kesin** olmaktir. Buradaki her kural, bilinen bir
 * saldiri kalibinin imzasidir ve yanlis pozitif orani neredeyse sifir olacak
 * sekilde daraltilmistir. Belirsiz vakalar L1'de eslesmez; onlar L2 dizi
 * modelinin isidir.
 *
 * Kurallar OTA imzali paket olarak guncellenir — yeni bir saldiri kalibi
 * icin uygulama guncellemesi beklenmez.
 */
abstract class Rule {
    /** Kararli kimlik. Action Ledger ve kullanici geri bildirimi buna baglanir. */
    abstract val id: String

    abstract val threatClass: ThreatClass

    /** Kural eslestiginde uretilen taban skor. */
    abstract val baseScore: RiskScore

    /** Bu kuralin degerlendirilebilmesi icin gereken cihaz yetkisi. */
    open val requiredCapability: Capability = Capability.UNROOTED

    /**
     * Kuralin ilgilendigi olay turleri. Motor bunu on-filtre olarak kullanir:
     * penceredeki hicbir olay bu kumeye girmiyorsa kural hic calistirilmaz.
     */
    abstract val interestedIn: Set<EventType>

    /** `true` donerse kural eslesmistir; kanitlar [RuleContext] uzerinden toplanir. */
    abstract fun RuleContext.evaluate(): Boolean

    internal fun apply(window: CorrelationWindow): RuleMatch? {
        if (window.events.none { it.type in interestedIn }) return null
        val context = RuleContext(window)
        val matched = with(this) { context.evaluate() }
        if (!matched) return null
        val evidence = context.normalizedEvidence()
        if (evidence.isEmpty()) return null
        return RuleMatch(
            ruleId = id,
            threatClass = threatClass,
            score = context.adjustedScore ?: baseScore,
            attributions = evidence,
            matchedEventIds = context.matchedEventIds(),
        )
    }
}

data class RuleMatch(
    val ruleId: String,
    val threatClass: ThreatClass,
    val score: RiskScore,
    val attributions: List<Attribution>,
    val matchedEventIds: List<Long>,
)

/**
 * Kural govdesinin calistigi baglam.
 *
 * Kritik tasarim: kanit toplama, kosul kontrolunun **yan urunudur**. Bir
 * kural `need(...)` cagirdiginda hem kosulu sinar hem de eslesen olayi
 * attribution olarak kaydeder. Boylece "kural eslesti ama neden eslestigini
 * aciklayamiyoruz" durumu yapisal olarak olusamaz.
 */
class RuleContext(val window: CorrelationWindow) {
    private val evidence = mutableListOf<Pair<SecurityEvent, Float>>()

    /** Kural, taban skoru baglama gore yukseltmek isterse. */
    var adjustedScore: RiskScore? = null
        private set

    /**
     * Zorunlu kosul. Eslesmezse kanitlar temizlenir ve kural basarisiz olur.
     * @param weight bu kanitin karardaki goreli agirligi (normalize edilir).
     */
    fun need(
        type: EventType,
        weight: Float,
        where: (SecurityEvent) -> Boolean = { true },
    ): Boolean {
        val event = window.latest(type, where)
        if (event == null) {
            evidence.clear()
            return false
        }
        evidence += event to weight
        return true
    }

    /** Kosullardan en az birinin saglanmasi yeterli. */
    fun needAnyOf(
        weight: Float,
        vararg types: EventType,
    ): Boolean {
        val event = types.firstNotNullOfOrNull { window.latest(it) }
        if (event == null) {
            evidence.clear()
            return false
        }
        evidence += event to weight
        return true
    }

    /** Sirali kosul: [first] olayindan sonra [millis] icinde [second]. */
    fun needSequence(
        first: EventType,
        second: EventType,
        withinMillis: Long,
        weight: Float,
    ): Boolean {
        val pair = window.sequence(first, second, withinMillis)
        if (pair == null) {
            evidence.clear()
            return false
        }
        evidence += pair.first to weight / 2f
        evidence += pair.second to weight / 2f
        return true
    }

    /** Varsa kanita eklenir, yoksa kural yine de gecerlidir. */
    fun optional(
        type: EventType,
        weight: Float,
        where: (SecurityEvent) -> Boolean = { true },
    ) {
        window.latest(type, where)?.let { evidence += it to weight }
    }

    /** Baglamsal siddetlendirme — or. hedef bir bankacilik uygulamasiysa. */
    fun escalateTo(score: RiskScore) {
        adjustedScore = score
    }

    internal fun matchedEventIds(): List<Long> = evidence.map { it.first.id }.distinct()

    /** Agirliklari 1.0'a normalize eder; UI'daki cubuk grafik buna dayanir. */
    internal fun normalizedEvidence(): List<Attribution> {
        val total = evidence.sumOf { it.second.toDouble() }.toFloat()
        if (total <= 0f) return emptyList()
        return evidence
            .groupBy { it.first.id }
            .map { (_, group) ->
                val event = group.first().first
                val weight = group.sumOf { it.second.toDouble() }.toFloat() / total
                Attribution(
                    eventId = event.id,
                    eventType = event.type,
                    weight = weight.coerceIn(0f, 1f),
                    explanationKey = ExplanationKeys.of(event.type),
                    explanationArgs = ExplanationKeys.argsOf(event),
                )
            }
            .sortedByDescending { it.weight }
    }
}
