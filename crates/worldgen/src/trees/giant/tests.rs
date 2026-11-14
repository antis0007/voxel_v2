use super::*;
use crate::{tree_block, tree_profile_for_cell, trees};
use engine_core::VoxelScalePreset;
use std::collections::VecDeque;


fn cfg(version: u32, voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: version, seed: crate::test_world::seed(), world_type: WorldType::Hills, voxel_scale }
}

/// Giants among the lottery cells of a block square around the origin.
fn giants(cfg: &WorldSampleConfig, blocks: i32) -> Vec<TreeProfile> {
    let mut found = Vec::new();
    for bz in -blocks..blocks {
        for bx in -blocks..blocks {
            if let Some((x, z)) = designated(cfg, bx, bz)
                && let Some(t) = tree_profile_for_cell(cfg, x, z)
                && t.form == TreeForm::Giant
            {
                found.push(t);
            }
        }
    }
    found
}

#[test]
fn giants_grow_rarely_and_only_from_generator_52() {
    let current = cfg(VERSION, VoxelScalePreset::OneMeter);
    let found = giants(&current, 20);
    let km2 = (40.0 * BLOCK as f32 * 12.0 / 1000.0).powi(2);
    let density = found.len() as f32 / km2;
    for t in found.iter().take(6) {
        println!("giant cell ({}, {}) anchor ({}, {}) ground {} habit {:?}", t.cell_x, t.cell_z,
            t.anchor_x, t.anchor_z, t.ground_y, habit(&current, t.cell_x, t.cell_z));
    }
    println!("{} giants, {density:.2} per km2", found.len());
    assert!(found.iter().any(|t| t.anchor_x < 0 && t.anchor_z < 0), "no giant in the negative quadrant");
    assert!((0.3..6.0).contains(&density), "giant density {density:.2} per km2");
    assert!(giants(&cfg(51, VoxelScalePreset::OneMeter), 20).is_empty());
}

/// Every voxel the tree places, on a dense grid around its anchor.
struct Grid {
    span: i32,
    height: i32,
    cells: Vec<BlockId>,
}

impl Grid {
    fn sample(cfg: &WorldSampleConfig, t: TreeProfile) -> Self {
        let scale = cfg.voxel_meters();
        let span = (24.0 / scale).ceil() as i32 + 2;
        let height = ((trees::crown_top(cfg, &t) + 2.0) / scale).ceil() as i32;
        let side = 2 * span + 1;
        let mut cells = vec![BlockId::Air; (side * side * height) as usize];
        for y in 0..height {
            for z in -span..=span {
                for x in -span..=span {
                    let p = IVec3::new(t.anchor_x + x, t.ground_y + 1 + y, t.anchor_z + z);
                    cells[(((y * side) + z + span) * side + x + span) as usize] = tree_block(cfg, t, p);
                }
            }
        }
        Self { span, height, cells }
    }

    fn index(&self, q: IVec3) -> Option<usize> {
        let side = 2 * self.span + 1;
        let inside = q.y >= 0 && q.y < self.height && q.x.abs() <= self.span && q.z.abs() <= self.span;
        inside.then(|| (((q.y * side) + q.z + self.span) * side + q.x + self.span) as usize)
    }

    fn position(&self, i: usize) -> IVec3 {
        let side = (2 * self.span + 1) as usize;
        IVec3::new((i % side) as i32 - self.span, (i / (side * side)) as i32, ((i / side) % side) as i32 - self.span)
    }

