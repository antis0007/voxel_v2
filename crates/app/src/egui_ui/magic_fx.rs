//! Masked, palette-bound UI magic. One reusable GPU buffer, no per-icon GPU allocations.
use egui::{Id, Painter, Rect};
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor};

const CAPACITY: u32 = 256;
const COUNTER: &str = "grimoire-fx-count";

#[derive(Clone, Copy)]
pub enum Effect {
    Enchanted = 1,
    Unstable = 2,
    Cursed = 3,
}

pub fn begin_frame(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(Id::new(COUNTER), 0_u32));
}

pub fn reduced(ctx: &egui::Context) -> bool {
    super::magic_style::get(ctx).reduced
}

pub fn motion_setting(ui: &mut egui::Ui) {
    let mut value = reduced(ui.ctx());
    if ui
        .checkbox(&mut value, "Reduce UI motion and magic shimmer")
        .changed()
    {
        let mut settings = super::magic_style::get(ui.ctx());
        settings.reduced = value;
        super::magic_style::set(ui.ctx(), settings);
    }
    if !std::env::var("VOXEL_GRIMOIRE_REVIEW").is_ok_and(|v| v == "components") {
        super::magic_style::panel(ui);
    }
}

/// Body pixels only: keep outlines, labels, counts and transparent cavities untouched.
pub fn paint(
    p: &Painter,
    rect: Rect,
    rows: &[&str],
    effect: Effect,
    source: Option<egui::Color32>,
) {
    if !p.is_visible() || !rect.intersects(p.clip_rect()) {
        return;
    }
    let index = p.ctx().data_mut(|d| {
        let n = d.get_temp::<u32>(Id::new(COUNTER)).unwrap_or(0);
        d.insert_temp(Id::new(COUNTER), n.saturating_add(1));
        n
    });
    if index >= CAPACITY {
        return;
    } // Base sprite is always retained.
    let mut mask = [0_u32; 8];
    for (y, row) in rows.iter().take(16).enumerate() {
        for (x, ch) in row.bytes().take(16).enumerate() {
            if matches!(ch, b'1' | b'2' | b'4' | b'5' | b'6') {
                let bit = y * 16 + x;
                mask[bit / 32] |= 1 << (bit % 32);
            }
        }
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(1).min(16);
    let still = reduced(p.ctx());
    let tick = if still {
        9
    } else {
        p.ctx().input(|i| (i.time * 8.0) as u32)
    };
    let mut data = [0_u32; 32];
    data[..4].copy_from_slice(&[tick, effect as u32, width as u32, rows.len().min(16) as u32]);
    data[4..12].copy_from_slice(&mask);
    let mut style = super::magic_style::get(p.ctx()).profiles[effect as usize - 1];
    if let Some(source) = source.filter(|_| matches!(effect, Effect::Enchanted)) {
        style = style.with_source(source);
    }
    data[12..].copy_from_slice(&style.shader_words());
    p.add(Callback::new_paint_callback(
        rect,
        MagicCallback { index, data },
    ));
    if !still {
        p.ctx()
            .request_repaint_after(std::time::Duration::from_millis(125));
    }
}

struct Resources {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
    stride: u32,
}
struct MagicCallback {
    index: u32,
    data: [u32; 32],
}

pub fn install(
    renderer: &mut egui_wgpu::Renderer,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) {
    let stride = device.limits().min_uniform_buffer_offset_alignment.max(128);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("UI magic uniforms"),
        size: (stride * CAPACITY) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("UI magic mask"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(128),
            },
            count: None,
        }],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("UI magic"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(128),
            }),
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Pixel enchantment / instability / curse"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::UI_MAGIC_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("UI magic"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("UI pixel magic"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    renderer.callback_resources.insert(Resources {
        pipeline,
        buffer,
        bind,
        stride,
    });
}

impl CallbackTrait for MagicCallback {
    fn prepare(
        &self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r = resources.get::<Resources>().expect("UI magic installed");
        queue.write_buffer(
            &r.buffer,
            (self.index * r.stride) as u64,
            bytemuck::cast_slice(&self.data),
        );
        Vec::new()
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let r = resources.get::<Resources>().expect("UI magic installed");
        pass.set_pipeline(&r.pipeline);
        pass.set_bind_group(0, &r.bind, &[self.index * r.stride]);
        pass.draw(0..3, 0..1);
    }
}
