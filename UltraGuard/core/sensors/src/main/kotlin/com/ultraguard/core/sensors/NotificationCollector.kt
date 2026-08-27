package com.ultraguard.core.sensors

import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.engine.EventBus
import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.Subject
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

/**
 * Bildirim telemetrisi.
 *
 * **Gizlilik sozlesmesi:** bildirim metni bu sinifin disina cikmaz. Metin
 * [SensitivePatternMatcher] tarafindan RAM'de siniflandirilir ve yalnizca
 * turetilmis etiket (`otp_code`, `urgency_phishing`) olaya yazilir. Mesaj
 * iceriginiz UltraGuard'in veritabaninda **yoktur**.
 *
 * Neden bu izne ihtiyacimiz var: OTP sizdirma saldirilarinda kotucul paket
 * bildirimi okur ve kodu saniyeler icinde disari gonderir. Bu diziyi
 * gormeden saldiriyi tespit etmenin baska bir yolu yoktur.
 */
@AndroidEntryPoint
class NotificationCollector : NotificationListenerService() {

    @Inject lateinit var eventBus: EventBus
    @Inject lateinit var clock: Clock
    @Inject lateinit var patternMatcher: SensitivePatternMatcher

    override fun onNotificationPosted(sbn: StatusBarNotification?) {
        val notification = sbn ?: return
        val packageName = notification.packageName ?: return
        if (packageName == this.packageName) return

        val now = clock.nowMillis()
        val uid = notification.uid

        eventBus.publish(
            SecurityEvent(
                timestampMillis = now,
                type = EventType.NOTIFICATION_POSTED,
                subject = Subject.App(packageName, uid),
                source = SensorSource.NOTIFICATION_LISTENER,
            ),
        )

        // Metin burada okunur, siniflandirilir ve **hemen unutulur**.
        val extras = notification.notification?.extras ?: return
        val classification = sequenceOf(
            extras.getCharSequence(android.app.Notification.EXTRA_TITLE),
            extras.getCharSequence(android.app.Notification.EXTRA_TEXT),
            extras.getCharSequence(android.app.Notification.EXTRA_BIG_TEXT),
        ).firstNotNullOfOrNull(patternMatcher::classify) ?: return

        eventBus.publish(
            SecurityEvent(
                timestampMillis = now,
                type = EventType.NOTIFICATION_PHISHING_PATTERN,
                subject = Subject.App(packageName, uid),
                source = SensorSource.NOTIFICATION_LISTENER,
                attributes = mapOf(EventAttributes.MATCHED_PATTERN to classification),
            ),
        )
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification?) = Unit
}
