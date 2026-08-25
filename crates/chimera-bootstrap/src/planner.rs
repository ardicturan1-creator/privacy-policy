//! Kuantizasyon planlayicisi — "Out of Memory" hatasinin matematiksel olarak
//! olanaksiz hale getirildigi yer.
//!
//! Akis:
//!   1) Kapali-form bellek modeli ile ADAY plan uret
//!   2) Sozlukbilimsel tercih sirasiyla en iyi adayi sec
//!   3) KANARYA TAHSISI ile plani fiziksel olarak DOGRULA
//!   4) Basarisizsa bir kademe in ve tekrar dogrula (en fazla 4 kez)
//!
//! 3. adim bu modulun varlik sebebidir. Hesap her zaman yanilabilir:
//! surucu surumu, ECC, MIG bolumlemesi, o anda buyuyen baska bir surec.
//! Sistem "hesabima gore sigar" demez; "denedim, sigdi" der.

use crate::hw::{human, Backend, HostProfile, MIB};

/// Guvenlik faktoru: fragmentasyon ve surucu dalgalanmasi payi.
/// 0.88 deneysel bir degerdir; 0.95 fragmentasyona, 0.80 gereksiz kalite
/// kaybina yol acar.
const SAFETY: f64 = 0.88;

const CANARY_MAX_STEPDOWNS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quant {
    IQ2M,
    IQ3M,
    IQ4XS,
    Q4KM,
    Q5KM,
    Q6K,
    Q8_0,
}

impl Quant {
    /// Kalite sirasi: en iyiden en kotuye. Arama bu sirayla ilerler.
    pub const PREFERENCE: [Quant; 7] = [
        Quant::Q8_0,
        Quant::Q6K,
        Quant::Q5KM,
        Quant::Q4KM,
        Quant::IQ4XS,
        Quant::IQ3M,
        Quant::IQ2M,
    ];

    /// Agirlik basina ortalama bit. GGUF k-quant'lari blok basina olcek ve
    /// min degerleri tasidigi icin degerler tam sayi degildir.
    pub fn bits_per_weight(self) -> f64 {
        match self {
            Quant::Q8_0 => 8.50,
            Quant::Q6K => 6.56,
            Quant::Q5KM => 5.67,
            Quant::Q4KM => 4.85,
            Quant::IQ4XS => 4.25,
            Quant::IQ3M => 3.66,
            Quant::IQ2M => 2.70,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Quant::Q8_0 => "Q8_0",
            Quant::Q6K => "Q6_K",
            Quant::Q5KM => "Q5_K_M",
            Quant::Q4KM => "Q4_K_M",
            Quant::IQ4XS => "IQ4_XS",
            Quant::IQ3M => "IQ3_M",
            Quant::IQ2M => "IQ2_M",
        }
    }
}

/// GGUF metadata'sindan okunan model geometrisi. Hicbiri varsayilmaz —
/// ozellikle `n_kv_head`: GQA/MQA modellerde KV cache maliyetini
/// birkac kat degistirir ve en sik yapilan hesap hatasi buradan cikar.
#[derive(Debug, Clone)]
pub struct ModelGeometry {
    pub name: String,
    pub params: u64,
    pub n_layer: u32,
    pub n_kv_head: u32,
    pub head_dim: u32,
    /// KV cache eleman boyutu (f16 = 2, q8 = 1). Kuantize KV cache uzun
    /// baglamda kuantizasyon dusurmekten daha ucuz bir tavizdir.
    pub kv_bytes: u32,
}

impl ModelGeometry {
    /// Belirli bir kuantizasyonda toplam agirlik boyutu.
    pub fn weight_bytes(&self, q: Quant) -> u64 {
        (self.params as f64 * q.bits_per_weight() / 8.0) as u64
    }

    pub fn bytes_per_layer(&self, q: Quant) -> u64 {
        (self.weight_bytes(q) / self.n_layer.max(1) as u64).max(1)
    }

    /// KV(ctx) = 2 · n_layer · n_kv_head · head_dim · ctx · kv_bytes
    /// `2` carpani K ve V icindir.
    pub fn kv_cache_bytes(&self, ctx: u32) -> u64 {
        2u64 * self.n_layer as u64
            * self.n_kv_head as u64
            * self.head_dim as u64
            * ctx as u64
            * self.kv_bytes as u64
    }

