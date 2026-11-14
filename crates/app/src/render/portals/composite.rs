//! Offscreen portal composition. Each destination renders with its own camera
//! uniform into pooled targets; nothing mutates the primary view uniforms.
use super::super::postprocess::SCENE_FORMAT;
use crate::world::portals::views::{MAX_TARGET_PIXELS, MAX_TARGET_SIDE, MAX_VIEWS, Plan, View};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Composite {
    pub view_projection: [[f32; 4]; 4],
    pub aperture: [[f32; 4]; 4],
    /// xy: parent target size; z: seconds; w: veil strength, 0 = clear window.
    pub parent_size: [f32; 4],
    pub clip_planes: [[f32; 4]; 20],
    /// x: active clip planes; y: 1 when no destination image is available.
    pub clip_count: [u32; 4],
}

/// One destination view: HDR scene, indirect receiver, depth and its camera.
pub(super) struct Target {
    size: [u32; 2],
    pub scene: wgpu::TextureView,
    pub receiver: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    pub camera: wgpu::Buffer,
    pub camera_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
}

/// Opening kinds without a rendered destination.
pub(super) const MIST: u32 = 1;
pub(super) const VISTA: u32 = 2;
/// Mist over an opening whose far realm has no stand-in.
pub(super) const REALM_MIST: u32 = 3;
pub(super) const HELL_MIST: u32 = 4;
/// Pooled veil quads.
const MAX_VEILS: usize = 8;

/// Veil quad for an opening whose destination is not resident yet.
struct Veil {
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
}

pub(super) struct Renderer {
    layout: wgpu::BindGroupLayout,
    camera_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    window: wgpu::RenderPipeline,
    veil: wgpu::RenderPipeline,
    blank: wgpu::TextureView,
    targets: Vec<Target>,
    veils: Vec<Veil>,
}

