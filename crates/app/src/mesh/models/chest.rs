//! Chests: a hinged lid on a plank body, trimmed by tier. Each chest is
//! built in its own frame (front toward -Z, units of 1/16 cell like a
//! Minecraft chest), the lid group is swung about its back hinge, then the
//! whole chest turns to its facing. Texels are baked before either rotation
//! (`material_box`), so the boards travel with the lid.
use super::Model;
use crate::world::{World, chests};
use glam::{Quat, Vec3};
use std::f32::consts::FRAC_PI_2;

/// Shader family for chest parts (`chest_surface` in world_shader.wgsl).
const KIND: f32 = 23.0;
const WOOD: f32 = 0.0;
const METAL: f32 = 1.0;
/// Chests further than this are not drawn, metres.
const DRAW_METRES: f32 = 64.0;

struct Look {
    body: [f32; 3],
    body_finish: f32,
    /// Corner posts and lid rim: dark oak on wood, metal on the others.
    trim: Option<([f32; 3], f32)>,
    /// Iron and steel add a band around the middle of the body.
    band: bool,
    latch: [f32; 3],
}

fn look(tier: u8) -> Look {
    let oak = [0.56, 0.37, 0.19];
    match tier {
        0 => Look { body: oak, body_finish: WOOD, trim: Some(([0.33, 0.2, 0.1], WOOD)), band: false, latch: [0.58, 0.58, 0.6] },
        1 => Look { body: oak, body_finish: WOOD, trim: Some(([0.78, 0.46, 0.24], METAL)), band: false, latch: [0.86, 0.55, 0.3] },
        2 => Look { body: [0.5, 0.33, 0.17], body_finish: WOOD, trim: Some(([0.64, 0.65, 0.68], METAL)), band: true, latch: [0.8, 0.81, 0.83] },
        _ => Look { body: [0.38, 0.42, 0.48], body_finish: METAL, trim: Some(([0.25, 0.28, 0.33], METAL)), band: true, latch: [0.86, 0.74, 0.42] },
    }
}

/// Four thin bars framing a rectangle `half` wide in x and z at height `y`.
fn frame(mesh: &mut Model, c: Vec3, half: Vec3, bar: f32, (color, finish): ([f32; 3], f32)) {
    for (offset, size) in [
        (Vec3::new(0.0, 0.0, -half.z), Vec3::new(half.x * 2.0 + bar, half.y, bar)),
        (Vec3::new(0.0, 0.0, half.z), Vec3::new(half.x * 2.0 + bar, half.y, bar)),
        (Vec3::new(-half.x, 0.0, 0.0), Vec3::new(bar, half.y, half.z * 2.0 - bar)),
        (Vec3::new(half.x, 0.0, 0.0), Vec3::new(bar, half.y, half.z * 2.0 - bar)),
    ] {
        mesh.material_box(c + offset, size, Quat::IDENTITY, color, KIND, finish);
    }
}

pub fn build(mesh: &mut Model, world: &World, viewer: Vec3) {
    let s = world.voxel_scale.meters();
    for (&(x, y, z), chest) in &world.chests.cells {
        // Bottom centre of the cell, metres.
        let base = (Vec3::new(x as f32, y as f32, z as f32) + Vec3::new(0.5, 0.0, 0.5)) * s;
        let coverage = super::range_fade((base - viewer).length(), DRAW_METRES);
        if coverage <= 0.0 {
            continue;
        }
        let mark = mesh.mark();
        draw(mesh, base, s, chest.tier, chest.facing, chest.lid);
        mesh.fade_since(mark, coverage);
    }
}


/// Shared shell for real chests and mimics, with the same hinge and material trim.
pub(crate) fn draw(mesh: &mut Model, base: Vec3, scale: f32, tier: u8, facing: u8, openness: f32) {
    let u = scale / 16.0;
    let look = look(tier);
    let start = mesh.mark();

    // Body: 14 × 10 × 14.
    mesh.material_box(base + Vec3::Y * (5.0 * u), Vec3::new(14.0, 10.0, 14.0) * u,
        Quat::IDENTITY, look.body, KIND, look.body_finish);
    // The dark hollow, seen only once the lid lifts. It sits inside the
    // closed lid's volume, so a shut chest never shows it.
    mesh.material_box(base + Vec3::Y * (10.0 * u + 0.25 * u), Vec3::new(12.0, 0.5, 12.0) * u,
        Quat::IDENTITY, [0.11, 0.07, 0.04], KIND, WOOD);
    if let Some(trim) = look.trim {
        // Corner posts stand half a unit proud of the boards.
        for (cx, cz) in [(-7.0, -7.0), (7.0, -7.0), (-7.0, 7.0), (7.0, 7.0)] {
            mesh.material_box(base + Vec3::new(cx * u, 5.0 * u, cz * u), Vec3::new(2.0, 10.0, 2.0) * u,
                Quat::IDENTITY, trim.0, KIND, trim.1);
        }
        if look.band {
            frame(mesh, base + Vec3::Y * (5.0 * u), Vec3::new(7.25 * u, 1.5 * u, 7.25 * u), u, trim);
        }
    }

    // Lid: 14 × 4 × 14 above the body, with its latch and rim.
    let lid = mesh.mark();
    mesh.material_box(base + Vec3::Y * (12.0 * u), Vec3::new(14.0, 4.0, 14.0) * u,
        Quat::IDENTITY, look.body, KIND, look.body_finish);
    mesh.material_box(base + Vec3::new(0.0, 9.0 * u, -7.5 * u), Vec3::new(2.0, 4.0, 1.0) * u,
        Quat::IDENTITY, look.latch, KIND, METAL);
    if let Some(trim) = look.trim {
        frame(mesh, base + Vec3::Y * (13.5 * u), Vec3::new(7.25 * u, 1.0 * u, 7.25 * u), u, trim);
    }
    let hinge = base + Vec3::new(0.0, 10.0 * u, 7.0 * u);
    let swing = chests::lid_curve(openness) * FRAC_PI_2;
    mesh.rotate_since(lid, hinge, Quat::from_rotation_x(swing));

    mesh.rotate_since(start, base, Quat::from_rotation_y(f32::from(facing) * FRAC_PI_2));
}
