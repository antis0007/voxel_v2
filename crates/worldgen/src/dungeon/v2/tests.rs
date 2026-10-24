use super::*;
use crate::WorldType;
use engine_core::VoxelScalePreset;
use glam::IVec3;
use std::collections::{HashSet, VecDeque};

fn cfg(seed: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { generator_version: VERSION, seed, world_type: WorldType::Hills, voxel_scale: scale }
}

/// Complexes owned by cells in rings around the origin, with their cell.
fn complexes(c: &WorldSampleConfig, want: usize) -> Vec<((i32, i32), std::rc::Rc<Complex>)> {
    let mut out = Vec::new();
    for ring in 2i32..12 {
        for iz in -ring..=ring {
            for ix in -ring..=ring {
                if ix.abs().max(iz.abs()) != ring {
                    continue;
                }
                if let Some(d) = super::super::complex_at(c, ix, iz) {
                    out.push(((ix, iz), d));
                    if out.len() >= want {
                        return out;
                    }
                }
            }
        }
    }
    out
}

/// Rasterised complex: blocks below the ground, natural rock where the complex
/// does not say otherwise.
struct Grid {
    min: IVec3,
    size: IVec3,
    v: f32,
    blocks: Vec<B>,
}

impl Grid {
    fn new(c: &WorldSampleConfig, d: &Complex) -> Self {
        Self::within(c, d, d.min, d.max)
    }
    /// Rasterises only a box of the complex.
    fn within(c: &WorldSampleConfig, d: &Complex, lo: Vec3, hi: Vec3) -> Self {
        let v = c.voxel_meters();
        let min = (lo / v).floor().as_ivec3();
        let max = ((hi.min(Vec3::new(f32::MAX, d.ground - 2.5, f32::MAX))) / v).ceil().as_ivec3();
        let size = max - min + IVec3::ONE;
        let height = c.blocks_for_height(d.ground + 200.0);
        let mut blocks = Vec::with_capacity((size.x * size.y * size.z) as usize);
        for y in 0..size.y {
            for z in 0..size.z {
                for x in 0..size.x {
                    let p = min + IVec3::new(x, y, z);
                    let m = (p.as_vec3() + Vec3::splat(0.5)) * v;
                    blocks.push(d.sample(c, m, height).unwrap_or(B::Stone));
                }
            }
        }
        Self { min, size, v, blocks }
    }
    fn get(&self, p: IVec3) -> B {
        let q = p - self.min;
        if q.cmplt(IVec3::ZERO).any() || q.cmpge(self.size).any() {
            return B::Stone;
        }
        self.blocks[((q.y * self.size.z + q.z) * self.size.x + q.x) as usize]
    }
    fn open(&self, p: IVec3) -> bool {
        let b = self.get(p);
        !b.is_solid() && b != B::Lava
    }
    fn stand(&self, p: IVec3, tall: i32) -> bool {
        (0..tall).all(|k| self.open(p + IVec3::Y * k))
            && (self.get(p - IVec3::Y).is_solid() || self.get(p) == B::Water || self.get(p - IVec3::Y) == B::Water)
    }
    /// Every standing cell reachable on foot (step up one voxel, drop up to
    /// four metres, swim) from `start`.
    fn reach(&self, start: IVec3) -> HashSet<IVec3> {
        let tall = (1.8 / self.v).ceil() as i32;
        let step = (1.0 / self.v).round() as i32;
        let drop = (4.0 / self.v).round() as i32;
        let mut seen = HashSet::new();
        let mut queue = VecDeque::new();
        let mut s = start;
        while !self.stand(s, tall) && s.y > self.min.y {
            s.y -= 1;
        }
        seen.insert(s);
        queue.push_back(s);
        while let Some(p) = queue.pop_front() {
            for dir in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                for dy in (-drop..=step).rev() {
                    let q = p + dir + IVec3::Y * dy;
                    // Climbing needs headroom over the current cell.
                    if dy > 0 && !(0..tall + dy).all(|k| self.open(p + IVec3::Y * k)) {
                        continue;
                    }
                    if dy < 0 && !(dy..0).all(|k| self.open(p + dir + IVec3::Y * (k + tall))) {
                        continue;
                    }
                    if self.stand(q, tall) {
                        if seen.insert(q) {
                            queue.push_back(q);
                        }
                        break;
                    }
                }
            }
            if self.get(p) == B::Water && self.stand(p + IVec3::Y, tall) && seen.insert(p + IVec3::Y) {
                queue.push_back(p + IVec3::Y);
            }
        }
        seen
    }
}

