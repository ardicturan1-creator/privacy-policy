package com.ultraguard.feature.timeline

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.ultraguard.core.designsystem.component.EmptyState
import com.ultraguard.core.designsystem.component.ListDivider
import com.ultraguard.core.designsystem.component.SectionHeader
import com.ultraguard.core.designsystem.component.TimeFormat
import com.ultraguard.core.model.EventDomain

/**
 * Zaman cizelgesi -- seffaflik vaadinin somut karsiligi.
 *
 * Kullanici, bizim gordugumuz her seyi burada gorebilir. Burada
 * gosterilmeyen bir olay toplanmamis demektir; gizli bir telemetri katmani
 * yoktur.
 */
@Composable
fun TimelineScreen(
    onEventClick: (String) -> Unit,
    modifier: Modifier = Modifier,
    viewModel: TimelineViewModel = hiltViewModel(),
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val now = remember { System.currentTimeMillis() }

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        item {
            SectionHeader(
                title = stringResource(R.string.timeline_title),
                subtitle = stringResource(R.string.timeline_subtitle),
            )
        }

        item {
            DomainFilterRow(
                selected = state.filter.domain,
                onSelect = viewModel::setDomainFilter,
                modifier = Modifier.padding(vertical = 8.dp),
            )
        }

        if (state.entries.isEmpty()) {
            item { EmptyState(stringResource(R.string.timeline_empty)) }
        } else {
            items(state.entries, key = { it.eventId }) { entry ->
                TimelineRow(
                    entry = entry,
                    nowMillis = now,
                    onClick = { onEventClick(entry.packageName) },
                )
                ListDivider()
            }
        }
    }
}

@Composable
private fun DomainFilterRow(
    selected: EventDomain?,
    onSelect: (EventDomain?) -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        FilterChip(
            selected = selected == null,
            onClick = { onSelect(null) },
            label = { Text(stringResource(R.string.timeline_filter_all)) },
            border = FilterChipDefaults.filterChipBorder(
                enabled = true,
                selected = selected == null,
            ),
        )
        EventDomain.entries.forEach { domain ->
            FilterChip(
                selected = selected == domain,
                onClick = { onSelect(if (selected == domain) null else domain) },
                label = { Text(domainLabel(domain)) },
                border = FilterChipDefaults.filterChipBorder(
                    enabled = true,
                    selected = selected == domain,
                ),
            )
        }
    }
}

