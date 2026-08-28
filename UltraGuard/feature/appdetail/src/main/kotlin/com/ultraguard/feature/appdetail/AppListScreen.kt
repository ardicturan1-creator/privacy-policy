package com.ultraguard.feature.appdetail

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.designsystem.R
import com.ultraguard.core.designsystem.component.EmptyState
import com.ultraguard.core.designsystem.component.ListDivider
import com.ultraguard.core.designsystem.component.SectionHeader
import com.ultraguard.core.designsystem.theme.LocalRiskColors
import com.ultraguard.core.model.RiskScore
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

/**
 * Uygulama listesi -- risk skoruna gore sirali.
 *
 * Sistem uygulamalari listenin sonunda gosterilir: platformun kendi guven
 * tabaninda olduklari icin ilgi cekici olmalari nadirdir ve listeyi
 * doldurmalari kullanicinin kendi kurdugu uygulamalari gormesini zorlastirir.
 */
@Composable
fun AppListScreen(
    onAppClick: (String) -> Unit,
    modifier: Modifier = Modifier,
    viewModel: AppListViewModel = hiltViewModel(),
) {
    val apps by viewModel.apps.collectAsStateWithLifecycle()
    val riskColors = LocalRiskColors.current

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        item { SectionHeader(stringResource(R.string.apps_title)) }

        if (apps.isEmpty()) {
            item { EmptyState(stringResource(R.string.apps_empty)) }
        } else {
            items(apps, key = { it.packageName }) { app ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onAppClick(app.packageName) }
                        .padding(vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(text = app.label, style = MaterialTheme.typography.bodyLarge)
                        Text(
                            text = app.packageName,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Text(
                        text = app.risk.value.toString(),
                        style = MaterialTheme.typography.titleMedium,
                        color = riskColors.forBand(app.risk.band),
                    )
                }
                ListDivider()
            }
        }
    }
}

@HiltViewModel
class AppListViewModel @Inject constructor(
    appProfileDao: AppProfileDao,
) : ViewModel() {

    val apps: StateFlow<List<AppRow>> = appProfileDao.allByRisk()
        .map { profiles ->
            profiles
                .sortedWith(
                    // Once risk, sonra kullanici uygulamalari, sonra ad.
                    compareByDescending<com.ultraguard.core.database.entity.AppProfileEntity> {
                        it.currentRisk
                    }
                        .thenBy { it.isSystemApp }
                        .thenBy { it.label.lowercase() },
                )
                .map { AppRow(it.packageName, it.label, RiskScore(it.currentRisk), it.isSystemApp) }
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())
}

data class AppRow(
    val packageName: String,
    val label: String,
    val risk: RiskScore,
    val isSystemApp: Boolean,
)
