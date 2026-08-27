package com.ultraguard.shield.di

import android.content.Context
import com.ultraguard.core.ai.inference.BehaviorSequenceModel
import com.ultraguard.core.ai.inference.DeviceInferenceProfile
import com.ultraguard.core.ai.inference.InferenceOptionsFactory
import com.ultraguard.core.ai.rules.RuleEngine
import com.ultraguard.core.ai.rules.RulePack
import com.ultraguard.core.common.log.UgLog
import com.ultraguard.core.engine.ModelProvider
import com.ultraguard.core.engine.SystemPackageOracle
import com.ultraguard.core.model.Capability
import com.ultraguard.core.security.ExpectedSignatures
import com.ultraguard.core.security.RootDetector
import com.ultraguard.shield.BuildConfig
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

@Module
@InstallIn(SingletonComponent::class)
object EngineModule {

    /**
     * Kural motoru, cihazin **fiilen sahip oldugu** yeteneklere gore kurulur.
     *
     * Root'suz bir cihazda eBPF kurallari hic degerlendirilmez: onlari
     * calistirmak bos yere CPU harcar ve daha kotusu, hicbir zaman
     * eslesemeyecek kurallar "kapsamimiz genis" yanilsamasi yaratir.
     */
    @Provides
    @Singleton
    fun provideRuleEngine(rootDetector: RootDetector): RuleEngine {
        val assessment = rootDetector.detect()
        val capabilities = assessment.grantedCapabilities(isDeviceOwner = false)
        UgLog.i(
            "EngineModule",
            "Kural motoru yetenekleri: ${capabilities.joinToString()} " +
                "(root gostergeleri: ${assessment.indicators.size})",
        )
        return RuleEngine(rules = RulePack.all(), availableCapabilities = capabilities)
    }

    @Provides
    @Singleton
    fun provideExpectedSignatures(): ExpectedSignatures = ExpectedSignatures(
        sha256Digests = BuildConfig.SIGNING_CERT_SHA256
            .takeIf { it.isNotBlank() }
            ?.let { setOf(it.lowercase()) }
            .orEmpty(),
    )

    /**
     * L2 modelinin tembel yuklenmesi.
     *
     * Model ~15 MB'dir ve mmap ile haritalanir. Uygulama acilisinda degil,
     * ilk gercek ihtiyacta yuklenir: cogu oturumda hicbir sey supheli
     * gorunmez ve model hic gerekmez. Erken yukleme, hicbir sey kazandirmadan
     * her acilisa gecikme ekler.
     */
    @Provides
    @Singleton
    fun provideModelProvider(
        @ApplicationContext context: Context,
    ): ModelProvider = LazyModelProvider(context)
}

private class LazyModelProvider(private val context: Context) : ModelProvider {

    private val mutex = Mutex()
    private var cached: BehaviorSequenceModel? = null

    override suspend fun model(): BehaviorSequenceModel? = mutex.withLock {
        cached?.let { return it }

        val factory = InferenceOptionsFactory(
            context = context,
            deviceProfile = DeviceInferenceProfile.CONSERVATIVE_DEFAULT,
        )

        // Model dosyasi yoksa (MVP derlemesi) L2 sessizce devre disi kalir
        // ve L1 tek basina calismaya devam eder. Koruma azalir, kesilmez.
        runCatching {
            BehaviorSequenceModel.load(
                context = context,
                assetName = MODEL_ASSET,
                modelVersion = MODEL_VERSION,
                options = factory.createDefault(),
            )
        }.onFailure {
            UgLog.i("ModelProvider", "L2 modeli yuklenemedi; yalnizca L1 aktif")
        }.getOrNull()?.also { cached = it }
    }

    override fun release() {
        cached?.close()
        cached = null
    }

    private companion object {
        const val MODEL_ASSET = "behavior_seq_int8.tflite"
        const val MODEL_VERSION = "behavior-seq-v1-int8"
    }
}

@Module
@InstallIn(SingletonComponent::class)
object SystemPackageModule {

    /**
     * Platformun guven tabanini tanimlar.
     *
     * Sistem uygulamalarini izlememek bir gudulme degil, gurultu
     * yonetimidir: onlar zaten platform imzasiyla imzalanmis ve cihazin
     * kendi guven zincirindedir. Ancak imza degisimi ve ekran yakalama
     * gibi olaylar sistem paketlerinde de her zaman incelenir
     * (bkz. `EventNormalizer.ALWAYS_INSPECT`).
     */
    @Provides
    @Singleton
    fun provideSystemPackageOracle(
        @ApplicationContext context: Context,
    ): SystemPackageOracle = object : SystemPackageOracle {

        private val cache = mutableMapOf<String, Boolean>()

        @Synchronized
        override fun isTrustedSystemPackage(packageName: String): Boolean =
            cache.getOrPut(packageName) {
                runCatching {
                    val info = context.packageManager.getApplicationInfo(packageName, 0)
                    val isSystem = info.flags and android.content.pm.ApplicationInfo.FLAG_SYSTEM != 0
                    val isUpdatedSystem =
                        info.flags and android.content.pm.ApplicationInfo.FLAG_UPDATED_SYSTEM_APP != 0
                    // Guncellenmis sistem uygulamalari guven tabaninda
                    // sayilmaz: OEM imzasiyla gelen bir uygulama magazadan
                    // guncellendiginde davranisi tamamen degisebilir.
                    isSystem && !isUpdatedSystem
                }.getOrDefault(false)
            }
    }
}
