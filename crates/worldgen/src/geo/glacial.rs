//! Ice discharge follows the drainage graph and leaves a glacial landscape.
//!
//! Heads quarry cirques; where several bite one summit a horn stands between
//! them. Trunks cut broad parabolic troughs, and tributaries cut shallower
//! ones, so their floors hang above the trunk. In the melt zone ice drops its
//! load: lateral moraines along the shoulders, an arc at the terminus, then
//! meltwater spreads an outwash fan downstream and buried ice blocks melt out
//! as kettle hollows that later fill as lakes. This
//! is a static landform pass, before the final lake and river routing.
use super::{SEA_M, climate::Climate, erosion, grid::Grid, noise::{smoothstep, unit}};

/// Ice flux (m² of snow catchment) that sustains a glacier.
const GLACIER_FLUX: f64 = 300_000.0;
const MAX_CUT_M: f64 = 36.0;
const MAX_CIRQUE_M: f64 = 60.0;
/// Glacial-maximum summer cooling against today's climate. Earth's last
/// maximum ran about 6 C colder, more toward the poles; compressed relief
/// needs the upper end to hold ice at all.
const GLACIAL_COOLING_C: f64 = 8.0;
/// Receiver steps an outwash fan runs below a snout.
const OUTWASH_CELLS: usize = 8;

/// Visits every cell within `radius_m` of cell `i` with its offset in metres.
fn near(grid: &Grid, i: usize, radius_m: f64, mut f: impl FnMut(usize, f64, f64)) {
    let reach = (radius_m / grid.cell_m).ceil() as i32 + 1;
    let (cx, cz) = ((i % grid.n) as i32, (i / grid.n) as i32);
    for oz in -reach..=reach {
        for ox in -reach..=reach {
            let (a, b) = (cx + ox, cz + oz);
            if a < 0 || b < 0 || a >= grid.n as i32 || b >= grid.n as i32 {
                continue;
            }
            f(b as usize * grid.n + a as usize, ox as f64 * grid.cell_m, oz as f64 * grid.cell_m);
        }
    }
}

/// Troughs only: the historical pass for solves before gate 129.
pub fn trough(grid: &Grid, height: &mut [f64], fixed: &[bool], climate: &Climate) {
    let snow: Vec<f64> = (0..grid.len()).map(|i| {
        let summer = climate.maat[i] as f64 + climate.season[i] as f64 - 8.0;
        (1.0 - smoothstep(-4.0, 3.0, summer)) * climate.wet[i] as f64
    }).collect();
    let flow = erosion::route(grid, height, fixed, &snow);
    let original = height.to_vec();
    let mut ice = vec![0.0; grid.len()];
    for &cell in flow.order.iter().rev() {
        let i = cell as usize;
        ice[i] += snow[i] * grid.cell_m * grid.cell_m;
        let r = flow.receiver[i] as usize;
        if r != i {
            let melt = smoothstep(-2.0, 12.0, climate.maat[i] as f64 + climate.season[i] as f64 - 8.0);
            ice[r] += ice[i] * (1.0 - 0.25 * melt);
        }
    }
    for i in 0..grid.len() {
        if fixed[i] || ice[i] < 300_000.0 { continue; }
        let r = flow.receiver[i] as usize;
        if r == i { continue; }
        let [x, z] = grid.at(i);
        let [rx, rz] = grid.at(r);
        let (dx, dz) = (rx - x, rz - z);
        let length = (dx * dx + dz * dz).sqrt();
        let strength = (ice[i] / 1.0e6).sqrt().min(5.0);
        let width = 60.0 + 40.0 * strength;
        let depth = 4.0 + 5.0 * strength;
        let reach = (width / grid.cell_m).ceil() as i32 + 1;
        let (cx, cz) = ((i % grid.n) as i32, (i / grid.n) as i32);
        for oz in -reach..=reach {
            for ox in -reach..=reach {
                let (a, b) = (cx + ox, cz + oz);
                if a < 0 || b < 0 || a >= grid.n as i32 || b >= grid.n as i32 { continue; }
                let j = b as usize * grid.n + a as usize;
                if fixed[j] { continue; }
                let (px, pz) = (ox as f64 * grid.cell_m, oz as f64 * grid.cell_m);
                let along = (px * dx + pz * dz) / length;
                if along.abs() > length { continue; }
                let across = ((px * dz - pz * dx) / length).abs() / width;
                if across >= 1.0 { continue; }
                let centre = original[i] + (original[r] - original[i]) * (along / length).clamp(0.0, 1.0);
                let trough = centre - depth + depth * across * across;
                let blend = 1.0 - smoothstep(0.65, 1.0, across);
                let cut = original[j] + (trough.min(original[j]) - original[j]) * blend;
                height[j] = height[j].min(cut.max(original[j] - 36.0).max(SEA_M + 0.6));
            }
        }
    }
}

