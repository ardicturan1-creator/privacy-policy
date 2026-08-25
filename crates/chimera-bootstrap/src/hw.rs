//! Donanim tespiti.
//!
//! Tasarim ilkesi: **hicbir GPU kutuphanesi derleme zamaninda linklenmez.**
//! NVML, DXGI, Metal ve Vulkan calisma zamaninda `dlopen`/`LoadLibrary` ile
//! aranir. Sürücü yoksa binary yine de acilir ve CPU planina duser.
//! Bu, "makinede CUDA yok" diye acilmayan kurulum dosyasi sinifini tamamen
//! ortadan kaldirir.
//!
//! Ikinci ilke: **her zaman `free`/`budget`, asla `total`.** Masaustunde
//! pencere yoneticisi ve tarayici VRAM tuketir; `total` uzerinden yapilan
//! her hesap yaniltici derecede iyimserdir.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub const MIB: u64 = 1024 * 1024;
pub const GIB: u64 = 1024 * MIB;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cuda,
    Rocm,
    Metal,
    Vulkan,
    Cpu,
}

impl Backend {
    /// Backend'in sabit ek yuku: context olusturma, BLAS workspace,
    /// allocator arena'si. Olcumle kalibre edilen deneysel sabitler.
    pub fn fixed_overhead(self) -> u64 {
        match self {
            Backend::Cuda => 380 * MIB,
            Backend::Rocm => 520 * MIB,
            Backend::Metal => 180 * MIB,
            Backend::Vulkan => 260 * MIB,
            Backend::Cpu => 0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Cuda => "cuda",
            Backend::Rocm => "rocm",
            Backend::Metal => "metal",
            Backend::Vulkan => "vulkan",
            Backend::Cpu => "cpu",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub backend: Backend,
    /// Fiziksel toplam. Yalnizca raporlama icin; planlamada KULLANILMAZ.
    pub vram_total: u64,
    /// Bu surecin gercekten talep edebilecegi miktar.
    /// NVML `free`, DXGI `Budget - CurrentUsage`,
    /// Metal `recommendedMaxWorkingSetSize`, Vulkan `heapBudget - heapUsage`.
    pub vram_budget: u64,
    /// Birlesik bellek (Apple Silicon). Ayri bir VRAM havuzu yoktur;
    /// planlayici RAM ile VRAM'i cift saymamalidir.
    pub unified: bool,
}

#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// Yalnizca performans cekirdekleri. SMT kardesleri ve E-core'lar haric.
    pub perf_cores: usize,
    pub logical: usize,
    pub numa_nodes: usize,
}

#[derive(Debug, Clone)]
pub struct HostProfile {
    pub cpu: CpuInfo,
    pub ram_available: u64,
    pub gpus: Vec<GpuInfo>,
}

impl HostProfile {
    /// Planlamaya girecek birincil hizlandirici. Cok GPU'lu sistemlerde
    /// MVP kapsaminda en genis butceye sahip olan secilir; tensor-parallel
    /// bolusturme sonraki asama.
    pub fn primary_gpu(&self) -> Option<&GpuInfo> {
        self.gpus.iter().max_by_key(|g| g.vram_budget)
    }

