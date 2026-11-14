//! Bounded GPU diorama shared by the title and loading screens.
//! Renders at a fixed pixel density, independent of terrain streaming.
use egui::{Painter, Rect};
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor};

struct Resources {
    scene: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    scene_bind: wgpu::BindGroup,
    image_layout: wgpu::BindGroupLayout,
    image_bind: wgpu::BindGroup,
    view: wgpu::TextureView,
    size: [u32; 2],
}

/// Offscreen size for a viewport of `pixels`: the largest target of at most
/// 360 rows whose texels each cover a whole number of screen pixels. A fixed
/// 640x360 target stretched 2.5x to 900p alternated 2- and 3-pixel texels.
/// `menu_present.wgsl` derives the same scale from the viewport height.
fn target_size(pixels: [f32; 2]) -> [u32; 2] {
    let scale = (pixels[1] / 360.0).ceil().max(1.0);
    [((pixels[0] / scale).ceil() as u32).clamp(1, 1024),
        ((pixels[1] / scale).ceil() as u32).clamp(1, 360)]
}

fn target(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size: [u32; 2])
    -> (wgpu::TextureView, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Title vista pixels"),
        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Title vista pixels"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }],
    });
    (view, bind)
}
struct SceneCallback {
    seconds: f32,
    aspect: f32,
    quiet_edge: f32,
    dim: f32,
    mood: u32,
    pose: u32,
    /// Painted rect in points; physical pixels are resolved in `prepare`.
    points: [f32; 2],
}

pub(super) const MOODS: u32 = 4;
pub(super) const POSES: u32 = 3;
const UNSET: u32 = u32::MAX;
static FORCED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(UNSET);

/// Pins the hour and standpoint; the art exporter uses it so captures repeat.
pub(super) fn force(mood: u32, pose: u32) {
    FORCED.store((mood % MOODS) * POSES + pose % POSES, std::sync::atomic::Ordering::Relaxed);
}

/// Hour (dusk, night, dawn, day) and standpoint for this launch. Drawn once
/// from the wall clock so each start shows another face of the same province;
/// `VOXEL_MENU_VARIANT=<mood>,<pose>` pins it for review. Presentation only:
/// nothing in the simulation reads it.
fn variant() -> (u32, u32) {
    static PICK: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    let forced = FORCED.load(std::sync::atomic::Ordering::Relaxed);
    let index = if forced != UNSET { forced } else {
        *PICK.get_or_init(|| {
            let pinned = std::env::var("VOXEL_MENU_VARIANT").ok().and_then(|v| {
                let (mood, pose) = v.split_once(',')?;
                Some((mood.trim().parse::<u32>().ok()? % MOODS) * POSES
                    + pose.trim().parse::<u32>().ok()? % POSES)
            });
            pinned.unwrap_or_else(|| {
                let seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                (seconds.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as u32 % (MOODS * POSES)
            })
        })
    };
    (index / POSES, index % POSES)
}

/// Column placement shared by the title stack and the loading screen: left
/// edge, width, and the quiet-band edge for `paint` (negative when centred).
/// Wide windows anchor the column in the vista's quiet left band so the town
/// and river stay visible; narrower windows centre it over an even shade.
pub(super) fn column(screen: Rect) -> (f32, f32, f32) {
    let width = (screen.width() - 48.0).clamp(80.0, 408.0);
    let wide = screen.width() >= 900.0;
    let left = if wide { (screen.width() * 0.08).max(48.0) } else { (screen.width() - width) * 0.5 };
    let quiet_edge = if wide { (left + width + 16.0) / screen.width() } else { -1.0 };
    (screen.left() + left, width, quiet_edge)
}

/// `quiet_edge` is where the menu column ends as a fraction of `rect` width,
/// or a negative value for an even shade; `dim` (0..1) darkens for loading.
pub(super) fn paint(p: &Painter, rect: Rect, quiet_edge: f32, dim: f32) {
    let still = super::magic_fx::reduced(p.ctx());
    let seconds = if still {
        0.0
    } else {
        p.ctx().input(|i| i.time as f32)
    };
    // The vista rises out of black over the first second after launch.
    let rise = if still { 1.0 } else {
        let x = ((p.ctx().input(|i| i.time) as f32 - 0.15) / 1.1).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let (mood, pose) = variant();
    p.add(Callback::new_paint_callback(
        rect,
        SceneCallback {
            seconds,
            aspect: rect.width() / rect.height().max(1.0),
            quiet_edge,
            dim: (1.0 - (1.0 - dim) * rise).clamp(0.0, 1.0),
            mood,
            pose,
            points: [rect.width(), rect.height()],
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
        label: Some("Title vista clock"),
        size: 32,
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
                min_binding_size: wgpu::BufferSize::new(32),
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
    // At most 1024x360 shaded pixels on any display; nearest presentation at a
    // whole-number scale keeps the same hard pixel edges as the world's
    // surfaces and the grimoire sprites. Resized in `prepare` per viewport.
    let size = [640, 360];
    let (view, image_bind) = target(device, &image_layout, size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Title vista"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::MENU_SCENE_SHADER.into()),
    });
    let image_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Meridian pixel presentation"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::MENU_PRESENT_SHADER.into()),
    });
    let make_pipeline = |shader: &wgpu::ShaderModule, layout: &wgpu::BindGroupLayout, format| {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Meridian room"),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
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
            multiview_mask: None,
            cache: None,
        })
    };
    renderer.callback_resources.insert(Resources {
        scene: make_pipeline(&shader, &scene_layout, wgpu::TextureFormat::Rgba8Unorm),
        present: make_pipeline(&image_shader, &image_layout, format),
        uniform,
        scene_bind,
        image_layout,
        image_bind,
        view,
        size,
    });
}

impl CallbackTrait for SceneCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = resources.get_mut::<Resources>() else {
            return Vec::new();
        };
        let size = target_size([(self.points[0] * screen.pixels_per_point).round(),
            (self.points[1] * screen.pixels_per_point).round()]);
        if size != r.size {
            (r.view, r.image_bind) = target(device, &r.image_layout, size);
            r.size = size;
        }
        queue.write_buffer(
            &r.uniform,
            0,
            bytemuck::cast_slice(&[self.seconds, self.aspect, self.quiet_edge, self.dim,
                self.mood as f32, self.pose as f32, size[0] as f32, size[1] as f32]),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
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
