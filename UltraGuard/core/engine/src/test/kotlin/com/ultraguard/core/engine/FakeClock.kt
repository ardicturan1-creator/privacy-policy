package com.ultraguard.core.engine

import com.ultraguard.core.common.time.Clock

/** Testlerde zamani elle ilerletmeyi saglayan saat. */
class FakeClock(
    private var wallClock: Long = 1_700_000_000_000L,
    private var elapsed: Long = 0L,
) : Clock {
    override fun nowMillis(): Long = wallClock
    override fun elapsedRealtimeMillis(): Long = elapsed

    fun advance(millis: Long) {
        wallClock += millis
        elapsed += millis
    }
}