    /// Voxels matching `pass` that a 6-connected walk from ground-level wood reaches.
    fn reached(&self, pass: impl Fn(BlockId) -> bool) -> Vec<bool> {
        let mut seen = vec![false; self.cells.len()];
        let mut queue = VecDeque::new();
        for (i, b) in self.cells.iter().enumerate() {
            if *b == BlockId::Log && self.position(i).y == 0 {
                seen[i] = true;
                queue.push_back(i);
            }
        }
        while let Some(i) = queue.pop_front() {
            let p = self.position(i);
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                if let Some(j) = self.index(p + d)
                    && !seen[j]
                    && pass(self.cells[j])
                {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        seen
    }
}

#[test]
fn giants_are_connected_from_roots_to_crown_at_every_scale() {
    let found = giants(&cfg(VERSION, VoxelScalePreset::OneMeter), 20);
    let negative = found.iter().find(|t| t.anchor_x < 0 && t.anchor_z < 0).map(|t| (t.cell_x, t.cell_z));
    let mut cells: Vec<_> = found.iter().take(2).map(|t| (t.cell_x, t.cell_z)).collect();
    cells.extend(negative.filter(|c| !cells.contains(c)));
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let cfg = cfg(VERSION, scale);
        let v = cfg.voxel_meters();
        // Quarter-metre giants hold millions of voxels; one is enough there.
        let take = if scale == VoxelScalePreset::QuarterMeter { 1 } else { cells.len() };
        for &(cx, cz) in cells.iter().rev().take(take) {
            let Some(t) = tree_profile_for_cell(&cfg, cx, cz).filter(|t| t.form == TreeForm::Giant) else {
                continue;
            };
            let grid = Grid::sample(&cfg, t);
            let (reach, top) = (trees::crown_reach(&cfg, &t), trees::crown_top(&cfg, &t));
            assert!(reach <= 24.0 && top - (t.trunk_h + t.canopy_height) as f32 * v <= v, "bounds {reach} {top}");
            let wood_ok = grid.reached(|b| b == BlockId::Log);
            let all_ok = grid.reached(|b| b == BlockId::Log || b == BlockId::Leaves);
            let (mut wood, mut leaves, mut loose_wood, mut loose_leaves) = (0, 0, 0, 0);
            for (i, b) in grid.cells.iter().enumerate() {
                let q = grid.position(i);
                match b {
                    BlockId::Log => {
                        wood += 1;
                        loose_wood += !wood_ok[i] as u32;
                    }
                    BlockId::Leaves => {
                        leaves += 1;
                        loose_leaves += !all_ok[i] as u32;
                        let horizontal = (q.x as f32).hypot(q.z as f32) * v;
                        assert!(horizontal <= reach + v, "leaf at {horizontal} m beyond reach {reach}");
                        assert!((q.y + 1) as f32 * v <= top + v, "leaf above top {top}");
                    }
                    _ => {}
                }
            }
            let height = (t.trunk_h + t.canopy_height) as f32 * v;
            println!("{scale:?} cell ({cx},{cz}) {height:.1} m, reach {reach:.1} m: {wood} wood, {leaves} leaves, \
                {loose_leaves} loose leaves");
            assert!((25.0..=45.0).contains(&height), "height {height}");
            assert!(wood > 0 && leaves > wood, "{wood} wood, {leaves} leaves");
            assert_eq!(loose_wood, 0, "floating wood at {scale:?}");
            assert!(loose_leaves * 200 <= leaves, "{loose_leaves} of {leaves} leaves float at {scale:?}");
        }
    }
}

#[test]
fn giants_clear_their_ground_and_generate_identically() {
    let cfg = cfg(VERSION, VoxelScalePreset::HalfMeter);
    let v = cfg.voxel_meters();
    let found = giants(&cfg, 20);
    assert!(!found.is_empty());
    for g in &found {
        for dz in -3..=3 {
            for dx in -3..=3 {
                let Some(t) = tree_profile_for_cell(&cfg, g.cell_x + dx, g.cell_z + dz) else { continue };
                if t.form == TreeForm::Giant {
                    continue;
                }
                let d = ((t.anchor_x - g.anchor_x) as f32).hypot((t.anchor_z - g.anchor_z) as f32) * v;
                assert!(d >= g.canopy_radius as f32 * v + 2.0, "ordinary tree {d} m from a giant");
            }
        }
    }
    // A fresh worker has empty profile and skeleton caches.
    let g = found[0];
    fn probe(cfg: &WorldSampleConfig, t: TreeProfile) -> Vec<BlockId> {
        let at = |i: i32| IVec3::new(t.anchor_x + i % 20 - 10, t.ground_y + 1 + i / 8, t.anchor_z + i % 7 - 3);
        (0..400).map(|i| tree_block(cfg, t, at(i))).collect()
    }
    let here = probe(&cfg, g);
    let cell = (g.cell_x, g.cell_z);
    let there = std::thread::spawn(move || {
        let t = tree_profile_for_cell(&cfg, cell.0, cell.1).filter(|t| t.form == TreeForm::Giant);
        t.map(|t| probe(&cfg, t))
    })
    .join()
    .ok()
    .flatten();
    assert_eq!(there.as_ref(), Some(&here));
    let top = top_near(&cfg, (g.anchor_x, g.anchor_z), (g.anchor_x, g.anchor_z));
    assert_eq!(top, Some(g.ground_y + g.trunk_h + g.canopy_height + 2));
}

/// Cost of the horizon's tree source: `tree_visual_lod` over a cell grid.
/// `cargo test --release -p engine_worldgen tree_visual_cost -- --ignored --nocapture`
#[test]
#[ignore]
fn tree_visual_cost() {
    for version in [51, VERSION, 51, VERSION] {
        let cfg = cfg(version, VoxelScalePreset::OneMeter);
        let started = std::time::Instant::now();
        let mut n = 0;
        for z in -150..150 {
            for x in -150..150 {
                n += crate::tree_visual_lod(&cfg, x * 3 + 7, z * 3 - 5, 8.0).is_some() as u32;
            }
        }
        println!("v{version} {n} trees of 90000 cells: {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
    }
}

