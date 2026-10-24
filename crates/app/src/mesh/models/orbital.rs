//! Bounded world-space orbital silhouettes; no camera-facing cards or terrain
//! chunks are allocated for the kilometre-long suspension braid.
use super::Model;
use crate::world::{
    DayNightCycle, World,
    orbital::{self, Class},
};
use glam::{Quat, Vec3};
mod architecture;
mod carriage;
mod dish;
mod megastructures;
mod skyhook;
mod meteors;
use architecture::{part, strut};

pub(super) fn build(
    mesh: &mut Model,
    world: &World,
    day: &DayNightCycle,
    viewer: Vec3,
    pixel_angle: f32,
) {
    // Matched terrain/lighting reviews can exclude independent scene traffic.
    // Restrict the override to explicit benchmark invocations so a leftover
    // environment variable cannot remove infrastructure from normal play.
    static REVIEW_DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *REVIEW_DISABLED.get_or_init(|| {
        std::env::var("VOXEL_REVIEW_NO_ORBITAL").as_deref() == Ok("1")
            && std::env::args().any(|arg| arg.starts_with("--benchmark-"))
    }) {
        return;
    }
    let cfg = world.sample_config();
    receiver(mesh, world, day, viewer, pixel_angle);
    dish::build(mesh,world,day,viewer);
    megastructures::build(mesh,world,day,viewer);
    skyhook::build(mesh, world, day, viewer, pixel_angle);
    meteors::build(mesh, world, day, viewer, pixel_angle);
    for contact in orbital::visual_contacts(world.seed, day, &world.orbital, viewer) {
        let service = contact.service;
        // Between visits the carriage remains high on the same persistent braid.
        let site = service.site;
        let ground = orbital::hold_height(&cfg, &world.orbital, site);
        let top = Vec3::new(site.x + 650.0, ground + orbital::UPPER_ANCHORAGE_OFFSET_M, site.z + 260.0);
        let foot = Vec3::new(site.x, ground, site.z);
        // `ground` is the carriage's hold height, `HOLD_CLEARANCE_M` above the
        // terrain. The braid and its plinth stand on the terrain itself, or the
        // corridor reads as hanging free of the ground.
        let footing = Vec3::new(site.x, ground - orbital::HOLD_CLEARANCE_M, site.z);
        let mark = mesh.mark();
        let hull = [0.26, 0.31, 0.34];
        let signal = if service.requested {
            [0.41, 0.83, 0.76]
        } else {
            [0.91, 0.60, 0.26]
        };
        let p = service.carriage_position(foot, top);
        // Beyond sight range a corridor is its braid and carriage only: a thin
        // line from the ground to the crown, with the carriage as a lit block that
        // slides along it. Crowns keep their full upper structure below.
        if contact.distance_m > orbital::SIGHT_RANGE_M && !matches!(site.class, Class::Conservatory) {
            // Keep the braid about 1.5 px wide and the carriage about 5 px across, or
            // a 3 m cable at kilometres is a sub-pixel streak the eye never finds.
            let reach = contact.distance_m.max(1.0);
            let width = (if matches!(site.class, Class::Freight) { 6.0_f32 } else { 3.0 }).max(reach * pixel_angle * 1.5);
            tether(mesh, footing, top, viewer, pixel_angle, [0.58, 0.55, 0.47], width);
            part(mesh, p, Vec3::splat((site.class.width() * 1.5).max(reach * pixel_angle * 5.0)), Quat::IDENTITY, signal, 0.8);
            megastructures::long_range_since(mesh, mark);
            continue;
        }
        let joint = p + Vec3::Y * site.class.width() * 0.71;
        let suspension_axis = (top - joint).normalize();
        architecture::anchorage(mesh, top, suspension_axis, site.class, site.key, day);
        if matches!(site.class,Class::Conservatory) {
            megastructures::long_range_since(mesh,mark);
        }
        if contact.distance_m>orbital::SIGHT_RANGE_M {continue;}
        // A reeled suspension line terminates at the moving joint. A fixed
        // ground cable would visibly detach from a carriage on this wide arc.
        let terminal = top - suspension_axis * site.class.upper_ring_radius_m() * 0.65;
        tether(mesh, joint, terminal, viewer, pixel_angle, hull, 1.2);
        // Lower braid: from the carriage joint down to the plinth on the ground.
        let lower = (contact.distance_m * pixel_angle).max(2.0);
        tether(mesh, footing, joint, viewer, pixel_angle, [0.58, 0.55, 0.47], lower);
        part(mesh, footing + Vec3::Y * 8.0, Vec3::new(site.class.width() * 2.5, 16.0, site.class.width() * 2.5), Quat::IDENTITY, hull, 0.5);
        carriage::build(mesh, p, suspension_axis, service, day, signal);
        // Family 62 owns local UVs; older pieces reserve surface.y for coverage.
        // The world shader
        // still applies atmosphere, weather and terrain occlusion, but excludes
        // these objects from the nearby terrain streaming-radius fade.
        for vertex in &mut mesh.vertices[mark..] {
            if vertex.surface[2] != 62.0 && vertex.surface[2] != 63.0 && vertex.surface[1] != 3.0 {
                vertex.surface[1] = 2.0;
            }
        }
    }
}

