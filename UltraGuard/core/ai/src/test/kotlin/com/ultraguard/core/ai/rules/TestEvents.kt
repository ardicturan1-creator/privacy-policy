package com.ultraguard.core.ai.rules

import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.Subject

internal const val T0 = 1_700_000_000_000L
internal const val TEST_PACKAGE = "com.example.kargotakip"
internal const val TEST_UID = 10234

private var nextId = 1L

internal fun event(
    type: EventType,
    atMillis: Long,
    source: SensorSource = SensorSource.PACKAGE_LIFECYCLE,
    packageName: String = TEST_PACKAGE,
    vararg attributes: Pair<String, String>,
): SecurityEvent = SecurityEvent(
    id = nextId++,
    timestampMillis = atMillis,
    type = type,
    subject = Subject.App(packageName, TEST_UID),
    source = source,
    attributes = attributes.toMap(),
)

internal fun windowOf(vararg events: SecurityEvent, nowMillis: Long = T0 + 600_000L) =
    CorrelationWindow(
        packageName = TEST_PACKAGE,
        events = events.sortedBy { it.timestampMillis },
        nowMillis = nowMillis,
    )
