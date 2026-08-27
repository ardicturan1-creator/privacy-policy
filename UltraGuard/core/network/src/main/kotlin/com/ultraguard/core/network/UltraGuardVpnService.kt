package com.ultraguard.core.network

import android.content.Intent
import android.net.VpnService
import android.os.ParcelFileDescriptor
import com.ultraguard.core.common.di.ApplicationScope
import com.ultraguard.core.common.di.IoDispatcher
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.model.NetworkStance
import dagger.hilt.android.AndroidEntryPoint
import java.io.FileInputStream
import java.io.FileOutputStream
import java.nio.ByteBuffer
import javax.inject.Inject
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Yerel Zero Trust ag katmani.
 *
 * **Onemli mimari not:** Bu bir VPN degildir. Trafik hicbir sunucuya
 * gonderilmez, hicbir yere tunellenmez, UltraGuard'in altyapisina ugramaz.
 * `VpnService`, Android'de bir uygulamanin kendi cihazinin ag trafigini
 * gorebilmesinin **tek** desteklenen yoludur; biz yalnizca bu API'yi
 * kullaniyoruz.
 *
 * Rakiplerin cogu ayni API'yi kullanir ama trafigi kendi bulutlarina
 * yonlendirir. Bu, kullanicinin tum ag etkinligini ucuncu bir tarafa
 * teslim etmek demektir. Bizim tunel cikisimiz yoktur: paketler incelenir,
 * karar verilir ve **ayni cihazda** gercek arayuze geri yazilir.
 *
 * Islenen sey yalnizca IP basligi ve TLS el sikismasinin acik metin ilk
 * paketidir; uygulama verisi ayristirilmaz.
 */
@AndroidEntryPoint
class UltraGuardVpnService : VpnService() {

    @Inject lateinit var policy: NetworkPolicyStore
    @Inject lateinit var inspector: FlowInspector
    @Inject @IoDispatcher lateinit var ioDispatcher: CoroutineDispatcher
    @Inject @ApplicationScope lateinit var scope: CoroutineScope

    private var tunnel: ParcelFileDescriptor? = null
    private var pumpJob: Job? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                teardown()
                return START_NOT_STICKY
            }
            else -> establish()
        }
        return START_STICKY
    }

    private fun establish() {
        if (tunnel != null) return

        val builder = Builder()
            .setSession(SESSION_NAME)
            .setMtu(MTU)
            .addAddress(LOCAL_IPV4, IPV4_PREFIX)
            .addAddress(LOCAL_IPV6, IPV6_PREFIX)
            .addRoute("0.0.0.0", 0)
            .addRoute("::", 0)
            .setBlocking(true)

        // UltraGuard'in kendisi tunelin disinda birakilir; aksi halde kendi
        // trafigimizi denetleme dongusune gireriz.
        runCatching { builder.addDisallowedApplication(packageName) }

        // Kullanicinin acikca muaf tuttugu uygulamalar (or. kurumsal VPN
        // istemcileri) tunele hic girmez. Seffaflik Merkezi'nde listelenir.
        policy.exemptPackages().forEach { exempt ->
            runCatching { builder.addDisallowedApplication(exempt) }
        }

        tunnel = runCatching { builder.establish() }.getOrElse { error ->
            UgLog.e(TAG, "VPN arayuzu kurulamadi", error)
            null
        } ?: return

        pumpJob = scope.launch(ioDispatcher) { pumpPackets() }
    }

    /**
     * Paket dongusu.
     *
     * Buradaki her satir, cihazin **tum** ag trafiginin sicak yolundadir.
     * Ayirma (allocation) yapilmaz: tek bir `ByteBuffer` yeniden kullanilir. Dongude nesne uretmek, saniyede binlerce paket altinda
     * GC baskisi yaratir ve pil butcesini tek basina tuketir.
     */
    private suspend fun pumpPackets() {
        val descriptor = tunnel ?: return
        val input = FileInputStream(descriptor.fileDescriptor)
        val output = FileOutputStream(descriptor.fileDescriptor)
        // Heap buffer kullaniliyor: `allocateDirect` ile ayrilan bir buffer'in
        // destekleyici dizisi yoktur ve `array()` cagrisi calisma zamaninda
        // UnsupportedOperationException atar. Akis API'si dizi uzerinden
        // calistigi icin burada heap buffer dogru secimdir.
        val buffer = ByteBuffer.allocate(MTU)

        try {
            while (scope.isActive) {
                buffer.clear()
                val length = input.read(buffer.array(), 0, MTU)
                if (length <= 0) continue
                buffer.limit(length)

                val decision = inspector.inspect(buffer)

                when (decision) {
                    FlowDecision.ALLOW -> {
                        buffer.rewind()
                        output.write(buffer.array(), 0, length)
                    }
                    FlowDecision.BLOCK -> {
                        // Paket sessizce dusurulur. Uygulama zaman asimi gorur.
                        // RST gondermek daha kibar olurdu ama kotucul yazilima
                        // "engellendin" sinyali vermek, kacinma davranisini
                        // tetikler; sessiz dusurme daha az bilgi sizdirir.
                    }
                }
            }
        } catch (error: Exception) {
            if (scope.isActive) UgLog.w(TAG, "Paket dongusu sonlandi", error)
        } finally {
            runCatching { input.close() }
            runCatching { output.close() }
        }
    }

    private fun teardown() {
        pumpJob?.cancel()
        pumpJob = null
        runCatching { tunnel?.close() }
        tunnel = null
        stopSelf()
    }

    override fun onDestroy() {
        teardown()
        super.onDestroy()
    }

    companion object {
        const val ACTION_STOP = "com.ultraguard.action.STOP_VPN"

        private const val TAG = "VpnService"
        private const val SESSION_NAME = "UltraGuard Zero Trust"
        private const val MTU = 1500
        private const val LOCAL_IPV4 = "10.111.222.1"
        private const val IPV4_PREFIX = 32
        private const val LOCAL_IPV6 = "fd00:1:ug::1"
        private const val IPV6_PREFIX = 128
    }
}

enum class FlowDecision { ALLOW, BLOCK }

/** Uygulama basina ag politikasi ve muafiyet listesi. */
interface NetworkPolicyStore {
    fun stanceFor(uid: Int): NetworkStance
    fun isBlocked(uid: Int): Boolean
    fun blockUid(uid: Int, untilMillis: Long?)
    fun unblockUid(uid: Int)
    fun exemptPackages(): Set<String>
}
