package com.ultraguard.feature.dashboard

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.VerdictDao
import com.ultraguard.core.datastore.SettingsStore
import com.ultraguard.core.model.DeviceTrustScore
import com.ultraguard.core.model.MonitoringState
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.RevertActor
import com.ultraguard.core.model.RiskBand
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.policy.ActionLedger
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

@HiltViewModel
class DashboardViewModel @Inject constructor(
    private val verdictDao: VerdictDao,
    private val settingsStore: SettingsStore,
    private val actionLedger: ActionLedger,
    private val trustScoreCalculator: TrustScoreCalculator,
    private val clock: Clock,
) : ViewModel() {

    @OptIn(ExperimentalCoroutinesApi::class)
    val uiState: StateFlow<DashboardUiState> = settingsStore.settings
        .flatMapLatest { settings ->
            val since = clock.nowMillis() - ATTENTION_WINDOW_MILLIS
            combine(
                // Dikkat gerektiren hukumler: yalnizca modun bildirim esigini
                // asanlar listelenir. Panoyu her bulguyla doldurmak, gercek
                // tehdidi gorunmez kilar.
                verdictDao.activeStream(since, settings.mode.notifyFrom.minimumScore()),
                actionLedger.recentStream(limit = LEDGER_PREVIEW_LIMIT),
                trustScoreCalculator.scoreStream(),
            ) { verdicts, ledger, trustScore ->
                DashboardUiState.Ready(
                    mode = settings.mode,
                    trustScore = trustScore,
                    attentionItems = verdicts.map { verdict ->
                        AttentionItem(
                            verdictId = verdict.id,
                            packageName = verdict.packageName,
                            threatClass = verdict.threatClass,
                            score = RiskScore(verdict.score),
                            createdAtMillis = verdict.createdAtMillis,
                            activeActionCount = ledger.count {
                                it.packageName == verdict.packageName && it.revertedAtMillis == null
                            },
                        )
                    },
                    recentActivity = ledger.take(ACTIVITY_PREVIEW_LIMIT).map { entry ->
                        ActivityItem(
                            entryId = entry.id,
                            packageName = entry.packageName,
                            actionKind = entry.actionKind,
                            timestampMillis = entry.timestampMillis,
                            reversible = entry.reversible && entry.revertedAtMillis == null,
                        )
                    },
                )
            }
        }
        .stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS),
            initialValue = DashboardUiState.Loading,
        )

    /**
     * "Geri al" dugmesi.
     *
     * Kullanicinin otonom bir yaptirimi tek dokunusla iptal edebilmesi, bu
     * urunun temel vaadidir. Bu fonksiyonun her zaman calismasi gerekir --
     * yanlis pozitif kacinilmazdir, geri alinamamasi kabul edilemez.
     */
    fun revertAction(entryId: Long) {
        viewModelScope.launch {
            actionLedger.revert(entryId, RevertActor.USER)
        }
    }

    fun setMode(mode: ProtectionMode) {
        viewModelScope.launch { settingsStore.setMode(mode) }
    }

    private fun RiskBand.minimumScore(): Int = when (this) {
        RiskBand.MINIMAL -> 0
        RiskBand.LOW -> 25
        RiskBand.ELEVATED -> 50
        RiskBand.HIGH -> 75
        RiskBand.CRITICAL -> 90
    }

    private companion object {
        const val ATTENTION_WINDOW_MILLIS = 7 * 24 * 60 * 60 * 1000L
        const val LEDGER_PREVIEW_LIMIT = 50
        const val ACTIVITY_PREVIEW_LIMIT = 6
        const val STOP_TIMEOUT_MILLIS = 5_000L
    }
}

sealed interface DashboardUiState {
    data object Loading : DashboardUiState

    data class Ready(
        val mode: ProtectionMode,
        val trustScore: DeviceTrustScore,
        val attentionItems: List<AttentionItem>,
        val recentActivity: List<ActivityItem>,
    ) : DashboardUiState {
        val isCalm: Boolean get() = attentionItems.isEmpty()
    }
}

data class AttentionItem(
    val verdictId: Long,
    val packageName: String,
    val threatClass: ThreatClass,
    val score: RiskScore,
    val createdAtMillis: Long,
    val activeActionCount: Int,
)

data class ActivityItem(
    val entryId: Long,
    val packageName: String,
    val actionKind: String,
    val timestampMillis: Long,
    val reversible: Boolean,
)

/** Cihaz Guven Skorunun bes bileseninin hesaplanmasi. */
interface TrustScoreCalculator {
    fun scoreStream(): kotlinx.coroutines.flow.Flow<DeviceTrustScore>
}
