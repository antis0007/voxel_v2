use super::{bend, grand, maze, shell};
use crate::{BlockId, WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;
use glam::{IVec3, Vec3};

fn world(version: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: version,
        seed: 424242,
        world_type: WorldType::Manyfold,
        voxel_scale: scale,
    }
}

fn probe(version: u32, ia: i32, iz: i32, frac_a: f32, frac_z: f32, above: f32) -> BlockId {
    let cfg = world(version, VoxelScalePreset::OneMeter);
    let z = (iz as f32 + frac_z) * bend::CELL_Z;
    let step = std::f32::consts::TAU / bend::AROUND as f32;
    let angle = ((ia as f32 + frac_a) * step - grand::twist(z)).rem_euclid(std::f32::consts::TAU);
    let radius = shell::shell_radius(version, z) + above;
    let axis = shell::center_of(version);
    let p = axis + Vec3::new(angle.sin() * radius, -angle.cos() * radius, z);
    let cell = (p / cfg.voxel_meters() - Vec3::splat(0.5))
        .floor()
        .as_ivec3();
    super::block(&cfg, cell)
}

fn radial(version: u32, scale: VoxelScalePreset, z: f32, angle: f32) -> Vec<BlockId> {
    let cfg = world(version, scale);
    let radius = shell::shell_radius(version, z);
    let axis = shell::center_of(version);
    let mut out = Vec::new();
    let mut last = IVec3::new(i32::MIN, i32::MIN, i32::MIN);
    for step in 0..64 {
        let dist = radius - 0.35 + step as f32 * scale.meters() * 0.5;
        let p = axis + Vec3::new(angle.sin() * dist, -angle.cos() * dist, z);
        let cell = (p / scale.meters()).floor().as_ivec3();
        if cell == last {
            continue;
        }
        last = cell;
        let centre = (cell.as_vec3() + Vec3::splat(0.5)) * scale.meters();
        let rel = centre - axis;
        let above = Vec3::new(rel.x, rel.y, 0.0).length() - shell::shell_radius(version, rel.z);
        if above < -0.05 {
            continue;
        }
        out.push(super::block(&cfg, cell));
    }
    out
}

fn rind(blocks: &[BlockId]) -> (i32, bool) {
    let mut bedrock = 0;
    let mut solid = false;
    let mut gap = false;
    let mut left = false;
    for block in blocks {
        if block.is_solid() {
            if left {
                gap = true;
            }
            solid = true;
            if *block == BlockId::Bedrock {
                bedrock += 1;
            }
        } else if solid {
            left = true;
        }
    }
    (bedrock, gap)
}

#[test]
fn the_rind_is_bedrock_without_holes() {
    let version = maze::BEND;
    assert!(version > maze::ROOM);
    assert!(crate::CURRENT_GENERATOR_VERSION >= version);
    let mut old_miss = 0;
    for z in [-40.0_f32, 0.0, 80.0] {
        for turn in 0..8 {
            let angle = turn as f32 / 8.0 * std::f32::consts::TAU;
            let (fresh, gap) = rind(&radial(version, VoxelScalePreset::OneMeter, z, angle));
            let thick = if turn % 4 == 0 { 2 } else { 1 };
            assert!(
                fresh >= thick && !gap,
                "z {z} turn {turn} bedrock {fresh} gap {gap}"
            );
            let (old, _) = rind(&radial(maze::ROOM, VoxelScalePreset::OneMeter, z, angle));
            if old == 0 {
                old_miss += 1;
            }
            let (half, half_gap) = rind(&radial(version, VoxelScalePreset::HalfMeter, z, angle));
            assert!(
                half >= thick && !half_gap,
                "half-metre z {z} turn {turn} bedrock {half}"
            );
        }
    }
    assert!(old_miss > 0, "version 119 rind already covered every ray");
    let y = shell::center_of(version).y;
    assert!(y + 275.0 + 4.0 < 512.0 && y - 275.0 - 4.0 > -64.0);
}

#[test]
fn passages_clear_a_body_and_keep_closed_edges() {
    let version = maze::BEND;
    let seed = 424242;
    let mut open_edges = 0;
    let mut shut_edges = 0;
    let mut walked = 0;
    let mut blocked = 0;
    let mut door = 0;
    for iz in -20..50 {
        let n = bend::bays(version, iz);
        for ia in 0..bend::AROUND {
            for sub in 0..n {
                let east = bend::east_open(seed, ia, iz, sub);
                let north = bend::north_open(seed, ia, iz, sub);
                assert!(east || north, "sealed cell {ia},{iz},{sub}");
                if east {
                    open_edges += 1;
                } else {
                    shut_edges += 1;
                }
                if north {
                    open_edges += 1;
                } else {
                    shut_edges += 1;
                }
                if iz % 3 != 0 || ia % 3 != 0 {
                    continue;
                }
                let frac_a = (sub as f32 + 0.72) / n as f32;
                for above in [-1.7_f32, -2.6] {
                    let block = probe(version, ia, iz, frac_a, 0.72, above);
                    assert!(
                        !block.is_solid(),
                        "body in {block:?} at {ia},{iz} above {above}"
                    );
                    walked += 1;
                }
                let next_sub = if sub + 1 == n { 0 } else { sub + 1 };
                let next_ia = if sub + 1 == n {
                    (ia + 1).rem_euclid(bend::AROUND)
                } else {
                    ia
                };
                let edge = (next_sub as f32 + 0.04) / n as f32;
                let at_edge = probe(version, next_ia, iz, edge, 0.62, -1.7);
                if east {
                    if !at_edge.is_solid() {
                        door += 1;
                    }
                } else if at_edge.is_solid() {
                    blocked += 1;
                }
            }
        }
    }
    assert!(
        shut_edges > 800 && open_edges > 800,
        "shut {shut_edges} open {open_edges}"
    );
    assert!(walked > 100, "walk samples {walked}");
    assert!(blocked > 30 && door > 30, "blocked {blocked} door {door}");
    let cfg = world(version, VoxelScalePreset::OneMeter);
    let feet = super::spawn(&cfg);
    assert!(
        !super::block(&cfg, feet.floor().as_ivec3()).is_solid(),
        "spawn inside a wall"
    );
    let (gallery, inside) = maze::mouths(version).expect("mouths");
    for mouth in [gallery, inside] {
        let cell = (mouth.center - Vec3::splat(0.5)).floor().as_ivec3();
        assert!(
            !super::block(&cfg, cell).is_solid(),
            "mouth {cell} is walled in"
        );
    }
}

#[test]
fn districts_change_floor_ceiling_and_openness() {
    let version = maze::BEND;
    let mut floors = std::collections::HashSet::new();
    let mut low_roof = 0;
    let mut high_room = 0;
    for iz in -30..60 {
        let n = bend::bays(version, iz);
        for ia in (0..bend::AROUND).step_by(2) {
            let frac_a = 0.72 / n as f32;
            let floor = probe(version, ia, iz, frac_a, 0.55, 1.0);
            if floor.is_solid() && floor != BlockId::Bedrock {
                floors.insert(floor);
            }
            let cap = probe(version, ia, iz, frac_a, 0.55, -5.5);
            if cap.is_solid() {
                low_roof += 1;
            } else {
                high_room += 1;
            }
        }
    }
    assert!(floors.len() >= 3, "floors {floors:?}");
    assert!(
        low_roof > 40 && high_room > 40,
        "low {low_roof} high {high_room}"
    );
}
