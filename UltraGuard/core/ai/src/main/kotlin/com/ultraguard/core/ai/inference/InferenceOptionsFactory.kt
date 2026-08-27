package com.ultraguard.core.ai.inference

import android.content.Context
import android.os.Build
import org.tensorflow.lite.Interpreter

/**
 * Cihazin yeteneklerine gore cikarim ayarlarini secer.
 *
 * Android NPU manzarasi parcalanmis durumdadir: ayni SoC ailesi icinde bile
 * NNAPI surucusu bazen CPU'dan yavastir. Bu yuzden delegate secimi
 * "varsa kullan" degil, **olculmus** olmalidir: ilk calistirmada kisa bir
 * mikro-benchmark yapilir, kazanan cihaz profiline yazilir.
 */
class InferenceOptionsFactory(
    private val context: Context,
    private val deviceProfile: DeviceInferenceProfile,
) {
    fun create(): Interpreter.Options = Interpreter.Options().apply {
        numThreads = deviceProfile.optimalThreadCount

        when (deviceProfile.preferredBackend) {
            InferenceBackend.NNAPI -> {
                // NNAPI Android 12+ oncesinde INT8 transformer'larda guvenilmez;
                // sessizce CPU'ya duser ve gecikme iki katina cikar.
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    useNNAPI = true
                }
            }
            InferenceBackend.XNNPACK -> {
                useXNNPACK = true
            }
        }
    }

    /** Kalibrasyon yapilmamis cihazlar icin guvenli baslangic. */
    fun createDefault(): Interpreter.Options = Interpreter.Options().apply {
        useXNNPACK = true
        numThreads = DEFAULT_THREADS
    }

    private companion object {
        const val DEFAULT_THREADS = 2
    }
}

enum class InferenceBackend { NNAPI, XNNPACK }

data class DeviceInferenceProfile(
    val preferredBackend: InferenceBackend,
    val optimalThreadCount: Int,
    /** Olculen medyan cikarim suresi; butce asilirsa model kademesi dusurulur. */
    val measuredLatencyMillis: Long,
) {
    /** 30 ms butcesini asan cihazlarda kucuk model varyanti kullanilir. */
    val needsSmallerModelTier: Boolean get() = measuredLatencyMillis > LATENCY_BUDGET_MILLIS

    companion object {
        const val LATENCY_BUDGET_MILLIS = 30L

        val CONSERVATIVE_DEFAULT = DeviceInferenceProfile(
            preferredBackend = InferenceBackend.XNNPACK,
            optimalThreadCount = 2,
            measuredLatencyMillis = 0L,
        )
    }
}
