package com.ultraguard.feature.assistant

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.ultraguard.core.designsystem.component.EmptyState
import com.ultraguard.core.designsystem.component.ListDivider
import com.ultraguard.core.designsystem.component.SectionHeader
import com.ultraguard.core.designsystem.component.TimeFormat
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Guardian asistan ekrani.
 *
 * Ekranin altindaki not tesadufi degil: **buradaki her rakam cihazdaki bir
 * kayda dayanir.** Bir guvenlik asistaninin uydurmasi, kullaniciya
 * olmayan bir guvence vermek demektir; bu yuzden asistan yalnizca
 * [GuardianAssistant]'in veritabanindan cikardigi olgulari gosterir.
 */
@Composable
fun AssistantScreen(
    onAppClick: (String) -> Unit,
    modifier: Modifier = Modifier,
    viewModel: AssistantViewModel = hiltViewModel(),
) {
    val explanation by viewModel.explanation.collectAsStateWithLifecycle()
    var query by remember { mutableStateOf("") }

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            SectionHeader(
                title = stringResource(R.string.assistant_title),
                subtitle = stringResource(R.string.assistant_grounding_note),
            )
        }

        item {
            OutlinedTextField(
                value = query,
                onValueChange = {
                    query = it
                    viewModel.explain(it.trim())
                },
                label = { Text(stringResource(R.string.assistant_prompt)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        val current = explanation
        if (current == null) {
            if (query.isNotBlank()) {
                item { EmptyState(stringResource(R.string.assistant_no_data)) }
            }
        } else {
            item {
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onAppClick(current.packageName) },
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant,
                    ),
                ) {
                    Text(
                        text = current.label,
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(14.dp),
                    )
                }
            }

            items(current.facts) { fact ->
                Text(
                    text = factText(fact),
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.fillMaxWidth(),
                )
                ListDivider()
            }
        }
    }
}

/**
 * Olguyu cumleye cevirir.
 *
 * Bilincli olarak sablon tabanlidir: her cumle bir [Fact] alt tipine
 * birebir baglidir, dolayisiyla kaynaksiz bir iddia uretilemez.
 */
private fun factText(fact: Fact): String = when (fact) {
    is Fact.InstallOrigin -> when (fact.source) {
        "PLAY_STORE" -> "Play Store üzerinden kuruldu."
        "SIDELOADED" -> "Play Store dışından kuruldu${fact.installerPackage?.let { " ($it)" } ?: ""}."
        "ADB" -> "Bilgisayardan ADB ile kuruldu."
        "PREINSTALLED" -> "Cihazla birlikte önyüklü geldi."
        else -> "Kurulum kaynağı: ${fact.source}"
    }
    is Fact.Age -> "${fact.days} gündür kurulu."
    is Fact.SensorUsage -> fact.countsByType.entries.joinToString(", ") { (type, count) ->
        "${sensorLabel(type)}: $count kez"
    } + " (son 7 gün)"
    is Fact.NetworkDestination ->
        "${fact.host} — ${fact.connectionCount} bağlantı, " +
            "${TimeFormat.bytes(fact.bytesOut)} gönderildi" +
            (fact.reputation?.let { " · itibar: $it" } ?: "")
    is Fact.DormantPermissions ->
        "${fact.permissions.size} izin istiyor ama 30 gündür kullanmadı: " +
            fact.permissions.take(3).joinToString(", ") { it.substringAfterLast('.') }
    is Fact.LatestVerdict -> "Son bulgu: ${fact.threatClass} (skor ${fact.score}, ${fact.originId})"
}

private fun sensorLabel(eventType: String): String = when {
    eventType.contains("CAMERA") -> "Kamera"
    eventType.contains("MICROPHONE") -> "Mikrofon"
    eventType.contains("LOCATION") -> "Konum"
    eventType.contains("CLIPBOARD") -> "Pano"
    else -> eventType
}

@HiltViewModel
class AssistantViewModel @Inject constructor(
    private val assistant: GuardianAssistant,
) : ViewModel() {

    private val _explanation = MutableStateFlow<AppExplanation?>(null)
    val explanation: StateFlow<AppExplanation?> = _explanation.asStateFlow()

    fun explain(packageName: String) {
        if (packageName.isBlank()) {
            _explanation.value = null
            return
        }
        viewModelScope.launch {
            _explanation.value = assistant.explainApp(packageName)
        }
    }
}
