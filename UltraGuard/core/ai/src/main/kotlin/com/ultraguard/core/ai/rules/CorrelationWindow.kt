package com.ultraguard.core.ai.rules

import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.SecurityEvent

/**
 * Tek bir paket icin, zaman sirali kayan olay penceresi.
 *
 * Kurallar tek bir olaya degil **olay dizisine** bakar. "Erisilebilirlik
 * servisi acildi" tek basina zararsizdir; "yan yuklendi → erisilebilirlik
 * acildi → banka penceresi sorgulandi → ekran ustune cizildi" dizisi
 * bankacilik trojaninin kendisidir. Bu sinif o diziyi sorgulanabilir kilar.
 */
class CorrelationWindow(
    val packageName: String,
    /** Zaman sirali (eskiden yeniye), penceredeki tum olaylar. */
    val events: List<SecurityEvent>,
    val nowMillis: Long,
) {
    val isEmpty: Boolean get() = events.isEmpty()

    val spanMillis: Long
        get() = if (events.size < 2) 0 else events.last().timestampMillis - events.first().timestampMillis

    fun ofType(type: EventType): List<SecurityEvent> = events.filter { it.type == type }

    fun has(type: EventType): Boolean = events.any { it.type == type }

    fun count(type: EventType): Int = events.count { it.type == type }

    /** Penceredeki en yeni eslesme; kural kaniti icin tercih edilen olay. */
    fun latest(type: EventType, where: (SecurityEvent) -> Boolean = { true }): SecurityEvent? =
        events.lastOrNull { it.type == type && where(it) }

    fun earliest(type: EventType, where: (SecurityEvent) -> Boolean = { true }): SecurityEvent? =
        events.firstOrNull { it.type == type && where(it) }

    /**
     * [first] tipinde bir olayin ardindan [thenWithinMillis] icinde [second]
     * tipinde bir olay geldi mi? Siralama, kural dogrulugu icin kritiktir:
     * once ag baglantisi sonra izin talebi, tersinden cok daha az anlamlidir.
     */
    fun sequence(
        first: EventType,
        second: EventType,
        thenWithinMillis: Long,
    ): Pair<SecurityEvent, SecurityEvent>? {
        val firsts = ofType(first)
        val seconds = ofType(second)
        for (f in firsts) {
            val match = seconds.firstOrNull { s ->
                s.timestampMillis >= f.timestampMillis &&
                    s.timestampMillis - f.timestampMillis <= thenWithinMillis
            }
            if (match != null) return f to match
        }
        return null
    }

    /** Son [millis] milisaniyedeki olaylarla sinirli alt pencere. */
    fun recent(millis: Long): CorrelationWindow {
        val cutoff = nowMillis - millis
        return CorrelationWindow(packageName, events.filter { it.timestampMillis >= cutoff }, nowMillis)
    }

    /** Paketin UltraGuard tarafindan ilk gorulmesinden bu yana gecen sure. */
    fun ageOfOldestEventMillis(): Long =
        events.firstOrNull()?.let { nowMillis - it.timestampMillis } ?: 0L
}
