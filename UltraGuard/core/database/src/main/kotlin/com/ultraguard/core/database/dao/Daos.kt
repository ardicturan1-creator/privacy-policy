package com.ultraguard.core.database.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Upsert
import com.ultraguard.core.database.entity.AppProfileEntity
import com.ultraguard.core.database.entity.EventEntity
import com.ultraguard.core.database.entity.LedgerEntryEntity
import com.ultraguard.core.database.entity.NetworkFlowEntity
import com.ultraguard.core.database.entity.VerdictEntity
import kotlinx.coroutines.flow.Flow

@Dao
interface EventDao {

    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insert(event: EventEntity): Long

    /**
     * Toplu yazim. Sensorler olaylari tek tek degil, kucuk yiginlar halinde
     * yazar: her `insert` bir fsync tetikler ve saniyede yuzlerce fsync
     * hem pil hem de flash omru acisindan kabul edilemez.
     */
    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insertAll(events: List<EventEntity>): List<Long>

    /** Korelasyon penceresinin sicak sorgusu. */
    @Query(
        """
        SELECT * FROM events
        WHERE package_name = :packageName AND timestamp_millis >= :sinceMillis
        ORDER BY timestamp_millis ASC
        LIMIT :limit
        """,
    )
    suspend fun windowFor(packageName: String, sinceMillis: Long, limit: Int = 256): List<EventEntity>

    @Query(
        """
        SELECT * FROM events
        WHERE timestamp_millis >= :sinceMillis
        ORDER BY timestamp_millis DESC
        LIMIT :limit
        """,
    )
    fun recentStream(sinceMillis: Long, limit: Int = 200): Flow<List<EventEntity>>

    @Query("SELECT * FROM events WHERE id IN (:ids)")
    suspend fun byIds(ids: List<Long>): List<EventEntity>

    @Query("SELECT COUNT(*) FROM events WHERE timestamp_millis >= :sinceMillis")
    suspend fun countSince(sinceMillis: Long): Int

    /**
     * Saklama suresi uygulamasi. Varsayilan 30 gun; kullanici 7-90 arasinda
     * degistirebilir. Bu, gizlilik taahhudunun kod tarafindaki karsiligidir —
     * "sakladigimiz veriyi siliyoruz" bir ayar degil, planlanmis bir istir.
     */
    @Query("DELETE FROM events WHERE timestamp_millis < :cutoffMillis")
    suspend fun deleteOlderThan(cutoffMillis: Long): Int

    @Query("DELETE FROM events WHERE package_name = :packageName")
    suspend fun deleteForPackage(packageName: String): Int

    @Query("DELETE FROM events")
    suspend fun deleteAll()
}

@Dao
interface VerdictDao {

    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insert(verdict: VerdictEntity): Long

    @Query(
        """
        SELECT * FROM verdicts
        WHERE created_at_millis >= :sinceMillis AND score >= :minScore
        ORDER BY created_at_millis DESC
        """,
    )
    fun activeStream(sinceMillis: Long, minScore: Int): Flow<List<VerdictEntity>>

    @Query("SELECT * FROM verdicts WHERE package_name = :packageName ORDER BY created_at_millis DESC LIMIT :limit")
    suspend fun historyFor(packageName: String, limit: Int = 50): List<VerdictEntity>

    @Query("SELECT * FROM verdicts WHERE id = :id")
    suspend fun byId(id: Long): VerdictEntity?

    @Query("UPDATE verdicts SET acknowledged = 1 WHERE id = :id")
    suspend fun acknowledge(id: Long)

    @Query("SELECT MAX(score) FROM verdicts WHERE package_name = :packageName AND created_at_millis >= :sinceMillis")
    suspend fun peakScoreSince(packageName: String, sinceMillis: Long): Int?

    @Query("DELETE FROM verdicts WHERE created_at_millis < :cutoffMillis")
    suspend fun deleteOlderThan(cutoffMillis: Long): Int
}

@Dao
interface AppProfileDao {

    @Upsert
    suspend fun upsert(profile: AppProfileEntity)

    @Query("SELECT * FROM app_profiles WHERE package_name = :packageName")
    suspend fun byPackage(packageName: String): AppProfileEntity?

    @Query("SELECT * FROM app_profiles WHERE package_name = :packageName")
    fun streamByPackage(packageName: String): Flow<AppProfileEntity?>

    @Query("SELECT * FROM app_profiles ORDER BY current_risk DESC, label ASC")
    fun allByRisk(): Flow<List<AppProfileEntity>>

    @Query("SELECT * FROM app_profiles WHERE is_system_app = 0 ORDER BY current_risk DESC LIMIT :limit")
    suspend fun riskiest(limit: Int): List<AppProfileEntity>

    @Query("UPDATE app_profiles SET current_risk = :score WHERE package_name = :packageName")
    suspend fun updateRisk(packageName: String, score: Int)

