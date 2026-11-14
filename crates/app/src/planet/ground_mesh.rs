//! Terrain mesh of the foreign body the player is anchored to, around the player.
//!
//! The patch is built in body axes around a snapped centre and cached; each
//! frame only moves it into the eye's chart frame. Beyond its rim the body's
//! sky disc stands in.
use super::influence::{self, Frame};
use crate::mesh::{Vertex3, models::AUTHORED_SURFACE};
use engine_core::cube_sphere::atlas;
use glam::{DVec3, IVec3};
use std::cell::RefCell;

const SEGMENTS: usize = 96;
const FIRST_RING_M: f64 = 0.75;
const RING_GROWTH: f64 = 1.1;
/// Centre snap of the cached patch, metres.
const SNAP_M: f64 = 2.0;
const MIN_REACH_M: f64 = 400.0;
const MAX_REACH_M: f64 = 8_000.0;
/// Regolith grey, sRGB.
const REGOLITH: [f32; 3] = [0.50, 0.49, 0.47];

struct Patch {
    key: (u32, u16, IVec3, i32),
    /// Body-axes points: the centre, then `SEGMENTS` per ring.
    points: Vec<DVec3>,
    normals: Vec<DVec3>,
    tones: Vec<f32>,
    rings: usize,
}

thread_local! {
    static PATCH: RefCell<Option<Patch>> = const { RefCell::new(None) };
}

fn ring_radii(reach: f64) -> Vec<f64> {
    let mut radii = vec![0.0];
    let mut r = FIRST_RING_M;
    while r < reach {
        radii.push(r);
        r *= RING_GROWTH;
    }
    radii.push(reach);
    radii
}

fn build(seed: u32, body: engine_space::BodyId, centre: DVec3, reach: f64, key: (u32, u16, IVec3, i32)) -> Patch {
    let radius = influence::body_radius(seed, body).max(1.0);
    let side = if centre.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let t1 = centre.cross(side).normalize();
    let t2 = centre.cross(t1);
    let radii = ring_radii(reach);
    let mut points = Vec::with_capacity(1 + (radii.len() - 1) * SEGMENTS);
    let ground = |dir: DVec3| dir * influence::ground_radius(seed, body, dir);
    points.push(ground(centre));
    for &r in &radii[1..] {
        let theta = (r / radius).min(std::f64::consts::PI * 0.9);
        for j in 0..SEGMENTS {
            let phi = j as f64 / SEGMENTS as f64 * std::f64::consts::TAU;
            let dir = centre * theta.cos() + (t1 * phi.cos() + t2 * phi.sin()) * theta.sin();
            points.push(ground(dir));
        }
    }
    let rings = radii.len() - 1;
    let at = |ring: usize, j: usize| if ring == 0 { 0 } else { 1 + (ring - 1) * SEGMENTS + j % SEGMENTS };
    let mut normals = vec![centre; points.len()];
    for ring in 1..=rings {
        for j in 0..SEGMENTS {
            let along = points[at(ring, j + 1)] - points[at(ring, j + SEGMENTS - 1)];
            let across = points[at((ring + 1).min(rings), j)] - points[at(ring - 1, j)];
            let n = across.cross(along).normalize_or(DVec3::ZERO);
            let outward = points[at(ring, j)].normalize();
            normals[at(ring, j)] = if n.dot(outward) < 0.0 { -n } else { n };
        }
    }
    let tones = points
        .iter()
        .map(|p| {
            let h = p.length() - radius;
            (0.92 + (h / 300.0) as f32).clamp(0.72, 1.12)
        })
        .collect();
    Patch { key, points, normals, tones, rings }
}

