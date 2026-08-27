package com.ultraguard.core.database

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.database.dao.EventDao
import com.ultraguard.core.database.dao.LedgerDao
import com.ultraguard.core.database.dao.NetworkFlowDao
import com.ultraguard.core.database.dao.VerdictDao
import com.ultraguard.core.database.entity.AppProfileEntity
import com.ultraguard.core.database.entity.EventEntity
import com.ultraguard.core.database.entity.LedgerEntryEntity
import com.ultraguard.core.database.entity.NetworkFlowEntity
import com.ultraguard.core.database.entity.VerdictEntity
import com.ultraguard.core.model.ActionOutcome
import com.ultraguard.core.model.DecisionTier
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.InstallSource
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.ThreatClass
import com.ultraguard.core.model.TrustOverride

@Database(
    entities = [
        EventEntity::class,
        VerdictEntity::class,
        AppProfileEntity::class,
        LedgerEntryEntity::class,
        NetworkFlowEntity::class,
    ],
    version = 1,
    exportSchema = true,
)
@TypeConverters(UltraGuardConverters::class)
abstract class UltraGuardDatabase : RoomDatabase() {
    abstract fun eventDao(): EventDao
    abstract fun verdictDao(): VerdictDao
    abstract fun appProfileDao(): AppProfileDao
    abstract fun ledgerDao(): LedgerDao
    abstract fun networkFlowDao(): NetworkFlowDao

    companion object {
        const val NAME = "ultraguard.db"
    }
}

/**
 * Enum donusturucular.
 *
 * Enum'lar **isimle** saklanir, ordinal ile degil. Ordinal saklamak,
 * [EventType] listesine ortadan bir deger eklendiginde tum gecmis verinin
 * sessizce yanlis yorumlanmasina yol acar — guvenlik gecmisinde bu, sessiz
 * ve geri donusu olmayan bir bozulmadir.
 */
class UltraGuardConverters {
    @TypeConverter fun eventTypeToString(value: EventType): String = value.name
    @TypeConverter fun stringToEventType(value: String): EventType = EventType.valueOf(value)

    @TypeConverter fun sensorSourceToString(value: SensorSource): String = value.name
    @TypeConverter fun stringToSensorSource(value: String): SensorSource = SensorSource.valueOf(value)

    @TypeConverter fun tierToString(value: DecisionTier): String = value.name
    @TypeConverter fun stringToTier(value: String): DecisionTier = DecisionTier.valueOf(value)

    @TypeConverter fun threatClassToString(value: ThreatClass): String = value.name
    @TypeConverter fun stringToThreatClass(value: String): ThreatClass = ThreatClass.valueOf(value)

    @TypeConverter fun installSourceToString(value: InstallSource): String = value.name
    @TypeConverter fun stringToInstallSource(value: String): InstallSource = InstallSource.valueOf(value)

    @TypeConverter fun modeToString(value: ProtectionMode): String = value.name
    @TypeConverter fun stringToMode(value: String): ProtectionMode = ProtectionMode.valueOf(value)

    @TypeConverter fun outcomeToString(value: ActionOutcome): String = value.name
    @TypeConverter fun stringToOutcome(value: String): ActionOutcome = ActionOutcome.valueOf(value)

    @TypeConverter fun trustToString(value: TrustOverride): String = value.name
    @TypeConverter fun stringToTrust(value: String): TrustOverride = TrustOverride.valueOf(value)
}
