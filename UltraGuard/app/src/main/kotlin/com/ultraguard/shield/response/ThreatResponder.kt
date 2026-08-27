package com.ultraguard.shield.response

import android.content.Context
import android.content.pm.PackageManager
import com.ultraguard.core.common.di.ApplicationScope
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.datastore.SettingsStore
import com.ultraguard.core.engine.MonitoringStateMachine
import com.ultraguard.core.engine.ThreatPipeline
import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.Verdict
import com.ultraguard.core.policy.EnforcementExecutor
import com.ultraguard.core.policy.EnforcementPlanner
import com.ultraguard.core.security.RootDetector
import com.ultraguard.core.security.SelfProtection
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach

/**
 * Karar zincirinin son halkasi: hukum → plan → yaptirim → bildirim.
 *
 * Bu sinif olmadan motor "goruyor ama yapmiyor" durumundadir. Buradaki tek
 * sorumluluk baglamaktir; karar [EnforcementPlanner]'in, uygulama
 * [EnforcementExecutor]'un isidir.
 */
@Singleton
class ThreatResponder @Inject constructor(
    @ApplicationContext private val context: Context,
    private val pipeline: ThreatPipeline,
    private val planner: EnforcementPlanner,
    private val executor: EnforcementExecutor,
    private val notifier: ThreatNotifier,
    private val stateMachine: MonitoringStateMachine,
    private val settingsStore: SettingsStore,
    private val rootDetector: RootDetector,
    private val selfProtection: SelfProtection,
    private val deviceOwnerChecker: DeviceOwnerChecker,
    @ApplicationScope private val scope: CoroutineScope,
) {

    /** Yetenek tespiti pahalidir (dosya sistemi taramasi); bir kez yapilir. */
    private val capabilities: Set<Capability> by lazy {
        rootDetector.detect().grantedCapabilities(deviceOwnerChecker.isDeviceOwner())
    }

    fun start() {
        pipeline.verdicts
            .onEach(::respond)
            .launchIn(scope)
    }

    private suspend fun respond(verdict: Verdict) {
        val settings = settingsStore.settings.first()

        // Kendi butunlugumuzden emin degilsek yaptirim uygulamayiz.
        // Kandirilmis bir motorun karar vermesindense hic karar vermemesi
        // yegdir: hooking cercevesi altinda "engelledim" demek, kullaniciya
        // olmayan bir koruma vaat etmektir.
        val selfStatus = selfProtection.assess()
        if (selfStatus.isCompromised) {
            UgLog.w(TAG, "Butunluk bulgusu var; yaptirim askiya alindi, yalnizca bildirim")
            notifier.notifyIntegrityCompromised(selfStatus.findings.map { it.name })
            notifier.notifyThreat(verdict, appliedActionCount = 0)
            return
        }

        val uid = uidOf(verdict.packageName) ?: return
        val plan = planner.plan(
            verdict = verdict,
            mode = settings.mode,
            uid = uid,
            capabilities = capabilities,
            runningPid = null, // surec kimligi yalnizca [R] modulunde cozulur
        )

        if (plan.isEmpty && !plan.notifyUser) return

        val report = executor.execute(plan, settings.mode, verdict.id)

        if (plan.escalateMonitoring) {
            stateMachine.onVerdict(verdict.packageName, verdict.score.band)
        }

        if (plan.notifyUser) {
            notifier.notifyThreat(verdict, appliedActionCount = report.applied.size)
        }
    }

    private fun uidOf(packageName: String): Int? = runCatching {
        context.packageManager
            .getApplicationInfo(packageName, PackageManager.ApplicationInfoFlags.of(0))
            .uid
    }.getOrNull()

    private companion object {
        const val TAG = "ThreatResponder"
    }
}

/** Device Owner (kurumsal yonetim) durumunun tespiti. */
interface DeviceOwnerChecker {
    fun isDeviceOwner(): Boolean
}
