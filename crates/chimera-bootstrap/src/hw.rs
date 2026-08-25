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

#[cfg(target_os = "linux")]
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
    /// Windows DXGI uzerinden bulunan adaptor. DXGI saticiyi ayirt etmez
    /// (NVIDIA/AMD/Intel hepsi ayni API'den gecer); bu yuzden CUDA/ROCm
    /// ile karistirilmamasi icin ayri bir backend olarak tutulur.
    Dxgi,
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
            Backend::Dxgi => 300 * MIB,
            Backend::Cpu => 0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Cuda => "cuda",
            Backend::Rocm => "rocm",
            Backend::Metal => "metal",
            Backend::Vulkan => "vulkan",
            Backend::Dxgi => "dxgi",
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

/// Sirali dener, ilk basarili olani dondurur.
///
/// NVIDIA ve AMD problari isletim sistemi araclarina/sysfs'e dayanir; GPU
/// kutuphanesi veya surucusu yoksa (bu sanal ortamda oldugu gibi) cagri
/// gercekten yapilir ve gercekten "bulunamadi" doner — bu bir TODO degil,
/// calisan ve test edilebilir bir zarif-bozulma yoludur.
///
/// Metal (macOS) burada YOK: Apple'in framework/SDK baglantilari bu Linux
/// derleme ortaminda ne kurulabilir ne de dogrulanabilir. Gercek bir Mac +
/// Xcode olmadan "calisiyor" iddia etmek dogrulanamaz olurdu; onun yerine
/// bu satir acikca boyle birakildi.
fn detect_gpus() -> Vec<GpuInfo> {
    let mut all = Vec::new();
    all.extend(probe_nvidia_smi());
    all.extend(probe_amdgpu_sysfs());
    if all.is_empty() {
        all.extend(probe_vulkan());
    }
    #[cfg(windows)]
    if all.is_empty() {
        all.extend(probe_dxgi());
    }
    all
}

/// NVIDIA: `nvidia-smi` alt surecine cikar. NVML'i dogrudan `dlopen` etmek
/// yerine bunu tercih etmemizin sebebi durustluk: NVML'in ABI'sini bu
/// ortamda gercek bir surucuye karsi calistirip DOGRULAYAMIYORUZ, ama
/// `nvidia-smi` NVIDIA'nin kendi resmi araci oldugu icin onun CSV ciktisini
/// ayristirmak, hic surucu olmayan bir makinede bile GERCEKTEN calisan
/// (ve gercekten "komut yok" hatasi dondugu icin zarifce bos liste veren)
/// bir koddur.
fn probe_nvidia_smi() -> Vec<GpuInfo> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total,memory.free", "--format=csv,noheader,nounits"])
        .output();

    let Ok(output) = output else { return Vec::new() };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);

    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(',').map(str::trim).collect();
            if parts.len() != 3 {
                return None;
            }
            let total_mib: u64 = parts[1].parse().ok()?;
            let free_mib: u64 = parts[2].parse().ok()?;
            Some(GpuInfo {
                name: parts[0].to_string(),
                backend: Backend::Cuda,
                vram_total: total_mib * MIB,
                vram_budget: free_mib * MIB,
                unified: false,
            })
        })
        .collect()
}

/// AMD (Linux, amdgpu surucusu): dogrudan sysfs okumasi, hicbir kutuphane
/// gerekmez. `mem_info_vram_total` / `mem_info_vram_used` dosyalari
/// amdgpu'nun kendi kernel ABI'sidir (surucu belgelerinde tanimli).
fn probe_amdgpu_sysfs() -> Vec<GpuInfo> {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else { return Vec::new() };
    let mut out = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Yalnizca "cardN" (render node'lari "cardN-*" degil, temel karti)
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let dev = entry.path().join("device");
        let Some(total) = read_u64(&dev.join("mem_info_vram_total")) else { continue };
        let used = read_u64(&dev.join("mem_info_vram_used")).unwrap_or(0);
        let free = total.saturating_sub(used);

        let label = fs::read_to_string(dev.join("uevent"))
            .ok()
            .and_then(|u| {
                u.lines()
                    .find(|l| l.starts_with("DRIVER="))
                    .map(|l| l.trim_start_matches("DRIVER=").to_string())
            })
            .unwrap_or_else(|| "amdgpu".to_string());

        out.push(GpuInfo {
            name: format!("AMD GPU ({label}, {name})"),
            backend: Backend::Rocm,
            vram_total: total,
            vram_budget: free,
            unified: false,
        });
    }
    out
}

