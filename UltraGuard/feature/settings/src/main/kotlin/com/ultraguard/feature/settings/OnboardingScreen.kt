package com.ultraguard.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel

/**
 * Izin karsilama akisi.
 *
 * Urunun en hassas noktasi burasidir: UltraGuard, kotucul uygulamalarin
 * istedigi izinlerin bir kismini kendisi de ister. Bu paradoks gizlenmez;
 * her adimda **neden istedigimiz** ve **o izinle ne YAPMADIGIMIZ** ayri
 * ayri yazar.
 *
 * Her adim atlanabilir. Zorunlu izin yoktur -- atlanan bir izin yalnizca
 * ilgili koruma katmanini kapatir ve bu durum ana ekranda gorunur kalir.
 * Kullaniciyi bir izne mecbur birakmak, izni "onaylatmak" olur, almak degil.
 */
@Composable
fun OnboardingScreen(
    onRequestAccessibility: () -> Unit,
    onRequestNotificationAccess: () -> Unit,
    onRequestVpn: () -> Unit,
    onRequestBatteryExemption: () -> Unit,
    onFinished: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    var step by remember { mutableIntStateOf(0) }
    val steps = remember { onboardingSteps() }
    val current = steps[step]

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            LinearProgressIndicator(
                progress = { (step + 1f) / steps.size },
                modifier = Modifier.fillMaxWidth(),
            )

            Text(
                text = stringResource(current.titleRes),
                style = MaterialTheme.typography.headlineMedium,
            )
            Text(
                text = stringResource(current.bodyRes),
                style = MaterialTheme.typography.bodyLarge,
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (current.grantAction != null) {
                TextButton(
                    onClick = {
                        if (step < steps.lastIndex) step++ else finish(viewModel, onFinished)
                    },
                    modifier = Modifier.weight(1f),
                ) {
                    Text(stringResource(R.string.action_skip))
                }
            }

            Button(
                onClick = {
                    when (current.grantAction) {
                        GrantAction.ACCESSIBILITY -> onRequestAccessibility()
                        GrantAction.NOTIFICATIONS -> onRequestNotificationAccess()
                        GrantAction.VPN -> onRequestVpn()
                        GrantAction.BATTERY -> onRequestBatteryExemption()
                        null -> Unit
                    }
                    if (step < steps.lastIndex) step++ else finish(viewModel, onFinished)
                },
                modifier = Modifier.weight(1f),
            ) {
                Text(
                    stringResource(
                        when {
                            step == steps.lastIndex -> R.string.action_done
                            current.grantAction != null -> R.string.action_grant
                            else -> R.string.action_continue
                        },
                    ),
                )
            }
        }
    }
}

private fun finish(viewModel: SettingsViewModel, onFinished: () -> Unit) {
    viewModel.completeOnboarding()
    onFinished()
}

private data class OnboardingStep(
    val titleRes: Int,
    val bodyRes: Int,
    val grantAction: GrantAction?,
)

private enum class GrantAction { ACCESSIBILITY, NOTIFICATIONS, VPN, BATTERY }

private fun onboardingSteps() = listOf(
    OnboardingStep(R.string.onboarding_welcome_title, R.string.onboarding_welcome_body, null),
    OnboardingStep(
        R.string.onboarding_a11y_title,
        R.string.onboarding_a11y_body,
        GrantAction.ACCESSIBILITY,
    ),
    OnboardingStep(
        R.string.onboarding_notification_title,
        R.string.onboarding_notification_body,
        GrantAction.NOTIFICATIONS,
    ),
    OnboardingStep(R.string.onboarding_vpn_title, R.string.onboarding_vpn_body, GrantAction.VPN),
    OnboardingStep(
        R.string.onboarding_battery_title,
        R.string.onboarding_battery_body,
        GrantAction.BATTERY,
    ),
    OnboardingStep(R.string.onboarding_done_title, R.string.onboarding_done_body, null),
)
