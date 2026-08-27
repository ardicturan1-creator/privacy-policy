package com.ultraguard.shield.navigation

import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.ultraguard.core.network.UltraGuardVpnService
import com.ultraguard.feature.appdetail.AppDetailScreen
import com.ultraguard.feature.appdetail.AppListScreen
import com.ultraguard.feature.assistant.AssistantScreen
import com.ultraguard.feature.dashboard.DashboardScreen
import com.ultraguard.feature.settings.OnboardingScreen
import com.ultraguard.feature.settings.SettingsScreen
import com.ultraguard.feature.timeline.TimelineScreen

/**
 * Navigasyon grafi.
 *
 * Feature modulleri birbirini tanimaz; gecisler yalnizca burada, `:app`
 * katmaninda birlestirilir. Bu, bir ozelligin baska bir ozellige derleme
 * zamani bagimliligi kurmasini yapisal olarak engeller.
 */
object Routes {
    const val ONBOARDING = "onboarding"
    const val DASHBOARD = "dashboard"
    const val TIMELINE = "timeline"
    const val APPS = "apps"
    const val ASSISTANT = "assistant"
    const val SETTINGS = "settings"

    const val ARG_PACKAGE = "packageName"
    const val ARG_VERDICT = "verdictId"

    const val APP_DETAIL = "app/{$ARG_PACKAGE}"
    const val THREAT_DETAIL = "threat/{$ARG_VERDICT}"

    fun appDetail(packageName: String) = "app/$packageName"
    fun threatDetail(verdictId: Long) = "threat/$verdictId"
}

@Composable
fun UltraGuardNavHost(
    navController: NavHostController,
    startDestination: String = Routes.DASHBOARD,
) {
    val context = LocalContext.current

    NavHost(navController = navController, startDestination = startDestination) {

        composable(Routes.ONBOARDING) {
            OnboardingScreen(
                onRequestAccessibility = {
                    context.openSystemSettings(Settings.ACTION_ACCESSIBILITY_SETTINGS)
                },
                onRequestNotificationAccess = {
                    context.openSystemSettings(
                        Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS,
                    )
                },
                onRequestVpn = { context.requestVpnConsent() },
                onRequestBatteryExemption = {
                    context.openSystemSettings(
                        Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS,
                    )
                },
                onFinished = {
                    navController.navigate(Routes.DASHBOARD) {
                        popUpTo(Routes.ONBOARDING) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.DASHBOARD) {
            DashboardScreen(
                onOpenThreat = { verdictId -> navController.navigate(Routes.threatDetail(verdictId)) },
                onOpenTimeline = { navController.navigate(Routes.TIMELINE) },
                onOpenApps = { navController.navigate(Routes.APPS) },
                onOpenAssistant = { navController.navigate(Routes.ASSISTANT) },
            )
        }

        composable(Routes.TIMELINE) {
            TimelineScreen(
                onEventClick = { packageName ->
                    navController.navigate(Routes.appDetail(packageName))
                },
            )
        }

        composable(Routes.APPS) {
            AppListScreen(
                onAppClick = { packageName ->
                    navController.navigate(Routes.appDetail(packageName))
                },
            )
        }

        composable(
            route = Routes.APP_DETAIL,
            arguments = listOf(navArgument(Routes.ARG_PACKAGE) { type = NavType.StringType }),
        ) {
            AppDetailScreen(onBack = { navController.popBackStack() })
        }

        // Tehdit detayi, ilgili paketin detay ekranidir: kullaniciyi ayri bir
        // "tehdit" gorunumune goturmek, ayni uygulama hakkinda iki farkli
        // dogruluk kaynagi olusturur.
        composable(
            route = Routes.THREAT_DETAIL,
            arguments = listOf(navArgument(Routes.ARG_VERDICT) { type = NavType.LongType }),
        ) {
            AppDetailScreen(onBack = { navController.popBackStack() })
        }

        composable(Routes.ASSISTANT) {
            AssistantScreen(
                onAppClick = { packageName ->
                    navController.navigate(Routes.appDetail(packageName))
                },
            )
        }

        composable(Routes.SETTINGS) {
            SettingsScreen()
        }
    }
}

private fun Context.openSystemSettings(action: String) {
    runCatching {
        startActivity(Intent(action).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}

/**
 * VPN onay diyalogunu acar.
 *
 * `VpnService.prepare` null donerse kullanici zaten onaylamistir ve servis
 * dogrudan baslatilir; aksi halde sistem kendi onay ekranini gosterir.
 * Bu onayi atlamanin bir yolu yoktur ve olmamalidir.
 */
private fun Context.requestVpnConsent() {
    val consentIntent = VpnService.prepare(this)
    if (consentIntent != null) {
        runCatching { startActivity(consentIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    } else {
        runCatching { startService(Intent(this, UltraGuardVpnService::class.java)) }
    }
}
