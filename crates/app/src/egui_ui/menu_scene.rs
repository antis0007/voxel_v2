//! Bounded GPU diorama shared by the title and loading screens.
//! Renders at a fixed pixel density, independent of terrain streaming.
use egui::{Painter, Rect};
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor};

struct Resources {
    scene: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    scene_bind: wgpu::BindGroup,
    image_bind: wgpu::BindGroup,
    view: wgpu::TextureView,
}
struct SceneCallback {
    seconds: f32,
    aspect: f32,
}

pub(super) fn paint(p: &Painter, rect: Rect) {
    let still = super::magic_fx::reduced(p.ctx());
    let seconds = if still {
        0.0
    } else {
        p.ctx().input(|i| i.time as f32)
    };
    p.add(Callback::new_paint_callback(
        rect,
        SceneCallback {
            seconds,
            aspect: rect.width() / rect.height().max(1.0),
        },
    ));
    if !still {
        p.ctx()
            .request_repaint_after(std::time::Duration::from_millis(50));
    }
}

pub(super) fn install(
    renderer: &mut egui_wgpu::Renderer,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) {
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Meridian room clock"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Meridian room"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(16),
            },
            count: None,
        }],
    });
    let scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Meridian room"),
        layout: &scene_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    // 230k shaded pixels even on a 4K display; nearest presentation retains the
    // same hard pixel edges as the world's surfaces and the grimoire sprites.
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Meridian room pixels"),
        size: wgpu::Extent3d {
            width: 640,
            height: 360,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Meridian pixel presentation"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let image_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Meridian pixels"),
        layout: &image_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Meridian instrument room"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::MENU_SCENE_SHADER.into()),
    });
    let image_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Meridian pixel presentation"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::MENU_PRESENT_SHADER.into()),
    });
    let make_pipeline = |shader: &wgpu::ShaderModule, layout: &wgpu::BindGroupLayout, format| {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Meridian room"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Meridian room"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
    };
    renderer.callback_resources.insert(Resources {
        scene: make_pipeline(&shader, &scene_layout, wgpu::TextureFormat::Rgba8Unorm),
        present: make_pipeline(&image_shader, &image_layout, format),
        uniform,
        scene_bind,
        image_bind,
        view,
    });
}

impl CallbackTrait for SceneCallback {
    fn prepare(
        &self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = resources.get::<Resources>() else {
            return Vec::new();
        };
        queue.write_buffer(
            &r.uniform,
            0,
            bytemuck::cast_slice(&[self.seconds, self.aspect, 0.0, 0.0]),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Meridian room"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &r.view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&r.scene);
        pass.set_bind_group(0, &r.scene_bind, &[]);
        pass.draw(0..3, 0..1);
        Vec::new()
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(r) = resources.get::<Resources>() else {
            return;
        };
        pass.set_pipeline(&r.present);
        pass.set_bind_group(0, &r.image_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
