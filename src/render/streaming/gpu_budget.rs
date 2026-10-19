//! Adapter-specific terrain allowance. A budget is a ceiling, not an allocation.
//! Native queries run outside the frame loop; the streamer reads atomics only.
use std::sync::{
    Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

const MIB: usize = 1024 * 1024;
pub const MAX_GPU_MIB: usize = if usize::MAX / MIB < 65536 {
    usize::MAX / MIB
} else {
    65536
};
// Zero is the persisted automatic policy, never a zero-byte residency budget.
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static EFFECTIVE: AtomicUsize = AtomicUsize::new(1024);
static FALLBACK: AtomicUsize = AtomicUsize::new(1024);
static CEILING: AtomicUsize = AtomicUsize::new(usize::MAX);
static RESIDENT: AtomicUsize = AtomicUsize::new(0);
static DEFERRED: AtomicUsize = AtomicUsize::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static MEMORY: Mutex<Option<Memory>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct Memory {
    dedicated: usize,
    budget: usize,
    usage: usize,
    live: bool,
}

#[derive(Clone, Copy)]
pub struct GpuResidencyStatus {
    pub automatic: bool,
    pub limit_mib: usize,
    pub ceiling_mib: usize,
    pub dedicated_mib: usize,
    pub os_budget_mib: usize,
    pub process_usage_mib: usize,
    pub live: bool,
    pub detected: bool,
    pub deferred: usize,
}

pub fn gpu_residency_limit_mib() -> usize {
    REQUESTED.load(Ordering::Relaxed)
}
pub(super) fn effective_mib() -> usize {
    EFFECTIVE.load(Ordering::Relaxed)
}
pub(super) fn observe_residency(bytes: usize, deferred: usize) {
    RESIDENT.store(bytes, Ordering::Relaxed);
    DEFERRED.store(deferred, Ordering::Relaxed);
}
pub fn gpu_residency_status() -> GpuResidencyStatus {
    let memory = *MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    let cap = CEILING.load(Ordering::Relaxed);
    GpuResidencyStatus {
        automatic: gpu_residency_limit_mib() == 0,
        limit_mib: effective_mib(),
        ceiling_mib: if cap == usize::MAX {
            FALLBACK.load(Ordering::Relaxed)
        } else {
            cap
        },
        dedicated_mib: memory.map_or(0, |m| m.dedicated / MIB),
        os_budget_mib: memory.map_or(0, |m| m.budget / MIB),
        process_usage_mib: memory.map_or(0, |m| m.usage / MIB),
        live: memory.is_some_and(|m| m.live),
        detected: memory.is_some(),
        deferred: DEFERRED.load(Ordering::Relaxed),
    }
}

fn target() -> usize {
    let requested = gpu_residency_limit_mib();
    let cap = CEILING.load(Ordering::Relaxed);
    if requested == 0 {
        if cap == usize::MAX {
            FALLBACK.load(Ordering::Relaxed)
        } else {
            cap
        }
    } else {
        requested.min(cap)
    }
}

pub(super) fn set_limit_mib(requested: usize) {
    REQUESTED.store(
        if requested == 0 {
            0
        } else {
            requested.clamp(64, MAX_GPU_MIB)
        },
        Ordering::Relaxed,
    );
    EFFECTIVE.store(target(), Ordering::Relaxed);
}

fn publish(memory: Memory) {
    // CurrentUsage is this process's OS-accounted local memory, including Vulkan
    // heaps, render targets and staging. Add back our terrain only once, avoiding
    // both counting free VRAM as total VRAM and charging terrain against itself.
    let budget = if memory.dedicated > 0 {
        memory.budget.min(memory.dedicated)
    } else {
        memory.budget
    };
    let reserve = (budget / 5).max(512 * MIB);
    let non_terrain = memory
        .usage
        .saturating_sub(RESIDENT.load(Ordering::Relaxed));
    let cap = (budget.saturating_sub(reserve).saturating_sub(non_terrain) / MIB / 64 * 64)
        .clamp(64, MAX_GPU_MIB);
    let first = MEMORY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace(memory)
        .is_none();
    CEILING.store(cap, Ordering::Relaxed);
    let desired = target();
    let old = effective_mib();
    // Shrink promptly under pressure; don't repeatedly wake all deferred work
    // for small upward changes in driver accounting.
    if first || desired < old || desired >= old.saturating_add(256) {
        EFFECTIVE.store(desired, Ordering::Relaxed);
        if first || old.abs_diff(desired) >= 256 {
            println!(
                "[gpu-budget] mode={} dedicated_mib={} os_budget_mib={} process_mib={} terrain_mib={} limit_mib={} live={}",
                if gpu_residency_limit_mib() == 0 {
                    "auto"
                } else {
                    "manual"
                },
                memory.dedicated / MIB,
                memory.budget / MIB,
                memory.usage / MIB,
                RESIDENT.load(Ordering::Relaxed) / MIB,
                desired,
                memory.live
            );
        }
    }
}

pub(crate) fn configure_gpu_residency(adapter: &wgpu::Adapter) {
    let info = adapter.get_info();
    FALLBACK.store(
        match info.device_type {
            wgpu::DeviceType::DiscreteGpu => 1024,
            wgpu::DeviceType::Cpu => 128,
            _ => 256,
        },
        Ordering::Relaxed,
    );
    *MEMORY.lock().unwrap_or_else(|e| e.into_inner()) = None;
    CEILING.store(usize::MAX, Ordering::Relaxed);
    RESIDENT.store(0, Ordering::Relaxed);
    if let Some(memory) = query(adapter, &info) {
        publish(memory);
    } else {
        EFFECTIVE.store(target(), Ordering::Relaxed);
        println!(
            "[gpu-budget] VRAM query unavailable; mode={} limit_mib={}",
            if gpu_residency_limit_mib() == 0 {
                "auto-fallback"
            } else {
                "manual"
            },
            effective_mib()
        );
    }
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let adapter = adapter.clone();
    let _ = std::thread::Builder::new()
        .name("gpu-memory-budget".into())
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                if GENERATION.load(Ordering::Relaxed) != generation {
                    break;
                }
                if let Some(memory) = query(&adapter, &info) {
                    publish(memory);
                }
            }
        });
}

