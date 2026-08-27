package com.ultraguard.core.sensors

import javax.inject.Inject
import javax.inject.Singleton

/**
 * Hassas icerik desen tanima -- **tamamen cihaz uzerinde ve iz birakmadan**.
 *
 * Bu sinifin sozlesmesi: girdi olarak metin alir, cikti olarak **yalnizca bir
 * etiket** dondurur. Metnin kendisi hicbir yere yazilmaz, loglanmaz, olaya
 * eklenmez. Cagiran taraf yalnizca `"crypto_address"` gibi turetilmis bir
 * sinyal gorur.
 *
 * Bu ayrim, bildirim ve pano izlemenin gizlilik acisindan savunulabilir
 * olmasinin tek nedenidir: OTP kodunu okuyup "OTP gordum" demek ile OTP
 * kodunu saklamak arasindaki fark, urunun tamamini belirler.
 */
@Singleton
class SensitivePatternMatcher @Inject constructor() {

    /** @return eslesen desen etiketi veya `null`. Metin asla geri donmez. */
    fun classify(text: CharSequence?): String? {
        if (text.isNullOrBlank() || text.length > MAX_SCAN_LENGTH) return null
        val value = text.toString()

        return when {
            CRYPTO_ADDRESS.containsMatchIn(value) -> "crypto_address"
            IBAN.containsMatchIn(value) -> "iban"
            OTP_CONTEXT.containsMatchIn(value) && OTP_CODE.containsMatchIn(value) -> "otp_code"
            CREDENTIAL_PROMPT.containsMatchIn(value) -> "credential_prompt"
            URGENCY_PHISHING.containsMatchIn(value) -> "urgency_phishing"
            else -> null
        }
    }

    private companion object {
        const val MAX_SCAN_LENGTH = 4_096

        /** BTC (legacy + bech32) ve ETH adres formatlari. */
        val CRYPTO_ADDRESS = Regex(
            """\b(?:[13][a-km-zA-HJ-NP-Z1-9]{25,34}|bc1[a-z0-9]{25,62}|0x[a-fA-F0-9]{40})\b""",
        )

        /** IBAN: iki harf ulke kodu + iki kontrol hanesi + 11-30 alfanumerik. */
        val IBAN = Regex("""\b[A-Z]{2}\d{2}[A-Z0-9]{11,30}\b""")

        /**
         * OTP tespiti iki kosula baglidir: hem baglam kelimesi hem de kod
         * formati bulunmali. Tek basina "4 haneli sayi" cok fazla yanlis
         * pozitif uretir -- fiyatlar, tarihler, sayaclar.
         */
        val OTP_CONTEXT = Regex(
            """(?i)\b(otp|kod|code|dogrulama|verification|sifre|password|2fa|tek kullanimlik)\b""",
        )
        val OTP_CODE = Regex("""\b\d{4,8}\b""")

        val CREDENTIAL_PROMPT = Regex(
            """(?i)\b(sifrenizi girin|enter your password|hesabinizi dogrulayin|verify your account|oturum acin|sign in to continue)\b""",
        )

        /** Sosyal muhendisligin degismez omurgasi: aciliyet + tehdit. */
        val URGENCY_PHISHING = Regex(
            """(?i)\b(hesabiniz (askiya alindi|kapatilacak)|acilen|derhal|son uyari|account (suspended|will be closed)|immediate action required|urgent)\b""",
        )
    }
}
