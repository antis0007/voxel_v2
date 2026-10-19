//! Half-resolution diffuse exchange and ambient-only obscurance, no temporal ghosts.
use super::postprocess::SCENE_FORMAT;
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn enabled() -> bool {
    ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn set_enabled(value: bool) {
    ENABLED.store(value, std::sync::atomic::Ordering::Relaxed);
}
pub struct IndirectLighting {
    pub receiver: wgpu::TextureView,
    field: wgpu::TextureView,
    gather_layout: wgpu::BindGroupLayout,
    compose_layout: wgpu::BindGroupLayout,
    gather_group: wgpu::BindGroup,
    compose_group: wgpu::BindGroup,
    gather: wgpu::RenderPipeline,
    compose: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}
impl IndirectLighting {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        camera: &wgpu::BindGroupLayout,
        depth: &wgpu::TextureView,
        scene: &wgpu::TextureView,
    ) -> Self {
        let layout = |name| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(name),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            })
        };
        let gather_layout = layout("diffuse exchange inputs");
        let compose_layout = layout("diffuse resolve inputs");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("diffuse bilinear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let receiver = Self::texture(
            device,
            config.width,
            config.height,
            "diffuse material and ambient",
        );
        let field = Self::texture(
            device,
            config.width.div_ceil(2),
            config.height.div_ceil(2),
            "half resolution diffuse exchange",
        );
        let group = |layout: &wgpu::BindGroupLayout, input: &wgpu::TextureView| {
            Self::group(device, layout, depth, &receiver, &sampler, input)
        };
        let gather_group = group(&gather_layout, scene);
        let compose_group = group(&compose_layout, &field);
        let pipeline = |composite: bool, layout: &wgpu::BindGroupLayout| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("diffuse exchange"),
                source: wgpu::ShaderSource::Wgsl(
                    crate::shader_sources::indirect_shader_source(composite).into(),
                ),
            });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("diffuse layout"),
                bind_group_layouts: &[camera, layout],
                push_constant_ranges: &[],
            });
            let blend = if composite {
                Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                })
            } else {
                None
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("diffuse exchange pipeline"),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SCENE_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let gather = pipeline(false, &gather_layout);
        let compose = pipeline(true, &compose_layout);
        Self {
            receiver,
            field,
            gather_layout,
            compose_layout,
            gather_group,
            compose_group,
            gather,
            compose,
            sampler,
        }
    }
    fn texture(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w.max(1),
                    height: h.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SCENE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }
    fn group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        depth: &wgpu::TextureView,
        receiver: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        input: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diffuse inputs"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(receiver),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(input),
                },
            ],
        })
    }
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        depth: &wgpu::TextureView,
        scene: &wgpu::TextureView,
    ) {
        self.receiver = Self::texture(device, config.width, config.height, "diffuse receiver");
        self.field = Self::texture(
            device,
            config.width.div_ceil(2),
            config.height.div_ceil(2),
            "diffuse exchange",
        );
        self.gather_group = Self::group(
            device,
            &self.gather_layout,
            depth,
            &self.receiver,
            &self.sampler,
            scene,
        );
        self.compose_group = Self::group(
            device,
            &self.compose_layout,
            depth,
            &self.receiver,
            &self.sampler,
            &self.field,
        );
    }
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera: &wgpu::BindGroup,
        scene: &wgpu::TextureView,
        timing: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        for composite in [false, true] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if composite {
                    "diffuse resolve"
                } else {
                    "diffuse gather"
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if composite { scene } else { &self.field },
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if composite {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: timing.map(|(query_set, start, end)| {
                    wgpu::RenderPassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: if composite { None } else { Some(start) },
                        end_of_pass_write_index: if composite { Some(end) } else { None },
                    }
                }),
                occlusion_query_set: None,
            });
            pass.set_pipeline(if composite {
                &self.compose
            } else {
                &self.gather
            });
            pass.set_bind_group(0, camera, &[]);
            pass.set_bind_group(
                1,
                if composite {
                    &self.compose_group
                } else {
                    &self.gather_group
                },
                &[],
            );
            pass.draw(0..3, 0..1);
        }
    }
}
