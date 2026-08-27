package com.ultraguard.core.common.log

import android.util.Log

/**
 * Gizlilik farkindaligi olan loglama.
 *
 * Guvenlik urunlerinde en sik gorulen veri sizintisi, logcat'e yazilan
 * hata ayiklama satirlaridir: paket adlari, URL'ler, bildirim metinleri.
 * Logcat'i cihazdaki baska bir uygulama (veya ADB uzerinden fiziksel
 * erisimi olan biri) okuyabilir.
 *
 * Bu yuzden: **release derlemesinde hassas log tamamen susturulur**, debug
 * derlemesinde ise hassas alanlar kismen maskelenir.
 */
object UgLog {

    private const val TAG_PREFIX = "UG."

    @Volatile
    var verboseEnabled: Boolean = false

    fun d(tag: String, message: () -> String) {
        if (verboseEnabled) Log.d(TAG_PREFIX + tag, message())
    }

    fun i(tag: String, message: String) = Log.i(TAG_PREFIX + tag, message)

    fun w(tag: String, message: String, error: Throwable? = null) {
        Log.w(TAG_PREFIX + tag, message, error)
    }

    fun e(tag: String, message: String, error: Throwable? = null) {
        Log.e(TAG_PREFIX + tag, message, error)
    }

    /**
     * Hassas bir degeri loglanabilir hale getirir: "com.example.kargotakip"
     * yerine "com.ex…ip(11)". Korelasyon icin yeterli, kimlik icin degil.
     */
    fun redact(value: String?): String {
        if (value.isNullOrEmpty()) return "∅"
        if (!verboseEnabled) return "…(${value.length})"
        if (value.length <= 8) return "…(${value.length})"
        return "${value.take(6)}…${value.takeLast(2)}(${value.length})"
    }
}
