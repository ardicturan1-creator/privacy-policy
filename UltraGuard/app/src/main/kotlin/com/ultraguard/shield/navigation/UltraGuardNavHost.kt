package com.ultraguard.shield.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.ultraguard.feature.dashboard.DashboardScreen

/**
 * Navigasyon grafi.
 *
 * Feature modulleri birbirini tanimaz; gecisler yalnizca burada, `:app`
 * katmaninda birlestirilir. Bu, bir ozelligin baska bir ozellige derleme
 * zamani bagimliligi kurmasini yapisal olarak engeller.
 */
object Routes {
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
fun UltraGuardNavHost(navController: NavHostController) {
    NavHost(navController = navController, startDestination = Routes.DASHBOARD) {

        composable(Routes.DASHBOARD) {
            DashboardScreen(
                onOpenThreat = { verdictId ->
                    navController.navigate(Routes.threatDetail(verdictId))
                },
                onOpenTimeline = { navController.navigate(Routes.TIMELINE) },
                onOpenApps = { navController.navigate(Routes.APPS) },
                onOpenAssistant = { navController.navigate(Routes.ASSISTANT) },
            )
        }

        composable(
            route = Routes.APP_DETAIL,
            arguments = listOf(navArgument(Routes.ARG_PACKAGE) { type = NavType.StringType }),
        ) {
            // AppDetailScreen: :feature:appdetail icinde
        }

        composable(
            route = Routes.THREAT_DETAIL,
            arguments = listOf(navArgument(Routes.ARG_VERDICT) { type = NavType.LongType }),
        ) {
            // ThreatDetailScreen: :feature:appdetail icinde
        }

        composable(Routes.TIMELINE) { /* TimelineScreen */ }
        composable(Routes.APPS) { /* AppListScreen */ }
        composable(Routes.ASSISTANT) { /* AssistantScreen */ }
        composable(Routes.SETTINGS) { /* SettingsScreen */ }
    }
}
