use super::{armed, enabled, layout, linked};
use crate::{mesh::models::Model, world::World};
use glam::{Quat, Vec3};
use std::f32::consts::TAU;

/// Strokes follow the floor. The arch is a marked threshold, not a remote view.
pub fn draw(mesh: &mut Model, world: &World) {
    if !enabled(world) {
        return;
    }
    let time = world.adventure.time;
    for district in 0..6 {
        let Some(active) = armed(world, district) else {
            continue;
        };
        let color = if !active {
            [0.16, 0.30, 0.28]
        } else {
            match district % 3 {
                0 => [0.82, 0.40, 0.14],
                1 => [0.30, 0.65, 0.90],
                _ => [0.78, 0.28, 0.55],
            }
        };
        let glow = if active {
            0.65 + 0.15 * (time * 2.0).sin()
        } else {
            0.05
        };
        for i in 0..24 {
            let at = |a: f32| {
                layout::point(
                    district,
                    a.cos() * 2.8,
                    0.65,
                    layout::TRAP_Z + a.sin() * 2.8,
                )
            };
            mesh.beam(
                at(i as f32 * TAU / 24.0),
                at((i + 1) as f32 * TAU / 24.0),
                0.035,
                color,
                glow,
            );
        }
        // A broken outer inscription, radial nib marks and a conductor to the
        // editable anchor make the spell read as drawn construction. These
        // strokes describe the existing seal state, never decorative power.
        let at = |x: f32, z: f32| layout::point(district, x, 0.65, layout::TRAP_Z + z);
        for i in 0..36 {
            if i % 12 < 3 {
                continue;
            }
            let a = i as f32 * TAU / 36.0;
            let b = (i + 1) as f32 * TAU / 36.0;
            mesh.beam(
                at(a.cos() * 3.75, a.sin() * 3.75),
                at(b.cos() * 3.75, b.sin() * 3.75),
                0.025,
                color,
                glow * 0.6,
            );
            if i % 3 == 0 {
                mesh.beam(
                    at(a.cos() * 3.5, a.sin() * 3.5),
                    at(a.cos() * 4.0, a.sin() * 4.0),
                    0.035,
                    color,
                    glow * 0.6,
                );
            }
        }
        for (a, b) in [((2.8, 0.0), (4.4, 0.0)), ((4.4, 0.0), (4.4, 2.2))] {
            mesh.beam(at(a.0, a.1), at(b.0, b.1), 0.03, color, glow * 0.65);
        }
        // One, two or three bars distinguish effects without relying on hue.
        for i in 0..=district % 3 {
            let z = layout::TRAP_Z + (i as f32 - (district % 3) as f32 * 0.5) * 0.45;
            mesh.beam(
                layout::point(district, -1.3, 0.65, z),
                layout::point(district, 1.3, 0.65, z),
                0.045,
                color,
                glow,
            );
        }
        // The chevron makes the line's inward crossing direction explicit.
        for side in [-1.0, 1.0] {
            mesh.beam(
                layout::point(district, side * 0.7, 0.65, layout::TRAP_Z + 1.8),
                layout::point(district, 0.0, 0.65, layout::TRAP_Z + 1.1),
                0.05,
                color,
                glow,
            );
        }
        let open = linked(world, district);
        let color = if open {
            [0.30, 0.88, 0.72]
        } else {
            [0.38, 0.30, 0.20]
        };
        let rotation = Quat::from_rotation_z(district as f32 * TAU / 6.0);
        let center = layout::point(district, 0.0, layout::FRAME_HEIGHT, layout::GATE_Z);
        // Matching one/two/three strokes identify the paired arch overhead.
        for i in 0..=district % 3 {
            let x = (i as f32 - (district % 3) as f32 * 0.5) * 0.5;
            mesh.beam(
                center + rotation * Vec3::new(x, 2.25, -0.1),
                center + rotation * Vec3::new(x, 2.9, -0.1),
                0.07,
                color,
                if open { 0.8 } else { 0.25 },
            );
        }
        for side in [-1.0, 1.0] {
            let x = side * layout::APERTURE_WIDTH;
            mesh.beam(
                center + rotation * Vec3::new(x, -2.4, 0.0),
                center + rotation * Vec3::new(x, 2.4, 0.0),
                0.06,
                color,
                if open { 0.8 } else { 0.05 },
            );
        }
        if open {
            // Sparse inward-flowing threads mark the passable magic surface.
            for i in 0..9 {
                let x = -2.0 + i as f32 * 0.5;
                let y = (time * 0.65 + i as f32 * 0.43).rem_euclid(4.4) - 2.2;
                mesh.cuboid(
                    center + rotation * Vec3::new(x, y, 0.0),
                    Vec3::new(0.06, 0.32, 0.06),
                    rotation,
                    color,
                    0.9,
                );
            }
        }
    }
}
