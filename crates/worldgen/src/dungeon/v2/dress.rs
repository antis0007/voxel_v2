//! What fills a room: per-kind furniture with seeded layout variants, the
//! damage that happened to it, and the job's landmark in the reveal hall
//! (`landmark.rs`). Every prop keeps a 2.4 m cross through the room centre
//! clear, so each door can reach each other door.
use super::{Complex, Damage, Kind, Room, chasm, kit::near_line, landmark, maze::labyrinth, outline, starry, treasure};
use crate::{BlockId as B, hash2, hash3, value_noise3};

pub(super) fn h(r: &Room, k: u32) -> f32 {
    hash2(k as i32, 3, r.salt)
}

fn cell(rx: f32, fy: f32, rz: f32, v: f32) -> (i32, i32, i32) {
    ((rx / v).floor() as i32, (fy / v).floor() as i32, (rz / v).floor() as i32)
}

/// Below-floor space: pits, basins, trenches, star floors.
pub fn below(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (w, d) = (r.w(), r.d());
    let lane = (rx - w * 0.5).abs() < 1.2 || (rz - d * 0.5).abs() < 1.2;
    let dwall = rx.min(w - rx).min(rz).min(d - rz);
    match r.kind {
        Kind::Pitfall => {
            // Real floor: the aisle cross, and a walkway in from every door.
            let (mx, mz) = (rx + r.x0, rz + r.z0);
            let walkway = c.corrs.iter().any(|k| {
                if k.along_x { mz >= k.z0 && mz < k.z1 && k.x1 > r.x0 && k.x0 < r.x1 } else { mx >= k.x0 && mx < k.x1 && k.z1 > r.z0 && k.z0 < r.z1 }
            });
            if lane || walkway {
                return None;
            }
            if fy >= -v {
                return Some(B::MirageBrick);
            }
            // Stepped ledges along the walls let a fallen player climb out.
            if dwall < 2.2 {
                let along = if rx.min(w - rx) < rz.min(d - rz) { rz } else { rx };
                let step = (along.rem_euclid(6.0) / 6.0 * 3.0 / v).floor() * v;
                return (fy < -3.0 + step).then_some(c.pal.trim).or(Some(B::Air));
            }
            Some(if fy < -2.4 { B::Water } else { B::Air })
        }
        Kind::Cistern | Kind::Sluice => {
            let column = near_line(rx, c.job.bay(), v, 0.6) && near_line(rz, c.job.bay(), v, 0.6);
            if dwall < 2.0 || (rx - w * 0.5).abs() < 1.2 || column {
                return None;
            }
            Some(if fy < -0.4 { B::Water } else { B::Air })
        }
        Kind::Foundry => {
            let (along, across) = if w >= d { (rx, rz - d * 0.5) } else { (rz, rx - w * 0.5) };
            if across.abs() < 0.9 && dwall > 1.5 && !near_line(along, 8.0, v, 1.0) {
                return Some(if fy < -0.3 { B::Lava } else { B::Air });
            }
            None
        }
        Kind::Chasm => chasm::below(c, r, rx, rz, fy, v),
        Kind::Orrery => starry::below(c, r, rx, rz, fy, v, outline::depth(r, rx, rz)),
        Kind::Reveal => landmark::below(c, r, rx, rz, fy, v),
        _ => None,
    }
}

/// Space between floor and ceiling. `None` is air.
pub fn furnish(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32, dwall: f32) -> Option<B> {
    let (w, d) = (r.w(), r.d());
    let lane = (rx - w * 0.5).abs() < 1.2 || (rz - d * 0.5).abs() < 1.2;
    if r.cache
        && let Some(b) = treasure::cache(c, r, rx, rz, fy, v)
    {
        return Some(b);
    }
    if c.version >= 46 && r.cache {
        let (x, z) = treasure::cache_spot(c, r);
        if (rx - x).abs() < 1.2 && (rz - z).abs() < 1.2 && fy < 3.0 {
            // Keep new desks and shelves from burying a branch's reward.
            return Some(B::Air);
        }
    }
    let prop = if lane && !matches!(r.kind, Kind::Reveal | Kind::Labyrinth | Kind::Orrery | Kind::Core) {
        None
    } else {
        props(c, r, rx, rz, fy, v, dwall)
    };
    if prop.is_some() {
        return prop;
    }
    damage(c, r, rx, rz, fy, v, dwall)
}

