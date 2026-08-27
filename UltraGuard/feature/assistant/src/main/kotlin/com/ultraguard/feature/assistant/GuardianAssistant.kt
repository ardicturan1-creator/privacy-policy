package com.ultraguard.feature.assistant

import com.ultraguard.core.common.di.IoDispatcher
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.database.dao.EventDao
import com.ultraguard.core.database.dao.NetworkFlowDao
import com.ultraguard.core.database.dao.VerdictDao
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.withContext

/**
 * Guardian -- cihaz uzerinde calisan guvenlik asistani.
 *
 * **Temellendirme (grounding) sozlesmesi -- bu sinifin varlik nedeni:**
 * Asistanin urettigi her sayi ve her iddia, veritabanindaki somut bir
 * kayda dayanmak zorundadir. Dil modeli **veri uretmez**, yalnizca burada
 * toplanan olgulari cumleye dokerken kullanilir.
 *
 * Neden bu kadar kati: bir guvenlik asistaninin halusinasyonu, kullaniciya
 * "bu uygulama guvenli" dedirtir. Uydurulmus bir guvence, hic cevap
 * vermemekten cok daha zararlidir. Bu yuzden veri katmani ile dil katmani
 * burada fiziksel olarak ayrilmistir: [gatherFacts] yalnizca sorgu yapar,
 * dil modeli yalnizca bu olgulari gorur ve baska hicbir bilgiye erisemez.
 */
@Singleton
class GuardianAssistant @Inject constructor(
    private val eventDao: EventDao,
    private val verdictDao: VerdictDao,
    private val appProfileDao: AppProfileDao,
    private val networkFlowDao: NetworkFlowDao,
    private val clock: Clock,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) {

    /**
     * "Bu uygulama ne yapiyor?" sorusunun cevabini olusturur.
     *
     * @return kullaniciya gosterilecek olgular ve onerilen eylemler.
     *   Cevap metni yoktur -- metin uretimi, bu olgulari girdi alan ayri bir
     *   katmanin isidir ve bu olgularin disina cikamaz.
     */
    suspend fun explainApp(packageName: String): AppExplanation? = withContext(ioDispatcher) {
        val profile = appProfileDao.byPackage(packageName) ?: return@withContext null
        val now = clock.nowMillis()
        val since = now - EXPLANATION_WINDOW_MILLIS
        val dayBucket = (since / DAY_MILLIS)

        val events = eventDao.windowFor(packageName, since, limit = MAX_FACTS_EVENTS)
        val verdicts = verdictDao.historyFor(packageName, limit = 10)
        val flows = networkFlowDao.flowsFor(packageName, dayBucket)

        val facts = buildList {
            add(Fact.InstallOrigin(profile.installSource.name, profile.installerPackage))
            add(Fact.Age(daysSince(profile.firstInstallMillis, now)))

            val sensorUsage = events
                .filter { it.type.domain == com.ultraguard.core.model.EventDomain.SENSOR }
                .groupingBy { it.type.name }
                .eachCount()
            if (sensorUsage.isNotEmpty()) add(Fact.SensorUsage(sensorUsage))

            flows.take(TOP_HOSTS).forEach { flow ->
                add(
                    Fact.NetworkDestination(
                        host = flow.remoteHost,
                        connectionCount = flow.connectionCount,
                        bytesOut = flow.bytesOut,
                        reputation = flow.reputation,
                    ),
                )
            }

            val dormant = dormantPermissions(profile.requestedPermissions, profile.exercisedPermissions, now)
            if (dormant.isNotEmpty()) add(Fact.DormantPermissions(dormant))

            verdicts.firstOrNull()?.let { verdict ->
                add(Fact.LatestVerdict(verdict.threatClass.name, verdict.score, verdict.originId))
            }
        }

        AppExplanation(
            packageName = packageName,
            label = profile.label,
            facts = facts,
            suggestedActions = suggestActions(facts),
        )
    }

    private fun dormantPermissions(
        requestedJson: String,
        exercisedJson: String,
        nowMillis: Long,
    ): List<String> {
        // Basit, bagimliliksiz ayristirma: bu alanlar depoda JSON dizisi ve
        // JSON nesnesi olarak tutulur. Asistan katmani veri sekli hakkinda
        // varsayim yapmaz; ayristirilamayan icerik sessizce atlanir --
        // uydurulmus bir izin listesi gostermektense hic gostermemek yegdir.
        val requested = requestedJson
            .removeSurrounding("[", "]")
            .split(',')
            .map { it.trim().trim('"') }
            .filter { it.isNotEmpty() }

        val cutoff = nowMillis - DORMANT_THRESHOLD_MILLIS
        return requested.filter { permission ->
            val marker = "\"$permission\":"
            val index = exercisedJson.indexOf(marker)
            if (index < 0) return@filter true
            val value = exercisedJson
                .substring(index + marker.length)
                .takeWhile { it.isDigit() }
                .toLongOrNull() ?: return@filter true
            value < cutoff
        }
    }

    private fun suggestActions(facts: List<Fact>): List<SuggestedAction> = buildList {
        facts.filterIsInstance<Fact.DormantPermissions>().firstOrNull()?.let { dormant ->
            dormant.permissions.forEach { permission ->
                add(SuggestedAction.RevokePermission(permission))
            }
        }
        if (facts.any { it is Fact.NetworkDestination && it.reputation == "suspicious" }) {
            add(SuggestedAction.BlockNetwork)
        }
        facts.filterIsInstance<Fact.LatestVerdict>().firstOrNull()?.let { verdict ->
            if (verdict.score >= UNINSTALL_SUGGESTION_THRESHOLD) add(SuggestedAction.Uninstall)
        }
    }

    private fun daysSince(millis: Long, nowMillis: Long): Int =
        ((nowMillis - millis) / DAY_MILLIS).toInt()

    private companion object {
        const val DAY_MILLIS = 86_400_000L
        const val EXPLANATION_WINDOW_MILLIS = 7 * DAY_MILLIS
        const val DORMANT_THRESHOLD_MILLIS = 30 * DAY_MILLIS
        const val MAX_FACTS_EVENTS = 256
        const val TOP_HOSTS = 5
        const val UNINSTALL_SUGGESTION_THRESHOLD = 75
    }
}

data class AppExplanation(
    val packageName: String,
    val label: String,
    /** Cevabin dayandigi dogrulanabilir olgular. Bos olamaz. */
    val facts: List<Fact>,
    val suggestedActions: List<SuggestedAction>,
)

/**
 * Asistanin kullanabilecegi tek bilgi turu.
 *
 * Kapali (sealed) hiyerarsi kasitlidir: asistan, burada tanimlanmamis bir
 * sey soyleyemez. Yeni bir iddia turu eklemek, once onu ureten sorguyu
 * yazmayi gerektirir.
 */
sealed interface Fact {
    data class InstallOrigin(val source: String, val installerPackage: String?) : Fact
    data class Age(val days: Int) : Fact
    data class SensorUsage(val countsByType: Map<String, Int>) : Fact
    data class NetworkDestination(
        val host: String,
        val connectionCount: Int,
        val bytesOut: Long,
        val reputation: String?,
    ) : Fact
    data class DormantPermissions(val permissions: List<String>) : Fact
    data class LatestVerdict(val threatClass: String, val score: Int, val originId: String) : Fact
}

sealed interface SuggestedAction {
    data class RevokePermission(val permission: String) : SuggestedAction
    data object BlockNetwork : SuggestedAction
    data object Uninstall : SuggestedAction
}
