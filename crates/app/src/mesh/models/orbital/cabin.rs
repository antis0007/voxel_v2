//! Express lift cabin and its passenger guide. Open upper panels preserve the
//! view, and painted surfaces keep the close-up trim distinct from the hull.
use super::{Model, Quat, Vec3, architecture::strut, palette};
use crate::world::World;
use crate::world::orbital::express::{self, DOCK_APPROACH_M, DOOR_HALF_M, GUIDE_OFFSET_M, GUIDE_SHOE_HALF_M,
    HALF_WIDTH_M, HEIGHT_M, SHELL_TRIM_M};
use crate::world::orbital::habitat::HATCH_X;
use engine_space::express::Phase;

const SLAB: f32 = SHELL_TRIM_M;
const RAIL_M: f32 = 1.05;
const AMBER: [f32; 3] = [0.95, 0.62, 0.18];
const GREEN: [f32; 3] = [0.25, 0.88, 0.64];

fn slab(mesh: &mut Model, at: Vec3, size: Vec3, colour: [f32; 3]) {
    mesh.with_surface_kind(super::super::AUTHORED_SURFACE,
        |m| m.cuboid(at, size, Quat::IDENTITY, colour, 0.0));
}

pub(super) fn build(mesh: &mut Model, world: &World, viewer: Vec3) {
    station_hatches(mesh, world, viewer);
    let Some(ride) = world.orbital.express.ride else { return };
    let floor = ride.cabin_m().as_vec3();
    if (viewer - floor).length() > 40_000.0 { return; }
    let w = HALF_WIDTH_M;
    let h = HEIGHT_M;
    let hull = palette::STEEL;
    let trim = palette::CERAMIC;
    let dark = palette::GRAPHITE;
    let guide = Vec3::NEG_Z * GUIDE_OFFSET_M;
    let bow = express::passenger_guide_bow(world);
    let length = ride.profile.length_m;
    let rail_at = |along| {
        ride.shaft_m(along).as_vec3() + guide + express::sway_m(bow, along, length)
    };
    let approach = (length - DOCK_APPROACH_M).max(0.0);
    for (start, end, sections) in [(0.0, approach, 16), (approach, length, 8)] {
        let step = (end - start) / f64::from(sections);
        for i in 0..sections {
            let along = start + f64::from(i) * step;
            strut(mesh, rail_at(along), rail_at(along + step), 0.28, hull);
        }
    }
    let shoe_width = GUIDE_SHOE_HALF_M * 2.0;
    slab(mesh, floor + guide + Vec3::Y * h * 0.5,
        Vec3::new(shoe_width, h, shoe_width), dark);
    for y in [0.25, h - 0.25] {
        // Yoke arms from the shoe to the cabin's -z pillars.
        for sx in [-1.0, 1.0] {
            strut(mesh, floor + guide + Vec3::new(sx * 0.38, y, 0.0), floor + Vec3::new(sx * w, y, -w), 0.25, hull);
        }
    }
    // Floor and roof.
    slab(mesh, floor - Vec3::Y * SLAB * 0.5, Vec3::new(2.0 * w, SLAB, 2.0 * w), dark);
    slab(mesh, floor + Vec3::Y * (h + SLAB * 0.5), Vec3::new(2.0 * w, SLAB, 2.0 * w), hull);
    // Corner pillars and knee panels below the rail; glazing above stays open.
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        slab(mesh, floor + Vec3::new(sx * w, h * 0.5, sz * w), Vec3::new(0.24, h, 0.24), hull);
    }
    // The +x panel splits round the door while the cabin is docked at the crown.
    let door_open = ride.ascending && ride.docked() && express::port_ready(world, &ride);
    for (axis, sign) in [(Vec3::X, -1.0), (Vec3::X, 1.0), (Vec3::Z, -1.0), (Vec3::Z, 1.0)] {
        let across = if axis == Vec3::X { Vec3::Z } else { Vec3::X };
        let centre = floor + axis * (sign * w);
        let door = axis == Vec3::X && sign > 0.0;
        let spans: &[(f32, f32)] = if door { &[(-w, -DOOR_HALF_M), (DOOR_HALF_M, w)] } else { &[(-w, w)] };
        for &(lo, hi) in spans {
            let mid = centre + across * (lo + hi) * 0.5;
            let panel = Vec3::splat(SLAB) * axis + across * (hi - lo);
            slab(mesh, mid + Vec3::Y * RAIL_M * 0.5, panel + Vec3::Y * RAIL_M, trim);
            strut(mesh, centre + Vec3::Y * RAIL_M + across * lo, centre + Vec3::Y * RAIL_M + across * hi, 0.05, palette::COPPER);
            let strip = across * (hi - lo - 0.15) + axis * 0.03 + Vec3::Y * 0.03;
            mesh.cuboid(mid - axis * (sign * SLAB) + Vec3::Y * 0.05, strip,
                Quat::IDENTITY, [0.96, 0.70, 0.32], 0.25);
        }
        if door && !door_open {
            for z in [-DOOR_HALF_M * 0.5, DOOR_HALF_M * 0.5] {
                slab(mesh, centre + Vec3::new(0.0, h * 0.5, z),
                    Vec3::new(SLAB, h, DOOR_HALF_M - 0.025), hull);
            }
        }
        strut(mesh, centre + Vec3::Y * (h - 0.05) - across * w, centre + Vec3::Y * (h - 0.05) + across * w, 0.06, hull);
    }
    // Door threshold: amber and graphite chevrons on the floor, lit only at the crown.
    if door_open {
        let gap = express::DOCK_SILL_GAP_M;
        slab(mesh, floor + Vec3::new(w + gap * 0.5, -SLAB * 0.5, 0.0),
            Vec3::new(gap, SLAB, 2.0 * DOOR_HALF_M), dark);
        let pitch = 2.0 * DOOR_HALF_M / 6.0;
        for i in 0..6 {
            let z = -DOOR_HALF_M + (i as f32 + 0.5) * pitch;
            let lit = i % 2 == 0;
            mesh.cuboid(floor + Vec3::new(w - 0.25, 0.012, z), Vec3::new(0.5, 0.02, pitch - 0.02), Quat::IDENTITY,
                if lit { AMBER } else { [0.12, 0.12, 0.13] }, if lit { 0.35 } else { 0.0 });
        }
        for z in [-DOOR_HALF_M, DOOR_HALF_M] {
            let jamb = floor + Vec3::new(w - 0.08, (h - 0.32) * 0.5, z);
            mesh.cuboid(jamb, Vec3::new(0.06, h - 0.32, 0.04), Quat::IDENTITY, GREEN, 0.45);
        }
        mesh.cuboid(floor + Vec3::new(w - 0.08, h - 0.32, 0.0), Vec3::new(0.06, 0.06, 2.0 * DOOR_HALF_M), Quat::IDENTITY, GREEN, 0.45);
    }
    // Ceiling lamp: the keel and the terminal hall both leave the cabin in shadow.
    mesh.cuboid(floor + Vec3::Y * (h - 0.05), Vec3::new(2.4, 0.05, 0.4), Quat::IDENTITY, [0.92, 0.88, 0.78], 0.7);
    // The instrument panel follows the lift phase.
    let console = floor + Vec3::new(w - 1.4, RAIL_M + 0.45, -(w - 0.2));
    slab(mesh, console, Vec3::new(1.6, 0.9, 0.16), trim);
    let lamp = match ride.phase() {
        Phase::Docked | Phase::Arriving | Phase::Departing => [0.32, 0.78, 0.52],
        Phase::Coasting => [0.86, 0.80, 0.42],
        Phase::Accelerating | Phase::Braking => [0.45, 0.72, 0.95],
    };
    mesh.cuboid(console + Vec3::new(0.0, 0.0, 0.1), Vec3::new(1.3, 0.6, 0.04), Quat::IDENTITY, lamp, 0.8);
}

fn station_hatches(mesh: &mut Model, world: &World, viewer: Vec3) {
    for h in &world.orbital.habitats.habitats {
        let origin = h.origin.as_vec3();
        if !h.ready || viewer.distance_squared(origin) > 96.0 * 96.0 { continue; }
        for (door, x) in HATCH_X.into_iter().enumerate() {
            let x = x as f32;
            let colour = if h.doors_open[door] { GREEN }
                else if h.cycle == door as i8 { AMBER }
                else { [0.86, 0.26, 0.15] };
            for face in [x - 0.015, x + 1.015] {
                mesh.cuboid(origin + Vec3::new(face, 2.72, 0.0),
                    Vec3::new(0.02, 0.07, 1.2), Quat::IDENTITY, colour, 0.42);
            }
        }
    }
}
