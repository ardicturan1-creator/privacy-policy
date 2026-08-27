package com.ultraguard.core.model

import kotlinx.serialization.Serializable

/**
 * UltraGuard'in bir tehdide karsi uygulayabilecegi yaptirim.
 *
 * Urunun en onemli guvenlik kisiti buradadir: **otonom olarak yalnizca
 * [reversible] eylemler uygulanir.** Uygulama kaldirmak veya veri silmek
 * geri alinamaz, dolayisiyla her zaman kullanici onayi ister. Bu, yanlis
 * pozitifin kullaniciya kalici zarar vermesini mimari olarak imkansiz kilar.
 */
@Serializable
sealed class EnforcementAction {
    abstract val packageName: String

    /** Otonom uygulanabilir mi? Geri alinamayan hicbir eylem otonom degildir. */
    abstract val reversible: Boolean

    /** Bu eylemin calismasi icin gereken yetki seviyesi. */
    abstract val requiredCapability: Capability

    /** Ag erisiminin VpnService duzeyinde UID bazli kesilmesi. */
    @Serializable
    data class SuspendNetwork(
        override val packageName: String,
        val uid: Int,
        val untilMillis: Long? = null,
    ) : EnforcementAction() {
        override val reversible = true
        override val requiredCapability = Capability.UNROOTED
    }

    /** Korunan ekranlarda ekran ustu pencerelerin gizlenmesi. */
    @Serializable
    data class HideOverlays(
        override val packageName: String,
    ) : EnforcementAction() {
        override val reversible = true
        override val requiredCapability = Capability.UNROOTED
    }

    /** Uygulamanin askiya alinmasi: Device Owner ile paket suspend. */
    @Serializable
    data class SuspendPackage(
        override val packageName: String,
    ) : EnforcementAction() {
        override val reversible = true
        override val requiredCapability = Capability.ENTERPRISE
    }

    /**
     * Surecin cgroup freezer ile dondurulmasi. Oldurmek yerine dondurmak
     * adli kaniti korur: bellekteki payload analiz icin erisilebilir kalir.
     */
    @Serializable
    data class FreezeProcess(
        override val packageName: String,
        val pid: Int,
    ) : EnforcementAction() {
        override val reversible = true
        override val requiredCapability = Capability.ROOTED
    }

    /** Calisma zamani izninin programatik iptali. */
    @Serializable
    data class RevokePermission(
        override val packageName: String,
        val permission: String,
    ) : EnforcementAction() {
        override val reversible = true
        override val requiredCapability = Capability.ENTERPRISE
    }

    /** Kullaniciyi sistem izin ekranina yonlendirir; karari kullanici verir. */
    @Serializable
    data class GuideUserToRevoke(
        override val packageName: String,
        val permission: String,
    ) : EnforcementAction() {
        override val reversible = true
        override val requiredCapability = Capability.UNROOTED
    }

    /**
     * Kaldirma akisi. **Otonom asla uygulanmaz** — sistem kaldirma diyalogu
     * acilir ve son sozu kullanici soyler.
     */
    @Serializable
    data class RequestUninstall(
        override val packageName: String,
    ) : EnforcementAction() {
        override val reversible = false
        override val requiredCapability = Capability.UNROOTED
    }
}

/** Bir yetenegin calismasi icin gereken cihaz yetki seviyesi. */
@Serializable
enum class Capability {
    /** [U] Standart kullanici cihazi. Urunun tamami bu seviyede islevseldir. */
    UNROOTED,

    /** [E] Device Owner / MDM saglanmis kurumsal cihaz. */
    ENTERPRISE,

    /** [R] Root / KernelSU. Yalnizca `:module:deepscan` kuruluysa. */
    ROOTED,
}

/**
 * Uygulanmis bir yaptirimin denetlenebilir kaydi.
 *
 * Action Ledger, hash zinciriyle korunur: her kaydin [previousHash] alani
 * bir onceki kaydin ozetidir. Kotu niyetli bir aktor gecmis bir kaydi
 * degistirirse zincir kirilir ve `LedgerIntegrityCheck` bunu yakalar.
 */
@Serializable
data class LedgerEntry(
    val id: Long = 0L,
    val timestampMillis: Long,
    val action: EnforcementAction,
    val triggeringVerdictId: Long?,
    val mode: ProtectionMode,
    val outcome: ActionOutcome,
    val revertedAtMillis: Long? = null,
    val revertedBy: RevertActor? = null,
    val previousHash: String,
    val hash: String,
) {
    val isActive: Boolean get() = outcome == ActionOutcome.APPLIED && revertedAtMillis == null
}

@Serializable
enum class ActionOutcome {
    APPLIED,
    /** Gerekli yetki (root/Device Owner) bulunmadigi icin uygulanamadi. */
    SKIPPED_NO_CAPABILITY,
    /** Kullanici onayi bekliyor — geri alinamaz eylemler burada durur. */
    AWAITING_USER_CONSENT,
    USER_DECLINED,
    FAILED,
}

@Serializable
enum class RevertActor { USER, SYSTEM_TIMEOUT, POLICY_CHANGE, FALSE_POSITIVE_FEEDBACK }
