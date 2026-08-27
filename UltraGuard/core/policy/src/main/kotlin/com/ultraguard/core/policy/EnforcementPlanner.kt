package com.ultraguard.core.policy

import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.EnforcementAction
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.RiskBand
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.model.Verdict
import javax.inject.Inject

/**
 * Bir hukumden yaptirim planini turetir.
 *
 * Urunun etik omurgasi burada kodlanmistir:
 *
 *  - **Otonom uygulanan her eylem geri alinabilir olmak zorundadir.**
 *    [EnforcementAction.reversible] `false` olan hicbir eylem otonom kuyruga
 *    girmez; kullanici onayina yonlendirilir. Yanlis pozitifin kullaniciya
 *    kalici zarar vermesi boylece **yapisal olarak** imkansizdir.
 *  - Cihazin sahip olmadigi yetki gerektiren eylemler plana alinir ama
 *    `SKIPPED_NO_CAPABILITY` olarak isaretlenir; kullanici neyin
 *    yapilamadigini gorur. Sessizce atlanan koruma, olmayan korumadir.
 */
class EnforcementPlanner @Inject constructor() {

    fun plan(
        verdict: Verdict,
        mode: ProtectionMode,
        uid: Int,
        capabilities: Set<Capability>,
        runningPid: Int?,
    ): EnforcementPlan {
        val candidates = candidateActions(verdict, uid, runningPid)

        val reachesThreshold = verdict.score >= mode.autonomousThreshold
        if (!reachesThreshold) {
            // Esigin altinda: hicbir sey uygulanmaz, yalnizca gozlem surer.
            // Kullaniciya bildirim gidip gitmeyecegine mod karar verir.
            return EnforcementPlan(
                autonomous = emptyList(),
                requiresConsent = emptyList(),
                unavailable = emptyList(),
                notifyUser = verdict.score.band >= mode.notifyFrom,
                escalateMonitoring = verdict.score.band >= RiskBand.ELEVATED,
            )
        }

        val (supported, unsupported) = candidates.partition { it.requiredCapability in capabilities }
        val (autonomous, consentNeeded) = supported.partition { it.reversible }

        return EnforcementPlan(
            autonomous = autonomous,
            requiresConsent = consentNeeded,
            unavailable = unsupported,
            notifyUser = verdict.score.band >= mode.notifyFrom,
            escalateMonitoring = true,
        )
    }

