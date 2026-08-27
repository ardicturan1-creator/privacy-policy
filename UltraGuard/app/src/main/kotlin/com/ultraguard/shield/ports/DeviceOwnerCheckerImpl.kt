package com.ultraguard.shield.ports

import android.app.admin.DevicePolicyManager
import android.content.Context
import com.ultraguard.shield.response.DeviceOwnerChecker
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DeviceOwnerCheckerImpl @Inject constructor(
    @ApplicationContext private val context: Context,
) : DeviceOwnerChecker {
    override fun isDeviceOwner(): Boolean = runCatching {
        context.getSystemService(DevicePolicyManager::class.java)
            .isDeviceOwnerApp(context.packageName)
    }.getOrDefault(false)
}
