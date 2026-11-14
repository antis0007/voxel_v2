//! Portal windows in the primary view: plan bounded destination views, render
//! each from its virtual eye into a pooled target, then composite with the veil.
mod composite;
mod scene;

use super::origin::RenderOrigin;
use super::streaming::{ChunkCache, ChunkKey, chunk_bounds_sphere, extract_planes};
use super::{CameraUniform, render_distance_settings};
use crate::world::portals::views::{Camera, ClipPlane, Plan, View};
use crate::world::portals::{Network, gates};
use engine_core::CHUNK_SIZE;
use glam::{DVec3, IVec3, Mat3, Mat4, Vec3};

struct Root {
    uniform: CameraUniform,
    camera: Camera,
    scale: f32,
    origin: RenderOrigin,
}

pub struct Stage {
    composite: composite::Renderer,
    root: Option<Root>,
    network: Network,
    revision: Option<u64>,
    plan: Plan,
    /// Per view index; index 0 (the primary view) stays empty.
    draws: Vec<Vec<ChunkKey>>,
    /// Openings without a rendered destination: parent view, aperture, kind.
    unready: Vec<(usize, [[f32; 3]; 4], u32)>,
    /// Openings into a realm that is not loaded; drawn as its vista.
    realm_gates: Vec<crate::world::portals::realm::Gate>,
}

impl Stage {
    pub fn new(device: &wgpu::Device, camera_layout: wgpu::BindGroupLayout) -> Self {
        Self {
            composite: composite::Renderer::new(device, camera_layout),
            root: None,
            network: Network::default(),
            revision: None,
            plan: Plan::default(),
            draws: Vec::new(),
            unready: Vec::new(),
            realm_gates: Vec::new(),
        }
    }

    /// Called with each primary camera upload; `projection` is reverse-Z.
    pub(super) fn set_root(&mut self, uniform: CameraUniform, eye: DVec3, forward: Vec3, up: Vec3,
        projection: Mat4, scale: f32, origin: RenderOrigin) {
        let right = forward.cross(up).normalize_or_zero();
        let rotation = glam::Quat::from_mat3(&Mat3::from_cols(right, right.cross(forward), -forward));
        if !rotation.is_finite() || right == Vec3::ZERO {
            self.root = None;
            return;
        }
        let camera = Camera {
            realm: gates::realm(),
            eye,
            rotation: rotation.normalize().as_dquat(),
            projection: projection.as_dmat4(),
        };
        self.root = Some(Root { uniform, camera, scale, origin });
    }

    /// Fresh worlds can share revision zero while their compiled layouts differ.
    /// Compare the bounded network too; saved-gate revisions do not own those links.
    pub fn set_network(&mut self, network: &Network, revision: u64) {
        if self.revision != Some(revision) || self.network != *network {
            self.network = network.clone();
            self.revision = Some(revision);
        }
    }

    /// Gates to another realm, front faces toward where they are seen from.
    pub fn set_realm_gates(&mut self, gates: Vec<crate::world::portals::realm::Gate>) {
        self.realm_gates = gates;
    }

    pub fn active(&self) -> bool {
        (!self.network.portals.is_empty() || !self.realm_gates.is_empty()) && self.root.is_some() && portals_enabled()
    }

