package com.ultraguard.core.engine

import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.MonitoringState
import com.ultraguard.core.model.RiskBand
import com.ultraguard.core.model.SecurityEvent
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Paket bazinda izleme yogunlugu durum makinesi.
 *
 * Pil butcesinin tamami buna baglidir. BASELINE'da olaylarin yalnizca
 * %5'i pahali L2 kademesine cikar; HEIGHTENED'da hepsi cikar ama bu durum
 * **her zaman sure sinirlidir**. Surekli yuksek tuketim mimari olarak
 * olusamaz: HEIGHTENED'a giren her paket [HEIGHTENED_DURATION_MILLIS]
 * sonunda otomatik olarak BASELINE'a doner.
 */
@Singleton
class MonitoringStateMachine @Inject constructor(
    private val clock: Clock,
) {
    private data class PackageState(
        val state: MonitoringState,
        val enteredAtMillis: Long,
        val reason: EscalationReason,
    )

    private val states = mutableMapOf<String, PackageState>()

    private val _globalState = MutableStateFlow(MonitoringState.BASELINE)
    val globalState: StateFlow<MonitoringState> = _globalState.asStateFlow()

    @Synchronized
    fun stateFor(packageName: String): MonitoringState {
        val current = states[packageName] ?: return MonitoringState.BASELINE
        val now = clock.elapsedRealtimeMillis()

        // Sure dolmus mu? CONTAINMENT elle dusurulur; HEIGHTENED kendiliginden.
        if (current.state == MonitoringState.HEIGHTENED &&
            now - current.enteredAtMillis > HEIGHTENED_DURATION_MILLIS
        ) {
            states.remove(packageName)
            recomputeGlobal()
            return MonitoringState.BASELINE
        }
        return current.state
    }

    /**
     * Yeni bir olayin yogunlugu artirip artirmadigina karar verir.
     *
     * @return durum degistiyse yeni durum, degismediyse `null`.
     */
    @Synchronized
    fun onEvent(event: SecurityEvent): MonitoringState? {
        val packageName = event.packageName ?: return null
        val reason = escalationReasonFor(event) ?: return null
        return escalate(packageName, MonitoringState.HEIGHTENED, reason)
    }

    @Synchronized
    fun onVerdict(packageName: String, band: RiskBand): MonitoringState? = when {
        band >= RiskBand.HIGH ->
            escalate(packageName, MonitoringState.CONTAINMENT, EscalationReason.ACTIVE_ENFORCEMENT)
        band >= RiskBand.ELEVATED ->
            escalate(packageName, MonitoringState.HEIGHTENED, EscalationReason.ELEVATED_RISK)
        else -> null
    }

    /** Tehdit giderildiginde CONTAINMENT'tan cikis. Yalnizca acikca cagrilir. */
    @Synchronized
    fun deescalate(packageName: String) {
        states.remove(packageName)
        recomputeGlobal()
    }

    @Synchronized
    fun packagesUnderContainment(): Set<String> =
        states.filterValues { it.state == MonitoringState.CONTAINMENT }.keys.toSet()

    private fun escalate(
        packageName: String,
        target: MonitoringState,
        reason: EscalationReason,
    ): MonitoringState? {
        val current = states[packageName]?.state ?: MonitoringState.BASELINE
        // CONTAINMENT'tan HEIGHTENED'a dusulmez; yalnizca yukari yonlu gecis.
        if (current.ordinal >= target.ordinal) return null

        states[packageName] = PackageState(target, clock.elapsedRealtimeMillis(), reason)
        recomputeGlobal()
        return target
    }

    private fun recomputeGlobal() {
        _globalState.value = states.values
            .maxByOrNull { it.state.ordinal }
            ?.state
            ?: MonitoringState.BASELINE
    }

    /**
     * Hangi olaylar yogunlugu artirir?
     *
     * Liste kasitli olarak kisadir. Her ek tur, pil butcesine dogrudan
     * yansir; buraya bir olay eklemek olculmus bir karar olmalidir.
     */
    private fun escalationReasonFor(event: SecurityEvent): EscalationReason? = when (event.type) {
        EventType.PACKAGE_INSTALLED,
        EventType.PACKAGE_SIDELOAD_DETECTED,
        EventType.PACKAGE_SIGNATURE_CHANGED,
        -> EscalationReason.NEW_PACKAGE

        EventType.ACCESSIBILITY_SERVICE_ENABLED,
        EventType.ACCESSIBILITY_GESTURE_CAPABILITY,
        -> EscalationReason.ACCESSIBILITY_GRANTED

        EventType.OVERLAY_ON_PROTECTED_SCREEN,
        EventType.MEDIA_PROJECTION_STARTED,
        -> EscalationReason.PROTECTED_SCREEN_TOUCHED

        EventType.NETWORK_REPUTATION_HIT,
        EventType.NETWORK_DGA_DOMAIN,
        -> EscalationReason.SUSPICIOUS_NETWORK

        else -> null
    }

    companion object {
        /**
         * Yeni bir paketin yogun gozlem penceresi. Onbir dakikalik bir
         * pencere daha cok sey gorur ama 24 saatlik pil butcesini asar;
         * 10 dakika, olculmus bir denge noktasidir.
         */
        const val HEIGHTENED_DURATION_MILLIS = 10 * 60 * 1000L
    }
}

enum class EscalationReason {
    NEW_PACKAGE,
    ACCESSIBILITY_GRANTED,
    PROTECTED_SCREEN_TOUCHED,
    SUSPICIOUS_NETWORK,
    ELEVATED_RISK,
    ACTIVE_ENFORCEMENT,
}
