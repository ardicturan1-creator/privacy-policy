package com.ultraguard.core.network

import android.content.Context
import android.net.ConnectivityManager
import android.os.Process
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.engine.EventBus
import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.NetworkStance
import com.ultraguard.core.model.SecurityEvent
import com.ultraguard.core.model.SensorSource
import com.ultraguard.core.model.Subject
import dagger.hilt.android.qualifiers.ApplicationContext
import java.net.InetAddress
import java.net.InetSocketAddress
import java.nio.ByteBuffer
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Tek bir paketi inceleyip izin/engel karari veren sicak yol.
 *
 * Butce: **paket basina 50 mikrosaniye alti**. Bu yuzden pahali islerin
 * hicbiri burada yapilmaz -- itibar sorgusu, DGA siniflandirmasi ve beacon
 * analizi olay olarak disari yayilir ve karar zincirinde asenkron islenir.
 * Burada yalnizca IP basligi ayristirilir ve UID engel listesine bakilir.
 */
@Singleton
class FlowInspector @Inject constructor(
    @ApplicationContext private val context: Context,
    private val policy: NetworkPolicyStore,
    private val eventBus: EventBus,
    private val beaconDetector: BeaconDetector,
    private val dgaClassifier: DgaClassifier,
    private val clock: Clock,
) {
    private val connectivityManager = context.getSystemService(ConnectivityManager::class.java)

    fun inspect(packet: ByteBuffer): FlowDecision {
        val header = IpPacket.parse(packet) ?: return FlowDecision.ALLOW

        val uid = resolveUid(header)
        if (uid == Process.INVALID_UID) return FlowDecision.ALLOW

        // Sicak yolun tamami bu iki satirdir; geri kalani asenkrondur.
        if (policy.isBlocked(uid)) {
            emitBlocked(uid, header)
            return FlowDecision.BLOCK
        }

        if (policy.stanceFor(uid) == NetworkStance.DENY_BY_DEFAULT &&
            !isAllowListed(uid, header)
        ) {
            emitBlocked(uid, header)
            return FlowDecision.BLOCK
        }

        analyzeAsync(uid, header, packet)
        return FlowDecision.ALLOW
    }

    /**
     * Paketi UID'ye baglar.
     *
     * `getConnectionOwnerUid` Android 10+ (API 29) itibariyla mevcuttur ve
     * urunun minSdk secimini tek basina belirleyen API'dir: bu olmadan
     * "hangi uygulama nereye baglaniyor" sorusunun cevabi yoktur ve ag
     * telemetrisi anonim bir paket akisina indirgenir.
     */
    private fun resolveUid(header: IpPacket): Int = runCatching {
        connectivityManager.getConnectionOwnerUid(
            header.protocol,
            InetSocketAddress(InetAddress.getByAddress(header.sourceAddress), header.sourcePort),
            InetSocketAddress(InetAddress.getByAddress(header.destinationAddress), header.destinationPort),
        )
    }.getOrDefault(Process.INVALID_UID)

    private fun isAllowListed(uid: Int, header: IpPacket): Boolean {
        // DNS ve NTP varsayilan-reddet modunda bile gecer: bunlar olmadan
        // cihaz calismaz ve kullanici korumayi kapatir.
        return header.destinationPort in ESSENTIAL_PORTS
    }

    /**
     * Pahali analiz. Sicak yolun disinda, olay veriyolu uzerinden.
     */
    private fun analyzeAsync(uid: Int, header: IpPacket, packet: ByteBuffer) {
        val now = clock.nowMillis()
        val packageName = packageNameFor(uid) ?: return
        val subject = Subject.App(packageName, uid)
        val remoteIp = InetAddress.getByAddress(header.destinationAddress).hostAddress.orEmpty()

        // TLS el sikismasi mi? Yalnizca 443 portunda ve ilk paketlerde bakilir.
        val hostname = if (header.destinationPort == HTTPS_PORT) {
            val payload = packet.duplicate().apply {
                position(minOf(header.payloadOffset, limit()))
            }
            val hello = TlsClientHelloParser.parse(payload)
            hello?.let {
                eventBus.publish(
                    SecurityEvent(
                        timestampMillis = now,
                        type = EventType.NETWORK_TLS_HANDSHAKE,
                        subject = subject,
                        source = SensorSource.NETWORK_FLOW,
                        attributes = buildMap {
                            put(EventAttributes.REMOTE_IP, remoteIp)
                            put(EventAttributes.REMOTE_PORT, header.destinationPort.toString())
                            put(EventAttributes.TLS_FINGERPRINT, it.fingerprint())
                            it.serverName?.let { sni -> put(EventAttributes.REMOTE_HOST, sni) }
                        },
                    ),
                )
            }
            hello?.serverName
        } else {
            null
        }

        val host = hostname ?: remoteIp

        // Beacon paterni
        beaconDetector.observe(BeaconDetector.FlowKey(packageName, host), now)?.let { beacon ->
            eventBus.publish(
                SecurityEvent(
                    timestampMillis = now,
                    type = EventType.NETWORK_BEACON_PATTERN,
                    subject = subject,
                    source = SensorSource.NETWORK_FLOW,
                    attributes = mapOf(
                        EventAttributes.REMOTE_HOST to host,
                        EventAttributes.BEACON_INTERVAL_MS to beacon.intervalMillis.toString(),
                    ),
                ),
            )
        }

        // DGA
        hostname?.let { name ->
            dgaClassifier.assess(name)?.let { assessment ->
                eventBus.publish(
                    SecurityEvent(
                        timestampMillis = now,
                        type = EventType.NETWORK_DGA_DOMAIN,
                        subject = subject,
                        source = SensorSource.NETWORK_FLOW,
                        attributes = mapOf(
                            EventAttributes.REMOTE_HOST to name,
                            EventAttributes.DOMAIN_ENTROPY to "%.2f".format(assessment.entropy),
                        ),
                    ),
                )
            }
        }
    }

    private fun emitBlocked(uid: Int, header: IpPacket) {
        val packageName = packageNameFor(uid) ?: return
        eventBus.publish(
            SecurityEvent(
                timestampMillis = clock.nowMillis(),
                type = EventType.NETWORK_BLOCKED_BY_POLICY,
                subject = Subject.App(packageName, uid),
                source = SensorSource.NETWORK_FLOW,
                attributes = mapOf(
                    EventAttributes.REMOTE_PORT to header.destinationPort.toString(),
                ),
            ),
        )
    }

    private fun packageNameFor(uid: Int): String? =
        context.packageManager.getPackagesForUid(uid)?.firstOrNull()

    private companion object {
        const val HTTPS_PORT = 443
        val ESSENTIAL_PORTS = setOf(53, 123, 853)
    }
}
