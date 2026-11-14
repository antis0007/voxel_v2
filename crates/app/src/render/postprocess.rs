//! Shared HDR scene resolve. Tone mapping happens after sky/cloud/terrain
//! composition; optional edge antialiasing and UI follow at native resolution.
pub(super) const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(super) struct PostProcess {
    pub scene: wgpu::TextureView,
    /// Scene size: the HDR target and depth are this big.
    size: (u32, u32),
    /// Window size: the resolve and present passes write at this size.
    surface_size: (u32, u32),
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    native_pipeline: wgpu::RenderPipeline,
    reference_resolve: bool,
    params: wgpu::Buffer,
    last_settings: Option<[f32; 32]>,
    // Scene and depth views share the resize lifetime. Uniform writes do not
    // change their bindings, nor does the acquired swapchain target.
    inputs: Option<wgpu::BindGroup>,
    present_layout: wgpu::BindGroupLayout,
    present_params: wgpu::Buffer,
    last_present: Option<[f32; 4]>,
    blue_noise_view: wgpu::TextureView,
    present_pipeline: wgpu::RenderPipeline,
    output_tex: Option<wgpu::Texture>,
    output_view: Option<wgpu::TextureView>,
    output_size: (u32, u32),
    present_bind_group: Option<wgpu::BindGroup>,
}

