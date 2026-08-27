package com.ultraguard.core.network

import java.nio.ByteBuffer

/**
 * TLS ClientHello ayristirici -- **sifreyi cozmeden** kimin nereye
 * baglandigini ogrenmenin yolu.
 *
 * UltraGuard TLS'i araya girerek acmaz (bkz. mimari dokumanindaki gerekce:
 * sertifika enjeksiyonu, urunumuzu tum banka trafiginin tek zafiyet noktasi
 * haline getirir). Bunun yerine el sikismanin **acik metin** olan ilk
 * paketinden iki sey cikarilir:
 *
 *  1. **SNI** -- baglanilan alan adi.
 *  2. **JA4 benzeri parmak izi** -- surum, sifre takimi listesi ve uzanti
 *     sirasi. Bu parmak izi TLS kutuphanesine ozgudur; "mesru bir Android
 *     bankacilik SDK'si" ile "gomulu bir Go dropper" farkli imza birakir.
 *     Paket adini degistirmek bunu degistirmez.
 *
 * ECH (Encrypted Client Hello) yayginlastikca SNI kaybolacaktir; parmak izi
 * ve zamanlama analizi o senaryoda da calismaya devam eder.
 */
object TlsClientHelloParser {

    fun parse(packet: ByteBuffer): ClientHello? = runCatching {
        val buffer = packet.duplicate()
        if (buffer.remaining() < MIN_RECORD_SIZE) return null

        // TLS kayit basligi: type(1) version(2) length(2)
        if (buffer.get().toInt() and 0xFF != RECORD_TYPE_HANDSHAKE) return null
        buffer.position(buffer.position() + 2) // kayit surumu -- guvenilmez, atlanir
        val recordLength = buffer.short.toInt() and 0xFFFF
        if (recordLength > buffer.remaining()) return null

        // El sikisma basligi: type(1) length(3)
        if (buffer.get().toInt() and 0xFF != HANDSHAKE_TYPE_CLIENT_HELLO) return null
        buffer.position(buffer.position() + 3)

        val clientVersion = buffer.short.toInt() and 0xFFFF
        buffer.position(buffer.position() + 32) // random

        // Oturum kimligi
        val sessionIdLength = buffer.get().toInt() and 0xFF
        buffer.position(buffer.position() + sessionIdLength)

        // Sifre takimlari
        val cipherSuitesLength = buffer.short.toInt() and 0xFFFF
        val cipherSuites = IntArray(cipherSuitesLength / 2) {
            buffer.short.toInt() and 0xFFFF
        }

        // Sikistirma yontemleri
        val compressionLength = buffer.get().toInt() and 0xFF
        buffer.position(buffer.position() + compressionLength)

        if (buffer.remaining() < 2) {
            return ClientHello(clientVersion, cipherSuites.toList(), emptyList(), null)
        }

        // Uzantilar
        val extensionsLength = buffer.short.toInt() and 0xFFFF
        val extensionsEnd = buffer.position() + extensionsLength
        val extensionTypes = mutableListOf<Int>()
        var serverName: String? = null

        while (buffer.position() + 4 <= extensionsEnd && buffer.remaining() >= 4) {
            val extensionType = buffer.short.toInt() and 0xFFFF
            val extensionLength = buffer.short.toInt() and 0xFFFF
            if (extensionLength > buffer.remaining()) break

            extensionTypes += extensionType

            if (extensionType == EXTENSION_SERVER_NAME) {
                serverName = readServerName(buffer, extensionLength)
            } else {
                buffer.position(buffer.position() + extensionLength)
            }
        }

        ClientHello(
            version = clientVersion,
            cipherSuites = cipherSuites.toList(),
            extensions = extensionTypes,
            serverName = serverName,
        )
    }.getOrNull()

    private fun readServerName(buffer: ByteBuffer, extensionLength: Int): String? {
        val end = buffer.position() + extensionLength
        return runCatching {
            buffer.short // sunucu adi listesi uzunlugu
            val nameType = buffer.get().toInt() and 0xFF
            if (nameType != SNI_TYPE_HOST_NAME) {
                buffer.position(end)
                return null
            }
            val nameLength = buffer.short.toInt() and 0xFFFF
            if (nameLength <= 0 || nameLength > MAX_HOSTNAME_LENGTH) {
                buffer.position(end)
                return null
            }
            val bytes = ByteArray(nameLength)
            buffer.get(bytes)
            buffer.position(end)
            String(bytes, Charsets.US_ASCII)
        }.getOrElse {
            if (end <= buffer.limit()) buffer.position(end)
            null
        }
    }

    private const val RECORD_TYPE_HANDSHAKE = 0x16
    private const val HANDSHAKE_TYPE_CLIENT_HELLO = 0x01
    private const val EXTENSION_SERVER_NAME = 0x0000
    private const val SNI_TYPE_HOST_NAME = 0x00
    private const val MIN_RECORD_SIZE = 45
    private const val MAX_HOSTNAME_LENGTH = 253
}

data class ClientHello(
    val version: Int,
    val cipherSuites: List<Int>,
    val extensions: List<Int>,
    val serverName: String?,
) {
    /**
     * JA4 ruhunda, sadelestirilmis bir istemci parmak izi.
     *
     * GREASE degerleri (RFC 8701) filtrelenir: tarayicilar ve modern TLS
     * kutuphaneleri rastgele GREASE degerleri ekler; bunlari dahil etmek
     * parmak izini her el sikismada degistirir ve tamamen ise yaramaz kilar.
     */
    fun fingerprint(): String {
        val suites = cipherSuites.filterNot(::isGrease)
        val exts = extensions.filterNot(::isGrease)
        return buildString {
            append("t"); append(version.toString(16))
            append("d"); append(suites.size.toString().padStart(2, '0'))
            append(exts.size.toString().padStart(2, '0'))
            append("_")
            append(shortHash(suites))
            append("_")
            append(shortHash(exts))
        }
    }

    private fun isGrease(value: Int): Boolean =
        (value and 0x0F0F) == 0x0A0A && (value shr 8) == (value and 0xFF)

    private fun shortHash(values: List<Int>): String {
        var hash = 0x811C9DC5L // FNV-1a offset basis
        values.forEach { value ->
            hash = (hash xor value.toLong()) * 0x01000193L and 0xFFFFFFFFL
        }
        return hash.toString(16).padStart(8, '0').take(8)
    }
}