    /// Build this frame's plan and draw lists. Returns the chunk coordinates
    /// that need draw-offset slots before the offset table uploads.
    pub(super) fn plan(&mut self, device: &wgpu::Device, cache: &ChunkCache, size: [u32; 2]) -> Vec<IVec3> {
        self.plan = Plan::default();
        self.draws.clear();
        self.unready.clear();
        if !self.active() {
            return Vec::new();
        }
        let Some(root) = &self.root else { return Vec::new() };
        let scale = f64::from(root.scale);
        let resident = |p: DVec3| {
            let chunk = (p / scale).floor().as_ivec3().div_euclid(IVec3::splat(CHUNK_SIZE));
            cache.gpu_meshes.contains_key(&chunk) || cache.published_coords.contains(&chunk)
        };
        self.plan = Plan::build(&self.network, root.camera.clone(), size, |_, _, exit| resident(exit.center));
        // An opening without a window (destination not resident, too deep or
        // over budget) still shows the veil, never a bare hole in the frame.
        for (index, view) in self.plan.views.iter().enumerate() {
            let through = view.portal_id.and_then(|id| self.network.portals.iter().find(|p| p.id == id));
            for portal in self.network.portals.iter().filter(|p| p.source == view.camera.realm)
                .filter(|p| through.is_none_or(|t| !crate::world::portals::views::coincides(p, t)))
            {
                let windowed = self.plan.views.iter().any(|c| c.parent == Some(index) && c.portal_id == Some(portal.id));
                if !windowed && portal.entrance.clip_distance(view.camera.eye) > 0.0
                    && view.clip_planes.iter().all(|c| portal.entrance.corners().iter().any(|&p| (p - c.point).dot(c.normal) >= 0.0))
                {
                    let corners = portal.entrance.corners().map(|p| (p - view.camera.eye).as_vec3().to_array());
                    self.unready.push((index, corners, composite::MIST));
                }
            }
            for gate in &self.realm_gates {
                let frame = gate.frame;
                let eye = view.camera.eye;
                let corners = frame.corners();
                if frame.clip_distance(eye) <= 0.0 || frame.center.distance(eye) >= 96.0
                    || !view.clip_planes.iter().all(|plane| corners.iter().any(|&p| {
                        (p - plane.point).dot(plane.normal) >= 0.0
                    }))
                {
                    continue;
                }
                let corners = corners.map(|p| (p - eye).as_vec3().to_array());
                let kind = if gate.hell {
                    composite::HELL_MIST
                } else if gate.vista {
                    composite::VISTA
                } else {
                    composite::REALM_MIST
                };
                self.unready.push((index, corners, kind));
            }
        }
        if let Err(e) = self.composite.prepare(device, &self.plan, std::mem::size_of::<CameraUniform>()) {
            eprintln!("[portals] plan rejected: {e:#}");
            self.plan = Plan::default();
            return Vec::new();
        }
        let mut coords = Vec::new();
        self.draws.push(Vec::new());
        for view in self.plan.views.iter().skip(1) {
            let Some(exit) = exit_plane(&self.network, view) else { self.draws.push(Vec::new()); continue };
            let list = draw_list(cache, view, exit, root.scale);
            coords.extend(list.iter().map(|k| k.coord));
            self.draws.push(list);
        }
        coords
    }

    /// Camera uniform for a destination view, derived from the primary one so
    /// lighting, weather and coverage terms match.
    pub(super) fn camera_uniform(&self, view: &View) -> Option<CameraUniform> {
        let root = self.root.as_ref()?;
        let camera = &view.camera;
        let eye = camera.eye.as_vec3();
        let residual = (camera.eye - eye.as_dvec3()).as_vec3();
        let rotation = camera.rotation.as_quat();
        let projection = camera.projection.as_mat4();
        let view_proj = projection * Mat4::from_quat(rotation.conjugate()) * Mat4::from_translation(-residual);
        let inv_view_proj = Mat4::from_translation(eye) * view_proj.inverse();
        let forward = rotation * Vec3::NEG_Z;
        let up = rotation * Vec3::Y;
        let right = forward.cross(up).normalize() / projection.x_axis.x;
        let ray_up = right.normalize().cross(forward) / projection.y_axis.y;
        let plane = exit_plane(&self.network, view)?;
        let mut u = root.uniform;
        u.view_proj = view_proj.to_cols_array_2d();
        u.prev_view_proj = u.view_proj;
        u.inv_view_proj = inv_view_proj.to_cols_array_2d();
        u.camera_pos = [eye.x, eye.y, eye.z, 0.0];
        u.previous_camera_pos = [eye.x, eye.y, eye.z, u.previous_camera_pos[3]];
        u.camera_forward = [forward.x, forward.y, forward.z, u.camera_forward[3]];
        u.ray_right = [right.x, right.y, right.z, projection.z_axis.z];
        u.ray_up = [ray_up.x, ray_up.y, ray_up.z, projection.w_axis.z];
        u.world_params[2] = 0.0;
        u.eye_from_origin = root.origin.eye_uniform(camera.eye, f64::from(root.scale), residual).extend(0.0).to_array();
        u.portal_clip = plane.relative_to(camera.eye);
        Some(u)
    }
}

