package com.ultraguard.core.ai.rules

import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.DecisionTier
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.model.Verdict

/**
 * L1 kural motoru.
 *
 * Karar zincirinin ilk ve en ucuz kademesi. Tum kurallari pencereye uygular
 * ve en yuksek skorlu eslesmeyi hukum olarak dondurur. Hedef butce:
 * pencere basina **1 ms alti**.
 */
class RuleEngine(
    private val rules: List<Rule>,
    /** Cihazin sahip oldugu yetkiler; ustundeki kurallar hic degerlendirilmez. */
    private val availableCapabilities: Set<Capability>,
) {
    private val applicableRules: List<Rule> =
        rules.filter { it.requiredCapability in availableCapabilities }

    /**
     * @return eslesme varsa hukum, yoksa `null`. `null` "temiz" anlamina
     *   gelmez — yalnizca "bilinen bir kalibi eslesmedi" demektir. Karar
     *   L2'ye devredilir.
     */
    fun evaluate(window: CorrelationWindow, nowMillis: Long): Verdict? {
        if (window.isEmpty) return null

        val matches = applicableRules.mapNotNull { it.apply(window) }
        if (matches.isEmpty()) return null

        // En yuksek skorlu eslesme hukmu belirler; digerleri kanit olarak
        // birlestirilir, boylece "uc ayri kural ayni anda tetiklendi" bilgisi
        // kullaniciya kaybolmadan ulasir.
        val primary = matches.maxBy { it.score.value }
        val corroborating = matches.filter { it.ruleId != primary.ruleId }

        val score = fuseCorroboration(primary.score, corroborating.size)

        return Verdict(
            packageName = window.packageName,
            createdAtMillis = nowMillis,
            tier = DecisionTier.RULE_ENGINE,
            score = score,
            threatClass = primary.threatClass,
            // L1 deterministiktir: eslestiyse eminiz. Belirsizlik L2'nin isidir.
            confidence = 1.0f,
            attributions = primary.attributions,
            originId = primary.ruleId,
            correlationWindowEventIds = matches.flatMap { it.matchedEventIds }.distinct(),
        )
    }

    /**
     * Birden fazla bagimsiz kuralin ayni pakette tetiklenmesi, tek bir
     * kuraldan daha guclu bir sinyaldir; ancak katkisi azalan verimlidir —
     * ucuncu kural ikinciden daha az sey soyler.
     */
    private fun fuseCorroboration(base: RiskScore, corroboratingCount: Int): RiskScore {
        if (corroboratingCount == 0) return base
        val bonus = (1..corroboratingCount).sumOf { 6.0 / it }.toInt()
        return RiskScore((base.value + bonus).coerceAtMost(100))
    }

    fun ruleCount(): Int = applicableRules.size

    fun threatClassesCovered(): Set<ThreatClass> = applicableRules.map { it.threatClass }.toSet()
}
