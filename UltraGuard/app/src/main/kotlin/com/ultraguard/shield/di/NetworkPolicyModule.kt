package com.ultraguard.shield.di

import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.model.NetworkStance
import com.ultraguard.core.network.NetworkPolicyStore
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object NetworkPolicyModule {

    @Provides
    @Singleton
    fun provideNetworkPolicyStore(clock: Clock): NetworkPolicyStore =
        InMemoryNetworkPolicyStore(clock)
}

/**
 * Uygulama basina ag politikasi.
 *
 * Bellekte tutulur cunku paket incelemesinin sicak yolundan sorgulanir:
 * her pakette bir veritabani sorgusu yapmak, cihazin tum ag trafigini
 * yavaslatir. Kalici politika degisiklikleri ayrica diske yazilir ve
 * acilista buraya yuklenir.
 */
private class InMemoryNetworkPolicyStore(
    private val clock: Clock,
) : NetworkPolicyStore {

    private data class Block(val untilMillis: Long?)

    private val blocked = ConcurrentHashMap<Int, Block>()
    private val stances = ConcurrentHashMap<Int, NetworkStance>()
    private val exempt = java.util.concurrent.CopyOnWriteArraySet<String>()

    override fun stanceFor(uid: Int): NetworkStance =
        stances[uid] ?: NetworkStance.ALLOW_WITH_INSPECTION

    override fun isBlocked(uid: Int): Boolean {
        val block = blocked[uid] ?: return false
        val until = block.untilMillis ?: return true

        // Suresi dolan engel kendiliginden kalkar. Geri alinabilirligin
        // en basit bicimi budur: unutulmus bir yaptirim kalici hale gelmez.
        if (clock.nowMillis() >= until) {
            blocked.remove(uid)
            return false
        }
        return true
    }

    override fun blockUid(uid: Int, untilMillis: Long?) {
        blocked[uid] = Block(untilMillis)
    }

    override fun unblockUid(uid: Int) {
        blocked.remove(uid)
    }

    override fun exemptPackages(): Set<String> = exempt.toSet()
}
