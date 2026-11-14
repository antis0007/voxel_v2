//! Thin fittings over the physical, protected constructor booth.
use super::{Model, Quat, Vec3, architecture::{part, strut}, palette};
use crate::world::{World, orbital::cargo::{LOAD_TICKS, ARRIVAL_TICKS}};

pub(super) fn build(mesh: &mut Model, world: &World, viewer: Vec3) {
    for s in &world.orbital.habitats.cargo.stations {
        let origin = s.origin.as_vec3();
        if s.ready && viewer.distance_squared(origin) < 250_000.0 {
            let active = s.shipment.as_ref().is_some_and(|q| q.ticks < LOAD_TICKS) && s.pause == 0;
            let color = if active { [0.92, 0.19, 0.08] } else { [0.12, 0.62, 0.86] };
            // Voxel glass at x=22..23 owns collision and projectile occlusion.
            // An emissive grid makes its protection legible from the room.
            for z in -3..=3 {
                mesh.cuboid(origin + Vec3::new(21.98, 3.0, z as f32),
                    Vec3::new(0.025, 4.0, 0.025), Quat::IDENTITY, color, 0.6);
            }
            for y in 1..=5 {
                mesh.cuboid(origin + Vec3::new(21.98, y as f32, 0.0),
                    Vec3::new(0.025, 0.025, 6.0), Quat::IDENTITY, color, 0.6);
            }
            for side in [-1.0, 1.0] {
                // Two manipulators inside the indestructible booth.
                let at = origin + Vec3::new(24.2, 3.0, side * 1.5);
                part(mesh, at, Vec3::new(1.0, 1.1, 0.8), Quat::IDENTITY, palette::STEEL, 0.1);
                strut(mesh, at, at + Vec3::new(-0.7, 0.6, -side * 0.4), 0.14, palette::COPPER);
                strut(mesh, at + Vec3::new(-0.7, 0.6, -side * 0.4),
                    at + Vec3::new(-0.95, -0.2, 0.0), 0.10, palette::STEEL);
                mesh.cuboid(at + Vec3::new(-0.52, 0.2, 0.0), Vec3::new(0.04, 0.18, 0.38), Quat::IDENTITY, color, 0.7);
                let pad = origin + Vec3::new(20.0, 1.02, side * 1.3);
                for z in [-0.42, 0.42] {
                    strut(mesh, pad + Vec3::new(-0.42, 0.0, z), pad + Vec3::new(0.42, 0.0, z), 0.04, color);
                }
            }
            // Controls sit on the accessible side of the field. The freight
            // terminal and interruption panel use these exact focus positions.
            for (offset, lamp) in [(Vec3::new(13.0, 1.2, 2.0), [0.2, 0.72, 0.46]),
                (Vec3::new(21.0, 2.0, 0.0), color)] {
                let at = origin + offset;
                part(mesh, at, Vec3::new(0.35, 0.6, 0.6), Quat::IDENTITY, palette::STEEL, 0.1);
                mesh.cuboid(at - Vec3::X * 0.19, Vec3::new(0.03, 0.35, 0.4), Quat::IDENTITY, lamp, 0.6);
            }
        }
        let Some(q) = &s.shipment else { continue };
        if q.ticks < LOAD_TICKS { continue; }
        let t = ((q.ticks - LOAD_TICKS) as f32 / (ARRIVAL_TICKS - LOAD_TICKS) as f32).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t);
        let at = (origin + Vec3::new(17.0, 1.0, 0.0)).lerp(q.destination + Vec3::Y * 0.6, t);
        if viewer.distance_squared(at) > 4_000_000.0 { continue; }
        part(mesh, at, Vec3::new(1.1, 1.2, 1.1), Quat::IDENTITY, palette::CERAMIC, 0.0);
        for x in [-0.4, 0.4] {
            part(mesh, at + Vec3::X * x, Vec3::new(0.06, 1.25, 1.15), Quat::IDENTITY, palette::STEEL, 0.1);
        }
        mesh.cuboid(at + Vec3::Y * 0.65, Vec3::new(0.35, 0.06, 0.35), Quat::IDENTITY, [0.2, 0.72, 0.46], 0.7);
    }
}