fn query(adapter: &wgpu::Adapter, info: &wgpu::AdapterInfo) -> Option<Memory> {
    #[cfg(all(not(target_arch = "wasm32"), not(target_vendor = "apple")))]
    if info.backend == wgpu::Backend::Vulkan {
        if let Some(memory) = query_vulkan(adapter) {
            return Some(memory);
        }
    }
    query_dxgi(info)
}

#[cfg(all(not(target_arch = "wasm32"), not(target_vendor = "apple")))]
fn query_vulkan(adapter: &wgpu::Adapter) -> Option<Memory> {
    use ash::{ext, khr, vk};
    // SAFETY: the wgpu guard keeps this physical device and its parent instance
    // alive. Queries are read-only; no raw handles/resources are destroyed or
    // submitted. A worker-owned Adapter clone pins them across budget samples.
    unsafe {
        let adapter = adapter.as_hal::<wgpu::hal::api::Vulkan>()?;
        let shared = adapter.shared_instance();
        let raw = shared.raw_instance();
        let device = adapter.raw_physical_device();
        let extended = adapter
            .physical_device_capabilities()
            .supports_extension(ext::memory_budget::NAME);
        let mut budget = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let mut properties = vk::PhysicalDeviceMemoryProperties2::default();
        let can_query2 = shared.instance_api_version() >= vk::API_VERSION_1_1
            || shared
                .extensions()
                .contains(&khr::get_physical_device_properties2::NAME);
        let live = extended && can_query2;
        if live {
            properties = properties.push_next(&mut budget);
            if shared.instance_api_version() >= vk::API_VERSION_1_1 {
                raw.get_physical_device_memory_properties2(device, &mut properties);
            } else {
                khr::get_physical_device_properties2::Instance::new(shared.entry(), raw)
                    .get_physical_device_memory_properties2(device, &mut properties);
            }
        } else {
            properties.memory_properties = raw.get_physical_device_memory_properties(device);
        }
        let heaps = properties.memory_properties;
        let mut memory = Memory {
            dedicated: 0,
            budget: 0,
            usage: 0,
            live,
        };
        for i in 0..heaps.memory_heap_count as usize {
            let heap = heaps.memory_heaps[i];
            if !heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
                continue;
            }
            let size = usize::try_from(heap.size).unwrap_or(usize::MAX);
            memory.dedicated = memory.dedicated.saturating_add(size);
            memory.budget = memory.budget.saturating_add(if live {
                usize::try_from(budget.heap_budget[i])
                    .unwrap_or(usize::MAX)
                    .min(size)
            } else {
                size / 2
            });
            if live {
                memory.usage = memory
                    .usage
                    .saturating_add(usize::try_from(budget.heap_usage[i]).unwrap_or(usize::MAX));
            }
        }
        (memory.budget > 0).then_some(memory)
    }
}

#[cfg(not(windows))]
fn query_dxgi(_: &wgpu::AdapterInfo) -> Option<Memory> {
    None
}

#[cfg(windows)]
fn query_dxgi(info: &wgpu::AdapterInfo) -> Option<Memory> {
    use windows::{
        Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
            IDXGIAdapter3, IDXGIFactory1,
        },
        core::Interface,
    };
    if !matches!(
        info.device_type,
        wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::IntegratedGpu
    ) {
        return None;
    }
    // SAFETY: windows-rs owns/releases all COM interfaces. Initialized output
    // structs remain alive for each call. Match the selected wgpu adapter; don't
    // accidentally budget against a second card or the system's integrated GPU.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut matched = None;
        let mut matches = 0;
        for index in 0..32 {
            let Ok(adapter) = factory.EnumAdapters1(index) else {
                break;
            };
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            if desc.VendorId != info.vendor || desc.DeviceId != info.device {
                continue;
            }
            // Identical multi-GPU adapters need a backend LUID before selecting
            // a budget safely. Fall back conservatively rather than guessing.
            matches += 1;
            if matches > 1 {
                return None;
            }
            let local = adapter.cast::<IDXGIAdapter3>().ok().and_then(|adapter| {
                let mut memory = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                adapter
                    .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut memory)
                    .ok()?;
                (memory.Budget > 0).then_some(memory)
            });
            let dedicated = desc.DedicatedVideoMemory;
            matched = if let Some(local) = local {
                Some(Memory {
                    dedicated,
                    budget: usize::try_from(local.Budget).unwrap_or(usize::MAX),
                    usage: usize::try_from(local.CurrentUsage).unwrap_or(usize::MAX),
                    live: true,
                })
            } else if dedicated > 0 {
                // Old drivers: leave half the physical memory for unreported use.
                Some(Memory {
                    dedicated,
                    budget: dedicated / 2,
                    usage: 0,
                    live: false,
                })
            } else {
                None
            };
        }
        matched
    }
}
