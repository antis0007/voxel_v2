//! Generator 122 placer gold on the solved planet face.
//!
//! Thin lode belts shed gold where they erode, the solve's receiver network
//! carries it downstream, and it settles where stream power falls: below a
//! steep reach, where a tributary meets a gentler trunk, at a lake inlet.
//! Grade is gold per unit of sediment, so big rivers dilute what they carry.
use std::cell::RefCell;
use std::sync::Arc;

use crate::WorldSampleConfig;
use crate::geo::Planet;

/// Belts are a few hundred metres wide and wind for tens of kilometres.
const BELT_SCALE_M: f32 = 7000.0;
const BELT_HALF_WIDTH: f32 = 0.035;

/// Riverbed cells above this grade generate as paydirt.
pub const PAYDIRT_GRADE: f32 = 0.3;

pub struct Field {
    /// Relative placer grade per solve cell, 0..1.
    grade: Vec<f32>,
}

/// Lode belt strength at a point, 0..1; 1 on a belt's axis.
pub fn lode(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> f32 {
    let warp = crate::value_noise2(wx / 2300.0, wz / 2300.0, cfg.seed.wrapping_add(0x60_1d)) - 0.5;
    let n = crate::value_noise2(wx / BELT_SCALE_M + warp * 0.6, wz / BELT_SCALE_M - warp * 0.6, cfg.seed.wrapping_add(0x60_1e));
    (1.0 - (n - 0.5).abs() / BELT_HALF_WIDTH).max(0.0)
}

/// Gold-bearing quartz at a point in metres: thin sheets in one isosurface of
/// slow 3D noise, stretched vertically so they dip steeply, inside a belt only.
pub fn vein(cfg: &WorldSampleConfig, m: glam::Vec3) -> bool {
    let belt = lode(cfg, m.x, m.z);
    if belt < 0.45 {
        return false;
    }
    let n = crate::value_noise3(m.x / 21.0, m.y / 34.0, m.z / 21.0, cfg.seed.wrapping_add(0x60_1f));
    (n - 0.5).abs() < 0.02 * belt
}

fn build(cfg: &WorldSampleConfig, pl: &Planet) -> Field {
    let n = pl.height.len();
    let g = &pl.grid;
    let drop = |i: usize| {
        let r = pl.receiver[i] as usize;
        if r == i { 0.0 } else { (pl.height[i] - pl.height[r]).max(0.0) / g.cell_m as f32 }
    };
    let power: Vec<f32> = (0..n).map(|i| pl.discharge[i].max(0.0).sqrt() * drop(i)).collect();
    // Donors before receivers: a receiver is never higher than its donor, and
    // ties break on index, so every machine visits cells in the same order.
    let mut order: Vec<u32> = (0..n as u32).collect();
    order.sort_by(|&a, &b| pl.height[b as usize].total_cmp(&pl.height[a as usize]).then(a.cmp(&b)));
    let mut flux = vec![0.0f32; n];
    let mut upstream_power = vec![0.0f32; n];
    for &i in &order {
        let i = i as usize;
        if pl.land[i] {
            let [x, z] = g.at(i);
            // Gold leaves a belt only where it is being eroded.
            flux[i] += lode(cfg, x as f32, z as f32) * drop(i).min(0.6);
        }
        let r = pl.receiver[i] as usize;
        if r != i {
            flux[r] += flux[i];
            upstream_power[r] = upstream_power[r].max(power[i]);
        }
    }
    let mut grade: Vec<f32> = (0..n)
        .map(|i| {
            if !pl.land[i] || flux[i] <= 0.0 {
                return 0.0;
            }
            let trap = ((upstream_power[i] - power[i]) / (upstream_power[i] + 1e-3)).clamp(0.0, 1.0);
            flux[i] * g.cell_m as f32 * g.cell_m as f32 / (pl.discharge[i].max(1.0) + 4.0e5) * (0.2 + trap)
        })
        .collect();
    // Normalise on the rich tail so one bonanza cell does not flatten the rest.
    let mut rich: Vec<f32> = grade.iter().copied().filter(|v| *v > 0.0).collect();
    rich.sort_by(f32::total_cmp);
    let top = rich.get(rich.len().saturating_sub(1).min(rich.len() * 98 / 100)).copied().unwrap_or(1.0).max(1e-9);
    for v in &mut grade {
        *v = (*v / top).min(1.0);
    }
    Field { grade }
}

thread_local! {
    static HOT: RefCell<Option<((u32, u64), Arc<Field>)>> = const { RefCell::new(None) };
}

fn field(cfg: &WorldSampleConfig) -> Arc<Field> {
    let pl = crate::geo::planet_for(cfg);
    let key = (cfg.seed, pl.checksum);
    if let Some(f) = HOT.with(|h| h.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, f)| Arc::clone(f))) {
        return f;
    }
    let f = Arc::new(build(cfg, &pl));
    HOT.with(|h| *h.borrow_mut() = Some((key, Arc::clone(&f))));
    f
}

/// Placer grade 0..1 at a world point on the solved face, else `None`.
pub fn grade(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> Option<f32> {
    if !crate::basin::solved_face(cfg) || !crate::geo::active(cfg) {
        return None;
    }
    let pl = crate::geo::planet_for(cfg);
    if !pl.grid.contains(wx as f64, wz as f64) {
        return None;
    }
    Some(pl.grid.bilinear(&field(cfg).grade, wx as f64, wz as f64) as f32)
}
