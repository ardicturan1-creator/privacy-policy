package com.ultraguard.core.network

import java.nio.ByteBuffer

/**
 * En kucuk IP basligi ayristiricisi.
 *
 * Yalnizca yonlendirme karari icin gereken alanlar okunur. Uygulama verisi
 * ayristirilmaz, kopyalanmaz, saklanmaz -- yalnizca TLS el sikismasinin
 * baslangicina bir ofset hesaplanir.
 */
data class IpPacket(
    val version: Int,
    val protocol: Int,
    val sourceAddress: ByteArray,
    val destinationAddress: ByteArray,
    val sourcePort: Int,
    val destinationPort: Int,
    /** Paket icindeki uygulama katmani verisinin baslangic ofseti. */
    val payloadOffset: Int,
) {
    // ByteArray alanlari nedeniyle data class'in uretilen equals/hashCode
    // metotlari referans karsilastirir; degere gore karsilastirma gerekiyor.
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is IpPacket) return false
        return version == other.version &&
            protocol == other.protocol &&
            sourcePort == other.sourcePort &&
            destinationPort == other.destinationPort &&
            payloadOffset == other.payloadOffset &&
            sourceAddress.contentEquals(other.sourceAddress) &&
            destinationAddress.contentEquals(other.destinationAddress)
    }

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + protocol
        result = 31 * result + sourceAddress.contentHashCode()
        result = 31 * result + destinationAddress.contentHashCode()
        result = 31 * result + sourcePort
        result = 31 * result + destinationPort
        result = 31 * result + payloadOffset
        return result
    }

    companion object {
        const val PROTOCOL_TCP = 6
        const val PROTOCOL_UDP = 17

        private const val IPV4_MIN_HEADER_BYTES = 20
        private const val IPV6_HEADER_BYTES = 40
        private const val TRANSPORT_HEADER_MIN = 4

        fun parse(packet: ByteBuffer): IpPacket? = runCatching {
            val buffer = packet.duplicate()
            if (buffer.remaining() < IPV4_MIN_HEADER_BYTES) return null

            val start = buffer.position()
            val versionAndIhl = buffer.get(start).toInt() and 0xFF
            return when (versionAndIhl shr 4) {
                4 -> parseIpv4(buffer, start, versionAndIhl and 0x0F)
                6 -> parseIpv6(buffer, start)
                else -> null
            }
        }.getOrNull()

        private fun parseIpv4(buffer: ByteBuffer, start: Int, ihlWords: Int): IpPacket? {
            val headerLength = ihlWords * 4
            if (headerLength < IPV4_MIN_HEADER_BYTES) return null
            if (buffer.limit() - start < headerLength + TRANSPORT_HEADER_MIN) return null

            val protocol = buffer.get(start + 9).toInt() and 0xFF
            val source = ByteArray(4) { buffer.get(start + 12 + it) }
            val destination = ByteArray(4) { buffer.get(start + 16 + it) }

            val transportStart = start + headerLength
            val sourcePort = readPort(buffer, transportStart)
            val destinationPort = readPort(buffer, transportStart + 2)

            return IpPacket(
                version = 4,
                protocol = protocol,
                sourceAddress = source,
                destinationAddress = destination,
                sourcePort = sourcePort,
                destinationPort = destinationPort,
                payloadOffset = transportStart + transportHeaderLength(buffer, protocol, transportStart),
            )
        }

        private fun parseIpv6(buffer: ByteBuffer, start: Int): IpPacket? {
            if (buffer.limit() - start < IPV6_HEADER_BYTES + TRANSPORT_HEADER_MIN) return null

            // Uzanti basliklari (Hop-by-Hop, Routing, ...) burada takip
            // edilmez: cihaz trafiginde neredeyse hic gorulmezler ve sicak
            // yolda zincir yurumek maliyetlidir. Bilinmeyen next-header
            // durumunda paket incelenmeden gecirilir -- guvenli varsayilan.
            val nextHeader = buffer.get(start + 6).toInt() and 0xFF
            if (nextHeader != PROTOCOL_TCP && nextHeader != PROTOCOL_UDP) return null

            val source = ByteArray(16) { buffer.get(start + 8 + it) }
            val destination = ByteArray(16) { buffer.get(start + 24 + it) }

            val transportStart = start + IPV6_HEADER_BYTES
            return IpPacket(
                version = 6,
                protocol = nextHeader,
                sourceAddress = source,
                destinationAddress = destination,
                sourcePort = readPort(buffer, transportStart),
                destinationPort = readPort(buffer, transportStart + 2),
                payloadOffset = transportStart +
                    transportHeaderLength(buffer, nextHeader, transportStart),
            )
        }

        private fun readPort(buffer: ByteBuffer, offset: Int): Int =
            ((buffer.get(offset).toInt() and 0xFF) shl 8) or (buffer.get(offset + 1).toInt() and 0xFF)

        private fun transportHeaderLength(buffer: ByteBuffer, protocol: Int, start: Int): Int =
            when (protocol) {
                PROTOCOL_UDP -> 8
                PROTOCOL_TCP -> {
                    // TCP veri ofseti: 12. baytin ust dort biti, 32-bit kelime cinsinden.
                    val dataOffsetWords = (buffer.get(start + 12).toInt() and 0xF0) shr 4
                    (dataOffsetWords * 4).coerceAtLeast(20)
                }
                else -> 0
            }
    }
}
