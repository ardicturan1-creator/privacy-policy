package com.ultraguard.core.security

import java.security.MessageDigest

/**
 * Action Ledger'in kurcalama-belirtici (tamper-evident) hash zinciri.
 *
 * Her kaydin ozeti bir onceki kaydin ozetini icerir. Gecmis bir kaydi
 * degistiren veya silen biri, sonrasindaki tum ozetleri de yeniden
 * hesaplamak zorunda kalir; [verify] bunu tek gecisle yakalar.
 *
 * Bu sifrelemenin yerini tutmaz -- amaci gizlemek degil, **degisikligi
 * gorunur kilmaktir**. UltraGuard'in "sana ne yaptigimi gizleyemem"
 * taahhudunun teknik karsiligidir.
 */
object HashChain {

    const val GENESIS = "0000000000000000000000000000000000000000000000000000000000000000"

    /**
     * Alanlarin sirasi ve ayraci **degistirilemez**: degistirilirse tum
     * gecmis zincir gecersiz hale gelir ve kullaniciya sahte bir kurcalama
     * uyarisi gosteririz.
     */
    fun digest(
        previousHash: String,
        timestampMillis: Long,
        packageName: String,
        actionKind: String,
        outcome: String,
    ): String {
        val payload = listOf(
            previousHash,
            timestampMillis.toString(),
            packageName,
            actionKind,
            outcome,
        ).joinToString(SEPARATOR)
        return sha256(payload)
    }

    /**
     * Zinciri bastan sona dogrular.
     * @return ilk bozuk kaydin indeksi; zincir saglamsa `null`.
     */
    fun verify(entries: List<ChainedRecord>): Int? {
        var expectedPrevious = GENESIS
        entries.forEachIndexed { index, record ->
            if (record.previousHash != expectedPrevious) return index
            val recomputed = digest(
                previousHash = record.previousHash,
                timestampMillis = record.timestampMillis,
                packageName = record.packageName,
                actionKind = record.actionKind,
                outcome = record.outcome,
            )
            if (recomputed != record.hash) return index
            expectedPrevious = record.hash
        }
        return null
    }

    private fun sha256(value: String): String =
        MessageDigest.getInstance("SHA-256")
            .digest(value.toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }

    /** Alan iceriginde gecemeyecek bir ayrac; ozet cakismasini engeller. */
    private const val SEPARATOR = "|#|"
}

/** [HashChain.verify] icin gereken en kucuk kayit yuzeyi. */
interface ChainedRecord {
    val timestampMillis: Long
    val packageName: String
    val actionKind: String
    val outcome: String
    val previousHash: String
    val hash: String
}
