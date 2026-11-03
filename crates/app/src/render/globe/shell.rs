//! Cloud shell coverage bake, night-light splats and the volumetric march handoff.

use super::chart::{self, HALF_M, PITCH_M};
use crate::world::{CloudControls, World, coverage_at};
use engine_core::hash::hash01;
use glam::{DVec3, IVec2, Vec2};

/// Shell height above `chart::RADIUS_M`; `GLOBE_SHELL_M` in globe_shader.wgsl.
pub const SHELL_HEIGHT_M: f64 = 1_600.0;
pub const MAX_LIGHTS: usize = 256;
const ANCHORAGE_INTENSITY: f32 = 3.0;
const ANCHORAGE_RADIUS_M: f32 = 1_500.0;
const FACTORY_INTENSITY: f32 = 0.6;
const FACTORY_RADIUS_M: f32 = 400.0;
const NODES_PER_LIGHT: u32 = 64;
const MIN_CLUSTER_NODES: u32 = 8;
/// The march's own cutoff margin above the top slab.
const MARCH_MARGIN_M: f64 = 500.0;
/// `CLOUD_ADVECTION_SPEED_SCALE * cloud_layer_speed_mult(low)` in cloud_shader.wgsl.
const LOW_LAYER_WIND_MULT: f32 = 8.0 * 1.65;

/// Texels per shell face edge: 512, or 256 on the lean tier.
pub fn shell_resolution() -> u32 {
    if crate::render::lean_shading() { 256 } else { 512 }
}

/// Inputs of one shell face bake.
#[derive(Clone, Copy, Debug)]
pub struct ShellConfig {
    pub resolution: u32,
    pub controls: CloudControls,
    pub weather_seed: u32,
}

/// One night light; 32 bytes, `GlobeLight` in globe_shader.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Light {
    pub body: [f32; 3],
    pub radius_m: f32,
    pub intensity: f32,
    pub pad: [f32; 3],
}
const _: [(); 32] = [(); std::mem::size_of::<Light>()];

/// World-space offset the low cloud deck has drifted by at `time_s`, as the march applies it.
pub fn wind_offset(controls: &CloudControls, time_s: f32) -> Vec2 {
    let angle = controls.wind_dir_angle.to_radians();
    Vec2::new(angle.cos(), angle.sin()) * time_s * controls.advection * controls.wind_speed.max(0.0) * LOW_LAYER_WIND_MULT
}

/// Chart metres of texel centre `i` on a face edge of `resolution` texels.
pub(crate) fn texel_chart_m(i: u32, resolution: u32) -> f64 {
    -HALF_M + (f64::from(i) + 0.5) * 2.0 * HALF_M / f64::from(resolution)
}

