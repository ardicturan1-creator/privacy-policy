package com.ultraguard.core.engine

import com.google.common.truth.Truth.assertThat
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.MonitoringState
import com.ultraguard.core.model.RiskBand
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.Subject
import org.junit.Test

class MonitoringStateMachineTest {

    private val clock = FakeClock()
    private val machine = MonitoringStateMachine(clock)

    private fun event(type: EventType, pkg: String = "com.example.app") = SecurityEvent(
        timestampMillis = clock.nowMillis(),
        type = type,
        subject = Subject.App(pkg, 10001),
        source = SensorSource.PACKAGE_LIFECYCLE,
    )

    @Test
    fun `bilinmeyen paket baseline durumundadir`() {
        assertThat(machine.stateFor("com.unknown")).isEqualTo(MonitoringState.BASELINE)
    }

    @Test
    fun `yeni paket kurulumu yogunlugu artirir`() {
        machine.onEvent(event(EventType.PACKAGE_INSTALLED))
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.HEIGHTENED)
    }

    @Test
    fun `heightened durumu sure sonunda kendiliginden duser`() {
        machine.onEvent(event(EventType.PACKAGE_INSTALLED))
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.HEIGHTENED)

        clock.advance(MonitoringStateMachine.HEIGHTENED_DURATION_MILLIS + 1_000)

        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.BASELINE)
    }

    @Test
    fun `siradan olay yogunlugu artirmaz`() {
        machine.onEvent(event(EventType.NOTIFICATION_POSTED))
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.BASELINE)
    }

    @Test
    fun `yuksek riskli hukum containment durumuna gecirir`() {
        machine.onVerdict("com.example.app", RiskBand.CRITICAL)
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.CONTAINMENT)
    }

    @Test
    fun `containment kendiliginden dusmez`() {
        machine.onVerdict("com.example.app", RiskBand.CRITICAL)
        clock.advance(MonitoringStateMachine.HEIGHTENED_DURATION_MILLIS * 10)
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.CONTAINMENT)
    }

    @Test
    fun `containment yalnizca acikca dusurulur`() {
        machine.onVerdict("com.example.app", RiskBand.CRITICAL)
        machine.deescalate("com.example.app")
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.BASELINE)
    }

    @Test
    fun `containment icindeyken heightened olayi durumu dusurmez`() {
        machine.onVerdict("com.example.app", RiskBand.CRITICAL)
        machine.onEvent(event(EventType.PACKAGE_INSTALLED))
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.CONTAINMENT)
    }

    @Test
    fun `global durum en yuksek paket durumunu yansitir`() {
        machine.onEvent(event(EventType.PACKAGE_INSTALLED, "com.a"))
        assertThat(machine.globalState.value).isEqualTo(MonitoringState.HEIGHTENED)

        machine.onVerdict("com.b", RiskBand.CRITICAL)
        assertThat(machine.globalState.value).isEqualTo(MonitoringState.CONTAINMENT)
    }

    @Test
    fun `dusuk riskli hukum durumu degistirmez`() {
        assertThat(machine.onVerdict("com.example.app", RiskBand.LOW)).isNull()
        assertThat(machine.stateFor("com.example.app")).isEqualTo(MonitoringState.BASELINE)
    }
}
