package com.ultraguard.module.deepscan

import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.SecurityEvent
import kotlinx.coroutines.flow.Flow

/**
 * Root / KernelSU cihazlar icin derin izleme koprusu.
 *
 * **Mimari izolasyon kurali:** bu modul ayri bir APK olarak dagitilir ve ana
 * uygulama onsuz tam islevseldir. Iki nedeni var:
 *
 *  1. **Play Store uyumlulugu.** Root araclariyla konusan kod, magaza
 *     politikalari acisindan risklidir; ana urunu buna baglamayiz.
 *  2. **Saldiri yuzeyi.** Yukseltilmis yetkiyle calisan kod, urunun geri
 *     kalanindan surec olarak ayrilir. Buradaki bir zafiyet, ana uygulamanin
 *     verisine dogrudan erisim vermez.
 *
 * Kopru, kullanilabilir degilse **sessizce** devre disi kalir: cagiran taraf
 * [isAvailable] uzerinden kontrol eder ve root olmayan cihazda hicbir sey
 * degismez.
 */
interface DeepScanBridge {

    /** Modul kurulu, yetkilendirilmis ve kernel yuzeyi erisilebilir mi? */
    suspend fun isAvailable(): Boolean

    /** Bu cihazda fiilen acilabilen derin izleme yetenekleri. */
    suspend fun availableProbes(): Set<DeepProbe>

    /**
     * Kernel olay akisini baslatir.
     *
     * eBPF programlari yalnizca GKI 5.10+ ve `CONFIG_BPF_SYSCALL` acik
     * kernel'lerde yuklenebilir. Yuklenemezse [DeepProbe.PROCFS_POLLING]
     * ile sinirli bir yedege dusulur -- daha az bilgi, ama hic yoktan iyi.
     */
    fun events(probes: Set<DeepProbe>): Flow<SecurityEvent>

    /**
     * Bir sureci cgroup v2 freezer ile dondurur.
     *
     * Oldurmek yerine dondurmayi tercih ederiz: bellekteki payload adli
     * inceleme icin korunur ve islem geri alinabilir kalir -- geri
     * alinabilirlik, otonom uygulanabilmesinin on kosuludur.
     */
    suspend fun freezeProcess(pid: Int): Boolean

    suspend fun unfreezeProcess(pid: Int): Boolean

    companion object {
        val REQUIRED_CAPABILITY = Capability.ROOTED
    }
}

/**
 * Derin izleme sondasi. Her biri ayri bir kernel yuzeyine dayanir ve
 * bagimsiz olarak kullanilamaz olabilir.
 */
enum class DeepProbe(val requiresEbpf: Boolean) {
    /** `execve` / `execveat` -- yeni surec calistirma. */
    SYSCALL_EXEC(requiresEbpf = true),

    /** `memfd_create` -- diske yazmayan dosyasiz yukleyicilerin imzasi. */
    SYSCALL_MEMFD(requiresEbpf = true),

    /** `ptrace` -- surec enjeksiyonu ve hata ayiklayici baglanmasi. */
    SYSCALL_PTRACE(requiresEbpf = true),

    /** `binder_transaction` tracepoint -- IPC uzerinden izin atlatma. */
    BINDER_TRANSACTIONS(requiresEbpf = true),

    /** `fanotify FAN_OPEN_EXEC_PERM` -- calistirma aninda engelleme. */
    EXEC_BLOCKING(requiresEbpf = false),

    /** eBPF yoksa yedek: dusuk frekansli procfs yoklamasi. */
    PROCFS_POLLING(requiresEbpf = false),
}

/** Modul kurulu olmadiginda kullanilan, her zaman bos davranan uygulama. */
object NoOpDeepScanBridge : DeepScanBridge {
    override suspend fun isAvailable(): Boolean = false
    override suspend fun availableProbes(): Set<DeepProbe> = emptySet()
    override fun events(probes: Set<DeepProbe>): Flow<SecurityEvent> =
        kotlinx.coroutines.flow.emptyFlow()
    override suspend fun freezeProcess(pid: Int): Boolean = false
    override suspend fun unfreezeProcess(pid: Int): Boolean = false
}