    pub fn backend(&self) -> Backend {
        self.primary_gpu().map(|g| g.backend).unwrap_or(Backend::Cpu)
    }
}

pub fn detect() -> HostProfile {
    HostProfile {
        cpu: detect_cpu(),
        ram_available: detect_available_ram(),
        gpus: detect_gpus(),
    }
}

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

fn detect_cpu() -> CpuInfo {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    CpuInfo {
        perf_cores: detect_perf_cores().unwrap_or_else(|| logical.max(1) / 2).max(1),
        logical,
        numa_nodes: detect_numa_nodes(),
    }
}

/// Fiziksel performans cekirdeklerini sayar.
///
/// Iki ayri tuzagi ayni anda cozer:
///   1. **SMT/Hyper-Threading** — `thread_siblings_list` uzerinden tekillestirme.
///   2. **Hibrit mimari** (Intel P/E-core, ARM big.LITTLE) — E-core'lara is
///      vermek senkronizasyon bariyerleri yuzunden TOPLAM hizi dusurur.
///      Ayirt etme sinyali `cpu_capacity`; yoksa maksimum frekans.
#[cfg(target_os = "linux")]
fn detect_perf_cores() -> Option<usize> {
    let base = Path::new("/sys/devices/system/cpu");
    let mut seen_cores: BTreeSet<String> = BTreeSet::new();
    let mut candidates: Vec<(usize, u64)> = Vec::new(); // (cpu_id, capacity)

    for entry in fs::read_dir(base).ok()? {
        let path = entry.ok()?.path();
        let fname = path.file_name()?.to_string_lossy().to_string();
        let Some(id_str) = fname.strip_prefix("cpu") else { continue };
        let Ok(cpu_id) = id_str.parse::<usize>() else { continue };

        // SMT tekillestirme: ayni fiziksel cekirdegin kardesleri tek kez sayilir.
        let siblings = fs::read_to_string(path.join("topology/thread_siblings_list"))
            .unwrap_or_else(|_| id_str.to_string());
        let key = siblings.trim().to_string();
        if !seen_cores.insert(key) {
            continue;
        }

        let capacity = read_u64(&path.join("cpu_capacity"))
            .or_else(|| read_u64(&path.join("cpufreq/cpuinfo_max_freq")))
            .unwrap_or(0);
        candidates.push((cpu_id, capacity));
    }

    if candidates.is_empty() {
        return None;
    }

    // Hibrit tespiti: en yuksek kapasitenin %75'inin altindaki cekirdekler
    // "verimlilik" sinifidir ve inference thread havuzuna alinmaz.
    let max_cap = candidates.iter().map(|(_, c)| *c).max().unwrap_or(0);
    if max_cap == 0 {
        return Some(candidates.len());
    }
    let threshold = max_cap * 3 / 4;
    let perf = candidates.iter().filter(|(_, c)| *c >= threshold).count();
    Some(perf.max(1))
}

#[cfg(not(target_os = "linux"))]
fn detect_perf_cores() -> Option<usize> {
    // TODO(ffi): Windows  -> GetLogicalProcessorInformationEx(RelationProcessorCore)
    //                        + EfficiencyClass alanindan P/E ayrimi
    //            macOS    -> sysctlbyname("hw.perflevel0.physicalcpu")
    None
}

#[cfg(target_os = "linux")]
fn detect_numa_nodes() -> usize {
    fs::read_dir("/sys/devices/system/node")
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .strip_prefix("node")
                        .map(|s| s.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(1)
        .max(1)
}

#[cfg(not(target_os = "linux"))]
fn detect_numa_nodes() -> usize {
    1
}

// ---------------------------------------------------------------------------
// RAM
// ---------------------------------------------------------------------------

/// `MemTotal` degil `MemAvailable`. Cekirdegin "geri alinabilir cache dahil,
/// bu surece gercekten verebilecegim miktar" tahminidir; page cache'i dogru
/// sayan tek alan budur.
#[cfg(target_os = "linux")]
fn detect_available_ram() -> u64 {
    let Ok(text) = fs::read_to_string("/proc/meminfo") else {
        return 2 * GIB;
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            if let Some(kb) = rest.split_whitespace().next().and_then(|v| v.parse::<u64>().ok()) {
                return kb * 1024;
            }
        }
    }
    2 * GIB
}

#[cfg(not(target_os = "linux"))]
fn detect_available_ram() -> u64 {
    // TODO(ffi): Windows -> GlobalMemoryStatusEx().ullAvailPhys
    //            macOS   -> host_statistics64(HOST_VM_INFO64): free + inactive + purgeable
    2 * GIB
}

// ---------------------------------------------------------------------------
// GPU
// ---------------------------------------------------------------------------

/// Sirali dener, ilk basarili olani dondurur. Her prob kendi kutuphanesini
/// `dlopen` ile arar; bulunamazsa sessizce bir sonrakine gecilir.
fn detect_gpus() -> Vec<GpuInfo> {
    for probe in [probe_nvml, probe_dxgi, probe_metal, probe_vulkan] {
        let found = probe();
        if !found.is_empty() {
            return found;
        }
    }
    Vec::new()
}

fn probe_nvml() -> Vec<GpuInfo> {
    // TODO(ffi): dlopen("libnvidia-ml.so.1") | LoadLibrary("nvml.dll")
    //   nvmlInit_v2()
    //   nvmlDeviceGetCount_v2(&n)
    //   for i in 0..n:
    //       nvmlDeviceGetHandleByIndex_v2(i, &h)
    //       nvmlDeviceGetName(h, buf, len)
    //       nvmlDeviceGetMemoryInfo_v2(h, &mem)   // v2: MIG'i dogru raporlar
    //       budget = mem.free
    //   nvmlShutdown()
    //
    // Not: MIG etkinse `free` bolume aittir, fiziksel karta degil.
    // v1 API bunu yanlis raporlar; bu yuzden acikca v2 kullanilir.
    Vec::new()
}

fn probe_dxgi() -> Vec<GpuInfo> {
    // TODO(ffi): Windows'ta saticidan bagimsiz ve EN DOGRU kaynak budur.
    //   CreateDXGIFactory1 -> EnumAdapters1 -> QueryInterface(IDXGIAdapter3)
    //   QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &info)
    //   budget = info.Budget.saturating_sub(info.CurrentUsage)
    //
    // `Budget`, isletim sisteminin bu surece AYIRMAYA RAZI OLDUGU miktardir
    // ve diger uygulamalarin baskisina gore dinamik degisir. "Oyun acikken
    // kurulum yaptim ve cokti" senaryosunu tek basina ortadan kaldirir.
    Vec::new()
}

fn probe_metal() -> Vec<GpuInfo> {
    // TODO(ffi): MTLCreateSystemDefaultDevice()
    //   budget = device.recommendedMaxWorkingSetSize
    //            - device.currentAllocatedSize
    //   unified = device.hasUnifiedMemory
    //
    // `hw.memsize` KULLANILMAZ: birlesik bellekte GPU'ya ayrilabilir pay
    // `iogpu.wired_limit_mb` ile sinirlidir ve toplam RAM'den kucuktur.
    Vec::new()
}

fn probe_vulkan() -> Vec<GpuInfo> {
    // TODO(ffi): AMD/Intel/mobil icin satici-bagimsiz yedek yol.
    //   VK_EXT_memory_budget + VkPhysicalDeviceMemoryBudgetPropertiesEXT
    //   DEVICE_LOCAL heap icin: heapBudget - heapUsage
    Vec::new()
}

// ---------------------------------------------------------------------------

fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

pub fn human(bytes: u64) -> String {
    let gib = bytes as f64 / GIB as f64;
    if gib >= 1.0 {
        format!("{gib:.2} GiB")
    } else {
        format!("{:.0} MiB", bytes as f64 / MIB as f64)
    }
}
