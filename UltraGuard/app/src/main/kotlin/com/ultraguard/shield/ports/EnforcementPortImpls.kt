package com.ultraguard.shield.ports

import android.app.admin.DevicePolicyManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.network.NetworkPolicyStore
import com.ultraguard.core.policy.DeviceAdminEnforcementPort
import com.ultraguard.core.policy.NetworkEnforcementPort
import com.ultraguard.core.policy.OverlayEnforcementPort
import com.ultraguard.core.policy.ProcessEnforcementPort
import com.ultraguard.core.policy.UserActionPort
import com.ultraguard.core.sensors.OverlaySuppressionRegistry
import com.ultraguard.module.deepscan.DeepScanBridge
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/** Ag engelleme: `:core:network` icindeki politika deposuna yazar. */
@Singleton
class NetworkEnforcementAdapter @Inject constructor(
    private val policyStore: NetworkPolicyStore,
) : NetworkEnforcementPort {
    override fun blockUid(uid: Int, untilMillis: Long?) = policyStore.blockUid(uid, untilMillis)
    override fun unblockUid(uid: Int) = policyStore.unblockUid(uid)
}

/**
 * Overlay bastirma.
 *
 * Android'de baska bir uygulamanin penceresini dogrudan kapatmak mumkun
 * degildir -- ve olmamalidir. Yapabildigimiz sey, korunan ekranlarimizda
 * `HIDE_OVERLAY_WINDOWS` ile tum ucuncu taraf overlay'lerini gizletmek ve
 * saldirgan paketi kayda alarak kullaniciyi uyarmaktir.
 *
 * Android 12 oncesinde bu API yoktur; o cihazlarda bastirma yerine yalnizca
 * uyari overlay'i gosterilir ve bu durum kullaniciya bildirilir.
 */
@Singleton
class OverlayEnforcementAdapter @Inject constructor(
    private val registry: OverlaySuppressionRegistry,
) : OverlayEnforcementPort {

    override fun hideOverlaysFor(packageName: String): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
            UgLog.i(TAG, "HIDE_OVERLAY_WINDOWS Android 12 oncesinde yok")
            return false
        }
        registry.suppress(packageName)
        return true
    }

    override fun stopHidingOverlaysFor(packageName: String) = registry.release(packageName)

    private companion object {
        const val TAG = "OverlayEnforcement"
    }
}

/**
 * Device Owner yaptirimlari.
 *
 * Yalnizca kurumsal olarak saglanmis cihazlarda (Fleet modu) calisir.
 * Bireysel cihazda her metot `false` doner ve plan bu eylemleri
 * "uygulanamadi" olarak isaretler -- sessizce atlanmaz.
 */
@Singleton
class DeviceAdminEnforcementAdapter @Inject constructor(
    @ApplicationContext private val context: Context,
) : DeviceAdminEnforcementPort {

    private val dpm = context.getSystemService(DevicePolicyManager::class.java)
    private val admin = ComponentName(context, UltraGuardDeviceAdminReceiver::class.java)

    private fun isOwner(): Boolean =
        runCatching { dpm.isDeviceOwnerApp(context.packageName) }.getOrDefault(false)

    override fun suspendPackage(packageName: String): Boolean = withOwner {
        dpm.setPackagesSuspended(admin, arrayOf(packageName), true).isEmpty()
    }

    override fun unsuspendPackage(packageName: String): Boolean = withOwner {
        dpm.setPackagesSuspended(admin, arrayOf(packageName), false).isEmpty()
    }

    override fun revokePermission(packageName: String, permission: String): Boolean = withOwner {
        dpm.setPermissionGrantState(
            admin,
            packageName,
            permission,
            DevicePolicyManager.PERMISSION_GRANT_STATE_DENIED,
        )
    }

    override fun grantPermission(packageName: String, permission: String): Boolean = withOwner {
        dpm.setPermissionGrantState(
            admin,
            packageName,
            permission,
            DevicePolicyManager.PERMISSION_GRANT_STATE_DEFAULT,
        )
    }

    private inline fun withOwner(block: () -> Boolean): Boolean {
        if (!isOwner()) return false
        return runCatching(block).getOrElse { error ->
            UgLog.w(TAG, "Device Owner yaptirimi basarisiz", error)
            false
        }
    }

    private companion object {
        const val TAG = "DeviceAdminEnforcement"
    }
}

/** Surec dondurma: yalnizca root/KernelSU modulu kuruluysa. */
@Singleton
class ProcessEnforcementAdapter @Inject constructor(
    private val deepScan: DeepScanBridge,
) : ProcessEnforcementPort {
    override suspend fun freeze(pid: Int): Boolean =
        if (deepScan.isAvailable()) deepScan.freezeProcess(pid) else false

    override suspend fun unfreeze(pid: Int): Boolean =
        if (deepScan.isAvailable()) deepScan.unfreezeProcess(pid) else false
}

/**
 * Kullanici onayi gerektiren sistem akislari.
 *
 * Buradaki hicbir cagri bir seyi kendiliginden yapmaz; yalnizca sistemin
 * kendi onay ekranini acar. Son sozu her zaman kullanici soyler.
 */
@Singleton
class UserActionAdapter @Inject constructor(
    @ApplicationContext private val context: Context,
) : UserActionPort {

    override fun requestUninstall(packageName: String) {
        val intent = Intent(Intent.ACTION_DELETE).apply {
            data = Uri.fromParts("package", packageName, null)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        launch(intent, "kaldirma ekrani")
    }

    override fun openPermissionSettings(packageName: String, permission: String) {
        // Erisilebilirlik icin ayri bir sistem ekrani vardir; dogrudan
        // uygulama detayina yonlendirmek kullaniciyi yanlis yere goturur.
        val intent = if (permission.contains("ACCESSIBILITY")) {
            Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)
        } else {
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.fromParts("package", packageName, null)
            }
        }.apply { addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }

        launch(intent, "izin ekrani")
    }

    private fun launch(intent: Intent, description: String) {
        runCatching { context.startActivity(intent) }.onFailure { error ->
            UgLog.w(TAG, "$description acilamadi", error)
        }
    }

    private companion object {
        const val TAG = "UserAction"
    }
}
