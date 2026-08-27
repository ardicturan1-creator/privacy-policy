package com.ultraguard.shield

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.navigation.compose.rememberNavController
import com.ultraguard.core.designsystem.theme.UltraGuardTheme
import com.ultraguard.shield.navigation.UltraGuardNavHost
import com.ultraguard.shield.service.ProtectionService
import dagger.hilt.android.AndroidEntryPoint

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Motor, kullanicinin uygulamayi acmasini beklemez; ancak acildiginda
        // calistigindan emin oluruz (or. kullanici pil optimizasyonuyla
        // servisi oldurmusse).
        ProtectionService.start(this)

        setContent {
            UltraGuardTheme {
                UltraGuardApp()
            }
        }
    }
}

@Composable
private fun UltraGuardApp() {
    val navController = rememberNavController()
    Scaffold(modifier = Modifier.fillMaxSize()) { padding ->
        androidx.compose.foundation.layout.Box(modifier = Modifier.padding(padding)) {
            UltraGuardNavHost(navController = navController)
        }
    }
}
