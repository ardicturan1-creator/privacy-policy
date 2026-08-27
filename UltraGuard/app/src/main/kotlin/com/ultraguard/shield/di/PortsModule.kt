package com.ultraguard.shield.di

import com.ultraguard.core.policy.DeviceAdminEnforcementPort
import com.ultraguard.core.policy.NetworkEnforcementPort
import com.ultraguard.core.policy.OverlayEnforcementPort
import com.ultraguard.core.policy.ProcessEnforcementPort
import com.ultraguard.core.policy.UserActionPort
import com.ultraguard.module.deepscan.DeepScanBridge
import com.ultraguard.module.deepscan.NoOpDeepScanBridge
import com.ultraguard.shield.ports.DeviceAdminEnforcementAdapter
import com.ultraguard.shield.ports.DeviceOwnerCheckerImpl
import com.ultraguard.shield.ports.NetworkEnforcementAdapter
import com.ultraguard.shield.ports.OverlayEnforcementAdapter
import com.ultraguard.shield.ports.ProcessEnforcementAdapter
import com.ultraguard.shield.ports.UserActionAdapter
import com.ultraguard.shield.response.DeviceOwnerChecker
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

/**
 * Politika katmaninin soyut uclarini somut Android uygulamalarina baglar.
 *
 * Bu modul, `:core:policy`'nin Android'e bagimli olmadan test edilebilir
 * kalmasini saglayan siniri olusturur: karar orada, uygulama burada.
 */
@Module
@InstallIn(SingletonComponent::class)
abstract class PortsModule {

    @Binds @Singleton
    abstract fun bindNetworkPort(impl: NetworkEnforcementAdapter): NetworkEnforcementPort

    @Binds @Singleton
    abstract fun bindOverlayPort(impl: OverlayEnforcementAdapter): OverlayEnforcementPort

    @Binds @Singleton
    abstract fun bindDeviceAdminPort(impl: DeviceAdminEnforcementAdapter): DeviceAdminEnforcementPort

    @Binds @Singleton
    abstract fun bindProcessPort(impl: ProcessEnforcementAdapter): ProcessEnforcementPort

    @Binds @Singleton
    abstract fun bindUserActionPort(impl: UserActionAdapter): UserActionPort

    @Binds @Singleton
    abstract fun bindDeviceOwnerChecker(impl: DeviceOwnerCheckerImpl): DeviceOwnerChecker
}

@Module
@InstallIn(SingletonComponent::class)
object DeepScanModule {

    /**
     * Derin izleme koprusu.
     *
     * Bu derlemede `:module:deepscan` yalnizca arayuz olarak bulunur ve
     * her zaman [NoOpDeepScanBridge] baglanir: root gerektiren yetenekler
     * ayri bir APK ile dagitilir. Kopru yoksa yaptirim planlayici ilgili
     * eylemleri "bu cihazda yapilamadi" olarak isaretler -- sessizce
     * atlanmaz, kullanici neyin eksik oldugunu gorur.
     */
    @Provides
    @Singleton
    fun provideDeepScanBridge(): DeepScanBridge = NoOpDeepScanBridge
}
