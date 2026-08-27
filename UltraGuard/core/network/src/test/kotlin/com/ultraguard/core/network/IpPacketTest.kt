package com.ultraguard.core.network

import com.google.common.truth.Truth.assertThat
import java.nio.ByteBuffer
import org.junit.Test

class IpPacketTest {

    private fun ipv4Tcp(
        sourcePort: Int = 44321,
        destinationPort: Int = 443,
        tcpDataOffsetWords: Int = 5,
    ): ByteBuffer {
        val packet = ByteArray(60)
        packet[0] = 0x45                              // version 4, IHL 5
        packet[9] = IpPacket.PROTOCOL_TCP.toByte()
        // kaynak 192.168.1.10
        packet[12] = 192.toByte(); packet[13] = 168.toByte(); packet[14] = 1; packet[15] = 10
        // hedef 93.184.216.34
        packet[16] = 93; packet[17] = 184.toByte(); packet[18] = 216.toByte(); packet[19] = 34

        packet[20] = (sourcePort shr 8).toByte()
        packet[21] = (sourcePort and 0xFF).toByte()
        packet[22] = (destinationPort shr 8).toByte()
        packet[23] = (destinationPort and 0xFF).toByte()
        packet[32] = (tcpDataOffsetWords shl 4).toByte()

        return ByteBuffer.wrap(packet)
    }

    @Test
    fun `ipv4 tcp basligi ayristirilir`() {
        val parsed = IpPacket.parse(ipv4Tcp())
        assertThat(parsed).isNotNull()
        assertThat(parsed!!.version).isEqualTo(4)
        assertThat(parsed.protocol).isEqualTo(IpPacket.PROTOCOL_TCP)
        assertThat(parsed.destinationPort).isEqualTo(443)
        assertThat(parsed.sourcePort).isEqualTo(44321)
    }

    @Test
    fun `payload ofseti tcp secenekleri dahil hesaplanir`() {
        val standard = IpPacket.parse(ipv4Tcp(tcpDataOffsetWords = 5))!!
        val withOptions = IpPacket.parse(ipv4Tcp(tcpDataOffsetWords = 8))!!

        assertThat(standard.payloadOffset).isEqualTo(40)   // 20 IP + 20 TCP
        assertThat(withOptions.payloadOffset).isEqualTo(52) // 20 IP + 32 TCP
    }

    @Test
    fun `cok kisa paket null doner`() {
        assertThat(IpPacket.parse(ByteBuffer.wrap(ByteArray(8)))).isNull()
    }

    @Test
    fun `gecersiz surum null doner`() {
        val packet = ByteArray(40)
        packet[0] = 0x95.toByte() // surum 9
        assertThat(IpPacket.parse(ByteBuffer.wrap(packet))).isNull()
    }

    @Test
    fun `gecersiz ihl degeri null doner`() {
        val packet = ByteArray(40)
        packet[0] = 0x43 // surum 4, IHL 3 -- minimumun altinda
        assertThat(IpPacket.parse(ByteBuffer.wrap(packet))).isNull()
    }
}
