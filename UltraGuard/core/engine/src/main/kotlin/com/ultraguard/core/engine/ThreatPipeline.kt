package com.ultraguard.core.engine

import com.ultraguard.core.ai.fusion.RiskFusion
import com.ultraguard.core.ai.inference.BehaviorSequenceModel
import com.ultraguard.core.ai.rules.CorrelationWindow
import com.ultraguard.core.ai.rules.RuleEngine
import com.ultraguard.core.common.di.ApplicationScope
import com.ultraguard.core.common.di.DefaultDispatcher
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.model.MonitoringState
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.TrustOverride
import com.ultraguard.core.model.Verdict
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.random.Random
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.buffer
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.withContext

/**
 * Kademeli karar zinciri: L0 → L1 → L2 → fuzyon.
 *
 * ```
 *   EventBus (2.000-15.000 olay/saat)
 *        │
 *        ▼  L0: normalizasyon + gurultu bastirma (~%97 elenir)
 *   EventNormalizer
 *        │
 *        ▼  L1: deterministik kural motoru (<1 ms)
 *   RuleEngine ──── eslesme varsa → hukum
 *        │
 *        ▼  L2: cihaz uzeri dizi modeli (~30 ms, orneklemeli)
 *   BehaviorSequenceModel
 *        │
 *        ▼
 *   RiskFusion → nihai Verdict → PolicyEngine
 * ```
 *
 * L2 **her olayda calismaz**. BASELINE durumunda olaylarin yalnizca
 * [MonitoringState.samplingRatio] kadari modele gider; kritik olan sey, L1'in
 * zaten bilinen saldirilari kacirmamasi ve L2'nin yalnizca "acikta kalan"
 * belirsizligi kapatmasidir.
 */
@Singleton
class ThreatPipeline @Inject constructor(
    private val eventBus: EventBus,
    private val normalizer: EventNormalizer,
    private val repository: EventRepository,
    private val ruleEngine: RuleEngine,
    private val modelProvider: ModelProvider,
    private val riskFusion: RiskFusion,
    private val stateMachine: MonitoringStateMachine,
    private val clock: Clock,
    @DefaultDispatcher private val computeDispatcher: CoroutineDispatcher,
    @ApplicationScope private val scope: CoroutineScope,
) {
    private val _verdicts = MutableSharedFlow<Verdict>(extraBufferCapacity = 64)
    val verdicts: SharedFlow<Verdict> = _verdicts.asSharedFlow()

    @Volatile
    private var currentMode: ProtectionMode = ProtectionMode.ACTIVE

    fun setMode(mode: ProtectionMode) {
        currentMode = mode
    }

    fun start() {
        eventBus.events
            // `buffer`, sensorlerin yazim hizini karar zincirinin isleme
            // hizindan ayirir; kisa yuklenme tepelerinde olay kaybi olmaz.
            .buffer(capacity = 256)
            .onEach(::process)
            .launchIn(scope)
    }

    private suspend fun process(raw: SecurityEvent) {
        // --- L0: normalizasyon ve gurultu bastirma ---------------------
        val event = normalizer.normalize(raw) ?: return
        val packageName = event.packageName ?: return

        val persisted = repository.record(event)
        val state = stateMachine.stateFor(packageName)
        stateMachine.onEvent(persisted)

        val window = repository.windowFor(packageName, clock.nowMillis())
        if (window.isEmpty) return

        // --- L1: deterministik kurallar --------------------------------
        val ruleVerdict = withContext(computeDispatcher) {
            ruleEngine.evaluate(window, clock.nowMillis())
        }

        // --- L2: cihaz uzeri model -------------------------------------
        val modelResult = if (shouldRunModel(state, ruleVerdict != null)) {
            runModel(window)
        } else {
            null
        }

        if (ruleVerdict == null && modelResult == null) return

        // --- Fuzyon -----------------------------------------------------
        val trust = repository.trustOverrideFor(packageName)
        val fused = riskFusion.fuse(
            packageName = packageName,
            ruleVerdict = ruleVerdict,
            modelResult = modelResult,
            trustOverride = trust,
            nowMillis = clock.nowMillis(),
        ) ?: return

        val stored = repository.record(fused)
        stateMachine.onVerdict(packageName, stored.score.band)
        _verdicts.emit(stored)
    }

    /**
     * L2 calistirma karari.
     *
     * Uc kural:
     *  1. L1 zaten eslestiyse model **her zaman** calisir -- ikinci bir
     *     bagimsiz gorus, kararin guvenini olcmemizi saglar.
     *  2. Pil koruma modunda model hic calismaz.
     *  3. Aksi halde izleme durumunun orneklem oranina gore rastgele secilir.
     */
    private fun shouldRunModel(state: MonitoringState, ruleMatched: Boolean): Boolean {
        if (!currentMode.runsOnDeviceModel) return false
        if (ruleMatched) return true
        return Random.nextFloat() < state.samplingRatio
    }

    private suspend fun runModel(window: CorrelationWindow) = withContext(computeDispatcher) {
        val model: BehaviorSequenceModel = modelProvider.model() ?: return@withContext null
        runCatching {
            model.infer(window.events, clock.nowMillis())
        }.onFailure { error ->
            // Model hatasi korumayi durdurmaz: L1 calismaya devam eder.
            // Sessiz bir sekilde tum korumayi kaybetmektense, kademeyi
            // kaybedip bunu bildirmek yegdir.
            UgLog.w(TAG, "L2 cikarimi basarisiz; L1 ile devam ediliyor", error)
        }.getOrNull()
    }

    private companion object {
        const val TAG = "ThreatPipeline"
    }
}

/** L2 modelinin tembel yuklenmesi ve pil durumuna gore serbest birakilmasi. */
interface ModelProvider {
    suspend fun model(): BehaviorSequenceModel?
    fun release()
}