impl PostProcess {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        blue_noise_view: &wgpu::TextureView,
        pipeline_cache: Option<&wgpu::PipelineCache>,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene antialiasing layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Reverse-Z scene depth for screen-space ambient occlusion.
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene antialiasing sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene antialiasing shader"),
            source: wgpu::ShaderSource::Wgsl(crate::shader_sources::POSTPROCESS_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene antialiasing pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let reference_offset = std::env::args().any(|arg| arg == "--benchmark-reference-pixel-centres");
        let resolve_constants = [
            ("REFERENCE_RESOLVE_OFFSET", if reference_offset { 1.0 } else { 0.0 }),
        ];
        let make_pipeline = |entry, format| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene antialiasing pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &resolve_constants,
                    ..Default::default()
                },
                targets: &[Some(wgpu::ColorTargetState {
                    // Keep the stylistic grade in linear 16-bit float. The
                    // present pass owns the only 8-bit write and dither, at
                    // native resolution.
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: pipeline_cache,
        });
        let pipeline = make_pipeline("fs_main", SCENE_FORMAT);
        let native_pipeline = make_pipeline("fs_native", config.format);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HDR resolve exposure saturation AA debug + pixel style + magic"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene = Self::target(device, config);

        // The resolve writes one texel per art pixel. A nearest-neighbour pass
        // then upscales that small target to the surface. This costs one ninth
        // of the old full-resolution multi-tap bloom at 3x, while producing
        // exact square pixels instead of sampling the scene at native rate and
        // discarding most of the result.
        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pixel resolve present layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let present_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel resolve present scale"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pixel resolve present shader"),
            source: wgpu::ShaderSource::Wgsl(crate::shader_sources::PRESENT_SHADER.into()),
        });
        let present_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pixel resolve present pipeline layout"),
                bind_group_layouts: &[Some(&present_layout)],
                immediate_size: 0,
            });
        let present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pixel resolve present pipeline"),
            layout: Some(&present_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &present_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &present_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: pipeline_cache,
        });

        Self {
            scene,
            size: super::scene_dims(config.width, config.height),
            surface_size: (config.width.max(1), config.height.max(1)),
            format: config.format,
            layout,
            sampler,
            pipeline,
            native_pipeline,
            reference_resolve: std::env::args().any(|arg| arg == "--benchmark-reference-resolve"),
            params,
            last_settings: None,
            inputs: None,
            present_layout,
            present_params,
            last_present: None,
            blue_noise_view: blue_noise_view.clone(),
            present_pipeline,
            output_tex: None,
            output_view: None,
            output_size: (0, 0),
            present_bind_group: None,
        }
    }

    fn ensure_output(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if self.output_size == size && self.present_bind_group.is_some() {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("low-resolution stylised resolve"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pixel resolve present inputs"),
            layout: &self.present_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.present_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.blue_noise_view),
                },
            ],
        });
        self.output_tex = Some(texture);
        self.output_view = Some(view);
        self.output_size = size;
        self.present_bind_group = Some(bind_group);
    }

    fn target(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("linear HDR scene before tone mapping"),
            size: wgpu::Extent3d {
                width: super::scene_dims(config.width, config.height).0,
                height: super::scene_dims(config.width, config.height).1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&Default::default())
    }

    pub fn resize(&mut self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
        self.inputs = None;
        self.scene = Self::target(device, config);
        self.size = super::scene_dims(config.width, config.height);
        self.surface_size = (config.width.max(1), config.height.max(1));
        // The low-resolution target is recreated on the next frame at the
        // pixel scale that frame actually uses.
        self.output_tex = None;
        self.output_view = None;
        self.output_size = (0, 0);
        self.present_bind_group = None;
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        mut settings: [f32; 32],
        timestamps: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let debug = settings[3] > 0.5;
        // `render_scale` is the art-pixel size in window pixels (1 = off). The scene
        // may also be smaller than the window; `ratio` is scene / window, so one
        // output texel covers `art * ratio` scene texels (below 1 means upscaling).
        let render_scale = if debug { 1.0 } else { settings[4].max(1.0) };
        let native = render_scale == 1.0 && !self.reference_resolve;
        let ratio = self.size.0 as f32 / self.surface_size.0.max(1) as f32;
        settings[4] = render_scale * ratio;
        // Scene dimensions round independently. Reusing X for Y subtly shifts
        // vertical sampling at odd window sizes and fractional render scales.
        settings[31] = render_scale * self.size.1 as f32 / self.surface_size.1.max(1) as f32;
        if self.last_settings != Some(settings) {
            queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&settings));
            self.last_settings = Some(settings);
        }
        if !native {
            let present = [render_scale, settings[3], 0.0, 0.0];
            if self.last_present != Some(present) {
                queue.write_buffer(&self.present_params, 0, bytemuck::cast_slice(&present));
                self.last_present = Some(present);
            }
            self.ensure_output(device, (
                (self.surface_size.0 as f32 / render_scale).ceil().max(1.0) as u32,
                (self.surface_size.1 as f32 / render_scale).ceil().max(1.0) as u32,
            ));
        }
        let resolved = if native { target } else { self.output_view.as_ref().expect("resolve target") };
        if self.inputs.is_none() {
            self.inputs = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene antialiasing inputs"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.blue_noise_view),
                },
            ],
            }));
        }
        let bind_group = self.inputs.as_ref().expect("resolve inputs");
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("scene antialiasing pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: resolved,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: timestamps.as_ref().map(|t| wgpu::RenderPassTimestampWrites {
                    query_set: t.query_set,
                    beginning_of_pass_write_index: t.beginning_of_pass_write_index,
                    end_of_pass_write_index: if native { t.end_of_pass_write_index } else { None },
                }),
                occlusion_query_set: None,
            });
            pass.set_pipeline(if native { &self.native_pipeline } else { &self.pipeline });
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        if native { return; }

        let mut present = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("pixel resolve present pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timestamps.as_ref().map(|t| wgpu::RenderPassTimestampWrites {
                query_set: t.query_set,
                beginning_of_pass_write_index: None,
                end_of_pass_write_index: t.end_of_pass_write_index,
            }),
            occlusion_query_set: None,
        });
        present.set_pipeline(&self.present_pipeline);
        present.set_bind_group(
            0,
            self.present_bind_group
                .as_ref()
                .expect("present bind group"),
            &[],
        );
        present.draw(0..3, 0..1);
    }
}
