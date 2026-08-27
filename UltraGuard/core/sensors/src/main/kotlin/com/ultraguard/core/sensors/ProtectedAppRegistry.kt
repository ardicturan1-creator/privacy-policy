package com.ultraguard.core.sensors

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import java.util.concurrent.atomic.AtomicReference
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Finansal Kalkan'in hedef listesi: hangi uygulamalar "korunan" sayilir.
 *
 * Liste uc kaynaktan beslenir:
 *  1. Cihazda kurulu ve bilinen bankacilik/odeme paketleri (OTA guncellenen
 *     kategori listesi).
 *  2. Sistemin kendi sinifladigi kategoriler (`ApplicationInfo.category`).
 *  3. Kullanicinin elle isaretledigi uygulamalar -- yerel bir banka veya
 *     kurumsal bir cuzdan listemizde olmayabilir; kullanici onu ekleyebilir.
 */
@Singleton
class ProtectedAppRegistry @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    private val foregroundApp = AtomicReference<String?>(null)
    private val userMarked = mutableSetOf<String>()

    fun setForegroundApp(packageName: String) {
        foregroundApp.set(packageName)
    }

    fun currentForegroundApp(): String? = foregroundApp.get()

    fun isProtected(packageName: String): Boolean =
        packageName in userMarked || packageName in KNOWN_FINANCIAL_PACKAGES

    fun categoryOf(packageName: String): String = when {
        packageName in KNOWN_FINANCIAL_PACKAGES || packageName in userMarked -> "financial"
        else -> "general"
    }

    @Synchronized
    fun markProtected(packageName: String) {
        userMarked += packageName
    }

    @Synchronized
    fun unmarkProtected(packageName: String) {
        userMarked -= packageName
    }

    private companion object {
        /**
         * Yer tutucu cekirdek liste. Uretimde bu, imzali OTA kategori
         * paketiyle gelir; buraya sabit kodlanmis bir liste hicbir zaman
         * yeterli olmaz -- her ulkenin kendi bankalari vardir.
         */
        val KNOWN_FINANCIAL_PACKAGES = setOf(
            "com.google.android.apps.walletnfcrel",
            "com.paypal.android.p2pmobile",
        )
    }
}
