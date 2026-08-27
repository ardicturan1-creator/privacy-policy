package com.ultraguard.core.sensors

import java.util.concurrent.CopyOnWriteArraySet
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Ekran ustu pencereleri bastirilan paketlerin kaydi.
 *
 * Bir uygulamanin penceresini dogrudan kapatamayiz; yapabildigimiz, kendi
 * korunan ekranlarimizda `HIDE_OVERLAY_WINDOWS` bayragini acmak ve
 * saldirganin uzerine kendi uyarimizi cizmektir. Bu kayit, erisilebilirlik
 * servisi ile yaptirim katmani arasindaki paylasilan durumdur.
 */
@Singleton
class OverlaySuppressionRegistry @Inject constructor() {

    private val suppressed = CopyOnWriteArraySet<String>()

    fun suppress(packageName: String) {
        suppressed += packageName
    }

    fun release(packageName: String) {
        suppressed -= packageName
    }

    fun isSuppressed(packageName: String): Boolean = packageName in suppressed

    fun all(): Set<String> = suppressed.toSet()

    val hasAny: Boolean get() = suppressed.isNotEmpty()
}
