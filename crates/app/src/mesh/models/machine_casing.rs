//! Shared casing for every single-block machine, from voxel assets on the
//! same 8-per-metre grid as the conduits. Per-kind machinery is drawn on top
//! of it by the caller; a kind with an authored mesh uses that instead.
use super::Model;
use super::wand_asset::Mesh;
use super::voxel_asset::{Tint, VoxelModel};
use crate::world::factory::{self, Kind, Node};
use glam::{IVec3, Quat, Vec3};
use std::sync::LazyLock;


static FURNACE: LazyLock<Mesh> = LazyLock::new(|| {
    Mesh::load(include_str!("../../../../../assets/models/machines/furnace.ron"), "furnace")
});

/// Authored block casing for a kind, when one exists. It replaces the procedural casing.
pub(super) fn authored(kind: Kind) -> Option<&'static Mesh> {
    match kind {
        Kind::Furnace => Some(&FURNACE),
        _ => None,
    }
}

/// Places an authored casing on its cell with the front toward the facing. The
/// asset is one unit block, so its corner is the cell corner and the rotation
/// pivots on the block centre.
pub(super) fn draw_authored(mesh: &mut Model, asset: &Mesh, o: Vec3, n: &Node, active: bool) {
    let block = Vec3::splat(mesh.scale_meters());
    let front = if n.facing.y.abs() > 0 { Vec3::NEG_Z } else { n.facing.as_vec3() };
    let rotation = Quat::from_rotation_arc(Vec3::Z, front);
    let origin = o + block * 0.5 - rotation * (block * 0.5);
    let status = if !n.enabled {
        [0.42, 0.12, 0.10]
    } else if active {
        [1.0, 0.70, 0.25]
    } else {
        [0.55, 0.38, 0.14]
    };
    asset.draw(mesh, origin, rotation, block, status, [1.0; 3]);
}

/// Trim colour per machine family, as a tier band would read in a tech mod.
pub(super) fn trim(kind: Kind, mode: u32) -> [f32; 3] {
    match kind {
        Kind::Generator if mode == 1 => [0.60, 0.35, 0.95],
        Kind::Generator | Kind::Battery | Kind::Transformer => [0.95, 0.42, 0.18],
        Kind::Crusher | Kind::Washer | Kind::MiningDrill => [0.30, 0.55, 0.95],
        Kind::Furnace | Kind::AlloyKiln => [0.95, 0.65, 0.20],
        Kind::Pump | Kind::Tank => [0.25, 0.70, 0.85],
        Kind::Bioreactor | Kind::Cultivator => [0.35, 0.80, 0.40],
        Kind::Fabricator | Kind::Assembler => [0.80, 0.45, 0.90],
        _ => [0.55, 0.60, 0.65],
    }
}

/// Box machines are a full closed cube; kinds with moving parts keep an
/// open top above 0.62 so their machinery reads.
pub(super) fn closed(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Feeder | Kind::Storage | Kind::Battery | Kind::Transformer | Kind::Fabricator
            | Kind::Furnace | Kind::AlloyKiln | Kind::Bioreactor | Kind::Cultivator
    )
}

macro_rules! asset {
    ($name:literal) => {
        VoxelModel::parse(include_str!(concat!("../../../../../assets/models/factory/", $name, ".vox")), $name)
    };
}

static SHELL_CLOSED: LazyLock<VoxelModel> = LazyLock::new(|| asset!("shell_closed"));
static SHELL_OPEN: LazyLock<VoxelModel> = LazyLock::new(|| asset!("shell_open"));
static BAR_CLOSED: LazyLock<VoxelModel> = LazyLock::new(|| asset!("bar_closed"));
static BAR_OPEN: LazyLock<VoxelModel> = LazyLock::new(|| asset!("bar_open"));

/// Per-kind parts above the open shell and the battery gauge; the `_0`/`_1`
/// pairs are the two stepped animation frames.
struct Modules {
    generator: [VoxelModel; 2],
    crusher: [VoxelModel; 2],
    drill: [VoxelModel; 2],
    washer: [VoxelModel; 2],
    assembler: [VoxelModel; 2],
    pump: VoxelModel,
    tank: [VoxelModel; 4],
    gauge: [VoxelModel; 5],
    quarry: Quarry,
}

