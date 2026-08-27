package com.ultraguard.core.sensors

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.AccessibilityServiceInfo
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityWindowInfo
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
 * Erisilebilirlik tabanli arayuz telemetrisi.
 *
 * **Gizlilik sozlesmesi -- bu sinifin en onemli ozelligi:**
 * Bu servis ekrandaki metni okuyabilir. Okumuyoruz. Bu dosyada
 * `AccessibilityNodeInfo.getText()` cagrisi **yoktur ve eklenmemelidir**.
 * Ilgilendigimiz tek sey pencerelerin *sahipleri* ve *turleri*: hangi paket
 * on planda, kimin ustune kim ciziyor, hangi servis jest yetkisi aldi.
 *
 * Ekran icerigi hicbir zaman RAM'den cikmaz, kaydedilmez, gonderilmez.
 * Kullaniciya "bu izni sizi izlemek icin degil, sizi izleyeni gormek icin
 * istiyorum" derken kastettigimiz sey budur.
 */
@AndroidEntryPoint
class UltraGuardAccessibilityService : AccessibilityService() {

    @Inject lateinit var eventBus: EventBus
    @Inject lateinit var clock: Clock
    @Inject lateinit var protectedAppRegistry: ProtectedAppRegistry

    private var lastForegroundPackage: String? = null

    override fun onServiceConnected() {
        super.onServiceConnected()
        serviceInfo = AccessibilityServiceInfo().apply {
            eventTypes = AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED or
                AccessibilityEvent.TYPE_WINDOWS_CHANGED
            feedbackType = AccessibilityServiceInfo.FEEDBACK_GENERIC
            // `FLAG_RETRIEVE_INTERACTIVE_WINDOWS` pencere *listesini* verir,
            // icerigini degil. Overlay tespiti icin gereken minimum yetkidir.
            flags = AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS or
                AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS
            notificationTimeout = NOTIFICATION_TIMEOUT_MILLIS
        }
        auditInstalledAccessibilityServices()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        val accessibilityEvent = event ?: return
        val packageName = accessibilityEvent.packageName?.toString() ?: return
        val now = clock.nowMillis()

        when (accessibilityEvent.eventType) {
            AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED -> {
                if (packageName != lastForegroundPackage) {
                    lastForegroundPackage = packageName
                    publish(EventType.FOREGROUND_APP_CHANGED, packageName, now)
                    protectedAppRegistry.setForegroundApp(packageName)
                }
            }

            AccessibilityEvent.TYPE_WINDOWS_CHANGED -> inspectWindows(now)
        }
    }

    /**
     * Pencere yigininin denetimi.
     *
     * Iki sey aranir:
     *  1. On plandaki korunan uygulamanin (banka, odeme) ustune baska bir
     *     paketin cizmesi -- overlay saldirisinin calisma anindaki imzasi.
     *  2. Bir uygulamanin korunan bir pencereyi sorgulamasi.
     */
    private fun inspectWindows(nowMillis: Long) {
        val allWindows = runCatching { windows }.getOrNull().orEmpty()
        if (allWindows.isEmpty()) return

        val foreground = protectedAppRegistry.currentForegroundApp()
        val foregroundIsProtected = foreground != null &&
            protectedAppRegistry.isProtected(foreground)

        allWindows.forEach { window ->
            val ownerPackage = window.ownerPackage() ?: return@forEach
            if (ownerPackage == packageName) return@forEach // kendi uyari overlay'imiz

            val isOverlayType = window.type == AccessibilityWindowInfo.TYPE_APPLICATION &&
                ownerPackage != foreground

            if (!isOverlayType) return@forEach

            if (foregroundIsProtected) {
                publish(
                    type = EventType.OVERLAY_ON_PROTECTED_SCREEN,
                    packageName = ownerPackage,
                    nowMillis = nowMillis,
                    EventAttributes.WINDOW_OWNER to ownerPackage,
                    EventAttributes.TARGET_PACKAGE to foreground.orEmpty(),
                    EventAttributes.PROTECTED_CATEGORY to
                        protectedAppRegistry.categoryOf(foreground.orEmpty()),
                )
            } else {
                publish(
                    type = EventType.OVERLAY_DRAWN,
                    packageName = ownerPackage,
                    nowMillis = nowMillis,
                    EventAttributes.WINDOW_OWNER to ownerPackage,
                )
            }
        }
    }

    /**
     * Cihazdaki diger erisilebilirlik servislerinin denetimi.
     *
     * Bu, urunun en yuksek getirili tek kontrolu: kotucul bir paketin
     * `canPerformGestures` yetkisiyle kayitli olmasi, kullanicinin farkinda
     * olmadan onun adina dokunma yapilabilecegi anlamina gelir.
     */
    private fun auditInstalledAccessibilityServices() {
        val now = clock.nowMillis()
        val enabled = runCatching {
            val manager = getSystemService(android.view.accessibility.AccessibilityManager::class.java)
            manager.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK)
        }.getOrNull().orEmpty()

        enabled.forEach { info ->
            val servicePackage = info.resolveInfo?.serviceInfo?.packageName ?: return@forEach
            if (servicePackage == packageName) return@forEach

            publish(EventType.ACCESSIBILITY_SERVICE_ENABLED, servicePackage, now)

            val canPerformGestures =
                info.capabilities and AccessibilityServiceInfo.CAPABILITY_CAN_PERFORM_GESTURES != 0

            if (canPerformGestures) {
                publish(
                    type = EventType.ACCESSIBILITY_GESTURE_CAPABILITY,
                    packageName = servicePackage,
                    nowMillis = now,
                    EventAttributes.CAN_PERFORM_GESTURES to "true",
                )
            }
        }
    }

    private fun AccessibilityWindowInfo.ownerPackage(): String? =
        runCatching { root?.packageName?.toString() }.getOrNull()

    private fun publish(
        type: EventType,
        packageName: String,
        nowMillis: Long,
        vararg attributes: Pair<String, String>,
    ) {
        eventBus.publish(
            SecurityEvent(
                timestampMillis = nowMillis,
                type = type,
                subject = Subject.App(packageName, uidOf(packageName)),
                source = SensorSource.ACCESSIBILITY,
                attributes = attributes.toMap(),
            ),
        )
    }

    private fun uidOf(packageName: String): Int = runCatching {
        packageManager.getApplicationInfo(packageName, 0).uid
    }.getOrDefault(UNKNOWN_UID)

    override fun onInterrupt() = Unit

    private companion object {
        const val NOTIFICATION_TIMEOUT_MILLIS = 100L
        const val UNKNOWN_UID = -1
    }
}
