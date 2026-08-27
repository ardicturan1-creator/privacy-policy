package com.ultraguard.core.policy

import com.google.common.truth.Truth.assertThat
import com.ultraguard.core.security.ChainedRecord
import com.ultraguard.core.security.HashChain
import org.junit.Test

class HashChainTest {

    private data class Record(
        override val timestampMillis: Long,
        override val packageName: String,
        override val actionKind: String,
        override val outcome: String,
        override val previousHash: String,
        override val hash: String,
    ) : ChainedRecord

    private fun chain(vararg specs: Triple<Long, String, String>): List<Record> {
        var previous = HashChain.GENESIS
        return specs.map { (timestamp, pkg, kind) ->
            val hash = HashChain.digest(previous, timestamp, pkg, kind, "APPLIED")
            Record(timestamp, pkg, kind, "APPLIED", previous, hash).also { previous = hash }
        }
    }

    @Test
    fun `saglam zincir dogrulanir`() {
        val entries = chain(
            Triple(1_000L, "com.a", "SuspendNetwork"),
            Triple(2_000L, "com.b", "HideOverlays"),
            Triple(3_000L, "com.c", "FreezeProcess"),
        )
        assertThat(HashChain.verify(entries)).isNull()
    }

    @Test
    fun `bos zincir gecerlidir`() {
        assertThat(HashChain.verify(emptyList())).isNull()
    }

    @Test
    fun `ortadaki kaydin degistirilmesi yakalanir`() {
        val entries = chain(
            Triple(1_000L, "com.a", "SuspendNetwork"),
            Triple(2_000L, "com.b", "HideOverlays"),
            Triple(3_000L, "com.c", "FreezeProcess"),
        ).toMutableList()

        // Saldirgan ikinci kaydin hedef paketini degistiriyor.
        entries[1] = entries[1].copy(packageName = "com.innocent")

        assertThat(HashChain.verify(entries)).isEqualTo(1)
    }

    @Test
    fun `kayit silinmesi zinciri kirar`() {
        val entries = chain(
            Triple(1_000L, "com.a", "SuspendNetwork"),
            Triple(2_000L, "com.b", "HideOverlays"),
            Triple(3_000L, "com.c", "FreezeProcess"),
        ).toMutableList()

        entries.removeAt(1)

        assertThat(HashChain.verify(entries)).isEqualTo(1)
    }

    @Test
    fun `ilk kayit genesis ile baslamak zorunda`() {
        val forged = Record(
            timestampMillis = 1_000L,
            packageName = "com.a",
            actionKind = "SuspendNetwork",
            outcome = "APPLIED",
            previousHash = "ff".repeat(32),
            hash = HashChain.digest("ff".repeat(32), 1_000L, "com.a", "SuspendNetwork", "APPLIED"),
        )
        assertThat(HashChain.verify(listOf(forged))).isEqualTo(0)
    }

    @Test
    fun `ayni girdi her zaman ayni ozeti uretir`() {
        val a = HashChain.digest(HashChain.GENESIS, 1L, "com.a", "K", "APPLIED")
        val b = HashChain.digest(HashChain.GENESIS, 1L, "com.a", "K", "APPLIED")
        assertThat(a).isEqualTo(b)
    }

    @Test
    fun `alan siniri ozet cakismasini engeller`() {
        // Ayrac olmasa "ab" + "c" ile "a" + "bc" ayni ozeti uretirdi.
        val a = HashChain.digest(HashChain.GENESIS, 1L, "ab", "c", "APPLIED")
        val b = HashChain.digest(HashChain.GENESIS, 1L, "a", "bc", "APPLIED")
        assertThat(a).isNotEqualTo(b)
    }
}
