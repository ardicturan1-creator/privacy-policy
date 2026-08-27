package com.ultraguard.feature.dashboard

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.ultraguard.core.designsystem.component.TrustScoreRing
import com.ultraguard.core.model.ThreatClass

@Composable
fun DashboardScreen(
    onOpenThreat: (Long) -> Unit,
    onOpenTimeline: () -> Unit,
    onOpenApps: () -> Unit,
    onOpenAssistant: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: DashboardViewModel = hiltViewModel(),
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()

    when (val current = state) {
        DashboardUiState.Loading -> Box(
            modifier = modifier.fillMaxSize(),
            contentAlignment = Alignment.Center,
        ) {
            CircularProgressIndicator()
        }

        is DashboardUiState.Ready -> DashboardContent(
            state = current,
            onOpenThreat = onOpenThreat,
            onOpenTimeline = onOpenTimeline,
            onOpenApps = onOpenApps,
            onOpenAssistant = onOpenAssistant,
            onRevert = viewModel::revertAction,
            modifier = modifier,
        )
    }
}

@Composable
private fun DashboardContent(
    state: DashboardUiState.Ready,
    onOpenThreat: (Long) -> Unit,
    onOpenTimeline: () -> Unit,
    onOpenApps: () -> Unit,
    onOpenAssistant: () -> Unit,
    onRevert: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        item {
            Column(
                modifier = Modifier.fillMaxWidth(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                TrustScoreRing(
                    score = state.trustScore.total,
                    label = "Cihaz Güveni",
                    contentDescription = trustDescription(state),
                )
                // Sakin durumda bile ne yaptigimizi soyleriz: sessizlik,
                // "hicbir sey olmuyor" degil "her sey izleniyor" demektir.
                Text(
                    text = summaryLine(state),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        if (state.attentionItems.isNotEmpty()) {
            item {
                Text(
                    text = "Dikkat gerektiriyor",
                    style = MaterialTheme.typography.titleMedium,
                )
            }
            items(state.attentionItems, key = { it.verdictId }) { item ->
                AttentionCard(item = item, onOpen = { onOpenThreat(item.verdictId) })
            }
        }

        item {
            Text(text = "Son etkinlik", style = MaterialTheme.typography.titleMedium)
        }

        if (state.recentActivity.isEmpty()) {
            item {
                Text(
                    text = "Son 7 günde bir müdahale gerekmedi.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            items(state.recentActivity, key = { it.entryId }) { activity ->
                ActivityRow(activity = activity, onRevert = { onRevert(activity.entryId) })
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(onClick = onOpenTimeline, modifier = Modifier.weight(1f)) {
                    Text("Zaman çizelgesi")
                }
                OutlinedButton(onClick = onOpenApps, modifier = Modifier.weight(1f)) {
                    Text("Uygulamalar")
                }
            }
        }

        item {
            OutlinedButton(onClick = onOpenAssistant, modifier = Modifier.fillMaxWidth()) {
                Text("\"Bu uygulama ne yapıyor?\" diye sor")
            }
        }
    }
}

@Composable
private fun AttentionCard(item: AttentionItem, onOpen: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer,
        ),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(text = item.packageName, style = MaterialTheme.typography.titleMedium)
            Text(
                text = threatLabel(item.threatClass),
                style = MaterialTheme.typography.bodyMedium,
            )
            if (item.activeActionCount > 0) {
                Text(
                    text = "${item.activeActionCount} koruma önlemi şu anda etkin",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            TextButton(onClick = onOpen) { Text("İncele") }
        }
    }
}

@Composable
private fun ActivityRow(activity: ActivityItem, onRevert: () -> Unit) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(text = activity.packageName, style = MaterialTheme.typography.bodyLarge)
                Text(
                    text = actionLabel(activity.actionKind),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (activity.reversible) {
                TextButton(onClick = onRevert) { Text("Geri al") }
            }
        }
        HorizontalDivider(modifier = Modifier.padding(top = 8.dp))
    }
}

private fun trustDescription(state: DashboardUiState.Ready): String =
    "Cihaz güven skoru ${state.trustScore.total}, 100 üzerinden. " +
        if (state.isCalm) "Dikkat gerektiren bulgu yok." else "${state.attentionItems.size} bulgu var."

private fun summaryLine(state: DashboardUiState.Ready): String = when {
    state.attentionItems.isNotEmpty() ->
        "${state.attentionItems.size} uygulama inceleniyor"
    state.recentActivity.isNotEmpty() ->
        "Son 7 günde ${state.recentActivity.size} müdahale yapıldı"
    else -> "İzleme etkin — ${modeLabel(state.mode)}"
}

private fun modeLabel(mode: com.ultraguard.core.model.ProtectionMode): String = when (mode) {
    com.ultraguard.core.model.ProtectionMode.ACTIVE -> "Active"
    com.ultraguard.core.model.ProtectionMode.STEALTH -> "Stealth"
    com.ultraguard.core.model.ProtectionMode.PARANOID -> "Paranoid"
    com.ultraguard.core.model.ProtectionMode.FLEET -> "Fleet"
    com.ultraguard.core.model.ProtectionMode.BATTERY_GUARD -> "Pil koruma"
}

private fun threatLabel(threatClass: ThreatClass): String = when (threatClass) {
    ThreatClass.BANKING_OVERLAY_TROJAN -> "Bankacılık ekranınızı taklit etmeye çalışıyor"
    ThreatClass.ACCESSIBILITY_ABUSE -> "Sizin adınıza dokunma yetkisi kullanıyor"
    ThreatClass.STALKERWARE -> "Sizi sürekli takip ediyor"
    ThreatClass.SPYWARE_GENERIC -> "Arka planda sensörlerinize erişiyor"
    ThreatClass.CRYPTO_CLIPPER -> "Panonuzdaki cüzdan adresini değiştirebilir"
    ThreatClass.C2_BEACON -> "Düzenli aralıklarla bilinmeyen bir sunucuya bağlanıyor"
    ThreatClass.DATA_EXFILTRATION -> "Dışarı veri gönderiyor"
    ThreatClass.CREDENTIAL_PHISHING -> "Giriş bilgilerinizi çalmaya çalışıyor"
    ThreatClass.SMS_FRAUD -> "SMS doğrulama kodlarınıza erişiyor"
    ThreatClass.DROPPER -> "Ek kod indirip çalıştırıyor"
    ThreatClass.FILELESS_LOADER -> "Diske yazmadan kod çalıştırıyor"
    ThreatClass.ROOT_EXPLOIT_ATTEMPT -> "Yetki yükseltmeye çalışıyor"
    ThreatClass.ADWARE_AGGRESSIVE -> "Agresif reklam davranışı gösteriyor"
    ThreatClass.ANOMALOUS_BEHAVIOR -> "Alışılmadık bir davranış dizisi gösterdi"
    ThreatClass.POLICY_VIOLATION -> "Güvenlik yapılandırması bulgusu"
    ThreatClass.NONE -> "Bulgu yok"
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
