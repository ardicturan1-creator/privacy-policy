package com.ultraguard.core.datastore

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.ultraguard.core.model.ProtectionMode
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "ultraguard_settings")

/**
 * Kullanici tercihleri.
 *
 * Gizlilikle ilgili her ayarin varsayilani **en koruyucu** degerdir:
 * bulut konsultasyonu kapali, federated learning kapali, telemetri
 * paylasimi kapali. Kullanici bunlari acabilir; acmadigi surece hicbir sey
 * cihazi terk etmez. Tersini yapip "kullanici isterse kapatir" demek,
 * pratikte varsayilanla yasayan cogunluktan riza almadan veri toplamaktir.
 */
@Singleton
class SettingsStore @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    val settings: Flow<UltraGuardSettings> = context.dataStore.data.map { preferences ->
        UltraGuardSettings(
            mode = preferences[KEY_MODE]
                ?.let { runCatching { ProtectionMode.valueOf(it) }.getOrNull() }
                ?: ProtectionMode.ACTIVE,
            cloudConsultationEnabled = preferences[KEY_CLOUD] ?: false,
            federatedLearningEnabled = preferences[KEY_FEDERATED] ?: false,
            retentionDays = preferences[KEY_RETENTION] ?: DEFAULT_RETENTION_DAYS,
            onboardingCompleted = preferences[KEY_ONBOARDED] ?: false,
            financialShieldEnabled = preferences[KEY_FINANCIAL_SHIELD] ?: true,
            zeroTrustNetworkEnabled = preferences[KEY_ZERO_TRUST] ?: false,
        )
    }

    suspend fun setMode(mode: ProtectionMode) = edit { it[KEY_MODE] = mode.name }

    suspend fun setCloudConsultation(enabled: Boolean) = edit { it[KEY_CLOUD] = enabled }

    suspend fun setFederatedLearning(enabled: Boolean) = edit { it[KEY_FEDERATED] = enabled }

    suspend fun setRetentionDays(days: Int) = edit {
        it[KEY_RETENTION] = days.coerceIn(MIN_RETENTION_DAYS, MAX_RETENTION_DAYS)
    }

    suspend fun setOnboardingCompleted(completed: Boolean) = edit { it[KEY_ONBOARDED] = completed }

    suspend fun setFinancialShield(enabled: Boolean) = edit { it[KEY_FINANCIAL_SHIELD] = enabled }

    suspend fun setZeroTrustNetwork(enabled: Boolean) = edit { it[KEY_ZERO_TRUST] = enabled }

    private suspend fun edit(block: (androidx.datastore.preferences.core.MutablePreferences) -> Unit) {
        context.dataStore.edit(block)
    }

    private companion object {
        val KEY_MODE = stringPreferencesKey("protection_mode")
        val KEY_CLOUD = booleanPreferencesKey("cloud_consultation")
        val KEY_FEDERATED = booleanPreferencesKey("federated_learning")
        val KEY_RETENTION = intPreferencesKey("retention_days")
        val KEY_ONBOARDED = booleanPreferencesKey("onboarding_completed")
        val KEY_FINANCIAL_SHIELD = booleanPreferencesKey("financial_shield")
        val KEY_ZERO_TRUST = booleanPreferencesKey("zero_trust_network")

        const val DEFAULT_RETENTION_DAYS = 30
        const val MIN_RETENTION_DAYS = 7
        const val MAX_RETENTION_DAYS = 90
    }
}

data class UltraGuardSettings(
    val mode: ProtectionMode,
    /** L3 bulut konsultasyonu. Varsayilan: KAPALI. */
    val cloudConsultationEnabled: Boolean,
    /** Federated learning katilimi. Varsayilan: KAPALI. */
    val federatedLearningEnabled: Boolean,
    /** Yerel olay saklama suresi (7-90 gun). */
    val retentionDays: Int,
    val onboardingCompleted: Boolean,
    val financialShieldEnabled: Boolean,
    val zeroTrustNetworkEnabled: Boolean,
)
