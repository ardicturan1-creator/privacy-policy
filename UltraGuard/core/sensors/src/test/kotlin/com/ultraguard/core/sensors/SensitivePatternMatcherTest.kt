package com.ultraguard.core.sensors

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class SensitivePatternMatcherTest {

    private val matcher = SensitivePatternMatcher()

    @Test
    fun `bos metin siniflandirilmaz`() {
        assertThat(matcher.classify(null)).isNull()
        assertThat(matcher.classify("   ")).isNull()
    }

    @Test
    fun `bitcoin adresi taninir`() {
        assertThat(matcher.classify("Odeme: 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"))
            .isEqualTo("crypto_address")
    }

    @Test
    fun `ethereum adresi taninir`() {
        assertThat(matcher.classify("0x742d35Cc6634C0532925a3b844Bc454e4438f44e"))
            .isEqualTo("crypto_address")
    }

    @Test
    fun `iban taninir`() {
        assertThat(matcher.classify("TR330006100519786457841326")).isEqualTo("iban")
    }

    @Test
    fun `otp yalnizca baglam kelimesiyle birlikte taninir`() {
        assertThat(matcher.classify("Dogrulama kodunuz: 483920")).isEqualTo("otp_code")
        // Baglam yok: sadece bir sayi. Yanlis pozitif uretmemeli.
        assertThat(matcher.classify("Siparisiniz 483920 numarali kargoda")).isNull()
    }

    @Test
    fun `aciliyet tabanli oltalama taninir`() {
        assertThat(matcher.classify("SON UYARI: hesabiniz askiya alindi"))
            .isEqualTo("urgency_phishing")
    }

    @Test
    fun `siradan bildirim metni siniflandirilmaz`() {
        assertThat(matcher.classify("Yarin hava 24 derece, gunes acik olacak")).isNull()
        assertThat(matcher.classify("Ahmet size bir mesaj gonderdi")).isNull()
    }

    @Test
    fun `cok uzun metin taranmaz`() {
        // DoS korumasi: kotucul bir uygulama devasa bildirimlerle regex
        // motorunu mesgul edemez.
        val huge = "a".repeat(10_000) + " 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
        assertThat(matcher.classify(huge)).isNull()
    }

    @Test
    fun `siniflandirma sonucu asla girdi metnini icermez`() {
        val secret = "Dogrulama kodunuz: 483920"
        val result = matcher.classify(secret)
        assertThat(result).isNotNull()
        assertThat(result).doesNotContain("483920")
    }
}
