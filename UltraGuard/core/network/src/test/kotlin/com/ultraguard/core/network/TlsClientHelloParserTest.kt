package com.ultraguard.core.network

import com.google.common.truth.Truth.assertThat
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import org.junit.Test

/**
 * SNI cikarimi, sifrelenmis trafik analizinin temelidir: yanlis ayristirma
 * kullaniciya yanlis alan adi gostermek demektir. Bu testler ayristiriciyi
 * hem gecerli hem de bozuk girdiye karsi dogrular -- bozuk paketler
 * saldirgan tarafindan bilincli olarak gonderilebilir.
 */
class TlsClientHelloParserTest {

    /** Gercekci bir ClientHello uretir. */
    private fun clientHello(
        serverName: String?,
        cipherSuites: List<Int> = listOf(0x1301, 0x1302, 0xC02B),
        includeGrease: Boolean = false,
    ): ByteBuffer {
        val extensions = ByteArrayOutputStream()

        if (includeGrease) {
            extensions.writeShort(0x0A0A) // GREASE uzanti tipi
            extensions.writeShort(0)
        }

        if (serverName != null) {
            val nameBytes = serverName.toByteArray(Charsets.US_ASCII)
            val sni = ByteArrayOutputStream().apply {
                writeShort(nameBytes.size + 3) // sunucu adi listesi uzunlugu
                write(0x00)                    // tip: host_name
                writeShort(nameBytes.size)
                write(nameBytes)
            }.toByteArray()

            extensions.writeShort(0x0000) // server_name uzantisi
            extensions.writeShort(sni.size)
            extensions.write(sni)
        }

        // supported_versions uzantisi
        extensions.writeShort(0x002B)
        extensions.writeShort(3)
        extensions.write(2)
        extensions.writeShort(0x0304)

        val extensionBytes = extensions.toByteArray()

        val body = ByteArrayOutputStream().apply {
            writeShort(0x0303)          // istemci surumu
            write(ByteArray(32) { 0x2A }) // random
            write(0)                    // oturum kimligi uzunlugu
            writeShort(cipherSuites.size * 2)
            cipherSuites.forEach { writeShort(it) }
            write(1)                    // sikistirma yontemi sayisi
            write(0)                    // null
            writeShort(extensionBytes.size)
            write(extensionBytes)
        }.toByteArray()

        val handshake = ByteArrayOutputStream().apply {
            write(0x01)                 // client_hello
            write((body.size shr 16) and 0xFF)
            write((body.size shr 8) and 0xFF)
            write(body.size and 0xFF)
            write(body)
        }.toByteArray()

        val record = ByteArrayOutputStream().apply {
            write(0x16)                 // handshake
            writeShort(0x0301)
            writeShort(handshake.size)
            write(handshake)
        }.toByteArray()

        return ByteBuffer.wrap(record)
    }

    private fun ByteArrayOutputStream.writeShort(value: Int) {
        write((value shr 8) and 0xFF)
        write(value and 0xFF)
    }

    @Test
    fun `sni dogru cikarilir`() {
        val hello = TlsClientHelloParser.parse(clientHello("bank.example.com"))
        assertThat(hello).isNotNull()
        assertThat(hello!!.serverName).isEqualTo("bank.example.com")
    }

    @Test
    fun `sni olmayan el sikismasi yine de ayristirilir`() {
        val hello = TlsClientHelloParser.parse(clientHello(null))
        assertThat(hello).isNotNull()
        assertThat(hello!!.serverName).isNull()
        assertThat(hello.cipherSuites).isNotEmpty()
    }

    @Test
    fun `el sikismasi olmayan paket reddedilir`() {
        val applicationData = ByteBuffer.wrap(
            byteArrayOf(0x17, 0x03, 0x03, 0x00, 0x10) + ByteArray(64),
        )
        assertThat(TlsClientHelloParser.parse(applicationData)).isNull()
    }

    @Test
    fun `cok kisa paket cokmez ve null doner`() {
        assertThat(TlsClientHelloParser.parse(ByteBuffer.wrap(byteArrayOf(0x16, 0x03)))).isNull()
    }

    @Test
    fun `bozuk uzunluk alani cokmeye yol acmaz`() {
        val bytes = clientHello("evil.example.com").array().copyOf()
        // Kayit uzunlugunu gercekte olandan cok daha buyuk gosterelim.
        bytes[3] = 0x7F
        bytes[4] = 0xFF.toByte()
        assertThat(TlsClientHelloParser.parse(ByteBuffer.wrap(bytes))).isNull()
    }

    @Test
    fun `parmak izi ayni istemci icin kararlidir`() {
        val a = TlsClientHelloParser.parse(clientHello("a.example.com"))!!
        val b = TlsClientHelloParser.parse(clientHello("b.example.com"))!!
        // Parmak izi istemci kutuphanesini tanimlar, hedefi degil:
        // farkli sunucuya giden ayni istemci ayni izi birakmalidir.
        assertThat(a.fingerprint()).isEqualTo(b.fingerprint())
    }

    @Test
    fun `farkli sifre takimi farkli parmak izi verir`() {
        val a = TlsClientHelloParser.parse(clientHello("x.example.com", listOf(0x1301)))!!
        val b = TlsClientHelloParser.parse(
            clientHello("x.example.com", listOf(0x1301, 0x1302, 0xC030)),
        )!!
        assertThat(a.fingerprint()).isNotEqualTo(b.fingerprint())
    }

    @Test
    fun `grease degerleri parmak izini bozmaz`() {
        val withoutGrease = TlsClientHelloParser.parse(clientHello("x.example.com"))!!
        val withGrease = TlsClientHelloParser.parse(
            clientHello("x.example.com", includeGrease = true),
        )!!
        assertThat(withGrease.fingerprint()).isEqualTo(withoutGrease.fingerprint())
    }
}
