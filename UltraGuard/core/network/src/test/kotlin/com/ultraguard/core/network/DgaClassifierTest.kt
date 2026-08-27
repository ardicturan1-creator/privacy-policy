package com.ultraguard.core.network

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class DgaClassifierTest {

    private val classifier = DgaClassifier()

    @Test
    fun `algoritma uretimi alan adi isaretlenir`() {
        assertThat(classifier.assess("xkqzwvbtrmpljh.com")).isNotNull()
        assertThat(classifier.assess("qzxjvbkmwptrfd.net")).isNotNull()
    }

    @Test
    fun `insan tarafindan secilmis alan adlari isaretlenmez`() {
        listOf(
            "google.com",
            "wikipedia.org",
            "haberturk.com",
            "stackoverflow.com",
            "cloudfront.net",
        ).forEach { domain ->
            assertThat(classifier.assess(domain)).isNull()
        }
    }

    @Test
    fun `kisa etiketler degerlendirilmez`() {
        // Kisa adlarda entropi olcumu istatistiksel olarak anlamsizdir.
        assertThat(classifier.assess("cdn.example.com")).isNull()
    }

    @Test
    fun `gecersiz karakter iceren ad reddedilir`() {
        assertThat(classifier.assess("not_a_valid_label.com")).isNull()
    }

    @Test
    fun `unlu orani yuksek uzun ad isaretlenmez`() {
        // Uzun ama dogal: unlu orani yuksek.
        assertThat(classifier.assess("uluslararasiiletisim.com")).isNull()
    }
}