fn start(d: &Complex, v: f32) -> IVec3 {
    let e = &d.rooms[d.entry];
    let (x, z) = e.centre();
    (Vec3::new(x + 2.5, e.floor + 1.0, z) / v).floor().as_ivec3()
}

fn reached(grid: &Grid, seen: &HashSet<IVec3>, r: &Room) -> bool {
    seen.iter().any(|p| {
        let m = (p.as_vec3() + Vec3::splat(0.5)) * grid.v;
        m.x >= r.x0 && m.x < r.x1 && m.z >= r.z0 && m.z < r.z1 && m.y >= r.floor - r.sub() - 0.5 && m.y < r.floor + r.top()
    })
}

#[test]
fn every_room_is_reachable_on_foot_from_the_shaft() {
    for seed in 0u32..24 {
        let c = cfg(seed.wrapping_mul(2654435761) ^ 424242, VoxelScalePreset::OneMeter);
        for (cell, d) in complexes(&c, 4) {
            let grid = Grid::new(&c, &d);
            let seen = grid.reach(start(&d, grid.v));
            let lost: Vec<_> = d.rooms.iter().enumerate().filter(|(_, r)| !reached(&grid, &seen, r)).map(|(i, r)| (i, r.kind, r.level, r.damage)).collect();
            assert!(lost.is_empty(), "seed {seed} cell {cell:?} {:?} entry {} rooms {}: unreachable {lost:?}", d.job, d.entry, d.rooms.len());
        }
    }
}

#[test]
fn half_metre_complexes_are_reachable_too() {
    for seed in [5u32, 6, 7] {
    let c = cfg(seed, VoxelScalePreset::HalfMeter);
    for (cell, d) in complexes(&c, 2) {
        let grid = Grid::new(&c, &d);
        let seen = grid.reach(start(&d, grid.v));
        for (i, r) in d.rooms.iter().enumerate() {
            assert!(reached(&grid, &seen, r), "seed {seed} cell {cell:?}: room {i} {:?} unreachable at 0.5 m", r.kind);
        }
    }
    }
}

#[test]
fn complexes_vary_in_job_shape_depth_and_rooms() {
    let c = cfg(424242, VoxelScalePreset::OneMeter);
    let found = complexes(&c, 40);
    assert!(found.len() >= 30, "only {} complexes", found.len());
    let jobs: HashSet<_> = found.iter().map(|(_, d)| d.job.name()).collect();
    let sizes: HashSet<_> = found.iter().map(|(_, d)| d.rooms.len()).collect();
    let depths: HashSet<_> = found.iter().map(|(_, d)| ((d.ground - d.rooms[d.entry].floor) / 4.0) as i32).collect();
    let levels: usize = found.iter().filter(|(_, d)| d.rooms.iter().any(|r| r.level == 2)).count();
    assert!(jobs.len() >= 6, "{jobs:?}");
    assert!(sizes.len() >= 4, "{sizes:?}");
    assert!(depths.len() >= 5, "{depths:?}");
    assert!(levels >= 5, "only {levels} complexes reach a third level");
    let damaged = found.iter().filter(|(_, d)| d.rooms.iter().any(|r| r.damage != Damage::Intact)).count();
    assert!(damaged >= found.len() / 2);
}

