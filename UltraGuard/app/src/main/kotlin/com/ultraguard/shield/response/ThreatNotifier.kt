package com.ultraguard.shield.response

import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import com.ultraguard.core.model.RiskBand
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.model.Verdict
import com.ultraguard.shield.MainActivity
import com.ultraguard.shield.R
import com.ultraguard.shield.UltraGuardApplication
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Tehdit bildirimleri.
 *
 * Iki kural:
 *  1. **Her bildirim bir eylem icerir.** Eylemsiz bildirim gondermeyiz;
 *     kullaniciyi bilgilendirip caresiz birakmak, uyari korune giden en
 *     kisa yoldur.
 *  2. **Ne yaptigimiz once soylenir.** Baslik "tehdit bulundu" degil,
 *     "internet erisimini durdurdum" der. Kullanici once durumun kontrol
 *     altinda oldugunu bilmeli, sonra ayrintiya inmeli.
 */
@Singleton
class ThreatNotifier @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    private val notificationManager = context.getSystemService(NotificationManager::class.java)

    fun notifyThreat(verdict: Verdict, appliedActionCount: Int) {
        val openDetail = PendingIntent.getActivity(
            context,
            verdict.id.toInt(),
            Intent(context, MainActivity::class.java).apply {
                action = MainActivity.ACTION_OPEN_THREAT
                putExtra(MainActivity.EXTRA_VERDICT_ID, verdict.id)
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            },
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val label = appLabel(verdict.packageName)
        val title = if (appliedActionCount > 0) {
            context.getString(R.string.threat_notification_title_acted, label)
        } else {
            context.getString(R.string.threat_notification_title_observed, label)
        }

        val notification = NotificationCompat.Builder(context, UltraGuardApplication.CHANNEL_THREATS)
            .setSmallIcon(android.R.drawable.stat_sys_warning)
            .setContentTitle(title)
            .setContentText(threatSummary(verdict.threatClass))
            .setStyle(
                NotificationCompat.BigTextStyle().bigText(
                    buildString {
                        append(threatSummary(verdict.threatClass))
                        if (appliedActionCount > 0) {
                            append("\n\n")
                            append(
                                context.resources.getQuantityString(
                                    R.plurals.threat_actions_applied,
                                    appliedActionCount,
                                    appliedActionCount,
                                ),
                            )
                        }
                    },
                ),
            )
            .setPriority(priorityFor(verdict.score.band))
            .setCategory(NotificationCompat.CATEGORY_ERROR)
            .setContentIntent(openDetail)
            .setAutoCancel(true)
            .addAction(0, context.getString(R.string.action_examine), openDetail)
            .build()

        notificationManager.notify(NOTIFICATION_ID_BASE + verdict.id.toInt(), notification)
    }

    /**
     * UltraGuard'in kendisine mudahale edildiginde.
     *
     * Bu, siradan bir tehdit bildirimi degildir: koruma katmaninin
     * kandirildigi anlamina gelir ve kullanici bunu mutlaka bilmelidir.
     */
    fun notifyIntegrityCompromised(findings: List<String>) {
        val open = PendingIntent.getActivity(
            context,
            INTEGRITY_REQUEST_CODE,
            Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            },
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val notification = NotificationCompat.Builder(context, UltraGuardApplication.CHANNEL_THREATS)
            .setSmallIcon(android.R.drawable.stat_sys_warning)
            .setContentTitle(context.getString(R.string.integrity_notification_title))
            .setStyle(
                NotificationCompat.BigTextStyle()
                    .bigText(context.getString(R.string.integrity_notification_body)),
            )
            .setPriority(NotificationCompat.PRIORITY_MAX)
            .setCategory(NotificationCompat.CATEGORY_ERROR)
            .setContentIntent(open)
            .setOngoing(true)
            .build()

        notificationManager.notify(NOTIFICATION_ID_INTEGRITY, notification)
    }

    fun clearThreat(verdictId: Long) {
        notificationManager.cancel(NOTIFICATION_ID_BASE + verdictId.toInt())
    }

    private fun appLabel(packageName: String): String = runCatching {
        val info = context.packageManager.getApplicationInfo(packageName, 0)
        context.packageManager.getApplicationLabel(info).toString()
    }.getOrDefault(packageName)

    private fun priorityFor(band: RiskBand): Int = when (band) {
        RiskBand.CRITICAL, RiskBand.HIGH -> NotificationCompat.PRIORITY_HIGH
        RiskBand.ELEVATED -> NotificationCompat.PRIORITY_DEFAULT
        else -> NotificationCompat.PRIORITY_LOW
    }

    private fun threatSummary(threatClass: ThreatClass): String {
        val resId = when (threatClass) {
            ThreatClass.BANKING_OVERLAY_TROJAN -> R.string.threat_banking_overlay
            ThreatClass.ACCESSIBILITY_ABUSE -> R.string.threat_a11y_abuse
            ThreatClass.STALKERWARE -> R.string.threat_stalkerware
            ThreatClass.SPYWARE_GENERIC -> R.string.threat_spyware
            ThreatClass.ADWARE_AGGRESSIVE -> R.string.threat_adware
            ThreatClass.DROPPER -> R.string.threat_dropper
            ThreatClass.FILELESS_LOADER -> R.string.threat_fileless
            ThreatClass.CREDENTIAL_PHISHING -> R.string.threat_phishing
            ThreatClass.SMS_FRAUD -> R.string.threat_sms_fraud
            ThreatClass.CRYPTO_CLIPPER -> R.string.threat_clipper
            ThreatClass.C2_BEACON -> R.string.threat_c2_beacon
            ThreatClass.DATA_EXFILTRATION -> R.string.threat_exfiltration
            ThreatClass.ROOT_EXPLOIT_ATTEMPT -> R.string.threat_root_exploit
            ThreatClass.POLICY_VIOLATION -> R.string.threat_policy
            ThreatClass.ANOMALOUS_BEHAVIOR -> R.string.threat_anomaly
            ThreatClass.NONE -> R.string.threat_none
        }
        return context.getString(resId)
    }

    private companion object {
        const val NOTIFICATION_ID_BASE = 2000
        const val NOTIFICATION_ID_INTEGRITY = 1999
        const val INTEGRITY_REQUEST_CODE = 9001
    }
}