/// A subpixel cable needs filtered coverage, otherwise rasterization produces
/// a dotted line. The centreline stays world-anchored; only its raster support
/// expands to two pixels. Coverage restores its physical 1.2 m width in HDR.
fn tether(
    mesh: &mut Model,
    a: Vec3,
    b: Vec3,
    viewer: Vec3,
    pixel: f32,
    color: [f32; 3],
    width: f32,
) {
    let axis = (b - a).normalize();
    let side = axis.cross((a + b) * 0.5 - viewer).normalize_or_zero();
    if side.length_squared() < 0.5 {
        return;
    }
    let normal = axis.cross(side);
    mesh.set_surface_kind(61.0);
    for i in 0..24 {
        let lo = a.lerp(b, i as f32 / 24.0);
        let hi = a.lerp(b, (i + 1) as f32 / 24.0);
        let wl = width.max((lo - viewer).length() * pixel * 2.0);
        let wh = width.max((hi - viewer).length() * pixel * 2.0);
        let mark = mesh.mark();
        mesh.triangle(
            [
                lo - side * wl * 0.5,
                hi - side * wh * 0.5,
                hi + side * wh * 0.5,
            ],
            normal,
            color,
            0.0,
        );
        mesh.triangle(
            [
                lo - side * wl * 0.5,
                hi + side * wh * 0.5,
                lo + side * wl * 0.5,
            ],
            normal,
            color,
            0.0,
        );
        for vertex in &mut mesh.vertices[mark..] {
            let pos = Vec3::from_array(vertex.pos) * mesh.scale;
            let t = ((pos - lo).dot(axis) / (hi - lo).length()).clamp(0.0, 1.0);
            vertex.surface[3] = (width / wl) * (1.0 - t) + (width / wh) * t;
        }
    }
    mesh.set_surface_kind(41.0);
}

fn receiver(mesh: &mut Model, world: &World, day: &DayNightCycle, viewer: Vec3, pixel: f32) {
    use orbital::receiver as station;
    let Some(p) = station::position(world) else {
        return;
    };
    if (p - viewer).length_squared() > 512.0 * 512.0 {
        return;
    }
    let a = world.orbital.receiver.anchor.unwrap();
    // Presentation must never synchronously read an unloaded region from disk.
    let present = world
        .chunks
        .get(&crate::world::chunk_coord_from_world_block(a))
        .is_some_and(|c| {
            c.blocks[crate::world::ChunkData::local_index(a.x, a.y, a.z)]
                == crate::world::BlockId::Workbench
        });
    if !present {
        return;
    }
    let mark = mesh.mark();
    let metal = [0.26, 0.31, 0.34];
    let ceramic = [0.76, 0.73, 0.63];
    // The actual workbench console and stone footing are editable world blocks.
    // These small fittings distinguish the registered station from a workbench.
    {
        mesh.cuboid(
            p + Vec3::Y * 0.54,
            Vec3::new(0.8, 0.12, 0.8),
            Quat::IDENTITY,
            metal,
            0.0,
        );
        mesh.cuboid(
            p + Vec3::new(0.0, 0.62, -0.25),
            Vec3::new(0.48, 0.08, 0.15),
            Quat::IDENTITY,
            if world.orbital.receiver.stage == 2 {
                [0.4, 0.7, 0.5]
            } else {
                [0.3, 0.75, 0.8]
            },
            0.7,
        );
        mesh.cuboid(
            p + Vec3::new(0.65, 1.0, 0.6),
            Vec3::new(0.12, 2.1, 0.12),
            Quat::IDENTITY,
            metal,
            0.0,
        );
        mesh.cuboid(
            p + Vec3::new(0.65, 2.05, 0.6),
            Vec3::new(1.0, 0.12, 0.75),
            Quat::from_rotation_z(0.25),
            ceramic,
            0.0,
        );
    }
    if let Some((upper, pad, progress)) = station::capsule(world, day) {
        let t = progress * progress * (3.0 - 2.0 * progress);
        let sway = (std::f32::consts::PI * progress).sin().powi(2)
            * (progress * std::f32::consts::TAU * 4.0).sin() * (-2.0 * progress).exp() * 0.45;
        let at = upper.lerp(pad, t) + Vec3::X * sway;
        if progress < 1.0 {
            tether(mesh, upper, at + Vec3::Y * 0.65, viewer, pixel, metal, 0.08);
        }
        let cargo = mesh.mark();
        part(mesh, at, Vec3::new(1.2, 1.2, 1.2), Quat::IDENTITY, ceramic, 0.0);
        for x in [-0.38, 0.38] {
            part(mesh,
                at + Vec3::X * x,
                Vec3::new(0.08, 1.24, 1.24),
                Quat::IDENTITY,
                metal,
                0.5,
            );
        }
        mesh.cuboid(
            at + Vec3::new(0.0, 0.0, 0.61),
            Vec3::new(0.35, 0.12, 0.04),
            Quat::IDENTITY,
            [0.3, 0.75, 0.8],
            0.7,
        );
        mesh.rotate_since(cargo, at, Quat::from_rotation_y((1.0 - t) * 0.7)
            * Quat::from_rotation_z(sway * 0.08));
    }
    for vertex in &mut mesh.vertices[mark..] {
        if vertex.surface[2] != 62.0 { vertex.surface[1] = 2.0; }
    }
}