fn damage(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32, dwall: f32) -> Option<B> {
    let (w, d) = (r.w(), r.d());
    let (i, j, k) = cell(rx, fy, rz, v);
    match r.damage {
        Damage::Intact => None,
        Damage::Flooded => (fy < 1.0).then_some(B::Water),
        Damage::Collapsed if (rx - w * 0.5).abs() < 1.2 || (rz - d * 0.5).abs() < 1.2 => None,
        Damage::Collapsed => {
            // A rubble cone under a hashed ceiling breach, off the centre lane.
            let (qx, qz) = super::collapse::centre(r);
            let reach = w.min(d) * 0.22 * (1.0 - fy / (r.h * 0.9)).max(0.0);
            let wobble = value_noise3(rx * 0.7, fy * 0.7, rz * 0.7, r.salt) * 1.2;
            ((rx - qx).hypot(rz - qz) + wobble < reach)
                .then(|| if hash3(i, j, k, r.salt) > 0.6 { B::Cobblestone } else { B::Gravel })
        }
        Damage::Overgrown => {
            if fy < v && hash3(i, 0, k, r.salt.wrapping_add(1)) > 0.55 {
                return Some(B::FungalSoil);
            }
            if (v..2.0 * v).contains(&fy) && hash3(i, 0, k, r.salt.wrapping_add(2)) > 0.9 {
                return Some(if hash3(i, 1, k, r.salt) > 0.5 { B::GlowCap } else { B::RedCap });
            }
            // Roots hanging from the ceiling on a loose grid.
            if c.version >= super::ruin::VERSION && v <= 0.5 {
                return super::ruin::root(r, rx, rz, fy, v);
            }
            let (gx, gz) = ((rx / 3.0).floor() as i32, (rz / 3.0).floor() as i32);
            let root = hash2(gx, gz, r.salt.wrapping_add(3));
            let (ox, oz) = (gx as f32 * 3.0 + 1.5, gz as f32 * 3.0 + 1.5);
            (root > 0.6 && (rx - ox).abs() < v.max(0.5) && (rz - oz).abs() < v.max(0.5) && fy > r.h - 1.0 - root * 2.5)
                .then_some(B::Log)
        }
        Damage::Breach => {
            // Rubble spilled from the broken wall (see `breach_hole`).
            let side = (h(r, 23) * 4.0) as u8 % 4;
            let out = [rx, w - rx, rz, d - rz][side as usize];
            let along = [rz - d * 0.5, rz - d * 0.5, rx - w * 0.5, rx - w * 0.5][side as usize];
            let spill = 3.0 - along.abs() * 0.5 - fy * 1.2;
            let _ = dwall;
            (out < spill && hash3(i, j, k, c.seed) > 0.25).then_some(B::Gravel)
        }
    }
}

/// A ragged hole through the shell on the breach side, rock beyond.
pub fn breach_hole(r: &Room, rx: f32, rz: f32, fy: f32) -> bool {
    if r.damage != Damage::Breach {
        return false;
    }
    let (w, d) = (r.w(), r.d());
    let side = (h(r, 23) * 4.0) as u8 % 4;
    let out = [-rx, rx - w, -rz, rz - d][side as usize];
    if out < 0.0 {
        return false;
    }
    let along = [rz - d * 0.5, rz - d * 0.5, rx - w * 0.5, rx - w * 0.5][side as usize];
    let ragged = value_noise3(rx * 0.5, fy * 0.5, rz * 0.5, r.salt.wrapping_add(7)) * 1.5;
    (along / 2.4).powi(2) + ((fy - 1.8) / 1.8).powi(2) < 1.0 + ragged * 0.4
}