/// Coverage 0..255 for one face, rows along chart z; chart metres equal world metres.
pub fn bake_shell(cfg: &ShellConfig, time_s: f32, face: u8) -> Vec<u8> {
    let res = cfg.resolution;
    let drift = wind_offset(&cfg.controls, time_s);
    let face_x = f64::from(face) * PITCH_M;
    let mut out = Vec::with_capacity((res * res) as usize);
    for j in 0..res {
        let z = texel_chart_m(j, res) as f32;
        for i in 0..res {
            let x = (face_x + texel_chart_m(i, res)) as f32;
            let c = coverage_at(Vec2::new(x, z) + drift, cfg.controls, cfg.weather_seed);
            out.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    out
}

/// Top of the highest cloud slab in metres; mirrors `cloud_layer_bounds().high_top`.
pub fn cloud_top_slab_m(controls: &CloudControls) -> f64 {
    let thick = f64::from(controls.layer_thickness.clamp(0.0, 1.0));
    let high = f64::from(controls.layer_separation.clamp(0.0, 1.0));
    let high_base_center = 380.0 + (550.0 - 380.0) * high;
    let high_depth_max = 90.0 + (240.0 - 90.0) * thick;
    high_base_center - 20.0 + high_depth_max + 2.0 + 7_000.0
}

/// Whether the volumetric cloud march runs; above the top slab the shell takes over.
pub fn volumetric_clouds_enabled(altitude_m: f64, top_slab_m: f64) -> bool {
    altitude_m <= top_slab_m + MARCH_MARGIN_M
}

/// Intensity and radius of the anchorage at integer chart cell `site`; hashed so a grid of sites does not read as one.
pub fn site_light(site: IVec2) -> (f32, f32) {
    let key = (site.x as u32).wrapping_mul(0x9e37_79b1) ^ (site.y as u32);
    let intensity = ANCHORAGE_INTENSITY * (0.5 + 0.5 * hash01(key));
    (intensity, ANCHORAGE_RADIUS_M * (0.6 + 0.4 * hash01(key ^ 0x9e37)))
}

/// Caps to `MAX_LIGHTS`: strongest first, then nearest `eye_body`, so equal lights favour the view.
/// Sites carry their chart cell for `site_light`. A chunk with fewer than `MIN_CLUSTER_NODES` nodes
/// gets no light; larger ones get one light of `0.6 * ceil(n / 64)`.
pub fn night_lights_from(sites: &[(DVec3, IVec2)], factory_clusters: &[(DVec3, u32)], eye_body: DVec3) -> Vec<Light> {
    let mut out: Vec<(f64, Light)> = Vec::with_capacity(sites.len() + factory_clusters.len());
    let mut push = |body: DVec3, radius_m, intensity| {
        let b = body.as_vec3();
        out.push((body.distance_squared(eye_body), Light { body: [b.x, b.y, b.z], radius_m, intensity, pad: [0.0; 3] }));
    };
    for (body, site) in sites {
        let (intensity, radius_m) = site_light(*site);
        push(*body, radius_m, intensity);
    }
    for (body, nodes) in factory_clusters.iter().filter(|(_, n)| *n >= MIN_CLUSTER_NODES) {
        push(*body, FACTORY_RADIUS_M, FACTORY_INTENSITY * nodes.div_ceil(NODES_PER_LIGHT) as f32);
    }
    out.sort_by(|a, b| b.1.intensity.total_cmp(&a.1.intensity).then(a.0.total_cmp(&b.0)));
    out.truncate(MAX_LIGHTS);
    out.into_iter().map(|(_, light)| light).collect()
}

/// Chart point to body space, or `None` outside the six faces.
fn body_of(world_x: f64, world_z: f64) -> Option<DVec3> {
    let (_, local) = chart::split(DVec3::new(world_x, 0.0, world_z));
    (local.x.abs() <= HALF_M && local.z.abs() <= HALF_M).then(|| chart::to_body(DVec3::new(world_x, 0.0, world_z)))
}

/// Anchorage sites over the whole chart and factory nodes clustered by chunk.
pub fn night_lights(world: &World, eye_body: DVec3) -> Vec<Light> {
    let cell = f64::from(crate::world::orbital::CELL_M);
    let (x0, x1) = ((-PITCH_M * 0.5 / cell).floor() as i32, ((5.5 * PITCH_M) / cell).ceil() as i32);
    let z_cells = (HALF_M / cell).ceil() as i32;
    let mut sites = Vec::new();
    for cz in -z_cells..=z_cells {
        for cx in x0..=x1 {
            let Some(site) = crate::world::orbital::site(world.seed, glam::IVec2::new(cx, cz)) else { continue };
            if let Some(body) = body_of(f64::from(site.x), f64::from(site.z)) {
                sites.push((body, glam::IVec2::new(cx, cz)));
            }
        }
    }
    let metres = f64::from(world.voxel_scale.meters());
    let mut chunks: std::collections::BTreeMap<[i32; 3], u32> = std::collections::BTreeMap::new();
    for pos in world.factory.nodes.keys() {
        let chunk = [pos.x.div_euclid(32), pos.y.div_euclid(32), pos.z.div_euclid(32)];
        *chunks.entry(chunk).or_default() += 1;
    }
    let clusters: Vec<(DVec3, u32)> = chunks
        .into_iter()
        .filter_map(|(c, n)| {
            let centre = (glam::IVec3::from(c).as_dvec3() + 0.5) * 32.0 * metres;
            body_of(centre.x, centre.z).map(|b| (b, n))
        })
        .collect();
    night_lights_from(&sites, &clusters, eye_body)
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
