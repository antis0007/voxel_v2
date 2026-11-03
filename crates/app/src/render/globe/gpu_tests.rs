//! GPU checks of the globe shader: uniform ABI, face seams and folded heights.

use super::bake::TileData;
use super::chart::{self, RADIUS_M};
use super::gpu::{Globe, GlobeUniform, body_view_proj};
use super::scheduler::Selection;
use super::tile::{TILE_SAMPLES, TileId};
use engine_core::cube_sphere::Face;
use glam::{DVec3, Mat4, Vec3};

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NEAR_M: f32 = 1.0;

fn wgsl_struct_size(name: &str) -> u32 {
    let source = crate::shader_sources::globe_shader_source();
    let module = wgpu::naga::front::wgsl::parse_str(&source).expect("globe WGSL parses");
    module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some(name))
        .map(|(_, ty)| ty.inner.size(module.to_ctx()))
        .expect("struct present")
}

struct Frame {
    size: u32,
    color: Vec<[u8; 4]>,
    depth: Vec<f32>,
}

fn projection(fov_deg: f32) -> Mat4 {
    crate::render::REVERSE_Z * Mat4::perspective_infinite_rh(fov_deg.to_radians(), 1.0, NEAR_M)
}

fn render(tiles: Vec<TileData>, eye_body: DVec3, view_proj: Mat4, size: u32) -> Frame {
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut globe = Globe::new(&device, COLOR);
    let draw: Vec<TileId> = tiles.iter().map(|t| t.id).collect();
    globe.upload(&queue, tiles);
    assert_eq!(globe.resident(), draw.len());
    let sun = eye_body.normalize().as_vec3();
    let eye = eye_body.as_vec3();
    globe.prepare(&queue, &Selection { draw, wanted: Vec::new() }, eye_body, GlobeUniform {
        eye_body: [eye.x, eye.y, eye.z, 0.0],
        sun_dir_body: [sun.x, sun.y, sun.z, 0.0],
        radius_m: RADIUS_M as f32,
        sea_level_m: 0.0,
        focal_px: size as f32,
        flags: 0,
        limb: [0.0; 4],
        view_proj: view_proj.to_cols_array_2d(),
    });
    let target = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: usage | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let color = target(COLOR, wgpu::TextureUsages::empty());
    let receiver = target(COLOR, wgpu::TextureUsages::empty());
    let depth = target(wgpu::TextureFormat::Depth32Float, wgpu::TextureUsages::empty());
    let views = [&color, &receiver, &depth].map(|t| t.create_view(&Default::default()));
    let row = (size * 4).div_ceil(256) * 256;
    let readback = |label| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: u64::from(row * size),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    };
    let (color_buf, depth_buf) = (readback("color"), readback("depth"));
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let attachment = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 }), store: wgpu::StoreOp::Store },
            })
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("globe test"),
            color_attachments: &[attachment(&views[0]), attachment(&views[1])],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &views[2],
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        globe.draw(&mut pass);
    }
    for (texture, buffer, aspect) in [(&color, &color_buf, wgpu::TextureAspect::All), (&depth, &depth_buf, wgpu::TextureAspect::DepthOnly)] {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size) },
            },
            wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        );
    }
    let submission = queue.submit([encoder.finish()]);
    for buffer in [&color_buf, &depth_buf] {
        buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
    }
    device
        .poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: Some(std::time::Duration::from_secs(10)) })
        .unwrap();
    let rows = |buffer: &wgpu::Buffer| -> Vec<[u8; 4]> {
        let mapped = buffer.slice(..).get_mapped_range();
        (0..size)
            .flat_map(|y| (0..size).map(move |x| (y * row + x * 4) as usize))
            .map(|o| mapped[o..o + 4].try_into().unwrap())
            .collect()
    };
    let color = rows(&color_buf);
    let depth = rows(&depth_buf).into_iter().map(f32::from_ne_bytes).collect();
    Frame { size, color, depth }
}

fn look(eye_body: DVec3, target: DVec3, fov_deg: f32) -> Mat4 {
    let forward = (target - eye_body).normalize().as_vec3();
    let up = if forward.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    projection(fov_deg) * Mat4::look_to_rh(Vec3::ZERO, forward, up)
}

