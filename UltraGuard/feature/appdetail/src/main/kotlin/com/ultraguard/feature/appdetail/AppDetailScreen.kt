package com.ultraguard.feature.appdetail

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.ultraguard.core.designsystem.component.EmptyState
import com.ultraguard.core.designsystem.component.EvidenceBreakdown
import com.ultraguard.core.designsystem.component.EvidenceItem
import com.ultraguard.core.designsystem.component.ListDivider
import com.ultraguard.core.designsystem.component.LoadingState
import com.ultraguard.core.designsystem.component.SectionHeader
import com.ultraguard.core.designsystem.component.TimeFormat
import com.ultraguard.core.designsystem.component.explanationOf
import com.ultraguard.core.designsystem.theme.LocalRiskColors
import com.ultraguard.core.model.TrustOverride

/**
 * Uygulama detayi ve tehdit kaniti.
 *
 * Ekranin omurgasi kanit dokumu: kullaniciya "bu uygulama tehlikeli" demek
 * yetmez, **neden** dedigimizi olay duzeyinde gostermek zorundayiz.
 * Kullanici kararimizi denetleyebilmeli ve gerekirse reddedebilmelidir.
 */
@Composable
fun AppDetailScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: AppDetailViewModel = hiltViewModel(),
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()

    when (val current = state) {
        AppDetailUiState.Loading -> LoadingState(modifier)

        AppDetailUiState.NotFound -> Column(
            modifier = modifier.fillMaxSize().padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(stringResource(R.string.appdetail_not_found))
            OutlinedButton(onClick = onBack) { Text(stringResource(R.string.action_back)) }
        }

        is AppDetailUiState.Ready -> AppDetailContent(
            state = current,
            onTrust = viewModel::markTrusted,
            onFalsePositive = viewModel::reportFalsePositive,
            onRevert = viewModel::revertAction,
            modifier = modifier,
        )
    }
}

@Composable
private fun AppDetailContent(
    state: AppDetailUiState.Ready,
    onTrust: () -> Unit,
    onFalsePositive: () -> Unit,
    onRevert: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val riskColors = LocalRiskColors.current

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(text = state.label, style = MaterialTheme.typography.headlineMedium)
                Text(
                    text = state.packageName,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(24.dp),
            ) {
                LabeledValue(
                    label = stringResource(R.string.appdetail_source),
                    value = installSourceLabel(state.installSource),
                )
                LabeledValue(
                    label = stringResource(R.string.appdetail_risk),
                    value = state.currentRisk.value.toString(),
                    color = riskColors.forBand(state.currentRisk.band),
                )
            }
        }

        if (state.trustOverride == TrustOverride.USER_TRUSTED) {
            item {
                Card(
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant,
                    ),
                ) {
                    Text(
                        text = stringResource(R.string.appdetail_trusted_note),
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(14.dp),
                    )
                }
            }
        }

        item { SectionHeader(stringResource(R.string.appdetail_evidence)) }

        if (state.evidence.isEmpty()) {
            item { EmptyState(stringResource(R.string.appdetail_evidence_empty)) }
        } else {
            item {
                EvidenceBreakdown(
                    evidence = state.evidence.map { attribution ->
                        EvidenceItem(
                            description = explanationOf(context, attribution),
                            weight = attribution.weight,
                        )
                    },
                )
            }
        }

        if (state.activeActions.isNotEmpty()) {
            item { SectionHeader(stringResource(R.string.appdetail_active_actions)) }
            items(state.activeActions, key = { it.entryId }) { action ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = actionLabel(action.actionKind),
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = { onRevert(action.entryId) }) {
                        Text(stringResource(R.string.action_revert))
                    }
                }
                ListDivider()
            }
        }

        item { SectionHeader(stringResource(R.string.appdetail_network)) }

        if (state.networkDestinations.isEmpty()) {
            item { EmptyState(stringResource(R.string.appdetail_network_empty)) }
        } else {
            items(state.networkDestinations, key = { it.host }) { row ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = row.host,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.weight(1f),
                    )
                    Text(
                        text = TimeFormat.bytes(row.bytesOut),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                ListDivider()
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(onClick = onTrust, modifier = Modifier.weight(1f)) {
                    Text(stringResource(R.string.action_trust))
                }
                OutlinedButton(onClick = onFalsePositive, modifier = Modifier.weight(1f)) {
                    Text(stringResource(R.string.action_false_positive))
                }
            }
        }
    }
}

@Composable
private fun LabeledValue(
    label: String,
    value: String,
    color: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurface,
) {
    Column {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(text = value, style = MaterialTheme.typography.titleMedium, color = color)
    }
}

private fun installSourceLabel(source: String): String = when (source) {
    "PLAY_STORE" -> "Play Store"
    "OTHER_APP_STORE" -> "Başka mağaza"
    "SIDELOADED" -> "Yan yükleme"
    "ADB" -> "ADB"
    "PREINSTALLED" -> "Önyüklü"
    else -> "Bilinmiyor"
}

private fun actionLabel(actionKind: String): String = when (actionKind) {
    "SuspendNetwork" -> "İnternet erişimi durduruldu"
    "HideOverlays" -> "Ekran üstü pencereleri gizlendi"
    "SuspendPackage" -> "Uygulama askıya alındı"
    "FreezeProcess" -> "Süreci donduruldu"
    "RevokePermission" -> "İzni geri alındı"
    "GuideUserToRevoke" -> "İzin kaldırma önerildi"
    "RequestUninstall" -> "Kaldırma önerildi"
    else -> actionKind
}
