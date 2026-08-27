package com.ultraguard.feature.appdetail

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.database.dao.NetworkFlowDao
import com.ultraguard.core.database.dao.VerdictDao
import com.ultraguard.core.model.Attribution
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.TrustOverride
import com.ultraguard.core.policy.ActionLedger
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

/**
 * Uygulama detayi ve tehdit kaniti ekrani.
 *
 * Ekranin omurgasi [AppDetailUiState.evidence]: bir hukmun neden verildigini
 * olay duzeyinde gosteren attribution listesi. Kullaniciya "guven bana"
 * demeyiz; kararin dayanagini gosterip kendi degerlendirmesini yapmasina
 * imkan veririz.
 */
@HiltViewModel
class AppDetailViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val appProfileDao: AppProfileDao,
    private val verdictDao: VerdictDao,
    private val networkFlowDao: NetworkFlowDao,
    private val actionLedger: ActionLedger,
    private val clock: Clock,
) : ViewModel() {

    private val packageName: String = checkNotNull(savedStateHandle[ARG_PACKAGE]) {
        "AppDetail icin paket adi zorunlu"
    }

    private val json = Json { ignoreUnknownKeys = true }

    private val _uiState = MutableStateFlow<AppDetailUiState>(AppDetailUiState.Loading)
    val uiState: StateFlow<AppDetailUiState> = _uiState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            val profile = appProfileDao.byPackage(packageName)
            if (profile == null) {
                _uiState.value = AppDetailUiState.NotFound
                return@launch
            }

            val verdicts = verdictDao.historyFor(packageName, limit = 20)
            val latest = verdicts.firstOrNull()
            val evidence = latest
                ?.let { runCatching { json.decodeFromString(ListSerializer(Attribution.serializer()), it.attributions) }.getOrNull() }
                .orEmpty()

            _uiState.value = AppDetailUiState.Ready(
                packageName = packageName,
                label = profile.label,
                installSource = profile.installSource.name,
                currentRisk = RiskScore(profile.currentRisk),
                trustOverride = profile.trustOverride,
                evidence = evidence,
                activeActions = actionLedger.activeActionsFor(packageName)
                    .map { ActiveActionRow(it.entryId, it.action::class.simpleName.orEmpty()) },
                networkDestinations = networkFlowDao
                    .flowsFor(packageName, (clock.nowMillis() - WEEK_MILLIS) / DAY_MILLIS)
                    .take(TOP_HOSTS)
                    .map { NetworkRow(it.remoteHost, it.connectionCount, it.bytesOut) },
            )
        }
    }

    /**
     * "Güveniyorum" -- kullanicinin karari her seyin ustundedir.
     *
     * Ancak izleme durmaz: skoru dusururuz, gozlemi surduruz ve KRITIK
     * bulgular yine gosterilir. Kullaniciyi kendi kararindan korumaya
     * calismayiz, ama onu karanlikta da birakmayiz.
     */
    fun markTrusted() = updateTrust(TrustOverride.USER_TRUSTED)

    fun reportFalsePositive() = updateTrust(TrustOverride.FALSE_POSITIVE_REPORTED)

    fun revertAction(entryId: Long) {
        viewModelScope.launch {
            actionLedger.revert(entryId, com.ultraguard.core.model.RevertActor.USER)
            refresh()
        }
    }

    private fun updateTrust(override: TrustOverride) {
        viewModelScope.launch {
            appProfileDao.updateTrust(packageName, override.name)
            refresh()
        }
    }

    companion object {
        const val ARG_PACKAGE = "packageName"
        private const val DAY_MILLIS = 86_400_000L
        private const val WEEK_MILLIS = 7 * DAY_MILLIS
        private const val TOP_HOSTS = 8
    }
}

sealed interface AppDetailUiState {
    data object Loading : AppDetailUiState
    data object NotFound : AppDetailUiState

    data class Ready(
        val packageName: String,
        val label: String,
        val installSource: String,
        val currentRisk: RiskScore,
        val trustOverride: TrustOverride,
        val evidence: List<Attribution>,
        val activeActions: List<ActiveActionRow>,
        val networkDestinations: List<NetworkRow>,
    ) : AppDetailUiState
}

data class ActiveActionRow(val entryId: Long, val actionKind: String)
data class NetworkRow(val host: String, val connectionCount: Int, val bytesOut: Long)