@Composable
private fun TimelineRow(
    entry: TimelineEntry,
    nowMillis: Long,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Text(
            text = TimeFormat.relative(entry.timestampMillis, nowMillis),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 2.dp),
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(text = entry.packageName, style = MaterialTheme.typography.bodyLarge)
            Text(
                text = eventLabel(entry.type),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

private fun domainLabel(domain: EventDomain): String = when (domain) {
    EventDomain.PACKAGE -> "Kurulum"
    EventDomain.PERMISSION -> "İzin"
    EventDomain.SENSOR -> "Sensör"
    EventDomain.UI -> "Ekran"
    EventDomain.NETWORK -> "Ağ"
    EventDomain.INTEGRITY -> "Bütünlük"
    EventDomain.KERNEL -> "Çekirdek"
}

/**
 * Olay turunun sade dildeki karsiligi.
 *
 * Kullanici `ACCESSIBILITY_GESTURE_CAPABILITY` degil, "sizin adiniza
 * dokunma yetkisi aldi" okur. Teknik ad, olayin detayinda hala gorunur.
 */
private fun eventLabel(type: com.ultraguard.core.model.EventType): String = when (type) {
    com.ultraguard.core.model.EventType.PACKAGE_INSTALLED -> "Kuruldu"
    com.ultraguard.core.model.EventType.PACKAGE_UPDATED -> "Güncellendi"
    com.ultraguard.core.model.EventType.PACKAGE_REMOVED -> "Kaldırıldı"
    com.ultraguard.core.model.EventType.PACKAGE_SIDELOAD_DETECTED -> "Play Store dışından kuruldu"
    com.ultraguard.core.model.EventType.PACKAGE_SIGNATURE_CHANGED -> "İmzası değişti"
    com.ultraguard.core.model.EventType.SENSOR_CAMERA_ACCESS -> "Kameraya erişti"
    com.ultraguard.core.model.EventType.SENSOR_MICROPHONE_ACCESS -> "Mikrofona erişti"
    com.ultraguard.core.model.EventType.SENSOR_LOCATION_ACCESS -> "Konuma erişti"
    com.ultraguard.core.model.EventType.SENSOR_BACKGROUND_ACCESS -> "Arka planda sensöre erişti"
    com.ultraguard.core.model.EventType.CLIPBOARD_READ -> "Panoyu okudu"
    com.ultraguard.core.model.EventType.CLIPBOARD_SENSITIVE_CONTENT -> "Panoda hassas içerik"
    com.ultraguard.core.model.EventType.MEDIA_PROJECTION_STARTED -> "Ekran yakalamaya başladı"
    com.ultraguard.core.model.EventType.ACCESSIBILITY_SERVICE_ENABLED -> "Erişilebilirlik servisi açtı"
    com.ultraguard.core.model.EventType.ACCESSIBILITY_GESTURE_CAPABILITY -> "Sizin adınıza dokunma yetkisi aldı"
    com.ultraguard.core.model.EventType.ACCESSIBILITY_WINDOW_QUERY -> "Başka bir pencereyi sorguladı"
    com.ultraguard.core.model.EventType.OVERLAY_DRAWN -> "Ekran üstüne çizdi"
    com.ultraguard.core.model.EventType.OVERLAY_ON_PROTECTED_SCREEN -> "Korunan ekranın üstüne çizdi"
    com.ultraguard.core.model.EventType.FOREGROUND_APP_CHANGED -> "Ön plana geldi"
    com.ultraguard.core.model.EventType.NOTIFICATION_POSTED -> "Bildirim gösterdi"
    com.ultraguard.core.model.EventType.NOTIFICATION_PHISHING_PATTERN -> "Bildiriminde oltalama kalıbı"
    com.ultraguard.core.model.EventType.NETWORK_CONNECTION_OPENED -> "Bağlantı açtı"
    com.ultraguard.core.model.EventType.NETWORK_DNS_QUERY -> "Alan adı sorguladı"
    com.ultraguard.core.model.EventType.NETWORK_TLS_HANDSHAKE -> "Şifreli bağlantı kurdu"
    com.ultraguard.core.model.EventType.NETWORK_BEACON_PATTERN -> "Düzenli aralıklarla bağlanıyor"
    com.ultraguard.core.model.EventType.NETWORK_DGA_DOMAIN -> "Otomatik üretilmiş alan adına bağlandı"
    com.ultraguard.core.model.EventType.NETWORK_REPUTATION_HIT -> "Kötü üne sahip adrese bağlandı"
    com.ultraguard.core.model.EventType.NETWORK_BULK_UPLOAD -> "Dışarı büyük veri gönderdi"
    com.ultraguard.core.model.EventType.NETWORK_BLOCKED_BY_POLICY -> "Bağlantısı engellendi"
    com.ultraguard.core.model.EventType.ADB_ENABLED -> "USB hata ayıklama açıldı"
    com.ultraguard.core.model.EventType.WIRELESS_DEBUGGING_ENABLED -> "Kablosuz hata ayıklama açıldı"
    com.ultraguard.core.model.EventType.UNKNOWN_SOURCES_ENABLED -> "Bilinmeyen kaynaklar açıldı"
    com.ultraguard.core.model.EventType.ROOT_INDICATOR_FOUND -> "Root göstergesi bulundu"
    com.ultraguard.core.model.EventType.HOOKING_FRAMEWORK_DETECTED -> "Müdahale çerçevesi tespit edildi"
    com.ultraguard.core.model.EventType.SELF_TAMPER_SUSPECTED -> "UltraGuard'a müdahale şüphesi"
    else -> type.name.lowercase().replace('_', ' ')
}