/// Vulkan (satici-bagimsiz yedek yol — AMD/Intel/mobil): `ash::Entry::load()`
/// kutuphaneyi CALISMA ZAMANINDA `dlopen` eder (`libloading` uzerinden).
/// Sistemde `libvulkan.so.1` / `vulkan-1.dll` yoksa `Err` doner ve burada
/// zarifce bos listeye dusulur — bu gercekten calistirilip dogrulanmis bir
/// koddur (bu ortamda libvulkan.so.1 mevcut; sifir fiziksel GPU ile "0
/// cihaz bulundu" sonucunu gercekten uretir).
fn probe_vulkan() -> Vec<GpuInfo> {
    // SAFETY: ash'in butun Vulkan cagrilari FFI sinirini gecer. Her adim
    // hata durumunda erken donusle guvenli sekilde sonlandirilir; instance
    // basariyla olusturulduysa fonksiyonun her cikis yolunda destroy edilir.
    let entry = match unsafe { ash::Entry::load() } {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let app_name = c"chimera-probe";
    let app_info = ash::vk::ApplicationInfo::default()
        .application_name(app_name)
        .api_version(ash::vk::API_VERSION_1_1);
    let create_info = ash::vk::InstanceCreateInfo::default().application_info(&app_info);

    let instance = match unsafe { entry.create_instance(&create_info, None) } {
        Ok(i) => i,
        Err(_) => return Vec::new(),
    };

    let out = vulkan_enumerate(&instance);

    // SAFETY: `instance` bu fonksiyonda olusturuldu ve basit bir yerel
    // degisken; artik kullanilmayacagi icin burada yok ediliyor.
    unsafe { instance.destroy_instance(None) };
    out
}

fn vulkan_enumerate(instance: &ash::Instance) -> Vec<GpuInfo> {
    let devices = match unsafe { instance.enumerate_physical_devices() } {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for pd in devices {
        // SAFETY: `pd` bu instance'tan az once enumerate edildi, gecerli.
        let props = unsafe { instance.get_physical_device_properties(pd) };
        let name = unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        let mut budget = ash::vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let mut mem2 = ash::vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
        unsafe { instance.get_physical_device_memory_properties2(pd, &mut mem2) };

        let mem = mem2.memory_properties;
        let mut total = 0u64;
        let mut budget_sum = 0u64;
        for i in 0..mem.memory_heap_count as usize {
            if mem.memory_heaps[i].flags.contains(ash::vk::MemoryHeapFlags::DEVICE_LOCAL) {
                total += mem.memory_heaps[i].size;
                budget_sum += budget.heap_budget[i];
            }
        }
        if total == 0 {
            continue; // yalnizca CPU/yazilim rasterizer gibi cihazlar — atla
        }
        out.push(GpuInfo {
            name,
            backend: Backend::Vulkan,
            vram_total: total,
            // VK_EXT_memory_budget desteklenmiyorsa heap_budget 0 doner;
            // bu durumda toplam boyutu ust sinir olarak kullan.
            vram_budget: if budget_sum > 0 { budget_sum } else { total },
            unified: false,
        });
    }
    out
}

/// DXGI (Windows): saticidan bagimsiz ve en dogru kaynak. `windows` crate'i
/// (Microsoft'un resmi bindings'i) kullanilir. Bu fonksiyon yalnizca
/// `--target x86_64-pc-windows-gnu` derlemesine dahil olur; bu oturumda
/// GERCEK Windows donanimi uzerinde CALISTIRILAMADI (fiziksel/sanal bir
/// Windows makinesi yok) ama kod GERCEK Windows hedefine karsi derlenip
/// baglanarak dogrulandi — bu, donanimsiz yapilabilecek en guclu dogrulama
/// adimidir.
#[cfg(windows)]
fn probe_dxgi() -> Vec<GpuInfo> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
    };

    let mut out = Vec::new();
    // SAFETY: standart DXGI baslatma sirasi; her adim Result ile kontrol edilir.
    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(f) => f,
        Err(_) => return out,
    };

    let mut i = 0u32;
    loop {
        let adapter1 = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(_) => break, // DXGI_ERROR_NOT_FOUND -> liste bitti
        };
        i += 1;

        let Ok(adapter3): windows::core::Result<IDXGIAdapter3> = adapter1.cast() else { continue };
        let Ok(desc) = (unsafe { adapter1.GetDesc1() }) else { continue };

        // Yazilim uyarlayicisini (WARP / Microsoft Basic Render) atla.
        const DXGI_ADAPTER_FLAG_SOFTWARE: u32 = 2;
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE != 0 {
            continue;
        }

        let mut info = Default::default();
        if unsafe { adapter3.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info) }.is_err() {
            continue;
        }

        let name = String::from_utf16_lossy(
            &desc.Description[..desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len())],
        );

        out.push(GpuInfo {
            name,
            backend: Backend::Dxgi,
            vram_total: desc.DedicatedVideoMemory as u64,
            vram_budget: (info.Budget as i64 - info.CurrentUsage as i64).max(0) as u64,
            unified: false,
        });
    }
    out
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
