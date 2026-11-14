//! One authored atlas for the held workshop kit, sampled without interpolation.
use wgpu::util::DeviceExt;

pub(super) fn atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    // The embedded atlases decode once, when the renderer creates their texture.
    let wand=if crate::viewmodel_review::current().name == crate::viewmodel_review::DEFAULT.name {
        crate::viewmodel_review::DEFAULT_ATLAS.as_ref()
    } else {crate::viewmodel_review::ATLAS.as_ref()};
    let hand=crate::viewmodel_review::DEFAULT_HAND_ATLAS.as_ref();
    let mut combined=Vec::with_capacity(512*256*4);
    for y in 0..256 {
        combined.extend_from_slice(&hand[y*1024..(y+1)*1024]);
        combined.extend_from_slice(&wand[y*1024..(y+1)*1024]);
    }
    device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("workshop pixel atlas"),
            size: wgpu::Extent3d { width: 512, height: 256, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &combined,
    ).create_view(&wgpu::TextureViewDescriptor::default())
}
