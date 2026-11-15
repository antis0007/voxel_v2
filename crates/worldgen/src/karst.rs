//! Generator 75: bounded limestone collapse chambers and perched cave gardens.
//! One deterministic site per 160 m cell; no neighbouring chunks are loaded.
use crate::{BlockId as B, WorldSampleConfig, hash2, sample_column};
use glam::{IVec3, Vec3};
use std::{cell::RefCell, collections::HashMap};

pub const VERSION: u32 = 75;
const CELL: f32 = 160.0;
#[derive(Clone, Copy)]
struct Site { x: f32, z: f32, floor: f32, radius: f32, height: f32 }
thread_local! {
    static SITES: RefCell<(Option<WorldSampleConfig>, HashMap<(i32,i32),Option<Site>>)> = RefCell::new((None, HashMap::new()));
}

fn site(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<Site> {
    SITES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.0 != Some(*cfg) { *cache = (Some(*cfg), HashMap::new()); }
        if let Some(s) = cache.1.get(&(ix, iz)) { return *s; }
        let h = |salt| hash2(ix, iz, cfg.seed.wrapping_add(salt));
        let found = (|| {
            if h(7401) < 0.68 { return None; }
            let x = (ix as f32 + 0.5) * CELL + (h(7403)-0.5)*54.0;
            let z = (iz as f32 + 0.5) * CELL + (h(7409)-0.5)*54.0;
            if x.hypot(z) < 160.0 || crate::valley::reserves(cfg, x, z) { return None; }
            let column = sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
            let ground = cfg.world_y(column.height);
            if ground < crate::WATER_LEVEL_METERS + 9.0 || matches!(column.surface, B::SnowGrass | B::Ice) { return None; }
            let floor = ground - 19.0 - h(7411)*12.0;
            if floor < cfg.world_y(cfg.min_world_y()) + 8.0 { return None; }
            Some(Site { x, z, floor, radius: 12.0+h(7417)*7.0, height: 9.0+h(7419)*5.0 })
        })();
        if cache.1.len() >= 2048 { cache.1.clear(); }
        cache.1.insert((ix, iz), found);
        found
    })
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3, surface: i32) -> Option<B> {
    if cfg.generator_version < VERSION || !crate::features::enabled(cfg, crate::features::Feature::Karst) { return None; }
    let v = cfg.voxel_meters();
    let m = (p.as_vec3()+Vec3::splat(0.5))*v;
    let s = site(cfg, (m.x/CELL).floor() as i32, (m.z/CELL).floor() as i32)?;
    let dx = m.x-s.x;
    let dz = m.z-s.z;
    if dx.abs() > s.radius+3.0 || dz.abs() > s.radius+3.0 || m.y < s.floor-4.0 || m.y > cfg.world_y(surface)+v { return None; }
    // A tilted bedding plane and broad lobes, rather than a spherical noise hole.
    let bend = (dx*0.23).sin()*0.9;
    let r2 = (dx/s.radius).powi(2)+((dz+bend)/(s.radius*0.76)).powi(2);
    let bedding = s.floor + dx*0.035;
    let roof = bedding + s.height*(1.0-r2.max(0.0)).max(0.0).sqrt();
    let shelf = bedding + (r2*3.0).floor().max(0.0)*0.65;
    let pool_r2 = ((dx+4.0)/4.5).powi(2)+(dz/3.8).powi(2);
    let floor = if pool_r2 < 1.0 { bedding-2.2*(1.0-pool_r2) } else { shelf };
    let floor_row = (floor/v).floor() as i32;
    let water_row = ((s.floor+0.35)/v).floor() as i32;
    // Collapse throat opens into the dry shelf, leaving a visible pool nearby.
    let shaft_x = dx-s.radius*0.38;
    let shaft_radius = 2.3;
    let shaft = shaft_x.abs() < shaft_radius && dz.abs() < shaft_radius*0.85;
    if shaft && m.y > shelf {
        // A continuous attached vine face provides a return route from the pocket.
        if shaft_x < -shaft_radius+v && dz.abs() < v.max(0.6) { return Some(B::VinesEast); }
        return Some(B::Air);
    }
    if r2 > 1.16 || m.y > roof+1.8 { return None; }
    if r2 >= 1.0 || p.y <= floor_row || m.y >= roof {
        if p.y == floor_row && r2 < 0.90 && pool_r2 >= 1.0 {
            return Some(if shaft_x.abs() < 4.0 { B::GroveMoss } else { B::FungalSoil });
        }
        // Darker sediment bands follow the same dip across walls and columns.
        return Some(if (m.y-dx*0.035).rem_euclid(3.2) < 0.35 { B::Stone } else { B::Limestone });
    }
    if pool_r2 < 1.0 && p.y <= water_row { return Some(B::Water); }
    let gx = (m.x/4.0).floor() as i32;
    let gz = (m.z/4.0).floor() as i32;
    let patch = hash2(gx, gz, cfg.seed.wrapping_add(7433));
    let ax = (gx as f32+0.5)*4.0;
    let az = (gz as f32+0.5)*4.0;
    let root = p.x == (ax/v).floor() as i32 && p.z == (az/v).floor() as i32;
    // Roots hang from the limestone cap around the daylight opening.
    if root && shaft_x.abs() < 6.0 && pool_r2 >= 1.0 && patch > 0.55
        && roof-m.y < 2.5 && m.y > floor+3.0 { return Some(B::VinesEast); }
    if pool_r2 >= 1.0 && p.y == floor_row+1 && root {
        if patch > 0.88 { return Some(B::Crystal); }
        if patch > 0.64 { return Some(B::GlowCap); }
        if patch > 0.48 { return Some(B::AmberCap); }
        if patch > 0.32 { return Some(B::UmberCap); }
    }
    // Sparse ceiling dripstone follows joints; it never fills the walking shelf.
    if root && patch < 0.22 && roof-m.y < 1.0+patch*7.0 && m.y > floor+3.5 {
        return Some(B::Limestone);
    }
    Some(B::Air)
}

/// Metre-space centre of a perched garden; shared by wildlife and review cameras.
pub fn garden(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<Vec3> {
    if cfg.generator_version < VERSION || !crate::features::enabled(cfg, crate::features::Feature::Karst) { return None; }
    let s = site(cfg, ix, iz)?;
    Some(Vec3::new(s.x, s.floor, s.z))
}

pub fn entrance(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<Vec3> {
    garden(cfg,ix,iz)?;
    let s=site(cfg,ix,iz)?;
    let x=s.x+s.radius*0.38;
    let column=sample_column(cfg,cfg.blocks_for_height(x),cfg.blocks_for_height(s.z));
    Some(Vec3::new(x,cfg.world_y(column.height)+cfg.voxel_meters(),s.z))
}

/// Only the guaranteed inner throat needs an overhead scan for skylighting.
pub fn skylight(cfg: &WorldSampleConfig, x: i32, z: i32) -> bool {
    if cfg.generator_version < VERSION || !crate::features::enabled(cfg, crate::features::Feature::Karst) { return false; }
    let mx = (x as f32+0.5)*cfg.voxel_meters();
    let mz = (z as f32+0.5)*cfg.voxel_meters();
    let Some(s) = site(cfg, (mx/CELL).floor() as i32, (mz/CELL).floor() as i32) else { return false; };
    (mx-s.x-s.radius*0.38).abs() < 2.0 && (mz-s.z).abs() < 1.7
}
