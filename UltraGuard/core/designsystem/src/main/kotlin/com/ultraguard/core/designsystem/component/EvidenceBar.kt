package com.ultraguard.core.designsystem.component

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/**
 * Bir hukmun kanit dokumu.
 *
 * Kullaniciya "bu uygulama tehlikeli" demek yetmez; **neden** dedigimizi
 * gostermek zorundayiz. Her cubuk bir olayin karardaki agirligini temsil
 * eder ve agirliklarin toplami 1.0'dir. Kara kutu bir skor bu urunde
 * kullaniciya hicbir zaman tek basina gosterilmez.
 */
@Composable
fun EvidenceBreakdown(
    evidence: List<EvidenceItem>,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        evidence.forEach { item ->
            EvidenceRow(item)
        }
    }
}

@Composable
private fun EvidenceRow(item: EvidenceItem) {
    val percent = (item.weight * 100).toInt()

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .semantics {
                contentDescription = "${item.description}, karardaki agirlik yuzde $percent"
            },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text = item.description,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )

        Box(
            modifier = Modifier
                .width(BAR_WIDTH_DP.dp)
                .height(BAR_HEIGHT_DP.dp)
                .clip(RoundedCornerShape(BAR_HEIGHT_DP.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant),
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth(fraction = item.weight.coerceIn(0f, 1f))
                    .height(BAR_HEIGHT_DP.dp)
                    .clip(RoundedCornerShape(BAR_HEIGHT_DP.dp))
                    .background(MaterialTheme.colorScheme.primary),
            )
        }

        Text(
            text = "%.2f".format(item.weight),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 2.dp),
        )
    }
}

data class EvidenceItem(
    val description: String,
    val weight: Float,
)

private const val BAR_WIDTH_DP = 96
private const val BAR_HEIGHT_DP = 6