#[test]
#[ignore = "needs a GPU; run with just test-gpu gpu_globe"]
fn gpu_globe_uniform_matches_wgsl_size() {
    assert_eq!(wgsl_struct_size("GlobeUniform"), 128);
    assert_eq!(wgsl_struct_size("GlobeUniform") as usize, std::mem::size_of::<GlobeUniform>());
    assert_eq!(wgsl_struct_size("TileInstance") as usize, std::mem::size_of::<super::gpu::TileInstance>());
}

#[test]
#[ignore = "needs a GPU; run with just test-gpu gpu_globe"]
fn gpu_globe_two_adjacent_faces_leave_no_crack_on_their_shared_edge() {
    // All six roots draw so the whole disc is covered; the eye sits over the PosX/PosY edge.
    let bisector = (Face::PosX.basis().n + Face::PosY.basis().n).as_dvec3().normalize();
    let distance = RADIUS_M + 60_000.0;
    let eye = bisector * distance;
    let tiles = (0..6).map(|f| TileData::empty(TileId::root(f))).collect();
    let fov = 36.0f32;
    let frame = render(tiles, eye, look(eye, DVec3::ZERO, fov), 256);
    let focal = frame.size as f64 * 0.5 / (f64::from(fov) * 0.5).to_radians().tan();
    let disc_px = focal * (RADIUS_M / distance).asin().tan();
    let c = f64::from(frame.size) * 0.5;
    let mut inside = 0;
    let mut background = 0;
    for (k, px) in frame.color.iter().enumerate() {
        let (x, y) = ((k as u32 % frame.size) as f64 + 0.5, (k as u32 / frame.size) as f64 + 0.5);
        if ((x - c).powi(2) + (y - c).powi(2)).sqrt() < disc_px - 2.0 {
            inside += 1;
            background += usize::from(*px == [255, 0, 255, 255]);
        }
    }
    assert!(inside > 10_000, "disc of {disc_px:.1} px covers {inside} pixels");
    assert_eq!(background, 0, "background pixels inside the disc");
}

#[test]
#[ignore = "needs a GPU; run with just test-gpu gpu_globe"]
fn gpu_globe_matches_cover_heights_at_a_probe_point() {
    let id = TileId { face: 2, level: 3, x: 4, y: 2 };
    let mut tile = TileData::empty(id);
    for j in 0..TILE_SAMPLES {
        for i in 0..TILE_SAMPLES {
            // Gentle relief: steep sawtooth would occlude the probe from the slanted eye.
            tile.height_m[(j * TILE_SAMPLES + i) as usize] = (300 + 4 * i + (i * j) / 8) as i16 - 3 * j as i16;
        }
    }
    let (i, j) = (20, 41);
    let h = f64::from(tile.height_m[(j * TILE_SAMPLES + i) as usize]);
    let st = id.sample_chart(i, j);
    let probe = chart::to_body(chart::join(Face::ALL[usize::from(id.face)], DVec3::new(st.x, h, st.y)));
    let up = probe.normalize();
    let side = up.cross(DVec3::Z).normalize();
    let eye = probe + (up * 0.95 + side * 0.3).normalize() * 50_000.0;
    // An odd size puts the centre pixel's sample exactly on the probe vertex.
    let frame = render(vec![tile], eye, look(eye, probe, 40.0), 257);
    let centre = (frame.size / 2 * frame.size + frame.size / 2) as usize;
    let depth = frame.depth[centre];
    assert!(depth > 0.0, "globe not drawn at the centre");
    let distance = f64::from(NEAR_M) / f64::from(depth);
    assert!((distance - 50_000.0).abs() < 2.0, "globe at {distance:.2} m, cover at 50000 m");
}

#[test]
fn body_view_proj_keeps_the_local_heading() {
    let eye_chart = DVec3::new(1200.0, 80.0, -300.0);
    let (eye_body, view_proj) = body_view_proj(eye_chart, Vec3::new(1.0, 0.0, 0.0), Mat4::IDENTITY);
    assert!((eye_body - chart::to_body(eye_chart)).length() < 1e-6);
    // A point 100 m east of the eye projects onto the view axis.
    let east = chart::to_body(eye_chart + DVec3::new(100.0, 0.0, 0.0)) - eye_body;
    let clip = view_proj * east.as_vec3().extend(1.0);
    assert!(clip.x.abs() < 0.5 && clip.y.abs() < 1.0 && clip.z < -99.0, "{clip:?}");
}