/// Gantry parts drawn round a wide drill's footprint.
struct Quarry {
    rail: VoxelModel,
    corner: VoxelModel,
    beam: VoxelModel,
    cart: VoxelModel,
    rod: VoxelModel,
    head: VoxelModel,
}

static MODULES: LazyLock<Modules> = LazyLock::new(|| Modules {
    generator: [asset!("mod_generator_0"), asset!("mod_generator_1")],
    crusher: [asset!("mod_crusher_0"), asset!("mod_crusher_1")],
    drill: [asset!("mod_drill_0"), asset!("mod_drill_1")],
    washer: [asset!("mod_washer_0"), asset!("mod_washer_1")],
    assembler: [asset!("mod_assembler_0"), asset!("mod_assembler_1")],
    pump: asset!("mod_pump"),
    tank: [asset!("mod_tank_0"), asset!("mod_tank_1"), asset!("mod_tank_2"), asset!("mod_tank_3")],
    gauge: [asset!("gauge_0"), asset!("gauge_1"), asset!("gauge_2"), asset!("gauge_3"), asset!("gauge_4")],
    quarry: Quarry {
        rail: asset!("quarry_rail"),
        corner: asset!("quarry_corner"),
        beam: asset!("quarry_beam"),
        cart: asset!("quarry_cart"),
        rod: asset!("quarry_rod"),
        head: asset!("quarry_head"),
    },
});

/// Frame, bridge, carriage and rod round a quarry footprint; the head sits in
/// the layer being cut. Offsets are metres from the anchor cell's corner.
fn draw_quarry(mesh: &mut Model, o: Vec3, n: &Node, size: i32, tint: &Tint) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let q = &MODULES.quarry;
    let half = factory::quarry::width_m(n.mode) / 2;
    let at = |x: i32, y: i32, z: i32| o + Vec3::new(x as f32, y as f32, z as f32);
    for i in -half..=half {
        let edge = i.abs() == half;
        if !edge {
            q.rail.draw(mesh, at(i, 0, -half), Quat::IDENTITY, tint);
            q.rail.draw(mesh, at(i, 0, half), Quat::from_rotation_y(PI), tint);
            q.rail.draw(mesh, at(-half, 0, i), Quat::from_rotation_y(FRAC_PI_2), tint);
            q.rail.draw(mesh, at(half, 0, i), Quat::from_rotation_y(-FRAC_PI_2), tint);
        }
    }
    q.corner.draw(mesh, at(-half, 0, -half), Quat::IDENTITY, tint);
    q.corner.draw(mesh, at(half, 0, -half), Quat::from_rotation_y(-FRAC_PI_2), tint);
    q.corner.draw(mesh, at(half, 0, half), Quat::from_rotation_y(PI), tint);
    q.corner.draw(mesh, at(-half, 0, half), Quat::from_rotation_y(FRAC_PI_2), tint);

    let cursor = factory::quarry::offset(n.excavation as i32, size, n.mode);
    let (cx, cz) = (cursor.x.div_euclid(size), cursor.z.div_euclid(size));
    for x in -half..=half {
        q.beam.draw(mesh, at(x, 0, cz), Quat::IDENTITY, tint);
    }
    q.cart.draw(mesh, at(cx, 0, cz), Quat::IDENTITY, tint);
    let depth = factory::quarry::depth_m(n.excavation as i32, size, n.mode);
    for k in 1..depth {
        q.rod.draw(mesh, at(cx, -k, cz), Quat::IDENTITY, tint);
    }
    q.head.draw(mesh, at(cx, -depth, cz), Quat::IDENTITY, tint);
}