#[test]
fn the_core_is_sealed_and_entered_only_through_its_puzzle() {
    let c = cfg(424242, VoxelScalePreset::OneMeter);
    let mut cores = 0;
    for (_, d) in complexes(&c, 20) {
        let Some(o) = d.objective else { continue };
        cores += 1;
        let core = &d.rooms[o];
        assert_eq!(core.kind, Kind::Core);
        assert!(core.sealed);
        let (x, z) = core.centre();
        let wall = Vec3::new(core.x0 - 0.5, core.floor + 2.5, z);
        let height = c.blocks_for_height(d.ground + 200.0);
        let shell = d.sample(&c, wall, height);
        assert!(matches!(shell, Some(B::TombStone) | Some(B::Air)), "{shell:?}");
        // Exactly one corridor touches the core, and its other end is a puzzle.
        let touching: Vec<_> = d.corrs.iter().filter(|k| k.x1 > core.x0 - 0.1 && k.x0 < core.x1 + 0.1 && k.z1 > core.z0 - 0.1 && k.z0 < core.z1 + 0.1).collect();
        assert!(!touching.is_empty());
        let ante = d.rooms.iter().filter(|r| r.kind.is_puzzle() || r.kind == Kind::Orrery).count();
        assert!(ante >= 1);
        assert_eq!(d.sample(&c, Vec3::new(x + 0.2, core.floor + 0.7, z + 0.2), height), Some(B::SurveyCache));
    }
    assert!(cores >= 10, "only {cores} cores");
}

#[test]
fn sampling_is_deterministic_and_cache_independent() {
    let c = cfg(77, VoxelScalePreset::OneMeter);
    let (cell, d) = complexes(&c, 1).remove(0);
    let fresh = {
        let (seed, cx, cz, ground) = super::super::site(&c, cell.0, cell.1).unwrap();
        generate(seed, cx, cz, ground)
    };
    let height = c.blocks_for_height(d.ground + 200.0);
    for k in 0..4000 {
        let t = Vec3::new(crate::hash2(k, 1, 9), crate::hash2(k, 2, 9), crate::hash2(k, 3, 9));
        let m = d.min + (d.max - d.min) * t;
        assert_eq!(d.sample(&c, m, height), fresh.sample(&c, m, height));
    }
}

/// `V2_DUMP=seed,ix,iz,room,scale_half` prints top-down slices around a room.
#[test]
#[ignore = "debug dump"]
fn dump_room() {
    let args: Vec<i64> = std::env::var("V2_DUMP").unwrap().split(',').map(|s| s.parse().unwrap()).collect();
    let scale = if args.get(4) == Some(&1) { VoxelScalePreset::HalfMeter } else { VoxelScalePreset::OneMeter };
    let c = cfg(args[0] as u32, scale);
    let d = super::super::complex_at(&c, args[1] as i32, args[2] as i32).unwrap();
    let r = &d.rooms[args[3] as usize];
    println!("{:?} room {:?} {:?} level {} floor {} h {} box {}..{} x {}..{}", d.job, r.kind, r.damage, r.level, r.floor, r.h, r.x0, r.x1, r.z0, r.z1);
    for (i, q) in d.rooms.iter().enumerate() {
        println!("room {i}: {:?} {:?} lvl {} floor {} x {}..{} z {}..{}", q.kind, q.damage, q.level, q.floor, q.x0, q.x1, q.z0, q.z1);
    }
    for (i, k) in d.corrs.iter().enumerate() {
        println!("corr {i}: x {}..{} z {}..{} along_x {} f {}..{} plug {:?}", k.x0, k.x1, k.z0, k.z1, k.along_x, k.fa, k.fb, k.plug);
    }
    let v = c.voxel_meters();
    let height = c.blocks_for_height(d.ground + 200.0);
    for fy in [-1.5f32, -0.5, 0.5, 1.5, 2.5] {
        println!("-- fy {fy}");
        let mut z = r.z0 - 8.0;
        while z < r.z1 + 8.0 {
            let mut line = String::new();
            let mut x = r.x0 - 8.0;
            while x < r.x1 + 8.0 {
                let b = d.sample(&c, Vec3::new(x + v * 0.5, r.floor + fy, z + v * 0.5), height);
                line.push(match b {
                    None => ' ',
                    Some(B::Air) => '.',
                    Some(B::Water) => '~',
                    Some(B::Torch) => 't',
                    Some(B::MirageBrick) => 'm',
                    Some(B::TombStone) => 'T',
                    Some(B::Lava) => 'L',
                    Some(b) if b.is_solid() => '#',
                    Some(_) => '?',
                });
                x += v;
            }
            println!("{line}");
            z += v;
        }
    }
}

