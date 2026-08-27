package com.ultraguard.core.policy

import com.google.common.truth.Truth.assertThat
import com.ultraguard.core.model.Attribution
import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.DecisionTier
import com.ultraguard.core.model.EnforcementAction
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.model.Verdict
import org.junit.Test

class EnforcementPlannerTest {

    private val planner = EnforcementPlanner()

    private fun verdict(
        score: Int,
        threatClass: ThreatClass = ThreatClass.BANKING_OVERLAY_TROJAN,
    ) = Verdict(
        packageName = "com.example.evil",
        createdAtMillis = 1_700_000_000_000L,
        tier = DecisionTier.RULE_ENGINE,
        score = RiskScore(score),
        threatClass = threatClass,
        confidence = 1f,
        attributions = listOf(
            Attribution(1L, EventType.OVERLAY_DRAWN, 1f, "expl_overlay"),
        ),
        originId = "R-A11Y-002",
    )

    @Test
    fun `otonom kuyrukta geri alinamaz eylem asla bulunmaz`() {
        val plan = planner.plan(
            verdict = verdict(95),
            mode = ProtectionMode.ACTIVE,
            uid = 10234,
            capabilities = setOf(Capability.UNROOTED, Capability.ENTERPRISE, Capability.ROOTED),
            runningPid = 4242,
        )

        assertThat(plan.autonomous).isNotEmpty()
        assertThat(plan.autonomous.all { it.reversible }).isTrue()
    }

    @Test
    fun `kaldirma her zaman kullanici onayina birakilir`() {
        val plan = planner.plan(
            verdict = verdict(99),
            mode = ProtectionMode.PARANOID,
            uid = 10234,
            capabilities = setOf(Capability.UNROOTED, Capability.ENTERPRISE, Capability.ROOTED),
            runningPid = 4242,
        )

        assertThat(plan.autonomous.filterIsInstance<EnforcementAction.RequestUninstall>()).isEmpty()
        assertThat(plan.requiresConsent.filterIsInstance<EnforcementAction.RequestUninstall>())
            .hasSize(1)
    }

    @Test
    fun `esigin altindaki hukum hicbir yaptirim uretmez`() {
        val plan = planner.plan(
            verdict = verdict(60),
            mode = ProtectionMode.ACTIVE, // esik 75
            uid = 10234,
            capabilities = setOf(Capability.UNROOTED),
            runningPid = null,
        )

        assertThat(plan.autonomous).isEmpty()
        assertThat(plan.requiresConsent).isEmpty()
    }

    @Test
    fun `paranoid mod ayni hukumde aktif moddan daha erken mudahale eder`() {
        val v = verdict(60)
        val active = planner.plan(v, ProtectionMode.ACTIVE, 10234, setOf(Capability.UNROOTED), null)
        val paranoid = planner.plan(v, ProtectionMode.PARANOID, 10234, setOf(Capability.UNROOTED), null)

        assertThat(active.autonomous).isEmpty()
        assertThat(paranoid.autonomous).isNotEmpty()
    }

    @Test
    fun `rootsuz cihazda surec dondurma uygulanamaz olarak isaretlenir`() {
        val plan = planner.plan(
            verdict = verdict(95),
            mode = ProtectionMode.ACTIVE,
            uid = 10234,
            capabilities = setOf(Capability.UNROOTED),
            runningPid = 4242,
        )

        assertThat(plan.autonomous.filterIsInstance<EnforcementAction.FreezeProcess>()).isEmpty()
        assertThat(plan.unavailable.filterIsInstance<EnforcementAction.FreezeProcess>()).hasSize(1)
    }

    @Test
    fun `anomali sinifi otonom yaptirim uretmez`() {
        val plan = planner.plan(
            verdict = verdict(95, ThreatClass.ANOMALOUS_BEHAVIOR),
            mode = ProtectionMode.ACTIVE,
            uid = 10234,
            capabilities = setOf(Capability.UNROOTED, Capability.ROOTED),
            runningPid = 4242,
        )

        assertThat(plan.autonomous).isEmpty()
        // Ama kullanici yine de bilgilendirilir.
        assertThat(plan.notifyUser).isTrue()
    }

    @Test
    fun `stealth mod yalnizca kritik bandda bildirim gonderir`() {
        val elevated = planner.plan(
            verdict(60), ProtectionMode.STEALTH, 10234, setOf(Capability.UNROOTED), null,
        )
        val critical = planner.plan(
            verdict(95), ProtectionMode.STEALTH, 10234, setOf(Capability.UNROOTED), null,
        )

        assertThat(elevated.notifyUser).isFalse()
        assertThat(critical.notifyUser).isTrue()
    }

    @Test
    fun `overlay gizleme bankacilik tehdidinde ilk hamledir`() {
        val plan = planner.plan(
            verdict(95), ProtectionMode.ACTIVE, 10234, setOf(Capability.UNROOTED), null,
        )
        assertThat(plan.autonomous.first()).isInstanceOf(EnforcementAction.HideOverlays::class.java)
    }
}