    /**
     * Tehdit sinifina gore hangi yaptirimlarin anlamli oldugunu belirler.
     * Sira onemlidir: en hizli etki eden ve en az yikici olan once gelir.
     */
    private fun candidateActions(
        verdict: Verdict,
        uid: Int,
        runningPid: Int?,
    ): List<EnforcementAction> {
        val packageName = verdict.packageName

        return buildList {
            when (verdict.threatClass) {
                ThreatClass.BANKING_OVERLAY_TROJAN,
                ThreatClass.CREDENTIAL_PHISHING,
                -> {
                    // Overlay'i gizlemek saldiriyi aninda etkisiz kilar ve
                    // kullanici hicbir sey kaybetmez; ilk hamle her zaman budur.
                    add(EnforcementAction.HideOverlays(packageName))
                    add(EnforcementAction.SuspendNetwork(packageName, uid))
                    runningPid?.let { add(EnforcementAction.FreezeProcess(packageName, it)) }
                    add(EnforcementAction.SuspendPackage(packageName))
                    add(EnforcementAction.RequestUninstall(packageName))
                }

                ThreatClass.STALKERWARE,
                ThreatClass.SPYWARE_GENERIC,
                -> {
                    // Casus yazilimda oncelik veri akisini kesmektir; toplanan
                    // veri cihazi terk etmedigi surece zarar sinirlanabilir.
                    add(EnforcementAction.SuspendNetwork(packageName, uid))
                    add(EnforcementAction.RevokePermission(packageName, PERM_LOCATION))
                    add(EnforcementAction.RevokePermission(packageName, PERM_MICROPHONE))
                    add(EnforcementAction.RevokePermission(packageName, PERM_CAMERA))
                    add(EnforcementAction.GuideUserToRevoke(packageName, PERM_LOCATION))
                    add(EnforcementAction.RequestUninstall(packageName))
                }

                ThreatClass.C2_BEACON,
                ThreatClass.DATA_EXFILTRATION,
                ThreatClass.CRYPTO_CLIPPER,
                -> {
                    add(EnforcementAction.SuspendNetwork(packageName, uid))
                    runningPid?.let { add(EnforcementAction.FreezeProcess(packageName, it)) }
                    add(EnforcementAction.RequestUninstall(packageName))
                }

                ThreatClass.ACCESSIBILITY_ABUSE -> {
                    add(EnforcementAction.HideOverlays(packageName))
                    add(EnforcementAction.SuspendNetwork(packageName, uid))
                    add(EnforcementAction.GuideUserToRevoke(packageName, PERM_ACCESSIBILITY))
                }

                ThreatClass.DROPPER,
                ThreatClass.FILELESS_LOADER,
                ThreatClass.ROOT_EXPLOIT_ATTEMPT,
                -> {
                    runningPid?.let { add(EnforcementAction.FreezeProcess(packageName, it)) }
                    add(EnforcementAction.SuspendNetwork(packageName, uid))
                    add(EnforcementAction.SuspendPackage(packageName))
                    add(EnforcementAction.RequestUninstall(packageName))
                }

                ThreatClass.SMS_FRAUD -> {
                    add(EnforcementAction.SuspendNetwork(packageName, uid))
                    add(EnforcementAction.RevokePermission(packageName, PERM_SMS))
                    add(EnforcementAction.GuideUserToRevoke(packageName, PERM_SMS))
                }

                ThreatClass.ADWARE_AGGRESSIVE -> {
                    add(EnforcementAction.HideOverlays(packageName))
                }

                // Anomali ve politika bulgulari otonom yaptirima yol acmaz:
                // "tuhaf" olmak "zararli" olmak degildir. Kullanici bilgilendirilir.
                ThreatClass.ANOMALOUS_BEHAVIOR,
                ThreatClass.POLICY_VIOLATION,
                ThreatClass.NONE,
                -> Unit
            }
        }
    }

    private companion object {
        const val PERM_LOCATION = "android.permission.ACCESS_FINE_LOCATION"
        const val PERM_MICROPHONE = "android.permission.RECORD_AUDIO"
        const val PERM_CAMERA = "android.permission.CAMERA"
        const val PERM_SMS = "android.permission.RECEIVE_SMS"
        const val PERM_ACCESSIBILITY = "android.permission.BIND_ACCESSIBILITY_SERVICE"
    }
}

/**
 * Bir hukmun yaptirim plani.
 *
 * Ucluye bolunmus olmasi kasitlidir: kullaniciya "ne yaptim", "onayin
 * gerekiyor" ve "bu cihazda yapamadigim" ayri ayri gosterilir.
 */
data class EnforcementPlan(
    /** Hemen, onay sormadan uygulanir. Tamami geri alinabilir. */
    val autonomous: List<EnforcementAction>,
    /** Geri alinamaz; kullanici onayi olmadan asla uygulanmaz. */
    val requiresConsent: List<EnforcementAction>,
    /** Gerekli yetki (root / Device Owner) bulunmadigi icin uygulanamaz. */
    val unavailable: List<EnforcementAction>,
    val notifyUser: Boolean,
    /** Izleme durumunu CONTAINMENT'a yukseltmeli miyiz? */
    val escalateMonitoring: Boolean,
) {
    val isEmpty: Boolean
        get() = autonomous.isEmpty() && requiresConsent.isEmpty() && unavailable.isEmpty()

    init {
        require(autonomous.all { it.reversible }) {
            "Geri alinamaz bir eylem otonom kuyruga alinamaz: " +
                autonomous.filterNot { it.reversible }.joinToString { it::class.simpleName.orEmpty() }
        }
    }
}
