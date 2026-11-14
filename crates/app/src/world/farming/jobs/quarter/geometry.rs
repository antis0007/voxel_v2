//! Parametric frontage, circulation and roofs share construction cells.
use super::{B, IVec3, Lot, Roof, Tradition};
use std::collections::BTreeMap;

pub const WIDTH: i32 = 9;
pub const DEPTH: i32 = 15;

pub fn plan_for_scale(
    lot: &Lot,
    style: &Tradition,
    scale: crate::fluid::VoxelScalePreset,
) -> Vec<(IVec3, B)> {
    let factor = scale.meters().recip().round() as i32;
    let coarse = plan(lot, style);
    let mut fine = Vec::with_capacity(coarse.len() * factor.pow(3) as usize);
    for (p, b) in coarse {
        for y in 0..factor {
            for z in 0..factor {
                for x in 0..factor {
                    let sign = if lot.turn { -1 } else { 1 };
                    fine.push((p * factor + IVec3::new(sign * x, y, sign * z), b));
                }
            }
        }
    }
    fine
}

pub fn at(lot: &Lot, p: IVec3) -> IVec3 {
    IVec3::from(lot.origin)
        + if lot.turn {
            IVec3::new(-p.x, p.y, -p.z)
        } else {
            p
        }
}

pub fn plan(lot: &Lot, style: &Tradition) -> Vec<(IVec3, B)> {
    let mut cells = BTreeMap::new();
    let mut put = |x, y, z, b| {
        cells.insert((y, z, x), b);
    };
    let levels = i32::from(lot.complexity);
    let top = levels * 4;
    for level in 0..levels {
        let base = level * 4;
        let stair_x = if level % 2 == 0 { WIDTH - 2 } else { 1 };
        let previous_stair_x = if level % 2 == 0 { 1 } else { WIDTH - 2 };
        for z in 0..DEPTH {
            for x in 0..WIDTH {
                let court =
                    style.form == Roof::Courtyard && (3..=5).contains(&x) && (5..=9).contains(&z);
                let stairwell =
                    level > 0 && x == previous_stair_x && (DEPTH - 5..=DEPTH - 2).contains(&z);
                if !court && !stairwell {
                    put(x, base, z, if level == 0 { style.frame } else { B::Planks });
                }
                let boundary = x == 0 || x == WIDTH - 1 || z == 0 || z == DEPTH - 1;
                if !boundary {
                    continue;
                }
                for h in 1..=3 {
                    let door = z == 0 && (3..=5).contains(&x) && h <= 2;
                    let window = h == 2
                        && ((z == 0 && (x == 1 || x == 7))
                            || ((x == 0 || x == WIDTH - 1)
                                && z % 4 == 1 + (lot.variant % 2) as i32));
                    if door {
                        continue;
                    }
                    let frame = x == 0 || x == WIDTH - 1 || z % 4 == 0 || h == 3;
                    put(
                        x,
                        base + h,
                        z,
                        if window {
                            B::WindowGlass
                        } else if frame {
                            style.frame
                        } else {
                            style.wall
                        },
                    );
                }
            }
        }
        // Each storey has a rear service stair and an interior stairwell.
        for i in 0..4 {
            for y in base..=base + i {
                put(stair_x, y, DEPTH - 2 - i, B::Planks);
            }
        }
        if level + 1 < levels {
            put(stair_x, base + 4, DEPTH - 6, B::Planks);
        }
        if level > 0 && lot.variant % 2 == 0 {
            for x in 2..=6 {
                for z in -2..0 {
                    put(x, base, z, B::Planks);
                }
            }
            for x in [2, 6] {
                put(x, base + 1, -2, style.frame);
            }
        }
    }
    // The rear stair gives the builder access before ceilings close.
    for i in 0..top {
        for x in WIDTH..=WIDTH + 1 {
            for y in 0..=i {
                put(x, y, DEPTH - 1 - i, style.frame);
            }
        }
    }
    match style.form {
        Roof::Gable => {
            for x in -1..=WIDTH {
                for z in -1..=DEPTH {
                    let rise = (x + 1).min(WIDTH - x).max(0);
                    put(x, top + rise, z, style.roof);
                }
            }
            for x in 0..WIDTH {
                for h in 0..(x + 1).min(WIDTH - x) {
                    for z in [0, DEPTH - 1] {
                        put(x, top + h, z, style.wall);
                    }
                }
            }
        }
        Roof::Terrace | Roof::Courtyard => {
            for z in 0..DEPTH {
                for x in 0..WIDTH {
                    let court = style.form == Roof::Courtyard
                        && (3..=5).contains(&x)
                        && (5..=9).contains(&z);
                    if !court {
                        put(x, top, z, style.roof);
                    }
                    if x == 0 || x == WIDTH - 1 || z == 0 || z == DEPTH - 1 {
                        // Keep a permanent one-metre gate aligned with the highest exterior step.
                        let gate = lot.roof_access && style.form == Roof::Courtyard
                            && x == WIDTH - 1 && z == DEPTH - top;
                        if !gate && (style.form == Roof::Courtyard || (x + z) % 2 == 0) {
                            put(x, top + 1, z, style.frame);
                        }
                    }
                }
            }
        }
    }
    if lot.frontage {
        for x in 3..=5 { for z in -2..0 { put(x, 0, z, B::Cobblestone); } }
    }
    if lot.market_cover {
        for (p, block) in market_cover() { put(p.x, p.y, p.z, block); }
    }
    cells
        .into_iter()
        .map(|((y, z, x), b)| (IVec3::new(x, y, z), b))
        .collect()
}

/// Low market cover and exterior work steps, paid through the same project.
pub fn market_cover() -> Vec<(IVec3, B)> {
    let mut cells = Vec::with_capacity(37);
    for x in [-12, -8] { for z in [4, 8] {
        for y in 0..2 { cells.push((IVec3::new(x, y, z), B::Log)); }
        let step = if x == -12 { x - 1 } else { x + 1 };
        cells.push((IVec3::new(step, 0, z), B::Planks));
    }}
    for x in -12..=-8 { for z in 4..=8 { cells.push((IVec3::new(x, 2, z), B::Planks)); }}
    cells
}
