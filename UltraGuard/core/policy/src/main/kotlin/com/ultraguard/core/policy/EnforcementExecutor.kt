package com.ultraguard.core.policy

import com.ultraguard.core.common.di.IoDispatcher
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.model.ActionOutcome
import com.ultraguard.core.model.EnforcementAction
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.RevertActor
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.withContext

/**
 * Yaptirim planini fiilen uygular ve her adimi Action Ledger'a yazar.
 *
 * Degismezler:
 *  1. **Once defter, sonra eylem degil; once eylem, sonra defter.** Eylem
 *     basarisiz olursa `FAILED` olarak kaydedilir. Uygulanmamis bir eylemi
 *     "uygulandi" diye kaydetmek, kullaniciya korunuyormus izlenimi verir --
 *     sessiz ve tehlikeli bir yalan.
 *  2. **Geri alinamaz eylem otonom uygulanmaz.** [EnforcementPlan] bunu zaten
 *     yapisal olarak engeller; burada ikinci bir savunma katmani olarak
 *     tekrar dogrulanir.
 *  3. **Yetki yoksa sessizce atlanmaz.** `SKIPPED_NO_CAPABILITY` kaydi
 *     olusur ve kullanici arayuzunde "bu cihazda yapilamadi" olarak gorunur.
 */
@Singleton
class EnforcementExecutor @Inject constructor(
    private val ledger: ActionLedger,
    private val network: NetworkEnforcementPort,
    private val overlay: OverlayEnforcementPort,
    private val deviceAdmin: DeviceAdminEnforcementPort,
    private val process: ProcessEnforcementPort,
    private val userActions: UserActionPort,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) {

    suspend fun execute(
        plan: EnforcementPlan,
        mode: ProtectionMode,
        verdictId: Long?,
    ): ExecutionReport = withContext(ioDispatcher) {
        val applied = mutableListOf<EnforcementAction>()
        val failed = mutableListOf<EnforcementAction>()

        plan.autonomous.forEach { action ->
            check(action.reversible) {
                "Geri alinamaz eylem otonom uygulanamaz: ${action::class.simpleName}"
            }
            val success = runCatching { apply(action) }.getOrElse { error ->
                UgLog.w(TAG, "Yaptirim uygulanamadi: ${action::class.simpleName}", error)
                false
            }
            ledger.append(
                action = action,
                outcome = if (success) ActionOutcome.APPLIED else ActionOutcome.FAILED,
                mode = mode,
                verdictId = verdictId,
            )
            if (success) applied += action else failed += action
        }

        // Onay bekleyen eylemler de deftere yazilir: kullaniciya ne
        // onerdigimiz, onerinin ne zaman yapildigi kadar onemlidir.
        plan.requiresConsent.forEach { action ->
            ledger.append(action, ActionOutcome.AWAITING_USER_CONSENT, mode, verdictId)
        }

        plan.unavailable.forEach { action ->
            ledger.append(action, ActionOutcome.SKIPPED_NO_CAPABILITY, mode, verdictId)
        }

        ExecutionReport(
            applied = applied,
            failed = failed,
            awaitingConsent = plan.requiresConsent,
            unavailable = plan.unavailable,
        )
    }

    /**
     * Kullanici onayindan sonra, geri alinamaz bir eylemi uygular.
     * Yalnizca kullanici arayuzunden, acik bir dokunusla cagrilir.
     */
    suspend fun executeWithConsent(
        action: EnforcementAction,
        mode: ProtectionMode,
        verdictId: Long?,
    ): Boolean = withContext(ioDispatcher) {
        val success = runCatching { apply(action) }.getOrDefault(false)
        ledger.append(
            action = action,
            outcome = if (success) ActionOutcome.APPLIED else ActionOutcome.FAILED,
            mode = mode,
            verdictId = verdictId,
        )
        success
    }

    /**
     * Bir yaptirimi geri alir.
     *
     * Defter kaydi isaretlendikten sonra sistem durumu da eski haline
     * getirilir. Sira onemlidir: defter once isaretlenir, boylece geri alma
     * sirasinda uygulama olurse kayit "geri alinmis" kalir ve bir sonraki
     * acilista tutarsizlik uzlastirilabilir.
     */
    suspend fun revert(entryId: Long, actor: RevertActor): Boolean = withContext(ioDispatcher) {
        val action = ledger.revert(entryId, actor) ?: return@withContext false
        runCatching { undo(action) }.getOrElse { error ->
            UgLog.w(TAG, "Geri alma basarisiz: ${action::class.simpleName}", error)
            false
        }
    }

    private suspend fun apply(action: EnforcementAction): Boolean = when (action) {
        is EnforcementAction.SuspendNetwork -> {
            network.blockUid(action.uid, action.untilMillis)
            true
        }
        is EnforcementAction.HideOverlays -> overlay.hideOverlaysFor(action.packageName)
        is EnforcementAction.SuspendPackage -> deviceAdmin.suspendPackage(action.packageName)
        is EnforcementAction.FreezeProcess -> process.freeze(action.pid)
        is EnforcementAction.RevokePermission ->
            deviceAdmin.revokePermission(action.packageName, action.permission)
        is EnforcementAction.GuideUserToRevoke -> {
            userActions.openPermissionSettings(action.packageName, action.permission)
            true
        }
        is EnforcementAction.RequestUninstall -> {
            userActions.requestUninstall(action.packageName)
            true
        }
    }

    private suspend fun undo(action: EnforcementAction): Boolean = when (action) {
        is EnforcementAction.SuspendNetwork -> {
            network.unblockUid(action.uid)
            true
        }
        is EnforcementAction.HideOverlays -> {
            overlay.stopHidingOverlaysFor(action.packageName)
            true
        }
        is EnforcementAction.SuspendPackage -> deviceAdmin.unsuspendPackage(action.packageName)
        is EnforcementAction.FreezeProcess -> process.unfreeze(action.pid)
        is EnforcementAction.RevokePermission ->
            deviceAdmin.grantPermission(action.packageName, action.permission)

        // Bu ikisi yalnizca kullaniciya bir ekran acar; geri alinacak bir
        // sistem durumu birakmazlar.
        is EnforcementAction.GuideUserToRevoke -> true
        is EnforcementAction.RequestUninstall -> false
    }

    private companion object {
        const val TAG = "EnforcementExecutor"
    }
}

data class ExecutionReport(
    val applied: List<EnforcementAction>,
    val failed: List<EnforcementAction>,
    val awaitingConsent: List<EnforcementAction>,
    val unavailable: List<EnforcementAction>,
) {
    val didSomething: Boolean get() = applied.isNotEmpty()
}