/// Carves glacial landforms into `height`; returns former ice cover per cell, 0..1.
pub fn carve(grid: &Grid, height: &mut [f64], fixed: &[bool], climate: &Climate, seed: u32) -> Vec<f32> {
    let summer = |i: usize| climate.maat[i] as f64 + climate.season[i] as f64 - 8.0 - GLACIAL_COOLING_C;
    // Relict valleys record a colder glacial climate, not today's snowline.
    let snow: Vec<f64> =
        (0..grid.len()).map(|i| (1.0 - smoothstep(-4.0, 3.0, summer(i))) * climate.wet[i] as f64).collect();
    let melt: Vec<f64> = (0..grid.len()).map(|i| smoothstep(-2.0, 12.0, summer(i))).collect();
    let flow = erosion::route(grid, height, fixed, &snow);
    let original = height.to_vec();
    let mut ice = vec![0.0; grid.len()];
    for &cell in flow.order.iter().rev() {
        let i = cell as usize;
        ice[i] += snow[i] * grid.cell_m * grid.cell_m;
        let r = flow.receiver[i] as usize;
        if r != i {
            ice[r] += ice[i] * (1.0 - 0.25 * melt[i]);
        }
    }
    let glacier: Vec<bool> = (0..grid.len()).map(|i| !fixed[i] && ice[i] >= GLACIER_FLUX).collect();
    let mut fed = vec![false; grid.len()];
    for i in 0..grid.len() {
        let r = flow.receiver[i] as usize;
        if glacier[i] && r != i {
            fed[r] = true;
        }
    }
    let mut cover = vec![0.0f32; grid.len()];
    let mut deposit = vec![0.0f64; grid.len()];
    let mut outwash = vec![0.0f64; grid.len()];
    let mut kettles: Vec<(usize, f64, f64)> = Vec::new();
    let lower = |height: &mut [f64], j: usize, cut: f64, limit: f64| {
        height[j] = height[j].min(cut.max(original[j] - limit).max(SEA_M + 0.6));
    };
    for i in 0..grid.len() {
        if !glacier[i] {
            continue;
        }
        let strength = (ice[i] / 1.0e6).sqrt().min(5.0);
        let width = 60.0 + 40.0 * strength;
        let depth = 4.0 + 5.0 * strength;
        // A cirque is quarried below the head's own level, so the uphill side
        // becomes a headwall and the summit between cirques sharpens.
        if !fed[i] {
            let radius = (1.6 * width).max(2.0 * grid.cell_m);
            near(grid, i, radius, |j, px, pz| {
                let k = (px * px + pz * pz).sqrt() / radius;
                if fixed[j] || k >= 1.0 {
                    return;
                }
                let floor = original[i] - 2.0 * depth * (1.0 - k * k);
                let blend = 1.0 - smoothstep(0.7, 1.0, k);
                lower(height, j, original[j] + (floor.min(original[j]) - original[j]) * blend, MAX_CIRQUE_M);
                cover[j] = cover[j].max(blend as f32);
            });
        }
        let r = flow.receiver[i] as usize;
        if r == i {
            continue;
        }
        let [x, z] = grid.at(i);
        let [rx, rz] = grid.at(r);
        let (dx, dz) = (rx - x, rz - z);
        let length = (dx * dx + dz * dz).sqrt();
        let ablation = melt[i] > 0.35;
        let moraine_m = (1.5 + 1.5 * strength) * melt[i];
        near(grid, i, 1.3 * width, |j, px, pz| {
            if fixed[j] {
                return;
            }
            let along = (px * dx + pz * dz) / length;
            if along.abs() > length {
                return;
            }
            let across = ((px * dz - pz * dx) / length).abs() / width;
            if across < 1.0 {
                let centre = original[i] + (original[r] - original[i]) * (along / length).clamp(0.0, 1.0);
                // Broad floor, steep shoulders. Blend into the untouched slope
                // at the edge; overlapping cuts combine by minimum.
                let trough = centre - depth + depth * across * across;
                let blend = 1.0 - smoothstep(0.65, 1.0, across);
                lower(height, j, original[j] + (trough.min(original[j]) - original[j]) * blend, MAX_CUT_M);
                cover[j] = cover[j].max(blend as f32);
            } else if ablation && across < 1.3 {
                let k = (across - 1.15) / 0.15;
                deposit[j] = deposit[j].max(moraine_m * (1.0 - k * k).max(0.0));
            }
        });
        // The snout stalls where melt balances supply and dumps an arc of till.
        if !glacier[r] {
            let crest_m = 3.0 + 3.0 * strength;
            near(grid, r, 1.4 * width, |j, px, pz| {
                if fixed[j] {
                    return;
                }
                let along = (px * dx + pz * dz) / length / (0.35 * width);
                let across = ((px * dz - pz * dx) / length).abs() / (1.4 * width);
                if along.abs() < 1.0 && across < 1.0 {
                    let arc = (1.0 - along * along) * (1.0 - smoothstep(0.6, 1.0, across));
                    deposit[j] = deposit[j].max(crest_m * arc);
                }
            });
            // Braided meltwater spreads sand and gravel, thinning downstream.
            let mut c = r;
            for k in 0..OUTWASH_CELLS {
                let next = flow.receiver[c] as usize;
                if next == c || fixed[c] {
                    break;
                }
                let fade = 1.0 - k as f64 / OUTWASH_CELLS as f64;
                let fan = width * (1.0 + 0.4 * k as f64);
                let thick = (1.5 + strength) * fade;
                near(grid, c, fan, |j, px, pz| {
                    let k = (px * px + pz * pz).sqrt() / fan;
                    if !fixed[j] && k < 1.0 {
                        outwash[j] = outwash[j].max(thick * (1.0 - k * k));
                    }
                });
                // Stranded ice blocks sit in the fan behind the moraine arc.
                if (1..4).contains(&k) && unit(c as i64, k as i64, seed ^ 0x6e77) < 0.45 {
                    kettles.push((c, (0.25 + 0.25 * unit(c as i64, 9, seed ^ 0x6e78)) * width, 3.0 + 3.0 * fade));
                }
                c = next;
            }
        }
    }
    // Till rests on uncut ground only, so moraines never fill a trough floor.
    for j in 0..grid.len() {
        if original[j] - height[j] < 1.0 {
            if deposit[j] > 0.0 {
                cover[j] = cover[j].max(0.5);
            }
            height[j] += deposit[j].max(outwash[j]);
        }
    }
    // Kettles cut through the deposit; routing then holds them as lakes.
    for &(c, radius, depth) in &kettles {
        let base = height[c];
        near(grid, c, radius, |j, px, pz| {
            let k = (px * px + pz * pz).sqrt() / radius;
            if !fixed[j] && k < 1.0 {
                height[j] = height[j].min((base - depth * (1.0 - k * k)).max(SEA_M + 0.6));
            }
        });
    }
    cover
}
