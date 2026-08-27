package com.ultraguard.core.policy

/**
 * Yaptirimin sistem tarafindaki uclari.
 *
 * `:core:policy` neyin yapilmasi gerektigine karar verir ama **nasil**
 * yapildigini bilmez. Ag engelleme `:core:network`'te, paket askiya alma
 * `:app` katmanindaki Device Owner sarmalayicisinda, surec dondurma ise
 * opsiyonel `:module:deepscan` APK'sinda yasar.
 *
 * Bu ayrim iki sey kazandirir: politika katmani saf ve test edilebilir kalir,
 * ve bir yetenegin bulunmamasi (root yok, Device Owner yok) karari degil
 * yalnizca uygulamayi etkiler.
 */
interface NetworkEnforcementPort {
    fun blockUid(uid: Int, untilMillis: Long?)
    fun unblockUid(uid: Int)
}

interface OverlayEnforcementPort {
    /** Korunan ekranlarda ucuncu taraf pencerelerini gizler. */
    fun hideOverlaysFor(packageName: String): Boolean
    fun stopHidingOverlaysFor(packageName: String)
}

/** [Capability.ENTERPRISE] gerektirir; Device Owner yoksa `false` doner. */
interface DeviceAdminEnforcementPort {
    fun suspendPackage(packageName: String): Boolean
    fun unsuspendPackage(packageName: String): Boolean
    fun revokePermission(packageName: String, permission: String): Boolean
    fun grantPermission(packageName: String, permission: String): Boolean
}

/** [Capability.ROOTED] gerektirir; `:module:deepscan` yoksa `false` doner. */
interface ProcessEnforcementPort {
    suspend fun freeze(pid: Int): Boolean
    suspend fun unfreeze(pid: Int): Boolean
}

/**
 * Kullanicinin karar vermesi gereken eylemler icin sistem akislarini acar.
 * Bunlarin hicbiri otonom cagrilmaz -- her biri kullanici onayina yol acar.
 */
interface UserActionPort {
    fun requestUninstall(packageName: String)
    fun openPermissionSettings(packageName: String, permission: String)
}
