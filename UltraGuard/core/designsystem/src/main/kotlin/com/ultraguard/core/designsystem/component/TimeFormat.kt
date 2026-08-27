package com.ultraguard.core.designsystem.component

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Zaman damgasi bicimlendirme.
 *
 * Guvenlik olaylarinda "2 dakika once" mutlak saatten daha okunakli olur;
 * ama bir gunden eski olaylarda tam tarih gerekir -- adli inceleme yapan
 * biri "3 gun once" ile calisamaz.
 */
object TimeFormat {

    private val timeOnly = SimpleDateFormat("HH:mm", Locale.getDefault())
    private val dateTime = SimpleDateFormat("d MMM HH:mm", Locale.getDefault())

    fun relative(timestampMillis: Long, nowMillis: Long): String {
        val delta = nowMillis - timestampMillis
        return when {
            delta < MINUTE -> "az önce"
            delta < HOUR -> "${delta / MINUTE} dk önce"
            delta < DAY -> timeOnly.format(Date(timestampMillis))
            else -> dateTime.format(Date(timestampMillis))
        }
    }

    fun absolute(timestampMillis: Long): String = dateTime.format(Date(timestampMillis))

    /** Bayt sayisini insan okunur hale getirir. */
    fun bytes(value: Long): String = when {
        value < 1024 -> "$value B"
        value < 1024 * 1024 -> "%.1f KB".format(value / 1024.0)
        value < 1024L * 1024 * 1024 -> "%.1f MB".format(value / (1024.0 * 1024))
        else -> "%.2f GB".format(value / (1024.0 * 1024 * 1024))
    }

    private const val MINUTE = 60_000L
    private const val HOUR = 60 * MINUTE
    private const val DAY = 24 * HOUR
}
