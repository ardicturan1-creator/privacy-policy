package com.ultraguard.shield

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.navigation.compose.rememberNavController
import com.ultraguard.core.designsystem.theme.UltraGuardTheme
import com.ultraguard.shield.navigation.Routes
import com.ultraguard.shield.navigation.UltraGuardNavHost
import com.ultraguard.shield.service.ProtectionService
import com.ultraguard.core.datastore.SettingsStore
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var settingsStore: SettingsStore

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Baslangic ekrani, ilk kare cizilmeden bilinmek zorunda: Compose
        // ilk once dashboard'u cizip sonra onboarding'e atlarsa kullanici
        // bir goz kirpma boyunca yanlis ekrani gorur. DataStore'un ilk
        // degeri diskten okunur ve pratikte mikrosaniyeler surer.
        val onboarded = runBlocking { settingsStore.settings.first().onboardingCompleted }
        val startDestination = if (onboarded) Routes.DASHBOARD else Routes.ONBOARDING

        // Motor, kullanicinin uygulamayi acmasini beklemez; ancak acildiginda
        // calistigindan emin oluruz (or. kullanici pil optimizasyonuyla
        // servisi oldurmusse).
        ProtectionService.start(this)

        // Bildirimden gelindiyse dogrudan ilgili tehdit ekrani acilir.
        // Kullaniciyi ana ekrana birakip "simdi bul" demek, acil bir uyarinin
        // en kotu bicimde sunulmasidir.
        val initialVerdictId = intent
            ?.takeIf { it.action == ACTION_OPEN_THREAT }
            ?.getLongExtra(EXTRA_VERDICT_ID, -1L)
            ?.takeIf { it >= 0 }

        setContent {
            UltraGuardTheme {
                UltraGuardApp(
                    startDestination = startDestination,
                    initialVerdictId = initialVerdictId,
                )
            }
        }
    }

    companion object {
        const val ACTION_OPEN_THREAT = "com.ultraguard.action.OPEN_THREAT"
        const val EXTRA_VERDICT_ID = "verdict_id"
    }
}

@Composable
private fun UltraGuardApp(startDestination: String, initialVerdictId: Long?) {
    val navController = rememberNavController()

    LaunchedEffect(initialVerdictId) {
        if (initialVerdictId != null) {
            navController.navigate(Routes.threatDetail(initialVerdictId))
        }
    }

    Scaffold(modifier = Modifier.fillMaxSize()) { padding ->
        Box(modifier = Modifier.padding(padding)) {
            UltraGuardNavHost(
                navController = navController,
                startDestination = startDestination,
            )
        }
    }
}