/// Draws the kind's own parts: voxel modules in two stepped frames, plus the
/// arcane pile core and the mana font, which are lights rather than voxels.
pub(super) fn module(mesh: &mut Model, o: Vec3, n: &Node, active: bool, time: f32, size: i32) {
    let m = &*MODULES;
    let front = if n.facing.y != 0 { IVec3::NEG_Z } else { n.facing };
    let yaw = Quat::from_rotation_y((-front.z as f32).atan2(front.x as f32));
    let frame = if active { (n.phase * 3.0) as usize & 1 } else { 0 };
    let trim = trim(n.kind, n.mode);
    let tint = Tint { stripe: trim, trim, glow: 1.0 };
    match n.kind {
        Kind::Generator => {
            m.generator[frame].draw(mesh, o, yaw, &tint);
            if n.mode == 1 {
                // Arcane pile: the core runs cyan when cool and white-orange as
                // heat climbs toward meltdown, and its pulse quickens.
                let heat = (n.progress as f32 / 1000.0).clamp(0.0, 1.0);
                let core = [0.35 + 0.65 * heat, 0.95 - 0.45 * heat, 1.0 - 0.85 * heat];
                let pulse = 0.8 + 0.2 * (time * (2.0 + 7.0 * heat)).sin();
                mesh.glow_orb(o + Vec3::new(0.5, 1.0, 0.5), 0.12 + 0.06 * heat, core, (1.6 + 3.2 * heat) * pulse, 0.85);
            }
        }
        Kind::Crusher => m.crusher[frame].draw(mesh, o, yaw, &tint),
        Kind::MiningDrill => {
            m.drill[frame].draw(mesh, o, yaw, &tint);
            if n.mode > 0 {
                draw_quarry(mesh, o, n, size, &tint);
            }
        }
        Kind::Washer => m.washer[frame].draw(mesh, o, yaw, &tint),
        Kind::Assembler => m.assembler[frame].draw(mesh, o, yaw, &tint),
        Kind::Pump => m.pump.draw(mesh, o, yaw, &tint),
        Kind::Tank => {
            let level = ((factory::tank_level(n) * 3.0).ceil() as usize).min(3);
            m.tank[level].draw(mesh, o, yaw, &tint);
        }
        Kind::Battery => {
            let charge = n.energy as f32 / factory::power::battery_capacity(n.upgrade) as f32;
            let lit = (charge.clamp(0.0, 1.0) * 4.0).ceil() as usize;
            m.gauge[lit].draw(mesh, o + yaw * Vec3::X * 0.004, yaw, &Tint { stripe: trim, trim: [0.30, 0.85, 1.0], glow: 1.0 });
            if n.mode == 3 && n.enabled && n.energy > 0 {
                // Mana font: a slow violet wisp above the bank.
                let pulse = 0.75 + 0.25 * (time * 2.2).sin();
                mesh.glow_orb(o + Vec3::new(0.5, 1.08 + 0.04 * (time * 1.3).sin(), 0.5), 0.07, [0.70, 0.45, 1.0], 2.2 * pulse, 0.85);
            }
        }
        _ => {}
    }
}

/// Draws the shared voxel shell: seamed steel panels, a trim-coloured window
/// with a status bar on the front, and framed ports on the other three sides,
/// the same socket form the conduits end in. Returns the trim colour.
pub(super) fn draw(mesh: &mut Model, o: Vec3, n: &Node, active: bool) -> [f32; 3] {
    let trim = trim(n.kind, n.mode);
    let (shell, bar) = if closed(n.kind) { (&SHELL_CLOSED, &BAR_CLOSED) } else { (&SHELL_OPEN, &BAR_OPEN) };
    let front = if n.facing.y != 0 { IVec3::NEG_Z } else { n.facing };
    let yaw = Quat::from_rotation_y((-front.z as f32).atan2(front.x as f32));
    shell.draw(mesh, o, yaw, &Tint { stripe: trim, trim, glow: 1.0 });
    let (lamp, glow) = if !n.enabled {
        ([0.42, 0.12, 0.10], 0.0)
    } else if active {
        (trim, 1.0)
    } else {
        (trim.map(|c| c * 0.45), 0.0)
    };
    if n.kind != Kind::Battery {
        bar.draw(mesh, o + yaw * Vec3::X * 0.004, yaw, &Tint { stripe: lamp, trim: lamp, glow });
    }
    trim
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_furnace_casing_fills_its_unit_block() {
        let asset = authored(Kind::Furnace).expect("furnace has an authored casing");
        let mut model = Model::with_vertices(1.0, Vec::new());
        asset.draw(&mut model, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE, [1.0; 3], [1.0; 3]);
        assert!(!model.vertices.is_empty(), "casing asset failed to load");
        let inside = |c: f32| (-1e-4..=1.0 + 1e-4).contains(&c);
        assert!(model.vertices.iter().all(|v| v.pos.iter().copied().all(inside)));
    }
}
