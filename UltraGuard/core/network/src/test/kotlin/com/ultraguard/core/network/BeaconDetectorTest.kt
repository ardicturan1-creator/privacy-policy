package com.ultraguard.core.network

import com.google.common.truth.Truth.assertThat
import kotlin.random.Random
import org.junit.Test

class BeaconDetectorTest {

    private val detector = BeaconDetector()
    private val key = BeaconDetector.FlowKey("com.example.implant", "c2.example.net")
    private val start = 1_700_000_000_000L

    private fun feed(intervals: List<Long>): BeaconVerdict? {
        var timestamp = start
        var verdict: BeaconVerdict? = null
        detector.observe(key, timestamp)
        intervals.forEach { interval ->
            timestamp += interval
            verdict = detector.observe(key, timestamp) ?: verdict
        }
        return verdict
    }

    @Test
    fun `az ornekle karar verilmez`() {
        assertThat(feed(listOf(60_000L, 60_000L))).isNull()
    }

    @Test
    fun `mukemmel duzenli beacon yakalanir`() {
        val verdict = feed(List(10) { 60_000L })
        assertThat(verdict).isNotNull()
        assertThat(verdict!!.intervalMillis).isEqualTo(60_000L)
    }

    @Test
    fun `yuzde yirmi jitterli beacon yine de yakalanir`() {
        val random = Random(1)
        val intervals = List(12) { 60_000L + random.nextLong(-12_000L, 12_000L) }
        assertThat(feed(intervals)).isNotNull()
    }

    @Test
    fun `insan kullanim ritmi beacon sayilmaz`() {
        // Gercek kullanim: bazen saniyeler, bazen dakikalar.
        val intervals = listOf(
            15_000L, 240_000L, 12_000L, 900_000L, 30_000L,
            18_000L, 600_000L, 45_000L, 1_200_000L, 22_000L,
        )
        assertThat(feed(intervals)).isNull()
    }

    @Test
    fun `cok sik yoklama beacon sayilmaz`() {
        // 2 saniyelik duzenli aralik: chat veya oyun senkronizasyonu.
        assertThat(feed(List(10) { 2_000L })).isNull()
    }

    @Test
    fun `cok seyrek senkronizasyon beacon sayilmaz`() {
        // 6 saatlik duzenli aralik: arka plan yedekleme.
        assertThat(feed(List(10) { 21_600_000L })).isNull()
    }

    @Test
    fun `paket unutuldugunda gecmis temizlenir`() {
        feed(List(10) { 60_000L })
        detector.forget("com.example.implant")
        assertThat(feed(listOf(60_000L, 60_000L))).isNull()
    }
}