/// `V2_SLICE=seed,ix,iz,z,x0,x1,y0,y1` prints a vertical x/y slice at depth z.
#[test]
#[ignore = "debug dump"]
fn dump_slice() {
    let a: Vec<f32> = std::env::var("V2_SLICE").unwrap().split(',').map(|s| s.parse().unwrap()).collect();
    let c = cfg(a[0] as u32, VoxelScalePreset::OneMeter);
    let d = super::super::complex_at(&c, a[1] as i32, a[2] as i32).unwrap();
    let height = c.blocks_for_height(d.ground + 200.0);
    let mut y = a[7];
    while y >= a[6] {
        let mut line = format!("{y:>5} ");
        let mut x = a[4];
        while x < a[5] {
            line.push(match d.sample(&c, Vec3::new(x + 0.5, y + 0.5, a[3] + 0.5), height) {
                None => ' ',
                Some(B::Air) => '.',
                Some(B::Water) => '~',
                Some(B::Torch) => 't',
                Some(b) if b.is_solid() => '#',
                Some(_) => '?',
            });
            x += 1.0;
        }
        println!("{line}");
        y -= 1.0;
    }
}

/// `V2_REACH=seed,ix,iz,z,x0,x1,y0,y1`: like `dump_slice`, with reached cells as `o`.
#[test]
#[ignore = "debug dump"]
fn dump_reach() {
    let a: Vec<f32> = std::env::var("V2_REACH").unwrap().split(',').map(|s| s.parse().unwrap()).collect();
    let c = cfg(a[0] as u32, VoxelScalePreset::OneMeter);
    let d = super::super::complex_at(&c, a[1] as i32, a[2] as i32).unwrap();
    let grid = Grid::new(&c, &d);
    let seen = grid.reach(start(&d, 1.0));
    println!("start {:?} entry {:?}", start(&d, 1.0), d.rooms[d.entry].centre());
    let mut y = a[7] as i32;
    while y >= a[6] as i32 {
        let mut line = format!("{y:>5} ");
        for x in a[4] as i32..a[5] as i32 {
            let p = IVec3::new(x, y, a[3] as i32);
            line.push(if seen.contains(&p) { 'o' } else {
                match grid.get(p) { B::Air => '.', B::Water => '~', b if b.is_solid() => '#', _ => '?' }
            });
        }
        println!("{line}");
        y -= 1;
    }
}

#[test]
fn the_gatehouse_stands_above_ground_in_the_world() {
    let c = cfg(424242, VoxelScalePreset::OneMeter);
    let mut seen = 0;
    for (_, d) in complexes(&c, 6) {
        let (sx, sz, _) = d.shaft;
        let solid_above = (-3..=3).flat_map(|dx| (-3..=3).map(move |dz| (dx, dz))).any(|(dx, dz)| {
            let (x, z) = (c.blocks_for_height(sx) + dx, c.blocks_for_height(sz) + dz);
            let ground = crate::sample_column(&c, x, z).height;
            (ground + 2..ground + 10).any(|y| crate::sample_block(&c, IVec3::new(x, y, z)).is_solid())
        });
        seen += usize::from(solid_above);
    }
    assert!(seen >= 5, "gatehouse above ground at only {seen} of 6 complexes");
}

#[path = "tests_rooms.rs"]
mod rooms;
