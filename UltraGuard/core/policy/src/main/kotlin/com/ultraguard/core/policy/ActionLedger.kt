package com.ultraguard.core.policy

import com.ultraguard.core.common.di.IoDispatcher
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.LedgerDao
import com.ultraguard.core.database.entity.LedgerEntryEntity
import com.ultraguard.core.model.ActionOutcome
import com.ultraguard.core.model.EnforcementAction
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.RevertActor
import com.ultraguard.core.security.ChainedRecord
import com.ultraguard.core.security.HashChain
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json

/**
 * UltraGuard'in yaptigi her seyin denetlenebilir defteri.
 *
 * Uc garantisi vardir:
 *  1. **Salt-ekleme.** Kayit silinmez, degistirilmez. Geri alma bile yeni bir
 *     isaretlemedir.
 *  2. **Kurcalama-belirtici.** Hash zinciri sayesinde gecmise mudahale
 *     [verifyIntegrity] tarafindan yakalanir.
 *  3. **Geri alinabilirlik.** Aktif her otonom eylem tek cagriyla geri alinir;
 *     kullanicinin "geri al" dugmesi bu API'nin dogrudan karsiligidir.
 */
@Singleton
class ActionLedger @Inject constructor(
    private val ledgerDao: LedgerDao,
    private val clock: Clock,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) {
    /**
     * Zincire yazim seri olmak zorundadir: iki es zamanli `append` ayni
     * `previousHash` degerini okursa zincir catallanir ve dogrulama kalici
     * olarak basarisiz olur.
     */
    private val appendMutex = Mutex()

    private val json = Json {
        encodeDefaults = true
        classDiscriminator = "kind"
    }

    suspend fun append(
        action: EnforcementAction,
        outcome: ActionOutcome,
        mode: ProtectionMode,
        verdictId: Long?,
    ): Long = withContext(ioDispatcher) {
        appendMutex.withLock {
            val now = clock.nowMillis()
            val previousHash = ledgerDao.last()?.hash ?: HashChain.GENESIS
            val actionKind = action::class.simpleName.orEmpty()

            val hash = HashChain.digest(
                previousHash = previousHash,
                timestampMillis = now,
                packageName = action.packageName,
                actionKind = actionKind,
                outcome = outcome.name,
            )

            ledgerDao.append(
                LedgerEntryEntity(
                    timestampMillis = now,
                    packageName = action.packageName,
                    action = json.encodeToString(EnforcementAction.serializer(), action),
                    actionKind = actionKind,
                    reversible = action.reversible,
                    triggeringVerdictId = verdictId,
                    mode = mode,
                    outcome = outcome,
                    revertedAtMillis = null,
                    revertedBy = null,
                    previousHash = previousHash,
                    hash = hash,
                ),
            )
        }
    }

    /**
     * Bir eylemi geri alir.
     *
     * @return geri alinacak eylem, cagiranin fiilen tersine cevirmesi icin.
     *   Defter yalnizca **kaydi** isaretler; sistem durumunu degistirmek
     *   `EnforcementExecutor`'un isidir. Bu ayrim, defterin yan etkisiz
     *   kalmasini ve dolayisiyla dogrulanabilir olmasini saglar.
     */
    suspend fun revert(entryId: Long, actor: RevertActor): EnforcementAction? =
        withContext(ioDispatcher) {
            val entry = ledgerDao.byId(entryId) ?: return@withContext null
            if (entry.revertedAtMillis != null) {
                UgLog.i(TAG, "Kayit zaten geri alinmis: $entryId")
                return@withContext null
            }
            if (!entry.reversible) {
                // Buraya normalde hic gelinmemeli: geri alinamaz eylemler
                // otonom uygulanmadigi icin geri alma talebi de olusmamali.
                UgLog.w(TAG, "Geri alinamaz eylem icin geri alma talebi: ${entry.actionKind}")
                return@withContext null
            }

            ledgerDao.markReverted(entryId, clock.nowMillis(), actor.name)
            json.decodeFromString(EnforcementAction.serializer(), entry.action)
        }

    /** Bir pakette su anda yururlukte olan tum otonom yaptirimlar. */
    suspend fun activeActionsFor(packageName: String): List<ActiveAction> =
        withContext(ioDispatcher) {
            ledgerDao.activeActionsFor(packageName).map { entry ->
                ActiveAction(
                    entryId = entry.id,
                    action = json.decodeFromString(EnforcementAction.serializer(), entry.action),
                    appliedAtMillis = entry.timestampMillis,
                )
            }
        }

    fun recentStream(limit: Int = 100): Flow<List<LedgerEntryEntity>> = ledgerDao.recentStream(limit)

    /**
     * Zincir butunlugunu dogrular. Uygulama her acilisinda ve gunluk bakim
     * isinde calisir. Bozulma tespit edilirse kullaniciya gosterilir --
     * gizlenmez.
     */
    suspend fun verifyIntegrity(): LedgerIntegrity = withContext(ioDispatcher) {
        val entries = ledgerDao.allAscending()
        val records = entries.map { it.asChainedRecord() }
        val brokenIndex = HashChain.verify(records)
        if (brokenIndex == null) {
            LedgerIntegrity.Intact(entryCount = entries.size)
        } else {
            LedgerIntegrity.Broken(
                firstBrokenEntryId = entries[brokenIndex].id,
                brokenAtIndex = brokenIndex,
                totalEntries = entries.size,
            )
        }
    }

    private fun LedgerEntryEntity.asChainedRecord() = object : ChainedRecord {
        override val timestampMillis = this@asChainedRecord.timestampMillis
        override val packageName = this@asChainedRecord.packageName
        override val actionKind = this@asChainedRecord.actionKind
        override val outcome = this@asChainedRecord.outcome.name
        override val previousHash = this@asChainedRecord.previousHash
        override val hash = this@asChainedRecord.hash
    }

    private companion object {
        const val TAG = "ActionLedger"
    }
}

data class ActiveAction(
    val entryId: Long,
    val action: EnforcementAction,
    val appliedAtMillis: Long,
)

sealed interface LedgerIntegrity {
    data class Intact(val entryCount: Int) : LedgerIntegrity

    /**
     * Defter kurcalanmis. Bu, cihazda UltraGuard'in verisine mudahale
     * edebilen bir aktor oldugu anlamina gelir -- kendi basina kritik bir
     * guvenlik bulgusudur ve kullaniciya bir tehdit gibi bildirilir.
     */
    data class Broken(
        val firstBrokenEntryId: Long,
        val brokenAtIndex: Int,
        val totalEntries: Int,
    ) : LedgerIntegrity
}
