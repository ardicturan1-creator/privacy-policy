package com.ultraguard.shield.ports

import android.app.admin.DeviceAdminReceiver

/**
 * Fleet (kurumsal) modu icin cihaz yoneticisi alicisi.
 *
 * **Kurumsal gizlilik siniri:** UltraGuard Fleet, isverene cihazin
 * *uyumluluk durumunu* gosterir -- yama duzeyi, root durumu, riskli
 * uygulama sayisi. Kurulu uygulamalarin listesi, kisisel kullanim verisi
 * ve konum, BYOD cihazlarda isverene **aktarilmaz**. Bu alici yalnizca
 * politika uygulamak icin vardir, gozetim icin degil.
 */
class UltraGuardDeviceAdminReceiver : DeviceAdminReceiver()
