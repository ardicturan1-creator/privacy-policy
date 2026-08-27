package com.ultraguard.core.database.entity

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey
import com.ultraguard.core.model.ActionOutcome
import com.ultraguard.core.model.DecisionTier
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.InstallSource
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.model.TrustOverride

/**
 * Olay tablosu.
 *
 * Indeksleme, korelasyon penceresi sorgusunun sicak yolu olmasina gore
 * secilmistir: "su paket icin, su zamandan sonraki olaylar" sorgusu saniyede
 * onlarca kez calisir ve tam tarama yapmasi pil butcesini tek basina yer.
 */
@Entity(
    tableName = "events",
    indices = [
        Index(value = ["package_name", "timestamp_millis"]),
        Index(value = ["timestamp_millis"]),
        Index(value = ["type"]),
    ],
)
data class EventEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "timestamp_millis") val timestampMillis: Long,
    @ColumnInfo(name = "type") val type: EventType,
    @ColumnInfo(name = "package_name") val packageName: String?,
    @ColumnInfo(name = "uid") val uid: Int?,
    @ColumnInfo(name = "source") val source: SensorSource,
    /** JSON olarak serilestirilmis `attributes` haritasi. */
    @ColumnInfo(name = "attributes") val attributes: String,
)

@Entity(
    tableName = "verdicts",
    indices = [
        Index(value = ["package_name", "created_at_millis"]),
        Index(value = ["created_at_millis"]),
    ],
)
data class VerdictEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "package_name") val packageName: String,
    @ColumnInfo(name = "created_at_millis") val createdAtMillis: Long,
    @ColumnInfo(name = "tier") val tier: DecisionTier,
    @ColumnInfo(name = "score") val score: Int,
    @ColumnInfo(name = "threat_class") val threatClass: ThreatClass,
    @ColumnInfo(name = "confidence") val confidence: Float,
    @ColumnInfo(name = "origin_id") val originId: String,
    /** JSON: List<Attribution>. Aciklanabilirlik verisi hukumden ayrilamaz. */
    @ColumnInfo(name = "attributions") val attributions: String,
    @ColumnInfo(name = "window_event_ids") val windowEventIds: String,
    @ColumnInfo(name = "acknowledged") val acknowledged: Boolean = false,
)

@Entity(tableName = "app_profiles")
data class AppProfileEntity(
    @PrimaryKey @ColumnInfo(name = "package_name") val packageName: String,
    @ColumnInfo(name = "uid") val uid: Int,
    @ColumnInfo(name = "label") val label: String,
    @ColumnInfo(name = "version_name") val versionName: String?,
    @ColumnInfo(name = "version_code") val versionCode: Long,
    @ColumnInfo(name = "install_source") val installSource: InstallSource,
    @ColumnInfo(name = "installer_package") val installerPackage: String?,
    @ColumnInfo(name = "first_install_millis") val firstInstallMillis: Long,
    @ColumnInfo(name = "last_update_millis") val lastUpdateMillis: Long,
    @ColumnInfo(name = "target_sdk") val targetSdk: Int,
    @ColumnInfo(name = "signature_sha256") val signatureSha256: String,
    @ColumnInfo(name = "certificate_age_days") val certificateAgeDays: Int,
    @ColumnInfo(name = "is_system_app") val isSystemApp: Boolean,
    @ColumnInfo(name = "requested_permissions") val requestedPermissions: String,
    @ColumnInfo(name = "exercised_permissions") val exercisedPermissions: String,
    @ColumnInfo(name = "current_risk") val currentRisk: Int,
    @ColumnInfo(name = "network_policy") val networkPolicy: String,
    @ColumnInfo(name = "trust_override") val trustOverride: TrustOverride,
)

/**
 * Action Ledger kaydi.
 *
 * [previousHash] ve [hash] alanlari kayitlari bir zincire baglar. Bu tablo
 * salt-ekleme (append-only) olarak kullanilir; DAO'da guncelleme veya silme
 * metodu **bilincli olarak tanimlanmamistir** — yalnizca geri alma isareti
 * ayri bir alan olarak eklenir.
 */
@Entity(
    tableName = "ledger",
    indices = [Index(value = ["timestamp_millis"]), Index(value = ["package_name"])],
)
data class LedgerEntryEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "timestamp_millis") val timestampMillis: Long,
    @ColumnInfo(name = "package_name") val packageName: String,
    /** JSON: EnforcementAction (polimorfik serilestirme). */
    @ColumnInfo(name = "action") val action: String,
    @ColumnInfo(name = "action_kind") val actionKind: String,
    @ColumnInfo(name = "reversible") val reversible: Boolean,
    @ColumnInfo(name = "verdict_id") val triggeringVerdictId: Long?,
    @ColumnInfo(name = "mode") val mode: ProtectionMode,
    @ColumnInfo(name = "outcome") val outcome: ActionOutcome,
    @ColumnInfo(name = "reverted_at_millis") val revertedAtMillis: Long?,
    @ColumnInfo(name = "reverted_by") val revertedBy: String?,
    @ColumnInfo(name = "previous_hash") val previousHash: String,
    @ColumnInfo(name = "hash") val hash: String,
)

/** Uygulama basina, gun bazinda toplanmis ag akisi ozeti. */
@Entity(
    tableName = "network_flows",
    primaryKeys = ["package_name", "remote_host", "day_bucket"],
    indices = [Index(value = ["day_bucket"])],
)
data class NetworkFlowEntity(
    @ColumnInfo(name = "package_name") val packageName: String,
    @ColumnInfo(name = "remote_host") val remoteHost: String,
    @ColumnInfo(name = "day_bucket") val dayBucket: Long,
    @ColumnInfo(name = "connection_count") val connectionCount: Int,
    @ColumnInfo(name = "bytes_out") val bytesOut: Long,
    @ColumnInfo(name = "bytes_in") val bytesIn: Long,
    @ColumnInfo(name = "blocked_count") val blockedCount: Int,
    @ColumnInfo(name = "reputation") val reputation: String?,
)
