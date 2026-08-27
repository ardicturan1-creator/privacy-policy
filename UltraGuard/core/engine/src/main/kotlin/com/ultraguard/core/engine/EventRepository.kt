package com.ultraguard.core.engine

import com.ultraguard.core.ai.rules.CorrelationWindow
import com.ultraguard.core.common.di.IoDispatcher
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.database.dao.EventDao
import com.ultraguard.core.database.dao.VerdictDao
import com.ultraguard.core.database.entity.EventEntity
import com.ultraguard.core.database.entity.VerdictEntity
import com.ultraguard.core.model.Attribution
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.Subject
import com.ultraguard.core.model.TrustOverride
import com.ultraguard.core.model.Verdict
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.withContext
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json

/**
 * Olaylarin ve hukumlerin kalici hale getirilmesi ile korelasyon
 * penceresinin kurulmasi.
 */
@Singleton
class EventRepository @Inject constructor(
    private val eventDao: EventDao,
    private val verdictDao: VerdictDao,
    private val appProfileDao: AppProfileDao,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) {
    private val json = Json { encodeDefaults = true }
    private val attributeSerializer = MapSerializer(String.serializer(), String.serializer())
    private val attributionSerializer = ListSerializer(Attribution.serializer())
    private val longListSerializer = ListSerializer(Long.serializer())

    /** @return veritabani kimligi atanmis olay. Kural motoru bu kimlige dayanir. */
    suspend fun record(event: SecurityEvent): SecurityEvent = withContext(ioDispatcher) {
        val id = eventDao.insert(event.toEntity())
        event.copy(id = id)
    }

    suspend fun record(verdict: Verdict): Verdict = withContext(ioDispatcher) {
        val id = verdictDao.insert(verdict.toEntity())
        appProfileDao.updateRisk(verdict.packageName, verdict.score.value)
        verdict.copy(id = id)
    }

    /**
     * Bir paket icin korelasyon penceresini kurar.
     *
     * Pencere hem **zamanla** ([WINDOW_SPAN_MILLIS]) hem de **sayiyla**
     * ([MAX_WINDOW_EVENTS]) sinirlidir. Ikisi de gerekli: cok konusan bir
     * uygulama zaman sinirini tek basina doldurur, sessiz bir uygulamanin
     * gecmisi ise saatler oncesine uzanabilir.
     */
    suspend fun windowFor(packageName: String, nowMillis: Long): CorrelationWindow =
        withContext(ioDispatcher) {
            val since = nowMillis - WINDOW_SPAN_MILLIS
            val rows = eventDao.windowFor(packageName, since, MAX_WINDOW_EVENTS)
            CorrelationWindow(
                packageName = packageName,
                events = rows.map { it.toDomain() },
                nowMillis = nowMillis,
            )
        }

    suspend fun trustOverrideFor(packageName: String): TrustOverride = withContext(ioDispatcher) {
        appProfileDao.byPackage(packageName)?.trustOverride ?: TrustOverride.NONE
    }

    /** Saklama suresi uygulamasi -- gunluk bakim isinden cagrilir. */
    suspend fun purgeOlderThan(cutoffMillis: Long): PurgeResult = withContext(ioDispatcher) {
        PurgeResult(
            eventsDeleted = eventDao.deleteOlderThan(cutoffMillis),
            verdictsDeleted = verdictDao.deleteOlderThan(cutoffMillis),
        )
    }

    // --- Esleme ---------------------------------------------------------

    private fun SecurityEvent.toEntity() = EventEntity(
        timestampMillis = timestampMillis,
        type = type,
        packageName = packageName,
        uid = uid,
        source = source,
        attributes = json.encodeToString(attributeSerializer, attributes),
    )

    private fun EventEntity.toDomain(): SecurityEvent {
        // Yerel degiskene alinmasi zorunlu: `packageName` ve `uid` baska bir
        // modulun (`:core:database`) public API'sinde tanimli oldugu icin
        // Kotlin akilli donusum (smart cast) yapamaz -- o modul bagimsiz
        // derlendiginden derleyici alanin arada degismeyecegini garanti edemez.
        val entityPackage = packageName
        val entityUid = uid

        return SecurityEvent(
            id = id,
            timestampMillis = timestampMillis,
            type = type,
            subject = if (entityPackage != null && entityUid != null) {
                Subject.App(entityPackage, entityUid)
            } else {
                Subject.System
            },
            source = source,
            attributes = json.decodeFromString(attributeSerializer, attributes),
        )
    }

    private fun Verdict.toEntity() = VerdictEntity(
        packageName = packageName,
        createdAtMillis = createdAtMillis,
        tier = tier,
        score = score.value,
        threatClass = threatClass,
        confidence = confidence,
        originId = originId,
        attributions = json.encodeToString(attributionSerializer, attributions),
        windowEventIds = json.encodeToString(longListSerializer, correlationWindowEventIds),
    )

    companion object {
        /**
         * Bir saatlik pencere, coklu adimli saldirilarin tamamini gormeye
         * yeter (kurulum → izin → istismar tipik olarak dakikalar icinde
         * gerceklesir) ve sorgu maliyetini sinirli tutar.
         */
        const val WINDOW_SPAN_MILLIS = 60 * 60 * 1000L
        const val MAX_WINDOW_EVENTS = 256
    }
}

data class PurgeResult(val eventsDeleted: Int, val verdictsDeleted: Int)
