package com.ultraguard.core.designsystem.component

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * Cihaz Guven Skoru halkasi.
 *
 * Tasarim notlari:
 *  - Halka **ince**dir. Kalin, dolu bir gosterge oyunlastirma hissi verir;
 *    burada amac kullaniciyi puan toplamaya tesvik etmek degil, durumu
 *    bildirmek.
 *  - Renk yalnizca skor dustugunde belirginlesir. 90 uzeri notr mavi-gridir;
 *    "her sey yolunda" durumunun kutlanacak bir rengi yoktur.
 *  - Erisilebilirlik: halka gorsel bir sustur; ekran okuyucu icin skor ve
 *    band birlikte okunur.
 */
@Composable
fun TrustScoreRing(
    score: Int,
    label: String,
    contentDescription: String,
    modifier: Modifier = Modifier,
    diameter: androidx.compose.ui.unit.Dp = 176.dp,
) {
    val animatedProgress by animateFloatAsState(
        targetValue = (score.coerceIn(0, 100)) / 100f,
        animationSpec = tween(durationMillis = PROGRESS_ANIMATION_MILLIS),
        label = "trustScore",
    )

    val trackColor = MaterialTheme.colorScheme.surfaceVariant
    val indicatorColor = indicatorColorFor(score)

    Box(
        modifier = modifier
            .size(diameter)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(modifier = Modifier.size(diameter)) {
            val stroke = Stroke(width = STROKE_WIDTH_DP.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round)
            val inset = stroke.width / 2f
            val arcSize = Size(size.width - stroke.width, size.height - stroke.width)
            val topLeft = androidx.compose.ui.geometry.Offset(inset, inset)

            drawArc(
                color = trackColor,
                startAngle = START_ANGLE,
                sweepAngle = SWEEP_ANGLE,
                useCenter = false,
                topLeft = topLeft,
                size = arcSize,
                style = stroke,
            )

            drawArc(
                color = indicatorColor,
                startAngle = START_ANGLE,
                sweepAngle = SWEEP_ANGLE * animatedProgress,
                useCenter = false,
                topLeft = topLeft,
                size = arcSize,
                style = stroke,
            )
        }

        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = score.toString(),
                style = MaterialTheme.typography.displayLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = label,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun indicatorColorFor(score: Int): Color {
    val risk = com.ultraguard.core.designsystem.theme.LocalRiskColors.current
    return when {
        score >= 90 -> risk.minimal   // notr: kutlama yok
        score >= 75 -> risk.low
        score >= 55 -> risk.elevated
        score >= 35 -> risk.high
        else -> risk.critical
    }
}

private const val START_ANGLE = 135f
private const val SWEEP_ANGLE = 270f
private const val STROKE_WIDTH_DP = 7
private const val PROGRESS_ANIMATION_MILLIS = 600
