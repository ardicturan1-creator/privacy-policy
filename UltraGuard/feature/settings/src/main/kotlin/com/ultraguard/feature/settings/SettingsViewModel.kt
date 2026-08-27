package com.ultraguard.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ultraguard.core.datastore.SettingsStore
import com.ultraguard.core.datastore.UltraGuardSettings
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.policy.ActionLedger
import com.ultraguard.core.policy.LedgerIntegrity
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/**
 * Ayarlar ve Seffaflik Merkezi.
 *
 * Bu ekranin ayirt edici parcasi [ledgerIntegrity]: kullanici, UltraGuard'in
 * kendi kayit defterinin kurcalanip kurcalanmadigini buradan dogrulayabilir.
 * Kendi butunlugunu kullanicinin denetimine acmayan bir guvenlik urunu,
 * kullanicidan kosulsuz guven istiyor demektir.
 */
@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settingsStore: SettingsStore,
    private val actionLedger: ActionLedger,
) : ViewModel() {

    val settings: StateFlow<UltraGuardSettings?> = settingsStore.settings
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), null)

    private val _ledgerIntegrity = MutableStateFlow<LedgerIntegrity?>(null)
    val ledgerIntegrity: StateFlow<LedgerIntegrity?> = _ledgerIntegrity.asStateFlow()

    init {
        verifyLedger()
    }

    fun verifyLedger() {
        viewModelScope.launch { _ledgerIntegrity.value = actionLedger.verifyIntegrity() }
    }

    fun setMode(mode: ProtectionMode) = launch { settingsStore.setMode(mode) }

    /**
     * L3 bulut konsultasyonu.
     *
     * Acildiginda cihazdan cikan tek sey: APK ozeti (SHA-256) ve anonim
     * davranis vektoru. Icerik, mesaj, dosya veya kimlik bilgisi hicbir
     * kosulda gonderilmez. Varsayilani KAPALI olmasinin nedeni budur:
     * riza, varsayilanla degil secimle alinir.
     */
    fun setCloudConsultation(enabled: Boolean) = launch {
        settingsStore.setCloudConsultation(enabled)
    }

    fun setFederatedLearning(enabled: Boolean) = launch {
        settingsStore.setFederatedLearning(enabled)
    }

    fun setRetentionDays(days: Int) = launch { settingsStore.setRetentionDays(days) }

    fun setFinancialShield(enabled: Boolean) = launch { settingsStore.setFinancialShield(enabled) }

    fun setZeroTrustNetwork(enabled: Boolean) = launch {
        settingsStore.setZeroTrustNetwork(enabled)
    }

    fun completeOnboarding() = launch { settingsStore.setOnboardingCompleted(true) }

    private fun launch(block: suspend () -> Unit) {
        viewModelScope.launch { block() }
    }

    private companion object {
        const val STOP_TIMEOUT_MILLIS = 5_000L
    }
}