fn props(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32, dwall: f32) -> Option<B> {
    if c.version >= 46 && matches!(r.kind, Kind::Library | Kind::Scriptorium
        | Kind::Refectory | Kind::Infirmary | Kind::Armory) {
        return super::inhabited::furnish(r, rx, rz, fy, v, dwall);
    }
    let (w, d) = (r.w(), r.d());
    let variant = (h(r, 1) * 3.0) as u32;
    let bay = c.job.bay();
    let (along, across, len, wid) = if w >= d { (rx, rz, w, d) } else { (rz, rx, d, w) };
    match r.kind {
        Kind::Entry => {
            let corner = rx.min(w - rx) < 3.0 && rz.min(d - rz) < 3.0 && dwall > 0.9;
            (corner && fy < 1.0 && h(r, 2) > 0.3 && (rx < w * 0.5) == (h(r, 3) < 0.5)).then_some(B::SalvageCrate)
        }
        Kind::Ossuary => {
            // Bone niches between the piers, and the tally: they kept count.
            if dwall < 1.2 && dwall >= v.max(0.6) && fy > 0.4 && fy < r.h - 1.2 {
                let niche = fy.rem_euclid(1.2) < 0.6 && !near_line(along, 1.5, v, 0.0);
                return Some(if niche { B::Gravel } else { c.pal.alt });
            }
            (variant == 0 && dwall > 3.0 && fy < 1.2 - ((rx - w * 0.5).abs() + (rz - d * 0.5).abs()) * 0.15)
                .then_some(B::Gravel)
        }
        Kind::Crypt | Kind::Core => {
            let tomb = match variant {
                0 => near_line(along, 4.0, v, 1.0) && (across.rem_euclid(5.0) - 2.5).abs() < 0.6,
                1 => dwall > 1.4 && dwall < 2.6 && !near_line(along, 4.0, v, 1.4),
                _ => (rx - w * 0.3).abs() < 1.4 && (rz - d * 0.3).abs() < 0.8,
            };
            if r.kind == Kind::Core {
                return treasure::core(c, r, rx, rz, fy, v);
            }
            if tomb && fy < 1.0 {
                return Some(if fy >= 1.0 - v { c.pal.trim } else { c.pal.alt });
            }
            None
        }
        Kind::Shrine => {
            // Dais and altar against one end wall, braziers either side.
            let end = if h(r, 4) < 0.5 { along } else { len - along };
            let mid = (across - wid * 0.5).abs();
            if end < 3.0 && mid < wid * 0.3 && fy < 0.5 {
                return Some(c.pal.trim);
            }
            if end < 1.6 && mid < 1.0 && fy < 1.5 {
                return Some(if fy >= 1.5 - v { c.pal.accent } else { c.pal.alt });
            }
            if end < 2.0 && (mid - wid * 0.3 - 0.6).abs() < 0.6 && fy < 1.8 + v {
                return Some(if fy >= 1.8 { B::Torch } else { c.pal.trim });
            }
            None
        }
        Kind::Barracks => {
            let bunk = dwall > 1.0 && dwall < 3.2 && !near_line(along, bay, v, 1.0);
            (bunk && ((fy < 0.6) || (1.4..1.4 + v).contains(&fy) || (near_line(along, 2.0, v, 0.0) && fy < 2.0)))
                .then_some(B::Planks)
        }
        Kind::Cistern | Kind::Sluice => {
            let column = near_line(rx, bay, v, 0.6) && near_line(rz, bay, v, 0.6) && dwall > 2.0;
            if column {
                return Some(if fy < 0.6 || fy > r.h - 1.0 { c.pal.trim } else { c.pal.wall });
            }
            // The sluice: a gate wall across the room with a wheel.
            if r.kind == Kind::Sluice && (along - len * 0.5).abs() < v && fy < r.h * 0.5 && (across - wid * 0.5).abs() > 1.2 {
                return Some(c.pal.accent);
            }
            None
        }
        Kind::Store => {
            let (gx, gz) = ((rx / 2.5).floor() as i32, (rz / 2.5).floor() as i32);
            let stack = hash2(gx, gz, r.salt.wrapping_add(9));
            (dwall > 0.9 && stack > 0.7 && fy < if stack > 0.85 { 2.0 } else { 1.0 }).then_some(if stack > 0.97 {
                B::SalvageCrate
            } else {
                B::Planks
            })
        }
        Kind::Forge => {
            // Furnaces along one wall: brick blocks with a lava hearth.
            let furnace = (1.0..3.5).contains(&dwall) && near_line(along, 7.0, v, 1.6);
            if furnace && fy < 3.0 {
                let hearth = dwall < 2.0 && fy < 1.0 && near_line(along, 7.0, v, 0.6);
                return Some(if hearth { B::Lava } else { c.pal.alt });
            }
            let anvil = (rx - w * 0.7).abs() < 0.6 && (rz - d * 0.7).abs() < 0.6 && fy < 1.0;
            let ore = dwall < 2.0 && near_line(along, 5.0, v, 0.6) && !furnace && fy < 0.6;
            anvil.then_some(B::Basalt).or(ore.then_some(if h(r, 5) < 0.5 { B::CoalOre } else { B::IronOre }))
        }
        Kind::Foundry => {
            let crucible = (across - wid * 0.5).abs() > 2.0 && (across - wid * 0.5).abs() < 3.2 && near_line(along, 8.0, v, 1.0);
            (crucible && fy < 2.5).then_some(if fy < 2.5 - v { c.pal.alt } else { c.pal.trim })
        }
        Kind::Garden => {
            let bed = dwall > 1.4 && near_line(across, 3.0, v, 0.9) && !near_line(along, 9.0, v, 1.0);
            if bed && fy < v {
                return Some(B::FungalSoil);
            }
            if bed && (v..2.0 * v).contains(&fy) {
                let (i, _, k) = cell(rx, fy, rz, v);
                let x = hash2(i, k, r.salt.wrapping_add(11));
                return (x > 0.5).then_some(if x > 0.8 { B::GlowCap } else { B::RedCap });
            }
            None
        }
        Kind::Cells => {
            // Cells along both long walls; doors sealed from outside.
            let depth = across.min(wid - across);
            if depth < 3.0 && near_line(along, 3.0, v, 0.0) {
                return Some(c.pal.wall);
            }
            if depth >= 3.0 - v && depth < 3.0 {
                let door = (along.rem_euclid(3.0) - 1.5).abs() < 0.7;
                return Some(if door {
                    if hash2((along / 3.0) as i32, (across > wid * 0.5) as i32, r.salt) > 0.5 { B::Planks } else { B::MirageBrick }
                } else if near_line(along, 1.0, v, 0.0) {
                    c.pal.trim
                } else {
                    return None;
                });
            }
            None
        }
        Kind::Library => {
            let shelf = near_line(across, 3.0, v, 0.0) && dwall > 1.4 && !near_line(along, 6.0, v, 1.2);
            if shelf && fy < 3.0 {
                let (i, j, k) = cell(rx, fy, rz, v);
                return Some(if fy.rem_euclid(1.0) < v * 0.5 || hash3(i, j, k, r.salt) > 0.7 { B::Planks } else { c.pal.alt });
            }
            None
        }
        Kind::Scriptorium | Kind::Refectory | Kind::Infirmary | Kind::Armory => None,
        Kind::Orrery => starry::furnish(c, r, rx, rz, fy, v),
        Kind::Chasm => chasm::furnish(c, r, rx, rz, fy, v),
        Kind::Labyrinth => labyrinth(c, r, rx, rz, fy, v),
        Kind::Reveal => landmark::furnish(c, r, rx, rz, fy, v),
        Kind::Pitfall => {
            let post = dwall < 1.0 && ((rx - w * 0.5).abs() < 1.8 || (rz - d * 0.5).abs() < 1.8) && ((rx - w * 0.5).abs() > 1.2 && (rz - d * 0.5).abs() > 1.2);
            // Pillars where the aisles meet the walls, a lamp on each.
            if post && (2.4..2.4 + v).contains(&fy) {
                return Some(B::Torch);
            }
            (post && fy < 2.4).then_some(c.pal.trim)
        }
    }
}
