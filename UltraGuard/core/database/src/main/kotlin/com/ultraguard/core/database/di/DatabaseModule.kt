package com.ultraguard.core.database.di

import android.content.Context
import androidx.room.Room
import com.ultraguard.core.database.UltraGuardDatabase
import com.ultraguard.core.database.dao.AppProfileDao
import com.ultraguard.core.database.dao.EventDao
import com.ultraguard.core.database.dao.LedgerDao
import com.ultraguard.core.database.dao.NetworkFlowDao
import com.ultraguard.core.database.dao.VerdictDao
import com.ultraguard.core.security.DatabaseKeyProvider
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton
import net.zetetic.database.sqlcipher.SupportOpenHelperFactory

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {

    /**
     * Veritabani SQLCipher ile sifrelenir ve anahtar **asla dosyada tutulmaz**:
     * [DatabaseKeyProvider] anahtari donanim destekli Keystore'da (mumkunse
     * StrongBox'ta) saklar ve yalnizca kullanilacagi anda cozer.
     *
     * Boylece cihaz kilitliyken veya bootloader acilarak flash imaji
     * cikarildiginda, guvenlik gecmisi -- hangi uygulamanin ne zaman ne
     * yaptigi -- okunamaz. Bu gecmis, yanlis ellerde bir gozetim veri
     * tabanidir; onu sifrelemek opsiyonel degildir.
     */
    @Provides
    @Singleton
    fun provideDatabase(
        @ApplicationContext context: Context,
        keyProvider: DatabaseKeyProvider,
    ): UltraGuardDatabase {
        System.loadLibrary("sqlcipher")

        // Parola dizisi burada SIFIRLANMAZ. `Room.build()` veritabanini
        // acmaz -- acilis ilk DAO cagrisinda, tembel olarak gerceklesir.
        // Diziyi burada sifirlamak, SQLCipher'in ilk acilista sifirlanmis
        // bir parola gormesine ve veritabanini acamamasina yol acar.
        //
        // `SupportOpenHelperFactory`, `clearPassphrase = true` ile
        // olusturuldugunda diziyi veritabanini actiktan hemen sonra kendisi
        // sifirlar; temizlik sorumlulugu bilincli olarak oraya birakilmistir.
        val passphrase = keyProvider.databasePassphrase()

        return Room.databaseBuilder(context, UltraGuardDatabase::class.java, UltraGuardDatabase.NAME)
            .openHelperFactory(
                SupportOpenHelperFactory(
                    /* passphrase = */ passphrase,
                    /* hook = */ null,
                    /* clearPassphrase = */ true,
                ),
            )
            // Migration'lar acikca yazilir. `fallbackToDestructiveMigration`
            // kullanilmaz: kullanicinin guvenlik gecmisini bir surum
            // yukseltmesi yuzunden sessizce silmek kabul edilemez.
            .setJournalMode(androidx.room.RoomDatabase.JournalMode.WRITE_AHEAD_LOGGING)
            .build()
    }

    @Provides fun provideEventDao(db: UltraGuardDatabase): EventDao = db.eventDao()
    @Provides fun provideVerdictDao(db: UltraGuardDatabase): VerdictDao = db.verdictDao()
    @Provides fun provideAppProfileDao(db: UltraGuardDatabase): AppProfileDao = db.appProfileDao()
    @Provides fun provideLedgerDao(db: UltraGuardDatabase): LedgerDao = db.ledgerDao()
    @Provides fun provideNetworkFlowDao(db: UltraGuardDatabase): NetworkFlowDao = db.networkFlowDao()
}