    /// Compute buffer: aktivasyonlar, logit tamponu, gecici tensorler.
    /// Baglam ile dogrusala yakin buyur.
    pub fn compute_buffer_bytes(&self, ctx: u32, batch: u32) -> u64 {
        let per_token = self.n_layer as u64 * self.head_dim as u64 * self.n_kv_head as u64 * 6;
        (per_token * batch as u64).max(192 * MIB) + (ctx as u64 * 2048)
    }
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub model: String,
    pub backend: Backend,
    pub quant: Quant,
    pub ctx: u32,
    pub n_gpu_layers: u32,
    pub n_layer_total: u32,
    pub n_threads: usize,
    pub vram_required: u64,
    pub vram_budget: u64,
    pub canary_attempts: usize,
    pub notes: Vec<String>,
}

impl Plan {
    pub fn fully_offloaded(&self) -> bool {
        self.n_gpu_layers >= self.n_layer_total
    }
}

/// Denenecek baglam pencereleri, genisten dara.
/// Baglami dusurmek kuantizasyonu dusurmekten ONCE gelir: 8K baglamli Q5
/// model, 64K baglamli Q2 modelden her olcutte daha faydalidir.
const CTX_LADDER: [u32; 4] = [65_536, 32_768, 16_384, 8_192];

/// Kanarya tahsisi arayuzu. Uretimde `CudaCanary`/`MetalCanary` implemente
/// eder; testte deterministik bir sahte kullanilir. Planlayicinin surucuye
/// dogrudan bagli olmamasi, bu mantigin CI'da test edilebilmesini saglar.
pub trait Canary {
    /// `bytes` kadar VRAM'i TEK parcada tahsis etmeyi dene, sonra birak.
    /// `largest_tensor` ile ikinci bir tahsis daha denenir: bu, "toplam yer
    /// var ama bitisik yer yok" fragmentasyon senaryosunu yakalar.
    fn try_reserve(&self, bytes: u64, largest_tensor: u64) -> bool;
}

/// GPU yokken kullanilan no-op kanarya: CPU planinda dogrulanacak bir
/// VRAM tahsisi yoktur.
pub struct NullCanary;
impl Canary for NullCanary {
    fn try_reserve(&self, _bytes: u64, _largest: u64) -> bool {
        true
    }
}

pub fn plan(host: &HostProfile, model: &ModelGeometry, canary: &dyn Canary) -> Plan {
    let backend = host.backend();
    let n_threads = thread_budget(host, backend);

    let Some(gpu) = host.primary_gpu() else {
        return cpu_plan(host, model, n_threads, "hizlandirici bulunamadi");
    };

    // Birlesik bellekte (Apple Silicon) VRAM ve RAM ayni fiziksel havuzdur;
    // ikisini toplamak cift sayma olur. Baglayici kisit GPU butcesidir.
    let budget = if gpu.unified {
        gpu.vram_budget.min(host.ram_available)
    } else {
        gpu.vram_budget
    };

    let usable = (budget as f64 * SAFETY) as u64;
    let overhead = backend.fixed_overhead();
    if usable <= overhead {
        return cpu_plan(host, model, n_threads, "GPU butcesi sabit ek yuku karsilamiyor");
    }

    // --- Asama 3: kisitli optimizasyon -----------------------------------
    let mut candidates: Vec<(Quant, u32, u32, u64)> = Vec::new(); // (q, ctx, n_off, bytes)

    // Oncelik 1: TAM offload. Tam GPU offload, kismi offload'a gore tipik
    // olarak bir buyukluk mertebesi daha hizlidir; bu yuzden
    // "Q4_K_M @ %100 GPU" > "Q6_K @ %60 GPU".
    'outer: for &q in Quant::PREFERENCE.iter() {
        for &ctx in CTX_LADDER.iter() {
            let need = required_vram(model, q, ctx, model.n_layer, overhead);
            if need <= usable {
                candidates.push((q, ctx, model.n_layer, need));
                break 'outer;
            }
        }
    }

    // Oncelik 2: tam sigmiyorsa, en iyi kuantizasyondan baslayarak maksimum
    // offload edilebilir katman sayisini coz.
    if candidates.is_empty() {
        'partial: for &q in Quant::PREFERENCE.iter() {
            for &ctx in CTX_LADDER.iter() {
                let fixed = model.kv_cache_bytes(ctx)
                    + model.compute_buffer_bytes(ctx, 512)
                    + overhead;
                if fixed >= usable {
                    continue;
                }
                let per_layer = model.bytes_per_layer(q);
                let n_off = ((usable - fixed) / per_layer) as u32;
                if n_off > 0 {
                    let n_off = n_off.min(model.n_layer);
                    let need = required_vram(model, q, ctx, n_off, overhead);
                    candidates.push((q, ctx, n_off, need));
                    break 'partial;
                }
            }
        }
    }

    if candidates.is_empty() {
        return cpu_plan(host, model, n_threads, "hicbir katman GPU'ya sigmiyor");
    }

    // --- Asama 4: kanarya tahsisi ile fiziksel dogrulama ------------------
    let (mut q, mut ctx, mut n_off, mut need) = candidates[0];
    let mut attempts = 0usize;
    let mut notes = Vec::new();

    loop {
        attempts += 1;
        let largest = model.bytes_per_layer(q).max(64 * MIB);
        if canary.try_reserve(need, largest) {
            break;
        }
        if attempts > CANARY_MAX_STEPDOWNS {
            notes.push("kanarya dogrulamasi tukendi, CPU planina dusuldu".into());
            return cpu_plan(host, model, n_threads, "kanarya tahsisi basarisiz");
        }
        notes.push(format!(
            "kanarya reddi #{attempts}: {} @ {} ({}), bir kademe iniliyor",
            q.as_str(),
            ctx,
            human(need)
        ));
        // Kademe inisi: once baglami dar, sonra kuantizasyonu dusur.
        if let Some(next_ctx) = CTX_LADDER.iter().copied().find(|&c| c < ctx) {
            ctx = next_ctx;
        } else if let Some(pos) = Quant::PREFERENCE.iter().position(|&p| p == q) {
            if pos + 1 < Quant::PREFERENCE.len() {
                q = Quant::PREFERENCE[pos + 1];
                ctx = CTX_LADDER[0];
            } else {
                return cpu_plan(host, model, n_threads, "en dusuk kademe de sigmadi");
            }
        }
        n_off = n_off.min(model.n_layer);
        need = required_vram(model, q, ctx, n_off, overhead);
    }

    if n_off < model.n_layer {
        notes.push(format!(
            "kismi offload: {}/{} katman GPU'da, kalani CPU'da",
            n_off, model.n_layer
        ));
    }

    Plan {
        model: model.name.clone(),
        backend,
        quant: q,
        ctx,
        n_gpu_layers: n_off,
        n_layer_total: model.n_layer,
        // Tam offload varsa CPU thread'leri yalnizca sampling ve tokenization
        // yapar; fazla thread saf ek yuktur.
        n_threads: if n_off >= model.n_layer { n_threads.min(4) } else { n_threads },
        vram_required: need,
        vram_budget: budget,
        canary_attempts: attempts,
        notes,
    }
}

