//! Palette-bound stepped panel materials and crisp pixel edges.
use egui::{Painter, Rect};
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor};
const CAPACITY: u32 = 256;
const COUNTER: &str = "prism-panel-count";

pub(super) fn active(ctx: &egui::Context) -> bool {
    super::themes::current(ctx).family == "Prism"
}
pub(super) fn begin_frame(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(COUNTER), 0_u32));
}

/// Paint before contents for a lit graphite face with a black lower bevel.
pub(super) fn surface(p: &Painter, r: Rect) {
    if !active(p.ctx()) || !p.is_visible() {
        return;
    }
    p.add(egui::Shape::mesh(face(p.ctx(), r)));
    edge(p, r, 0.60, false);
}

fn face(ctx: &egui::Context, r: Rect) -> egui::Mesh {
    let palette = super::themes::current(ctx);
    let mut mesh = egui::Mesh::default();
    // A stepped upper lip, quiet body and dark lower bevel.
    let levels = [r.top(), (r.top() + 2.0).min(r.bottom()),
        (r.top() + 6.0).min(r.bottom()), (r.bottom() - 3.0).max(r.top() + 6.0).min(r.bottom()), r.bottom()];
    for (band, color) in [palette.hover(), palette.panel, palette.panel, palette.recess]
        .into_iter().enumerate() {
        let top = levels[band].round();
        let bottom = levels[band + 1].round();
        let base = mesh.vertices.len() as u32;
        for pos in [egui::pos2(r.left(), top), egui::pos2(r.right(), top),
            egui::pos2(r.right(), bottom), egui::pos2(r.left(), bottom)] {
            mesh.colored_vertex(pos, color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

/// Keep the existing frame layout while lighting behind its contents and at its rim.
pub(super) struct LitFrame(pub egui::Frame);
impl LitFrame {
    pub(super) fn show<R>(
        self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let result = self.0.show(ui, |inner| {
            let background = active(inner.ctx()).then(|| inner.painter().add(egui::Shape::Noop));
            let result = contents(inner);
            if let Some(index) = background {
                inner
                    .painter()
                    .set(index, egui::Shape::mesh(face(inner.ctx(), inner.min_rect().expand(3.0))));
            }
            result
        });
        edge(ui.painter(), result.response.rect, 0.70, false);
        result
    }
}

pub(super) fn edge(p: &Painter, r: Rect, strength: f32, rainbow: bool) {
    if !active(p.ctx()) || !p.is_visible() || !r.intersects(p.clip_rect()) {
        return;
    }
    material(p, r, strength, if rainbow { 1.0 } else { 0.0 });
}

/// Rainbow glow just outside a selected control; every theme, GPU pool.
pub(super) fn halo(p: &Painter, r: Rect, strength: f32) {
    if !p.is_visible() || !r.expand(6.0).intersects(p.clip_rect()) { return; }
    material(p, r, strength, 3.0);
}

fn material(p: &Painter, r: Rect, strength: f32, kind: f32) {
    let index = p.ctx().data_mut(|d| {
        let key = egui::Id::new(COUNTER);
        let n = d.get_temp::<u32>(key).unwrap_or(0);
        d.insert_temp(key, n.saturating_add(1));
        n
    });
    if index >= CAPACITY {
        return;
    }
    let still = super::magic_fx::reduced(p.ctx());
    let time = if still {
        2.0
    } else {
        p.ctx().input(|i| i.time as f32).rem_euclid(3600.0)
    };
    let mut data = [0.0_f32; 16];
    let palette = super::themes::current(p.ctx());
    let strength = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
    data[..4].copy_from_slice(&[r.width().max(1.0), r.height().max(1.0), time, strength]);
    data[4..7].copy_from_slice(&[palette.accent.r() as f32 / 255.0,
        palette.accent.g() as f32 / 255.0, palette.accent.b() as f32 / 255.0]);
    data[8] = kind;
    if kind > 1.5 {
        let magic = super::magic_style::get(p.ctx()).profiles[0].with_source(palette.accent);
        for (i, channel) in magic.colors[1].iter().enumerate() {
            data[9 + i] = *channel as f32 / 255.0;
        }
    }
    data[12..15].copy_from_slice(&[palette.edge.r() as f32 / 255.0,
        palette.edge.g() as f32 / 255.0, palette.edge.b() as f32 / 255.0]);
    p.add(Callback::new_paint_callback(
        r.expand(6.0),
        PrismCallback { index, data },
    ));
    if kind > 0.5 && !still {
        p.ctx()
            .request_repaint_after(std::time::Duration::from_millis(83));
    }
}

struct PrismResources {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
    stride: u32,
}
struct PrismCallback {
    index: u32,
    data: [f32; 16],
}

/// Art export uses the production slot, icon, count and rainbow painters.
pub(super) fn showcase(ui: &mut egui::Ui) {
    use crate::world::{BlockId, inventory::Item};
    let frame = egui::Frame::new()
        .fill(super::themes::current(ui.ctx()).panel)
        .stroke(egui::Stroke::new(
            1.0,
            super::themes::current(ui.ctx()).edge,
        ))
        .inner_margin(egui::Margin::same(24));
    let framed = LitFrame(frame).show(ui, |ui| {
        ui.set_width(680.0);
        let background = ui.painter().add(egui::Shape::Noop);
        ui.heading("Inventory");
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            for label in ["Inventory", "Crafting", "Equipment"] {
                let response = ui.add(egui::Button::new(label).min_size(egui::vec2(140.0, 34.0)));
                edge(ui.painter(), response.rect, 0.75, false);
            }
        });
        ui.add_space(24.0);
        egui::Grid::new("prism-export-slots")
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for i in 0..27 {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(62.0, 62.0), egui::Sense::hover());
                    super::grimoire::slot(ui.painter(), r, i == 4);
                    if i < 9 {
                        let block = [BlockId::StoneBricks, BlockId::Log, BlockId::Torch][i % 3];
                        super::inventory::draw_item_icon(
                            ui.painter(),
                            r.shrink(8.0),
                            Item::Block(block),
                        );
                        super::inventory::stack_count(
                            ui.painter(),
                            r,
                            2.0,
                            [16, 64, 128, 999, 1024, 2048, 4096, 1, 32][i],
                        );
                    }
                    if (i + 1) % 9 == 0 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(54.0, 54.0), egui::Sense::hover());
            let icon = super::icons::Icon::Wand(3);
            super::icons::paint(ui.painter(), r, icon);
            super::magic_fx::paint(
                ui.painter(),
                r,
                super::icons::rows(icon).0,
                super::magic_fx::Effect::Enchanted,
                None,
            );
            ui.label("Wand");
        });
        // Keep a quiet darker bottom band, without a text overlay.
        let band = egui::Rect::from_min_size(
            ui.min_rect().left_bottom() - egui::vec2(0.0, 2.0),
            egui::vec2(680.0, 2.0),
        );
        ui.painter().set(
            background,
            egui::Shape::rect_filled(band, 0, egui::Color32::from_rgb(4, 6, 13)),
        );
    });
    edge(ui.painter(), framed.response.rect, 0.9, false);
}

pub fn install(
    renderer: &mut egui_wgpu::Renderer,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) {
    let stride = device.limits().min_uniform_buffer_offset_alignment.max(64);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Prism lighting uniforms"),
        size: (stride * CAPACITY) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Prism lighting mask"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(64),
            },
            count: None,
        }],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Prism lighting"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(64),
            }),
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Obsidian prism edge lighting"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::UI_PRISM_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Prism lighting"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Prism panel edges"),
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
    renderer.callback_resources.insert(PrismResources {
        pipeline,
        buffer,
        bind,
        stride,
    });
}

impl CallbackTrait for PrismCallback {
    fn prepare(
        &self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r = resources
            .get::<PrismResources>()
            .expect("Prism lighting installed");
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
        let r = resources
            .get::<PrismResources>()
            .expect("Prism lighting installed");
        pass.set_pipeline(&r.pipeline);
        pass.set_bind_group(0, &r.bind, &[self.index * r.stride]);
        pass.draw(0..3, 0..1);
    }
}