/// `VOXEL_PORTALS=0` keeps gates and crossings but draws no windows (A/B).
pub fn portals_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("VOXEL_PORTALS").as_deref() != Ok("0"))
}

/// The exit plane of the portal a view looks through. `View::clip_planes`
/// also holds the inherited and aperture side planes, in that order.
fn exit_plane(network: &Network, view: &View) -> Option<ClipPlane> {
    let portal = network.portals.iter().find(|p| Some(p.id) == view.portal_id)?;
    Some(ClipPlane { point: portal.exit.center, normal: portal.exit.normal() })
}

/// Veil strength, 0 to 1; `VOXEL_PORTAL_VEIL=0` shows the bare window (A/B).
pub fn veil_strength() -> f32 {
    static STRENGTH: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *STRENGTH.get_or_init(|| {
        std::env::var("VOXEL_PORTAL_VEIL").ok().and_then(|v| v.parse::<f32>().ok()).map_or(1.0, |v| v.clamp(0.0, 1.0))
    })
}

/// Published chunks in the virtual frustum and in front of the exit plane.
fn draw_list(cache: &ChunkCache, view: &View, exit: ClipPlane, scale: f32) -> Vec<ChunkKey> {
    let camera = &view.camera;
    let eye = (camera.eye / f64::from(scale)).as_vec3();
    let rotation = camera.rotation.as_quat();
    let p = camera.projection;
    let fov = 2.0 * (1.0 / p.y_axis.y).atan();
    let aspect = p.y_axis.y / p.x_axis.x;
    let far = (render_distance_settings().view_distance_chunks as f32 * CHUNK_SIZE as f32 * 1.2).max(640.0);
    let cull = glam::camera::rh::proj::directx::perspective(fov as f32, aspect as f32, 0.05, far)
        * glam::camera::rh::view::look_to_mat4(eye, rotation * Vec3::NEG_Z, rotation * Vec3::Y);
    let planes = extract_planes(cull);
    let exit_point = (exit.point / f64::from(scale)).as_vec3();
    let exit_normal = exit.normal.as_vec3();
    // Same ownership rules as the primary list: whole published columns only,
    // so a partial column never overlaps the horizon parent it suppresses.
    let coverage = cache.resident_coverage(Vec3::ZERO, render_distance_settings().view_distance_chunks, 1.0);
    let mut complete = std::collections::HashMap::new();
    for q in &cache.published_coords {
        let ready = cache.gpu_meshes.contains_key(q);
        complete.entry((q.x, q.z)).and_modify(|v| *v &= ready).or_insert(ready);
    }
    let mut out: Vec<ChunkKey> = cache
        .gpu_meshes
        .iter()
        .filter(|(coord, mesh)| mesh.index_count != 0 && cache.published_coords.contains(*coord)
            && complete.get(&(coord.x, coord.z)) == Some(&true)
            && coverage.contains_column(coord.x, coord.z))
        .filter(|(coord, _)| {
            let (center, radius) = chunk_bounds_sphere(**coord);
            planes[..4].iter().all(|pl| pl.truncate().dot(center) + pl.w >= -radius)
                && (center - exit_point).dot(exit_normal) >= -radius
        })
        .map(|(&coord, mesh)| ChunkKey { coord, lod: mesh.lod })
        .collect();
    let forward = rotation * Vec3::NEG_Z;
    out.sort_unstable_by(|a, b| forward.dot(a.coord.as_vec3()).total_cmp(&forward.dot(b.coord.as_vec3()))
        .then_with(|| (a.coord.x, a.coord.y, a.coord.z).cmp(&(b.coord.x, b.coord.y, b.coord.z))));
    out
}