fn texture(device: &wgpu::Device, label: &str, size: [u32; 2], format: wgpu::TextureFormat) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl Renderer {
    pub fn new(device: &wgpu::Device, camera_layout: wgpu::BindGroupLayout) -> Self {
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("portal composite"),
            entries: &[
                entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Composite>() as u64),
                }),
                entry(1, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                }),
                entry(2, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("portal linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("portal composite"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("portal composite"),
            source: wgpu::ShaderSource::Wgsl(crate::shader_sources::PORTAL_COMPOSITE_SHADER.into()),
        });
        let pipeline = |label, entry: &str, blend, depth_write| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
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
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: SCENE_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let window = pipeline("portal aperture", "fs_main", None, true);
        // An unready opening stays see-through: the veil blends over the scene.
        let veil = pipeline("portal veil", "fs_veil", Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), false);
        let blank = texture(device, "portal blank", [1, 1], SCENE_FORMAT);
        Self { layout, camera_layout, sampler, window, veil, blank, targets: Vec::new(), veils: Vec::new() }
    }

    fn group(&self, device: &wgpu::Device, uniform: &wgpu::Buffer, image: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("portal destination"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(image) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    fn uniform(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn target(&self, device: &wgpu::Device, size: [u32; 2], camera_bytes: usize) -> Target {
        let scene = texture(device, "portal HDR", size, SCENE_FORMAT);
        let receiver = texture(device, "portal diffuse receiver", size, SCENE_FORMAT);
        let depth = texture(device, "portal reverse depth", size, wgpu::TextureFormat::Depth32Float);
        let uniform = Self::uniform(device, "portal composite camera", std::mem::size_of::<Composite>());
        let camera = Self::uniform(device, "portal view camera", camera_bytes);
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("portal view camera"),
            layout: &self.camera_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera.as_entire_binding() }],
        });
        let group = self.group(device, &uniform, &scene);
        Target { size, scene, receiver, depth, camera, camera_group, uniform, group }
    }

    /// Size the pool to the plan; targets keep their textures while sizes hold.
    pub fn prepare(&mut self, device: &wgpu::Device, plan: &Plan, camera_bytes: usize) -> anyhow::Result<()> {
        anyhow::ensure!(plan.views.len() <= MAX_VIEWS, "Portal view budget exceeded");
        let mut pixels = 0u64;
        for (i, view) in plan.views.iter().enumerate().skip(1) {
            anyhow::ensure!(
                view.parent.is_some_and(|p| p < i) && view.camera.valid()
                    && view.clip_planes.len() <= 20
                    && view.clip_planes.iter().all(|p| p.point.is_finite() && p.normal.is_finite())
                    && view.aperture.iter().flatten().all(|n| n.is_finite())
                    && view.target_size.iter().all(|n| *n > 0 && *n <= MAX_TARGET_SIDE),
                "Invalid portal view"
            );
            pixels += view.target_size[0] as u64 * view.target_size[1] as u64;
        }
        anyhow::ensure!(pixels <= MAX_TARGET_PIXELS, "Portal pixel budget exceeded");
        let count = plan.views.len().saturating_sub(1);
        self.targets.truncate(count);
        for (slot, view) in plan.views.iter().skip(1).enumerate() {
            if slot == self.targets.len() {
                self.targets.push(self.target(device, view.target_size, camera_bytes));
            } else if self.targets[slot].size != view.target_size {
                self.targets[slot] = self.target(device, view.target_size, camera_bytes);
            }
        }
        Ok(())
    }

    pub fn target_of(&self, view_index: usize) -> &Target {
        &self.targets[view_index - 1]
    }

    /// Composite children into parents, deepest first. `rendered[i]` marks the
    /// destinations whose scene pass succeeded; the rest draw the veil only.
    pub fn composite(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        plan: &Plan,
        rendered: &[bool],
        root: (&wgpu::TextureView, &wgpu::TextureView),
        unready: &[(usize, [[f32; 3]; 4], u32)],
        time: f32,
    ) {
        let uniform = |parent: &View, aperture: [[f32; 3]; 4], kind: u32| {
            let mut clip_planes = [[0.0; 4]; 20];
            for (plane, dst) in parent.clip_planes.iter().zip(clip_planes.iter_mut()) {
                *dst = plane.relative_to(parent.camera.eye);
            }
            Composite {
                view_projection: parent.camera.relative_view_projection().as_mat4().to_cols_array_2d(),
                aperture: aperture.map(|p| [p[0], p[1], p[2], 1.0]),
                parent_size: [parent.target_size[0] as f32, parent.target_size[1] as f32, time, super::veil_strength()],
                clip_planes,
                clip_count: [parent.clip_planes.len().min(20) as u32, kind, 0, 0],
            }
        };
        for (slot, view) in plan.views.iter().enumerate().skip(1) {
            let Some(parent) = view.parent.and_then(|p| plan.views.get(p)) else { continue };
            let data = uniform(parent, view.aperture, 0);
            queue.write_buffer(&self.targets[slot - 1].uniform, 0, bytemuck::bytes_of(&data));
        }
        while self.veils.len() < unready.len().min(MAX_VEILS) {
            let uniform = Self::uniform(device, "portal veil", std::mem::size_of::<Composite>());
            let group = self.group(device, &uniform, &self.blank);
            self.veils.push(Veil { uniform, group });
        }
        for ((parent, aperture, kind), veil) in unready.iter().zip(&self.veils) {
            let Some(parent) = plan.views.get(*parent) else { continue };
            queue.write_buffer(&veil.uniform, 0, bytemuck::bytes_of(&uniform(parent, *aperture, *kind)));
        }
        for (index, view) in plan.render_order() {
            if index != 0 && !rendered[index] {
                continue;
            }
            let (scene, depth) = if index == 0 { root } else {
                let t = &self.targets[index - 1];
                (&t.scene, &t.depth)
            };
            let children: Vec<_> = plan.views.iter().enumerate().skip(1)
                .filter(|(i, c)| c.parent == Some(index) && rendered[*i]).collect();
            let veils: Vec<_> = unready.iter().zip(&self.veils).filter(|((p, _, _), _)| *p == index).collect();
            if children.is_empty() && veils.is_empty() {
                continue;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("portal aperture composition"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.window);
            for (child_index, child) in children {
                // Scissor to the child's screen bounds; the aperture quad and
                // depth bound it further.
                let Some((x, y, w, h)) = scissor(child.screen_rect.min, child.screen_rect.max, view.target_size) else { continue };
                pass.set_scissor_rect(x, y, w, h);
                pass.set_bind_group(0, &self.targets[child_index - 1].group, &[]);
                pass.draw(0..6, 0..1);
            }
            pass.set_scissor_rect(0, 0, view.target_size[0], view.target_size[1]);
            // Vistas are opaque windows; mist membranes blend over the scene.
            for ((_, _, kind), veil) in veils {
                pass.set_pipeline(if *kind == VISTA { &self.window } else { &self.veil });
                pass.set_bind_group(0, &veil.group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
    }
}

fn scissor(min: [f64; 2], max: [f64; 2], size: [u32; 2]) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (size[0] as f64, size[1] as f64);
    let x0 = ((min[0] + 1.0) * 0.5 * w).floor().clamp(0.0, w) as u32;
    let x1 = ((max[0] + 1.0) * 0.5 * w).ceil().clamp(0.0, w) as u32;
    let y0 = ((1.0 - max[1]) * 0.5 * h).floor().clamp(0.0, h) as u32;
    let y1 = ((1.0 - min[1]) * 0.5 * h).ceil().clamp(0.0, h) as u32;
    (x1 > x0 && y1 > y0).then(|| (x0, y0, x1 - x0, y1 - y0))
}
