package com.ultraguard.core.designsystem.component

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import com.ultraguard.core.model.Attribution

/**
 * Kanit anahtarini kullanicinin dilindeki cumleye cevirir.
 *
 * Kural motoru Android'e bagli olmadigi icin yalnizca bir anahtar
 * (`expl_overlay`) ve argumanlar uretir; metne donusum burada, sunum
 * katmaninda yapilir. Bu ayrim motorun saf JVM testinde calisabilmesini
 * saglayan seydir.
 *
 * Anahtar bulunamazsa bos metin degil, olay turunun kendisi gosterilir:
 * kullaniciya bos bir kanit satiri gostermek, hicbir sey gostermemekten
 * daha kotudur.
 */
@Composable
fun rememberExplanation(attribution: Attribution): String {
    val context = LocalContext.current
    return explanationOf(context, attribution)
}

fun explanationOf(context: Context, attribution: Attribution): String {
    val resourceId = context.resources.getIdentifier(
        attribution.explanationKey,
        "string",
        context.packageName,
    )
    if (resourceId == 0) return attribution.eventType.name

    return runCatching {
        if (attribution.explanationArgs.isEmpty()) {
            context.getString(resourceId)
        } else {
            context.getString(resourceId, *attribution.explanationArgs.toTypedArray())
        }
    }.getOrElse {
        // Bicimlendirme arguman sayisi tutmuyorsa (or. kural guncellendi ama
        // ceviri henuz yetismedi) cokmek yerine argumansiz metne duseriz.
        runCatching { context.getString(resourceId) }.getOrDefault(attribution.eventType.name)
    }
}
