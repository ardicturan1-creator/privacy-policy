package com.ultraguard.shield.work

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.NetworkFlowDao
import com.ultraguard.core.datastore.SettingsStore
import com.ultraguard.core.engine.EventRepository
import com.ultraguard.core.policy.ActionLedger
import com.ultraguard.core.policy.LedgerIntegrity
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.flow.first

/**
 * Saklama suresi uygulamasi ve gunluk butunluk denetimi.
 *
 * "Sakladigimiz veriyi siliyoruz" bir ayar degil, planlanmis bir istir.
 * Kullanicinin sectigi sure (7-90 gun) dolan olaylar burada gercekten
 * silinir; gizlilik taahhudunun kod tarafindaki karsiligi budur.
 *
 * Ayni is, Action Ledger'in hash zincirini de dogrular: defterin
 * kurcalanmasi, cihazda UltraGuard verisine mudahale edebilen bir aktor
 * oldugu anlamina gelir ve kendi basina kritik bir bulgudur.
 */
@HiltWorker
class RetentionWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val eventRepository: EventRepository,
    private val networkFlowDao: NetworkFlowDao,
    private val settingsStore: SettingsStore,
    private val actionLedger: ActionLedger,
    private val clock: Clock,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val settings = settingsStore.settings.first()
        val cutoff = clock.nowMillis() - settings.retentionDays * DAY_MILLIS

        val purge = eventRepository.purgeOlderThan(cutoff)
        val flowsDeleted = networkFlowDao.deleteOlderThan(cutoff / DAY_MILLIS)

        UgLog.i(
            TAG,
            "Saklama uygulandi (${settings.retentionDays} gun): " +
                "${purge.eventsDeleted} olay, ${purge.verdictsDeleted} hukum, " +
                "$flowsDeleted ag ozeti silindi",
        )

        when (val integrity = actionLedger.verifyIntegrity()) {
            is LedgerIntegrity.Intact ->
                UgLog.d(TAG) { "Defter saglam: ${integrity.entryCount} kayit" }
            is LedgerIntegrity.Broken ->
                // Defter salt-eklemedir; kirilmasi normal calisma sirasinda
                // imkansizdir. Gorulurse bu bir tehdittir ve oyle raporlanir.
                UgLog.e(
                    TAG,
                    "DEFTER KURCALANMIS: kayit ${integrity.firstBrokenEntryId}, " +
                        "indeks ${integrity.brokenAtIndex}/${integrity.totalEntries}",
                )
        }

        return Result.success()
    }

    companion object {
        const val NAME = "ultraguard_retention"
        private const val TAG = "RetentionWorker"
        private const val DAY_MILLIS = 86_400_000L
    }
}
