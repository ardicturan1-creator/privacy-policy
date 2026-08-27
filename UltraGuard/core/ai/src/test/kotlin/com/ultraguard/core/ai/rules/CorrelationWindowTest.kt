package com.ultraguard.core.ai.rules

import com.google.common.truth.Truth.assertThat
import com.ultraguard.core.model.EventType
import org.junit.Test

class CorrelationWindowTest {

    @Test
    fun `sequence yalnizca dogru siradaki olaylari eslestirir`() {
        val forward = windowOf(
            event(EventType.KERNEL_MEMFD_CREATE, T0),
            event(EventType.KERNEL_EXEC, T0 + 500),
        )
        val reversed = windowOf(
            event(EventType.KERNEL_EXEC, T0),
            event(EventType.KERNEL_MEMFD_CREATE, T0 + 500),
        )

        assertThat(forward.sequence(EventType.KERNEL_MEMFD_CREATE, EventType.KERNEL_EXEC, 2_000))
            .isNotNull()
        assertThat(reversed.sequence(EventType.KERNEL_MEMFD_CREATE, EventType.KERNEL_EXEC, 2_000))
            .isNull()
    }

    @Test
    fun `sequence zaman penceresini asan ciftleri reddeder`() {
        val window = windowOf(
            event(EventType.KERNEL_MEMFD_CREATE, T0),
            event(EventType.KERNEL_EXEC, T0 + 10_000),
        )
        assertThat(window.sequence(EventType.KERNEL_MEMFD_CREATE, EventType.KERNEL_EXEC, 2_000))
            .isNull()
    }

    @Test
    fun `recent yalnizca pencere icindeki olaylari birakir`() {
        val now = T0 + 600_000L
        val window = windowOf(
            event(EventType.SENSOR_LOCATION_ACCESS, T0),
            event(EventType.SENSOR_LOCATION_ACCESS, now - 30_000),
            nowMillis = now,
        )
        assertThat(window.recent(60_000).events).hasSize(1)
    }

    @Test
    fun `latest en yeni eslesmeyi dondurur`() {
        val window = windowOf(
            event(EventType.OVERLAY_DRAWN, T0),
            event(EventType.OVERLAY_DRAWN, T0 + 5_000),
        )
        assertThat(window.latest(EventType.OVERLAY_DRAWN)?.timestampMillis).isEqualTo(T0 + 5_000)
    }
}
