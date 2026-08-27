package com.ultraguard.shield.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import dagger.hilt.android.AndroidEntryPoint

/**
 * Yeniden baslatma sonrasi korumayi geri getirir.
 *
 * Cihazin acildigi ilk dakikalar, kalici kotucul yazilimin en avantajli
 * anidir: `RECEIVE_BOOT_COMPLETED` ile kendini baslatan bir implant, biz
 * calismaya baslamadan once hareket edebilir. Bu yuzden motor, kullanici
 * uygulamayi acmayi beklemeden ayaga kalkar.
 */
@AndroidEntryPoint
class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context?, intent: Intent?) {
        if (intent?.action != Intent.ACTION_BOOT_COMPLETED) return
        val appContext = context?.applicationContext ?: return
        ProtectionService.start(appContext)
    }
}
