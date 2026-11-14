//! GPU plumbing shared outside the renderer.
//!
//! GPU tests are named `gpu_*`, are `#[ignore]`d so `just test` never needs a
//! GPU, and run with `just test-gpu`. They get their adapter here.

pub mod image;
pub mod spatial;
pub mod atmosphere;
pub mod transfer;

/// The adapter every GPU test uses.
///
/// The backend comes from `WGPU_BACKEND` (`vulkan`, `dx12`, `metal`, `gl`) and
/// defaults to Vulkan, which the existing contracts were written against.
/// `VOXEL_TEST_ADAPTER_NAME` requires an adapter whose name contains it.
/// Panics when no adapter exists: `just test-gpu` fails, it never skips.
pub fn test_adapter() -> wgpu::Adapter {
    let backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..wgpu::InstanceDescriptor::new_without_display_handle() });
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    };
    let adapter = pollster::block_on(instance.request_adapter(&options))
        .unwrap_or_else(|e| panic!("no GPU adapter for {backends:?} ({e}); GPU tests need one"));
    let info = adapter.get_info();
    println!("[gpu-test] adapter={info:?}");
    if let Ok(name) = std::env::var("VOXEL_TEST_ADAPTER_NAME") {
        assert!(info.name.contains(&name), "required adapter {name:?}, got {:?}", info.name);
    }
    adapter
}
