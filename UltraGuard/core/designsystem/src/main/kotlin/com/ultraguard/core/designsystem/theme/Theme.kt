package com.ultraguard.core.designsystem.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

/**
 * UltraGuard renk sistemi.
 *
 * Tasarim ilkesi: **sukunet varsayilandir.** Guvenlik uygulamalarinin
 * kullaniciyi surekli kirmizi-yesil uyari dongusunde tutmasi bir karanlik
 * kalip ve uyari koru olusturur: her sey acilse hicbir sey acil degildir.
 *
 * Bu yuzden ana palet notrdur. Renk yalnizca gercekten bir sey olduğunda
 * -- ve siddetiyle orantili olarak -- devreye girer. "Her sey yolunda"
 * durumunun kendi kutlama rengi yoktur; sadece sessizdir.
 */
private val UltraGuardLightColors = lightColorScheme(
    primary = Color(0xFF2C5F7C),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFCDE5F5),
    onPrimaryContainer = Color(0xFF0A1D28),
    secondary = Color(0xFF4E626D),
    surface = Color(0xFFFAFCFE),
    onSurface = Color(0xFF191C1E),
    surfaceVariant = Color(0xFFDCE4E9),
    onSurfaceVariant = Color(0xFF40484C),
    error = Color(0xFFA33A2F),
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFFFDAD5),
    outline = Color(0xFF70787D),
)

private val UltraGuardDarkColors = darkColorScheme(
    primary = Color(0xFF95CDE9),
    onPrimary = Color(0xFF0A1D28),
    primaryContainer = Color(0xFF1F4759),
    onPrimaryContainer = Color(0xFFCDE5F5),
    secondary = Color(0xFFB6CAD6),
    surface = Color(0xFF0F1417),
    onSurface = Color(0xFFE1E3E5),
    surfaceVariant = Color(0xFF40484C),
    onSurfaceVariant = Color(0xFFC0C8CC),
    error = Color(0xFFFFB4A8),
    onError = Color(0xFF561E16),
    errorContainer = Color(0xFF73342A),
    outline = Color(0xFF8A9296),
)

/**
 * Risk bandi renkleri.
 *
 * Material renk semasindan ayri tutulur cunku bunlar **anlam tasiyan**
 * renklerdir: kullanicinin dinamik renk tercihine gore degismemeleri gerekir.
 * Bir tehdidin rengi, duvar kagidina gore degismemelidir.
 */
data class RiskColors(
    val minimal: Color,
    val low: Color,
    val elevated: Color,
    val high: Color,
    val critical: Color,
) {
    fun forBand(band: com.ultraguard.core.model.RiskBand): Color = when (band) {
        com.ultraguard.core.model.RiskBand.MINIMAL -> minimal
        com.ultraguard.core.model.RiskBand.LOW -> low
        com.ultraguard.core.model.RiskBand.ELEVATED -> elevated
        com.ultraguard.core.model.RiskBand.HIGH -> high
        com.ultraguard.core.model.RiskBand.CRITICAL -> critical
    }
}

private val LightRiskColors = RiskColors(
    minimal = Color(0xFF5B7C8D),
    low = Color(0xFF6E8A6B),
    elevated = Color(0xFFB08442),
    high = Color(0xFFC2612F),
    critical = Color(0xFFA33A2F),
)

private val DarkRiskColors = RiskColors(
    minimal = Color(0xFF8FAEBE),
    low = Color(0xFF9BBA97),
    elevated = Color(0xFFDDB271),
    high = Color(0xFFE8905C),
    critical = Color(0xFFFF8F7E),
)

val LocalRiskColors = staticCompositionLocalOf { LightRiskColors }

@Composable
fun UltraGuardTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    /**
     * Dinamik renk varsayilan olarak **kapalidir**. Guvenlik durumunun
     * okunabilirligi, kisisellestirmeden onceliklidir: duvar kagidindan
     * turetilmis bir palet, kritik bir uyariyi silik gosterebilir.
     */
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit,
) {
    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val context = LocalContext.current
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        darkTheme -> UltraGuardDarkColors
        else -> UltraGuardLightColors
    }

    CompositionLocalProvider(
        LocalRiskColors provides if (darkTheme) DarkRiskColors else LightRiskColors,
    ) {
        MaterialTheme(
            colorScheme = colorScheme,
            typography = UltraGuardTypography,
            content = content,
        )
    }
}