fn required_vram(m: &ModelGeometry, q: Quant, ctx: u32, n_off: u32, overhead: u64) -> u64 {
    let weights = m.bytes_per_layer(q) * n_off as u64;
    weights + m.kv_cache_bytes(ctx) + m.compute_buffer_bytes(ctx, 512) + overhead
}

fn cpu_plan(host: &HostProfile, m: &ModelGeometry, threads: usize, why: &str) -> Plan {
    // CPU'da baglami RAM'e sigacak sekilde kis.
    let usable = (host.ram_available as f64 * SAFETY) as u64;
    let mut chosen = (Quant::IQ4XS, CTX_LADDER[CTX_LADDER.len() - 1]);
    'search: for &q in Quant::PREFERENCE.iter() {
        for &ctx in CTX_LADDER.iter() {
            if m.weight_bytes(q) + m.kv_cache_bytes(ctx) <= usable {
                chosen = (q, ctx);
                break 'search;
            }
        }
    }
    Plan {
        model: m.name.clone(),
        backend: Backend::Cpu,
        quant: chosen.0,
        ctx: chosen.1,
        n_gpu_layers: 0,
        n_layer_total: m.n_layer,
        n_threads: threads,
        vram_required: 0,
        vram_budget: 0,
        canary_attempts: 0,
        notes: vec![format!("CPU planina dusuldu: {why}")],
    }
}

fn thread_budget(host: &HostProfile, backend: Backend) -> usize {
    // Bir cekirdek watchdog ve eBPF kullanici-alani tuketicisi icin ayrilir.
    let base = host.cpu.perf_cores.saturating_sub(1).max(1);
    match backend {
        Backend::Cpu => base,
        _ => base.min(8),
    }
}

