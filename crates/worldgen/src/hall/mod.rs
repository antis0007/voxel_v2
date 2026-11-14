//! Hall of the Crimson King: one hanging precinct and its charted arches.
mod geom;
mod plan;
#[cfg(test)]
mod tests;

use crate::{BlockId, ColumnSample, WorldSampleConfig, WorldType};
use geom::{COURT_Y, CHOIR_Y, THRONE_Y};
use glam::{IVec3, Vec2, Vec3};

pub const VERSION: u32 = 106;
pub const FURNACE: u32 = 46;
pub const ASTRONOMER: u32 = 47;
pub const FURNACE_SITE: IVec3 = IVec3::new(-40, 500, 0);
pub const ASTRONOMER_SITE: IVec3 = IVec3::new(0, 501, 0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chart {
    Hall,
    Hills,
    Nether,
    Manyfold,
}

pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    let mx = (x as f32 + 0.5) * cfg.voxel_meters();
    let mz = (z as f32 + 0.5) * cfg.voxel_meters();
    let top = geom::floor_top(mx, mz).unwrap_or(geom::CRUST);
    let height = cfg.blocks_for_height(top - cfg.voxel_meters());
    let surface = if geom::shape_at(mx, mz).is_some() {
        if geom::on_chart(mx, mz) { BlockId::Crystal } else { BlockId::Limestone }
    } else {
        BlockId::VoidStone
    };
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone,
        height,
        surface,
        filler: BlockId::VoidStone,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3) -> BlockId {
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    geom::block_at(m.x, m.y, m.z, cfg.voxel_meters(), &plan::of(cfg))
}

pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    let scale = cfg.voxel_meters();
    let x = (0.0 / scale).floor();
    let z = (4.0 / scale).floor();
    let h = column(cfg, x as i32, z as i32).height;
    Vec3::new(x + 0.5, h as f32 + 1.0, z + 0.5)
}

pub fn encounters(player: Vec3) -> Vec<(IVec3, Vec3, u32)> {
    [(FURNACE_SITE, Vec3::new(-40.0, COURT_Y, 30.0), FURNACE), (ASTRONOMER_SITE, Vec3::new(0.0, CHOIR_Y, 68.0), ASTRONOMER)]
        .into_iter()
        .filter(|(_, p, _)| (*p - player).length() < 72.0)
        .collect()
}

pub fn canonical_gate(cfg: &WorldSampleConfig) -> Vec3 {
    crate::hell_gate::center(cfg, Vec3::new(240.0, 0.0, -64.0))
}

pub fn departure_metres(cfg: &WorldSampleConfig) -> Vec3 {
    let gate = canonical_gate(cfg);
    let axis = crate::hell_gate::approach::road_axis(cfg, gate);
    let across = Vec2::new(-axis.y, axis.x);
    let flat = Vec2::new(gate.x, gate.z) + axis * 22.0 + across * 8.5;
    let cell = ((Vec3::new(flat.x, 0.0, flat.y)) / cfg.voxel_meters()).floor().as_ivec3();
    let hills = WorldSampleConfig { world_type: WorldType::Hills, ..*cfg };
    let ground = (crate::sample_column(&hills, cell.x, cell.z).height as f32 + 1.0) * cfg.voxel_meters();
    Vec3::new(flat.x, ground, flat.y)
}

pub fn departure_block(cfg: &WorldSampleConfig, gate: Vec3, p: IVec3, column: ColumnSample) -> Option<BlockId> {
    if cfg.generator_version < VERSION || cfg.world_type != WorldType::Hills {
        return None;
    }
    let canon = canonical_gate(cfg);
    if (gate.x - canon.x).abs() > 0.5 || (gate.z - canon.z).abs() > 0.5 {
        return None;
    }
    let axis = crate::hell_gate::approach::road_axis(cfg, gate);
    let across = Vec2::new(-axis.y, axis.x);
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    let d = Vec2::new(m.x - gate.x, m.z - gate.z);
    let u = d.dot(axis);
    let w = d.dot(across);
    let ground = (column.height as f32 + 1.0) * cfg.voxel_meters();
    let interior = (u - 22.0).abs() < 1.15 && (w - 8.5).abs() < 1.15;
    let frame = (u - 22.0).abs() < 2.05 && (w - 8.5).abs() < 2.05;
    let path = (u - 22.0).abs() < 1.25 && (2.6..7.4).contains(&w);
    let slab = m.y >= ground - cfg.voxel_meters() && m.y < ground;
    let head = m.y >= ground && m.y < ground + 3.2;
    if interior || path {
        if slab {
            return Some(if interior && (u - 22.0).abs() < 0.5 && (w - 8.5).abs() < 0.5 { BlockId::Crystal } else { BlockId::Limestone });
        }
        if head {
            return Some(BlockId::Air);
        }
        return None;
    }
    if frame && slab {
        return Some(BlockId::Limestone);
    }
    if frame && m.y >= ground && m.y < ground + 3.6 {
        return Some(BlockId::CopperBlock);
    }
    None
}

fn near(feet: Vec3, at: Vec3) -> bool {
    let d = feet - at;
    d.x.hypot(d.z) < 1.2 && (-0.2..3.0).contains(&d.y)
}

pub fn standing(cfg: &WorldSampleConfig, feet: Vec3) -> Option<Chart> {
    if cfg.world_type == WorldType::Hills && cfg.generator_version >= VERSION && near(feet, departure_metres(cfg)) {
        return Some(Chart::Hall);
    }
    if cfg.world_type != WorldType::Hall {
        return None;
    }
    if near(feet, Vec3::new(0.0, COURT_Y, 2.0)) {
        return Some(Chart::Hills);
    }
    if near(feet, Vec3::new(-6.0, THRONE_Y, 94.0)) {
        return Some(Chart::Nether);
    }
    if near(feet, Vec3::new(6.0, THRONE_Y, 94.0)) {
        return Some(Chart::Manyfold);
    }
    None
}

pub fn arrival_voxels(cfg: &WorldSampleConfig, chart: Chart) -> Vec3 {
    let scale = cfg.voxel_meters();
    match chart {
        Chart::Hall => spawn(cfg),
        Chart::Hills => departure_metres(cfg) / scale,
        Chart::Nether => {
            let gate = canonical_gate(cfg);
            let base = crate::hell_gate::floor(cfg, gate);
            Vec3::new(gate.x, base, gate.z) / scale
        }
        Chart::Manyfold => {
            let maze = WorldSampleConfig { world_type: WorldType::Manyfold, ..*cfg };
            crate::manyfold::spawn(&maze)
        }
    }
}

pub fn ring_cells(cfg: &WorldSampleConfig) -> [IVec3; 8] {
    let scale = cfg.voxel_meters();
    let ring = plan::of(cfg).ring;
    let mut out = [IVec3::ZERO; 8];
    for (i, cell) in out.iter_mut().enumerate() {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let m = Vec3::new(a.cos() * ring, CHOIR_Y + 3.5, 68.0 + a.sin() * ring);
        *cell = (m / scale).floor().as_ivec3();
    }
    out
}

pub fn pillars(cfg: &WorldSampleConfig) -> [(f32, f32); 4] {
    plan::of(cfg).pillars
}
