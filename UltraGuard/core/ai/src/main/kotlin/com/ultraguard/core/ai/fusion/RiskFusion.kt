package com.ultraguard.core.ai.fusion

import com.ultraguard.core.ai.inference.InferenceResult
import com.ultraguard.core.model.DecisionTier
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.model.TrustOverride
import com.ultraguard.core.model.Verdict
import kotlin.math.roundToInt

/**
 * L1 ve L2 hukumlerini tek bir nihai hukme birlestirir.
 *
 * Fuzyon kurallari, urunun yanlis pozitif duruşunu kodlar:
 *
 *  1. **L1 eslesirse L1 kazanir.** Deterministik kural, olasilikci modelden
 *     ustundur. L2 yalnizca skoru yukseltebilir, dusuremez — modelin bilinen
 *     bir saldiri kalibini "sorun yok" diye ezmesine izin verilmez.
 *  2. **Yalniz L2 sinyali tavanlanir.** Kural destegi olmayan bir model
 *     hukmu 85'i asamaz; otonom kaldirma benzeri agir sonuclar icin tek
 *     basina model cikisi yeterli kabul edilmez.
 *  3. **Kullanici guveni skoru bastirir, susturmaz.** "Guveniyorum" denen
 *     bir uygulamanin skoru dusurulur ama izleme surer ve KRITIK bulgular
 *     yine gosterilir. Kullaniciyi kendi kararindan korumaya calismayiz,
 *     ama onu karanlikta da birakmayiz.
 */
class RiskFusion {

    fun fuse(
        packageName: String,
        ruleVerdict: Verdict?,
        modelResult: InferenceResult?,
        trustOverride: TrustOverride,
        nowMillis: Long,
    ): Verdict? {
        val fused = when {
            ruleVerdict != null && modelResult != null ->
                combineRuleAndModel(ruleVerdict, modelResult)

            ruleVerdict != null -> ruleVerdict

            modelResult != null -> modelOnly(packageName, modelResult, nowMillis)

            else -> null
        } ?: return null

        return applyTrustOverride(fused, trustOverride)
    }

    private fun combineRuleAndModel(rule: Verdict, model: InferenceResult): Verdict {
        // Model ayni tehdit sinifini dogruluyorsa bu bagimsiz bir teyittir.
        val agrees = model.threatClass == rule.threatClass
        val boost = when {
            agrees -> (model.confidence * 10f).roundToInt()
            model.score > rule.score -> ((model.score.value - rule.score.value) * 0.25f).roundToInt()
            else -> 0
        }

        return rule.copy(
            score = RiskScore((rule.score.value + boost).coerceIn(rule.score.value, 100)),
            confidence = if (agrees) 1.0f else rule.confidence,
            // Kanit birlesir: kuralin gosterdigi olaylar + modelin dikkat verdigi
            // olaylar. Ayni olay iki kez gecmez.
            attributions = mergeAttributions(rule, model),
            originId = if (agrees) "${rule.originId}+${model.modelVersion}" else rule.originId,
        )
    }

    private fun mergeAttributions(rule: Verdict, model: InferenceResult) =
        (rule.attributions + model.attributions)
            .groupBy { it.eventId }
            .map { (_, group) -> group.maxBy { it.weight } }
            .sortedByDescending { it.weight }
            .take(MAX_MERGED_ATTRIBUTIONS)

    private fun modelOnly(packageName: String, model: InferenceResult, nowMillis: Long): Verdict? {
        if (model.threatClass == ThreatClass.NONE && model.anomaly < ANOMALY_FLOOR) return null
        if (model.attributions.isEmpty()) return null

        val capped = RiskScore(model.score.value.coerceAtMost(MODEL_ONLY_CEILING))
        return Verdict(
            packageName = packageName,
            createdAtMillis = nowMillis,
            tier = DecisionTier.ON_DEVICE_MODEL,
            score = capped,
            threatClass = if (model.threatClass == ThreatClass.NONE) {
                ThreatClass.ANOMALOUS_BEHAVIOR
            } else {
                model.threatClass
            },
            confidence = model.confidence,
            attributions = model.attributions,
            originId = model.modelVersion,
        )
    }

    private fun applyTrustOverride(verdict: Verdict, override: TrustOverride): Verdict =
        when (override) {
            TrustOverride.NONE -> verdict

            TrustOverride.USER_TRUSTED -> verdict.copy(
                score = RiskScore((verdict.score.value - TRUSTED_DISCOUNT).coerceAtLeast(0)),
                tier = DecisionTier.USER_OVERRIDE,
            )

            TrustOverride.FALSE_POSITIVE_REPORTED -> verdict.copy(
                score = RiskScore((verdict.score.value - FALSE_POSITIVE_DISCOUNT).coerceAtLeast(0)),
                tier = DecisionTier.USER_OVERRIDE,
            )
        }

    private companion object {
        /** Kural destegi olmayan model hukmunun ust siniri. */
        const val MODEL_ONLY_CEILING = 85

        /** Bunun altindaki saf anomali sinyali gurultu kabul edilir. */
        const val ANOMALY_FLOOR = 0.72f

        const val TRUSTED_DISCOUNT = 25
        const val FALSE_POSITIVE_DISCOUNT = 40
        const val MAX_MERGED_ATTRIBUTIONS = 6
    }
}
