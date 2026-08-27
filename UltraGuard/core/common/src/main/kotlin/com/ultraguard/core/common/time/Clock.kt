package com.ultraguard.core.common.time

import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Zaman kaynagi soyutlamasi.
 *
 * Kural motorunun tamami zamana bagli oldugu icin ("kurulumdan sonraki ilk
 * saatte", "5 saniye icinde") zamanin test edilebilir olmasi zorunludur.
 * Uretimde [SystemClock], testlerde ilerletilebilir sahte saat kullanilir.
 */
interface Clock {
    fun nowMillis(): Long
    /** Cihaz yeniden baslatildiginda sifirlanmayan, geri alinamaz sayac. */
    fun elapsedRealtimeMillis(): Long
}

@Singleton
class SystemClock @Inject constructor() : Clock {
    override fun nowMillis(): Long = System.currentTimeMillis()
    override fun elapsedRealtimeMillis(): Long = android.os.SystemClock.elapsedRealtime()
}

@Module
@InstallIn(SingletonComponent::class)
abstract class ClockModule {
    @Binds @Singleton
    abstract fun bindClock(impl: SystemClock): Clock
}
