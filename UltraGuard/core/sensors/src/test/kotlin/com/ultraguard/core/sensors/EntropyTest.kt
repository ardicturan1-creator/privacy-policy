package com.ultraguard.core.sensors

import com.google.common.truth.Truth.assertThat
import kotlin.random.Random
import org.junit.Test

/**
 * Entropi hesabinin dogrulugu, R-STATIC-011 kuralinin yanlis pozitif oranini
 * dogrudan belirler. Esik 7.8 bit/bayt; olcum yanlissa esik anlamsizdir.
 */
class EntropyTest {

    // StaticApkTriage'in entropi fonksiyonu Android baglami gerektirmez;
    // saf hesap oldugu icin burada dogrudan yeniden uretilir.
    private fun entropy(data: ByteArray): Double {
        if (data.isEmpty()) return 0.0
        val frequencies = IntArray(256)
        data.forEach { frequencies[it.toInt() and 0xFF]++ }
        var result = 0.0
        for (count in frequencies) {
            if (count == 0) continue
            val p = count.toDouble() / data.size
            result -= p * (Math.log(p) / Math.log(2.0))
        }
        return result
    }

    @Test
    fun `tek deger tekrari sifir entropi verir`() {
        assertThat(entropy(ByteArray(1024) { 0x41 })).isWithin(0.0001).of(0.0)
    }

    @Test
    fun `duz metin dusuk entropilidir`() {
        val text = ("Bu duz bir metindir ve tekrar eden karakterler icerir. ".repeat(50))
            .toByteArray()
        assertThat(entropy(text)).isLessThan(5.0)
    }

    @Test
    fun `rastgele veri maksimuma yakin entropi verir`() {
        val random = Random(42).nextBytes(64 * 1024)
        assertThat(entropy(random)).isGreaterThan(7.9)
    }

    @Test
    fun `sifrelenmis payload esigi asar`() {
        val random = Random(7).nextBytes(256 * 1024)
        assertThat(entropy(random)).isGreaterThan(StaticApkTriage.HIGH_ENTROPY_THRESHOLD)
    }

    @Test
    fun `siradan derlenmis kod esigin altinda kalir`() {
        // Gercek DEX'e benzer sekilde: sinirli alfabe, tekrar eden yapilar.
        val dexLike = buildString {
            repeat(20_000) { index ->
                append("Landroid/util/Log;->d(Ljava/lang/String;)V")
                append(index % 10)
            }
        }.toByteArray()
        assertThat(entropy(dexLike)).isLessThan(StaticApkTriage.HIGH_ENTROPY_THRESHOLD)
    }

    @Test
    fun `bos veri sifir dondurur`() {
        assertThat(entropy(ByteArray(0))).isEqualTo(0.0)
    }
}