/// Appends the anchored body's ground around the eye to the model scene.
pub fn draw(vertices: &mut Vec<Vertex3>, world: &crate::world::World, player: &crate::player::Player) {
    let Some(anchor) = player.anchor.filter(|a| influence::within_sphere(world, a)) else { return };
    let Some(frame) = Frame::for_world(world, anchor.body, world.orbital.planet_tick) else { return };
    let scale = f64::from(world.voxel_scale.meters());
    let eye_atlas = player.precise_eye(world.voxel_scale) * scale;
    let eye = frame.to_body(eye_atlas);
    let dir = anchor.pos.normalize_or(DVec3::Y);
    let ground = influence::ground_radius(world.seed, anchor.body, dir);
    let altitude = (eye.length() - ground).max(0.0);
    let horizon = (2.0 * ground * altitude + altitude * altitude).sqrt() + 300.0;
    let reach_step = (horizon.clamp(MIN_REACH_M, MAX_REACH_M).ln() / 1.25f64.ln()).ceil() as i32;
    let reach = 1.25f64.powi(reach_step).min(MAX_REACH_M)
        .min(influence::body_radius(world.seed, anchor.body) * std::f64::consts::PI * 0.9);
    let snapped = (dir * ground / SNAP_M).round().as_ivec3();
    let key = (world.seed, anchor.body.0, snapped, reach_step);
    let axes = frame.axes(eye_atlas);
    let palette = if world.sample_config().is_hell() { [0.45, 0.32, 0.29] } else { REGOLITH };
    // The shader lowers props by the home tangent-plane drop; adding it here keeps this ground on its own sphere.
    let lift = 1.0 / (2.0 * atlas::RADIUS_M);
    PATCH.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.as_ref().is_none_or(|p| p.key != key) {
            let centre = (snapped.as_dvec3() * SNAP_M).normalize_or(dir);
            *cell = Some(build(world.seed, anchor.body, centre, reach, key));
        }
        let Some(patch) = cell.as_ref() else { return };
        let vertex = |i: usize| {
            let mut rel = axes * (patch.points[i] - eye);
            rel.y += (rel.x * rel.x + rel.z * rel.z) * lift;
            let tone = patch.tones[i];
            Vertex3 {
                pos: ((eye_atlas + rel) / scale).as_vec3().to_array(),
                color: palette.map(|c| c * tone),
                normal: (axes * patch.normals[i]).as_vec3().to_array(),
                surface: [0.48, 0.42, AUTHORED_SURFACE, 0.0],
                ao_strength: 1.0,
            }
        };
        let at = |ring: usize, j: usize| if ring == 0 { 0 } else { 1 + (ring - 1) * SEGMENTS + j % SEGMENTS };
        let mut triangle = |a: usize, b: usize, c: usize| {
            let (pa, pb, pc) = (patch.points[a], patch.points[b], patch.points[c]);
            let (b, c) = if (pb - pa).cross(pc - pa).dot(pa) < 0.0 { (c, b) } else { (b, c) };
            vertices.extend([vertex(a), vertex(b), vertex(c)]);
        };
        for ring in 1..=patch.rings {
            for j in 0..SEGMENTS {
                if ring == 1 {
                    triangle(0, at(1, j), at(1, j + 1));
                    continue;
                }
                let (a, b, c, d) = (at(ring - 1, j), at(ring - 1, j + 1), at(ring, j), at(ring, j + 1));
                triangle(a, c, d);
                triangle(a, d, b);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_grow_to_the_reach() {
        let radii = ring_radii(1_000.0);
        assert_eq!(radii[0], 0.0);
        assert_eq!(*radii.last().unwrap(), 1_000.0);
        assert!(radii.windows(2).all(|w| w[1] > w[0]));
    }

    #[test]
    fn patch_normals_face_outward_and_points_sit_on_the_ground() {
        let moon = influence::system().unwrap().id("moon").unwrap();
        let centre = DVec3::new(0.3, 0.9, 0.2).normalize();
        let patch = build(5, moon, centre, 600.0, (5, moon.0, IVec3::ZERO, 0));
        for (p, n) in patch.points.iter().zip(&patch.normals) {
            assert!(n.dot(p.normalize()) > 0.2, "normal {n} at {p}");
            let ground = influence::ground_radius(5, moon, p.normalize());
            assert!((p.length() - ground).abs() < 1e-6);
        }
    }
}