    @Query("UPDATE app_profiles SET trust_override = :override WHERE package_name = :packageName")
    suspend fun updateTrust(packageName: String, override: String)

    @Query("DELETE FROM app_profiles WHERE package_name = :packageName")
    suspend fun delete(packageName: String)

    @Query("SELECT COUNT(*) FROM app_profiles WHERE is_system_app = 0")
    suspend fun userAppCount(): Int
}

/**
 * Action Ledger DAO.
 *
 * Dikkat: burada `@Update` veya `@Delete` **yoktur ve eklenmemelidir**.
 * Defter salt-eklemedir; geri alma bile yeni bir isaretlemedir, gecmisin
 * silinmesi degil. Hash zincirinin anlami buna baglidir.
 */
@Dao
interface LedgerDao {

    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun append(entry: LedgerEntryEntity): Long

    @Query("SELECT * FROM ledger ORDER BY id DESC LIMIT 1")
    suspend fun last(): LedgerEntryEntity?

    @Query("SELECT * FROM ledger WHERE id = :id")
    suspend fun byId(id: Long): LedgerEntryEntity?

    @Query("SELECT * FROM ledger ORDER BY id ASC")
    suspend fun allAscending(): List<LedgerEntryEntity>

    @Query("SELECT * FROM ledger ORDER BY timestamp_millis DESC LIMIT :limit")
    fun recentStream(limit: Int = 100): Flow<List<LedgerEntryEntity>>

    @Query(
        """
        SELECT * FROM ledger
        WHERE package_name = :packageName AND reverted_at_millis IS NULL AND outcome = 'APPLIED'
        ORDER BY timestamp_millis DESC
        """,
    )
    suspend fun activeActionsFor(packageName: String): List<LedgerEntryEntity>

    @Query("SELECT * FROM ledger WHERE reverted_at_millis IS NULL AND outcome = 'APPLIED'")
    suspend fun allActiveActions(): List<LedgerEntryEntity>

    /** Geri alma, kaydi silmez — yalnizca isaretler. */
    @Query("UPDATE ledger SET reverted_at_millis = :atMillis, reverted_by = :actor WHERE id = :id")
    suspend fun markReverted(id: Long, atMillis: Long, actor: String)

    @Query("UPDATE ledger SET outcome = :outcome WHERE id = :id")
    suspend fun updateOutcome(id: Long, outcome: ActionOutcomeColumn)

    @Query("SELECT COUNT(*) FROM ledger")
    suspend fun count(): Int
}

/** [NetworkFlowDao.deviceWideSummary] projeksiyonu. */
data class NetworkSummary(
    val totalConnections: Int,
    val blockedConnections: Int,
)

/** Room'un enum donusturucusu ile uyumlu olmasi icin tip takma adi. */
typealias ActionOutcomeColumn = com.ultraguard.core.model.ActionOutcome

@Dao
interface NetworkFlowDao {

    @Upsert
    suspend fun upsert(flow: NetworkFlowEntity)

    @Query(
        """
        UPDATE network_flows
        SET connection_count = connection_count + 1,
            bytes_out = bytes_out + :bytesOut,
            bytes_in = bytes_in + :bytesIn
        WHERE package_name = :packageName AND remote_host = :host AND day_bucket = :dayBucket
        """,
    )
    suspend fun accumulate(
        packageName: String,
        host: String,
        dayBucket: Long,
        bytesOut: Long,
        bytesIn: Long,
    ): Int

    @Query("SELECT * FROM network_flows WHERE package_name = :packageName AND day_bucket >= :sinceDay ORDER BY bytes_out DESC")
    suspend fun flowsFor(packageName: String, sinceDay: Long): List<NetworkFlowEntity>

    @Query("SELECT SUM(blocked_count) FROM network_flows WHERE day_bucket >= :sinceDay")
    fun blockedCountStream(sinceDay: Long): Flow<Int?>

    /**
     * Cihaz genelinde baglanti ozeti -- Cihaz Guven Skorunun ag hijyeni
     * bileseni bunu kullanir. Toplama SQL tarafinda yapilir: binlerce satiri
     * bellege alip Kotlin'de toplamak, skor her yenilendiginde gereksiz
     * ayirma ve gecikme uretir.
     */
    @Query(
        """
        SELECT
            COALESCE(SUM(connection_count), 0) AS totalConnections,
            COALESCE(SUM(blocked_count), 0) AS blockedConnections
        FROM network_flows
        WHERE day_bucket >= :sinceDay
        """,
    )
    suspend fun deviceWideSummary(sinceDay: Long): NetworkSummary

    @Query("DELETE FROM network_flows WHERE day_bucket < :cutoffDay")
    suspend fun deleteOlderThan(cutoffDay: Long): Int
}