/// `runtime/plan.json`. Harici serde bagimliligi olmadan, elle ve kacisla.
pub fn plan_to_json(p: &Plan) -> String {
    let notes = p
        .notes
        .iter()
        .map(|n| format!("\"{}\"", escape(n)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "{{\n",
            "  \"model\": \"{}\",\n",
            "  \"backend\": \"{}\",\n",
            "  \"quant\": \"{}\",\n",
            "  \"ctx\": {},\n",
            "  \"n_gpu_layers\": {},\n",
            "  \"n_layer_total\": {},\n",
            "  \"n_threads\": {},\n",
            "  \"vram_required_bytes\": {},\n",
            "  \"vram_budget_bytes\": {},\n",
            "  \"canary_attempts\": {},\n",
            "  \"notes\": [{}]\n",
            "}}\n"
        ),
        escape(&p.model),
        p.backend.as_str(),
        p.quant.as_str(),
        p.ctx,
        p.n_gpu_layers,
        p.n_layer_total,
        p.n_threads,
        p.vram_required,
        p.vram_budget,
        p.canary_attempts,
        notes
    )
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Kurulum ekraninin durust ozeti. "Optimize ediliyor..." demez;
/// ne alindigini, neyin alinamadigini ve nedenini soyler.
pub fn render_summary(host: &HostProfile, p: &Plan) -> String {
    let mut out = String::new();
    let gpu = host.primary_gpu();
    out.push_str(&format!(
        "  GPU        : {}\n",
        gpu.map(|g| g.name.as_str()).unwrap_or("yok (CPU modu)")
    ));
    if let Some(g) = gpu {
        out.push_str(&format!(
            "  Butce      : {} kullanilabilir ({} toplam)\n",
            human(g.vram_budget),
            human(g.vram_total)
        ));
    }
    out.push_str(&format!(
        "  Plan       : {} @ {}, {}/{} katman GPU'da, ctx {}K, {} thread\n",
        p.model,
        p.quant.as_str(),
        p.n_gpu_layers,
        p.n_layer_total,
        p.ctx / 1024,
        p.n_threads
    ));
    if p.vram_required > 0 {
        out.push_str(&format!(
            "  Kanarya    : {} bitisik tahsis {} ({}. denemede)\n",
            human(p.vram_required),
            if p.canary_attempts > 0 { "OK" } else { "-" },
            p.canary_attempts
        ));
    }
    for n in &p.notes {
        out.push_str(&format!("  Not        : {n}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hw::{CpuInfo, GpuInfo, GIB};

    fn model_14b() -> ModelGeometry {
        ModelGeometry {
            name: "artificer-14b".into(),
            params: 14_000_000_000,
            n_layer: 48,
            n_kv_head: 8, // GQA
            head_dim: 128,
            kv_bytes: 2,
        }
    }

    fn host_with(vram: u64) -> HostProfile {
        HostProfile {
            cpu: CpuInfo { perf_cores: 8, logical: 16, numa_nodes: 1 },
            ram_available: 32 * GIB,
            gpus: vec![GpuInfo {
                name: "test-gpu".into(),
                backend: Backend::Cuda,
                vram_total: vram,
                vram_budget: vram,
                unified: false,
            }],
        }
    }

    struct AlwaysOk;
    impl Canary for AlwaysOk {
        fn try_reserve(&self, _b: u64, _l: u64) -> bool { true }
    }

    /// Kanaryayi ilk N denemede reddeder — surucunun hesaptan daha kotumser
    /// oldugu gercek dunya durumunu taklit eder.
    struct RejectFirst(std::cell::Cell<usize>);
    impl Canary for RejectFirst {
        fn try_reserve(&self, _b: u64, _l: u64) -> bool {
            let n = self.0.get();
            self.0.set(n + 1);
            n >= 2
        }
    }

    #[test]
    fn plan_never_exceeds_safety_budget() {
        for vram in [6, 8, 12, 16, 24, 48] {
            let host = host_with(vram * GIB);
            let p = plan(&host, &model_14b(), &AlwaysOk);
            if p.backend != Backend::Cpu {
                let cap = (p.vram_budget as f64 * SAFETY) as u64;
                assert!(
                    p.vram_required <= cap,
                    "{vram} GiB: {} > {}",
                    p.vram_required,
                    cap
                );
            }
        }
    }

    #[test]
    fn large_vram_prefers_full_offload() {
        let p = plan(&host_with(48 * GIB), &model_14b(), &AlwaysOk);
        assert!(p.fully_offloaded());
    }

    #[test]
    fn canary_rejection_steps_down_and_still_succeeds() {
        let host = host_with(16 * GIB);
        let p = plan(&host, &model_14b(), &RejectFirst(std::cell::Cell::new(0)));
        assert!(p.canary_attempts >= 3, "kademe inisi calismali");
        assert!(!p.notes.is_empty());
    }

    #[test]
    fn tiny_vram_falls_back_to_cpu_not_a_crash() {
        let host = host_with(512 * MIB);
        let p = plan(&host, &model_14b(), &AlwaysOk);
        assert_eq!(p.backend, Backend::Cpu);
        assert_eq!(p.n_gpu_layers, 0);
    }

    #[test]
    fn gqa_kv_cache_is_not_overcounted() {
        let m = model_14b();
        // 8 KV head, 48 katman, 128 head_dim, f16, 32K baglam
        // 2 * 48 * 8 * 128 * 32768 * 2 = 6 GiB
        assert_eq!(m.kv_cache_bytes(32_768), 6 * GIB);
    }
}
