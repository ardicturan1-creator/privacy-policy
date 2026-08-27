package com.ultraguard.core.ai.inference

import android.content.Context
import com.ultraguard.core.model.Attribution
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.ThreatClass
import java.io.FileInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.channels.FileChannel
import org.tensorflow.lite.Interpreter

/**
 * L2 — cihaz uzeri davranis dizisi modeli.
 *
 * Model kucuk bir causal transformer'dir (~8-15M parametre, INT8 nicelenmis).
 * Iki basligi vardir:
 *  - **Anomali basligi**: dizinin perplexity'si. Yuksek deger, modelin daha
 *    once gormedigi bir davranis dizisi demektir — sifirinci gun tespitinin
 *    temeli budur.
 *  - **Siniflandirma basligi**: bilinen saldiri ailelerine olasilik dagilimi.
 *
 * Butce: <30 ms cikarim, <40 MB tepe RAM.
 */
class BehaviorSequenceModel(
    private val interpreter: Interpreter,
    private val vectorizer: EventVectorizer = EventVectorizer(),
    val modelVersion: String,
) : AutoCloseable {

    private val inputBuffer: ByteBuffer = ByteBuffer
        .allocateDirect(EventVectorizer.SEQUENCE_LENGTH * EventVectorizer.FEATURE_COUNT * Float.SIZE_BYTES)
        .order(ByteOrder.nativeOrder())

    private val anomalyOutput = Array(1) { FloatArray(1) }
    private val classOutput = Array(1) { FloatArray(THREAT_CLASSES.size) }
    private val attentionOutput = Array(1) { FloatArray(EventVectorizer.SEQUENCE_LENGTH) }

    /**
     * @param events pencerenin zaman sirali olaylari (en fazla son 64'u kullanilir).
     * @return risk skoru, tehdit sinifi ve **olay bazli attribution**.
     */
    fun infer(events: List<SecurityEvent>, nowMillis: Long): InferenceResult {
        require(events.isNotEmpty()) { "Bos pencere modele verilemez" }

        val tensor = vectorizer.vectorize(events, nowMillis)
        inputBuffer.rewind()
        tensor.forEach(inputBuffer::putFloat)
        inputBuffer.rewind()

        interpreter.runForMultipleInputsOutputs(
            arrayOf<Any>(inputBuffer),
            mapOf(
                OUTPUT_ANOMALY to anomalyOutput,
                OUTPUT_CLASS to classOutput,
                OUTPUT_ATTENTION to attentionOutput,
            ),
        )

        val anomaly = anomalyOutput[0][0].coerceIn(0f, 1f)
        val probabilities = classOutput[0]
        val bestIndex = probabilities.indices.maxBy { probabilities[it] }
        val confidence = probabilities[bestIndex].coerceIn(0f, 1f)
        val threatClass = THREAT_CLASSES[bestIndex]

        // Nihai skor: anomali ve siniflandirma birbirini dogruluyorsa yukselir.
        // Yalnizca anomali yuksekse (bilinmeyen ama tuhaf davranis) skor
        // kasitli olarak orta bantta kalir — kullaniciyi tanimadigimiz bir sey
        // icin panige surukleyemeyiz.
        val fused = if (threatClass == ThreatClass.NONE) {
            anomaly * 0.55f
        } else {
            (anomaly * 0.4f + confidence * 0.6f)
        }

        return InferenceResult(
            score = RiskScore.fromProbability(fused),
            threatClass = threatClass,
            confidence = confidence,
            anomaly = anomaly,
            attributions = buildAttributions(events, attentionOutput[0]),
            modelVersion = modelVersion,
        )
    }

    /**
     * Dikkat agirliklarini gercek olaylara geri esler.
     *
     * Vektorlestirici diziyi **sona hizaladigi** icin, dolgu adimlari basta
     * kalir ve buradaki ofset hesabi buna gore yapilir. Bu esleme yanlis
     * olursa kullaniciya yanlis kanit gosteririz — sessiz ama ciddi bir hata.
     */
    private fun buildAttributions(
        events: List<SecurityEvent>,
        attention: FloatArray,
    ): List<Attribution> {
        val window = events.takeLast(EventVectorizer.SEQUENCE_LENGTH)
        val offset = EventVectorizer.SEQUENCE_LENGTH - window.size

        val weighted = window.mapIndexed { index, event ->
            event to attention[offset + index]
        }.sortedByDescending { it.second }.take(MAX_ATTRIBUTIONS)

        val total = weighted.sumOf { it.second.toDouble() }.toFloat()
        if (total <= 0f) {
            // Model dikkat uretmediyse esit agirlik veririz; aciklamasiz
            // hukum uretmek Verdict tarafindan zaten reddedilir.
            val even = 1f / weighted.size.coerceAtLeast(1)
            return weighted.map { (event, _) ->
                Attribution(event.id, event.type, even, "expl_model_attention")
            }
        }

        return weighted.map { (event, weight) ->
            Attribution(
                eventId = event.id,
                eventType = event.type,
                weight = (weight / total).coerceIn(0f, 1f),
                explanationKey = "expl_model_attention",
                explanationArgs = listOf(event.type.name),
            )
        }
    }

    override fun close() = interpreter.close()

    companion object {
        private const val OUTPUT_ANOMALY = 0
        private const val OUTPUT_CLASS = 1
        private const val OUTPUT_ATTENTION = 2
        private const val MAX_ATTRIBUTIONS = 5

        /**
         * Modelin siniflandirma basligindaki sinif sirasi. **Egitim
         * pipeline'indaki sirayla birebir ayni olmak zorundadir**; aksi halde
         * model dogru calisir ama yanlis etiket doneriz.
         */
        val THREAT_CLASSES = listOf(
            ThreatClass.NONE,
            ThreatClass.BANKING_OVERLAY_TROJAN,
            ThreatClass.ACCESSIBILITY_ABUSE,
            ThreatClass.STALKERWARE,
            ThreatClass.SPYWARE_GENERIC,
            ThreatClass.ADWARE_AGGRESSIVE,
            ThreatClass.DROPPER,
            ThreatClass.FILELESS_LOADER,
            ThreatClass.CREDENTIAL_PHISHING,
            ThreatClass.SMS_FRAUD,
            ThreatClass.CRYPTO_CLIPPER,
            ThreatClass.C2_BEACON,
            ThreatClass.DATA_EXFILTRATION,
            ThreatClass.ANOMALOUS_BEHAVIOR,
        )

        /**
         * Modeli assets'ten mmap ile yukler. Kopyalamak yerine haritalamak,
         * ~15 MB'lik dosyanin RAM'e acilmasini engeller.
         */
        fun load(
            context: Context,
            assetName: String,
            modelVersion: String,
            options: Interpreter.Options,
        ): BehaviorSequenceModel {
            val descriptor = context.assets.openFd(assetName)
            val buffer = FileInputStream(descriptor.fileDescriptor).use { stream ->
                stream.channel.map(
                    FileChannel.MapMode.READ_ONLY,
                    descriptor.startOffset,
                    descriptor.declaredLength,
                )
            }
            return BehaviorSequenceModel(
                interpreter = Interpreter(buffer, options),
                modelVersion = modelVersion,
            )
        }
    }
}

data class InferenceResult(
    val score: RiskScore,
    val threatClass: ThreatClass,
    val confidence: Float,
    /** Dizinin perplexity'si: modelin bu davranisi ne kadar "tanimadigi". */
    val anomaly: Float,
    val attributions: List<Attribution>,
    val modelVersion: String,
)
