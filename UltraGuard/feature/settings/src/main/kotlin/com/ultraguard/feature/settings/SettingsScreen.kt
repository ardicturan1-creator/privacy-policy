package com.ultraguard.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.ultraguard.core.designsystem.component.LoadingState
import com.ultraguard.core.designsystem.component.RadioOption
import com.ultraguard.core.designsystem.component.SectionHeader
import com.ultraguard.core.designsystem.component.SettingSwitch
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.policy.LedgerIntegrity

/**
 * Ayarlar ve Seffaflik Merkezi.
 *
 * Iki tasarim karari:
 *  1. **Gizlilik anahtarlari kapali baslar ve neden acmak isteyebileceginizi
 *     yaninda yazar.** "Ayrintilar" dugmesine tiklamayi gerektiren bir
 *     gizlilik ayari, pratikte aciklanmamis bir ayardir.
 *  2. **Kendi butunlugumuzu kullanicinin denetimine aciyoruz.** Defter
 *     dogrulama dugmesi, urunun kendisine kosulsuz guven istememesinin
 *     somut karsiligidir.
 */
@Composable
fun SettingsScreen(
    modifier: Modifier = Modifier,
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val integrity by viewModel.ledgerIntegrity.collectAsStateWithLifecycle()

    val current = settings
    if (current == null) {
        LoadingState(modifier)
        return
    }

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        item { SectionHeader(stringResource(R.string.settings_mode_header)) }

        // FLEET kurumsal saglamayla gelir, BATTERY_GUARD otomatiktir;
        // ikisi de kullanicinin elle secebilecegi mod degildir.
        for (mode in listOf(
            ProtectionMode.ACTIVE,
            ProtectionMode.STEALTH,
            ProtectionMode.PARANOID,
        )) {
            item(key = mode.name) {
                RadioOption(
                    title = stringResource(modeTitle(mode)),
                    description = stringResource(modeDescription(mode)),
                    selected = current.mode == mode,
                    onSelect = { viewModel.setMode(mode) },
                )
            }
        }

        if (current.mode == ProtectionMode.BATTERY_GUARD) {
            item {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant,
                    ),
                ) {
                    Text(
                        text = stringResource(R.string.mode_battery_desc),
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(14.dp),
                    )
                }
            }
        }

        item { SectionHeader(stringResource(R.string.settings_protection_header)) }

        item {
            SettingSwitch(
                title = stringResource(R.string.settings_financial_shield),
                description = stringResource(R.string.settings_financial_shield_desc),
                checked = current.financialShieldEnabled,
                onCheckedChange = viewModel::setFinancialShield,
            )
        }

        item {
            SettingSwitch(
                title = stringResource(R.string.settings_zero_trust),
                description = stringResource(R.string.settings_zero_trust_desc),
                checked = current.zeroTrustNetworkEnabled,
                onCheckedChange = viewModel::setZeroTrustNetwork,
            )
        }

        item {
            SectionHeader(
                title = stringResource(R.string.settings_privacy_header),
                subtitle = stringResource(R.string.settings_privacy_note),
            )
        }

        item {
            SettingSwitch(
                title = stringResource(R.string.settings_cloud),
                description = stringResource(R.string.settings_cloud_desc),
                checked = current.cloudConsultationEnabled,
                onCheckedChange = viewModel::setCloudConsultation,
            )
        }

        item {
            SettingSwitch(
                title = stringResource(R.string.settings_federated),
                description = stringResource(R.string.settings_federated_desc),
                checked = current.federatedLearningEnabled,
                onCheckedChange = viewModel::setFederatedLearning,
            )
        }

        item {
            Column(modifier = Modifier.padding(vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.settings_retention),
                    style = MaterialTheme.typography.bodyLarge,
                )
                Text(
                    text = stringResource(R.string.settings_retention_value, current.retentionDays),
                    style = MaterialTheme.typography.titleMedium,
                )
                Slider(
                    value = current.retentionDays.toFloat(),
                    onValueChange = { viewModel.setRetentionDays(it.toInt()) },
                    valueRange = 7f..90f,
                    steps = RETENTION_STEPS,
                )
                Text(
                    text = stringResource(R.string.settings_retention_desc),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        item {
            SectionHeader(
                title = stringResource(R.string.settings_transparency_header),
                subtitle = stringResource(R.string.settings_ledger_desc),
            )
        }

        item {
            when (val result = integrity) {
                is LedgerIntegrity.Intact -> Text(
                    text = stringResource(R.string.settings_ledger_intact, result.entryCount),
                    style = MaterialTheme.typography.bodyMedium,
                )

                is LedgerIntegrity.Broken -> Text(
                    text = stringResource(
                        R.string.settings_ledger_broken,
                        result.firstBrokenEntryId,
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )

                null -> Text(
                    text = stringResource(R.string.loading),
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }

        item {
            OutlinedButton(
                onClick = viewModel::verifyLedger,
                modifier = Modifier.padding(top = 8.dp),
            ) {
                Text(stringResource(R.string.settings_ledger_verify))
            }
        }
    }
}

private fun modeTitle(mode: ProtectionMode): Int = when (mode) {
    ProtectionMode.ACTIVE -> R.string.mode_active
    ProtectionMode.STEALTH -> R.string.mode_stealth
    ProtectionMode.PARANOID -> R.string.mode_paranoid
    ProtectionMode.FLEET -> R.string.mode_fleet
    ProtectionMode.BATTERY_GUARD -> R.string.mode_battery
}

private fun modeDescription(mode: ProtectionMode): Int = when (mode) {
    ProtectionMode.ACTIVE -> R.string.mode_active_desc
    ProtectionMode.STEALTH -> R.string.mode_stealth_desc
    ProtectionMode.PARANOID -> R.string.mode_paranoid_desc
    ProtectionMode.FLEET -> R.string.mode_fleet_desc
    ProtectionMode.BATTERY_GUARD -> R.string.mode_battery_desc
}

/** 7-90 gun arasi, birer gunluk adimlar. */
private const val RETENTION_STEPS = 82
