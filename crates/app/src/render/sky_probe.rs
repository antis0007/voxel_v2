//! Per-frame sky and sun terms for terrain shading (`SkyProbe` in
//! `atmosphere_runtime_shared.wgsl`).
//!
//! The terrain fragment shader used to evaluate seven sky colours and sun
//! transmittances whose inputs are the same for every pixel: five
//! `atmosphere_color` calls and two transmittance lookups, each several LUT
//! reads. A one-thread compute pass now evaluates them with the same functions,
//! and the result is copied into a uniform in the atmosphere bind group. On an
//! Intel UHD 770 at 1280x720 this was about a quarter of the terrain pass.
use wgpu::util::DeviceExt;

/// `array<vec4<f32>, 8>`.
pub(super) const SKY_PROBE_BYTES: u64 = 8 * 16;

pub(super) struct SkyProbe {
    /// Written by the compute pass; never bound where it is also read.
    output: wgpu::Buffer,
    output_bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl SkyProbe {
    /// The uniform the atmosphere bind group exposes as `sky_probe`.
    pub(super) fn create_uniform(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sky probe uniform"),
            contents: &[0; SKY_PROBE_BYTES as usize],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    pub(super) fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        lighting_layout: &wgpu::BindGroupLayout,
        atmosphere_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky probe output"),
            size: SKY_PROBE_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky probe output layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(SKY_PROBE_BYTES),
                },
                count: None,
            }],
        });
        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky probe output bind group"),
            layout: &output_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: output.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky probe pipeline layout"),
            bind_group_layouts: &[camera_layout, lighting_layout, atmosphere_layout, &output_layout],
            push_constant_ranges: &[],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky probe shader"),
            source: wgpu::ShaderSource::Wgsl(
                crate::shader_sources::sky_probe_shader_source().into(),
            ),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("sky probe pipeline"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("cs_sky_probe"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self { output, output_bind_group, pipeline }
    }

    /// Evaluate this frame's terms and publish them to `uniform`. Record after
    /// the frame's camera and lighting writes and before any terrain draw.
    pub(super) fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera: &wgpu::BindGroup,
        lighting: &wgpu::BindGroup,
        atmosphere: &wgpu::BindGroup,
        uniform: &wgpu::Buffer,
    ) {
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("sky probe pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, camera, &[]);
            pass.set_bind_group(1, lighting, &[]);
            pass.set_bind_group(2, atmosphere, &[]);
            pass.set_bind_group(3, &self.output_bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.output, 0, uniform, 0, SKY_PROBE_BYTES);
    }
}
