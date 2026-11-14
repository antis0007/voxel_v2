//! A second, physically connected circuit beneath the radial streets.
use super::{architecture, weave};
use crate::{BlockId, WorldSampleConfig};
use glam::{IVec3, Vec3};

pub const VERSION: u32 = 87;

pub fn heart(cfg: &WorldSampleConfig) -> IVec3 {
    (weave::point(0, 0.0, -2.6, -22.3) / cfg.voxel_meters()).floor().as_ivec3()
}

pub(super) fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    let (d, x, h, z) = weave::coordinates(p);
    let old = || architecture::block(cfg, cell);
    // Preserve the outer safety shell and both end caps.
    if h <= -6.0 || z.abs() >= 24.0 || h > 12.0 { return old(); }
    if cell == heart(cfg) { return BlockId::Crystal; }

    // Two mouth-shaped entrances. Their steps descend into the same continuous
    // outer circuit; neither entrance requires a fold or a quest flag.
    let entrance = d == 0 || d == 3;
    let sx = x + 8.4;
    if entrance && sx.abs() < 1.7 && (-19.5..=-7.5).contains(&z) {
        let floor = ((z + 9.0) * 0.5).clamp(-5.0, 0.0);
        if h > floor && h < floor + 3.6 { return BlockId::Air; }
        if h <= floor && h > floor - 0.75 { return BlockId::Limestone; }
    }
    if entrance && sx.abs() < 0.5 && (z + 9.1).abs() < 0.65 && (3.7..5.4).contains(&h) {
        return BlockId::CarvedStone;
    }
    if entrance && sx.abs() < 2.5 && (z + 10.2).abs() < 0.9 && (0.0..8.5).contains(&h) {
        let mouth = sx.abs() < 1.7 && h < 2.9;
        let eyes = (0.7..1.9).contains(&sx.abs()) && (5.4..6.6).contains(&h);
        let broken_crown = h > 7.0 && sx > 0.3 + (8.5 - h) * 0.8;
        let split_cheek = (sx + 1.95).abs() < 0.18 && (3.3..5.4).contains(&h);
        return if mouth || eyes || broken_crown || split_cheek { BlockId::Air }
            else if h > 7.7 || sx.abs() > 2.0 { BlockId::WeatheredBricks }
            else { BlockId::Limestone };
    }

    // Keep a metre of original shell above the ceiling. Circular travel takes
    // the player under all six districts, with two independently usable exits.
    let room = (d == 0 || d == 3) && x.abs() < 6.4 && (-23.4..-14.8).contains(&z);
    let passage = (z + 19.0).abs() < 2.4;
    if !room && !passage { return old(); }
    if h <= -5.0 && h > -5.8 {
        return if (z + 19.0).abs() < 0.3 { BlockId::CopperBlock } else { BlockId::WeatheredBricks };
    }
    if !(-5.0..-0.85).contains(&h) { return old(); }

    // Deep ribs leave a broad central route. Broken side niches hold ore that
    // is recovered through ordinary mining, with no respawning reward trigger.
    if !room {
        if x.abs() < 0.65 && ((z + 19.0).abs() > 1.5 || h > -1.6) {
            return BlockId::CarvedStone;
        }
        if (d == 1 || d == 4) && x.abs() < 2.0 && z < -20.8 && h < -3.5 {
            return if d == 1 { BlockId::CopperOre } else { BlockId::IronOre };
        }
        return BlockId::Air;
    }
    // Four piers provide real line-of-sight cover from the resonator.
    if (x.abs() - 4.1).abs() < 0.7 && ((z + 17.0).abs() < 0.65 || (z + 21.4).abs() < 0.65) {
        return if h > -1.7 || h < -4.3 { BlockId::CarvedStone } else { BlockId::DressedStone };
    }
    if d == 0 {
        // Shallow vault ribs and suspended funerary masks make the far wall
        // recognisable from the service passage without filling the exits.
        let rib = (z + 16.0).abs() < 0.3 || (z + 20.2).abs() < 0.3;
        if rib && h > -1.25 - 0.75 * (x / 6.4).powi(2) { return BlockId::CarvedStone; }
        let mask_x = x.abs() - 2.8;
        if (z + 22.8).abs() < 0.45 {
            if mask_x.abs() < 0.2 && h > -1.8 { return BlockId::CopperBlock; }
            if mask_x.abs() < 0.8 && (-3.4..-1.8).contains(&h) {
                // A slit through the pale mask exposes the dark wall behind.
                return if (-2.6..-2.15).contains(&h) && mask_x.abs() > 0.2 {
                    BlockId::Air
                } else { BlockId::Limestone };
            }
        }
        // A squat faceless idol; the exposed throat is the actual editable
        // source. Its surrounding masonry does not regenerate after mining.
        if x.abs() < 1.9 && (z + 22.8).abs() < 0.6 {
            return if h > -1.65 || (x.abs() > 0.75 && h > -3.5) {
                BlockId::Limestone
            } else { BlockId::Basalt };
        }
        if (x.abs() - 2.8).abs() < 0.65 && z < -22.0 && h < -3.7 {
            return BlockId::TinOre;
        }
    } else {
        // Empty benches face a permanently closed ticket window. Quiet room:
        // no source, no enemy, no trigger hidden behind the furnishings.
        let seat = (x.abs() - 2.7).abs() < 0.7 && (z + 19.0).abs() < 2.5;
        if seat && (h < -4.2 || ((x.abs() - 3.15).abs() < 0.22 && h < -3.3)) {
            return BlockId::Planks;
        }
        if x.abs() < 2.0 && z < -22.4 && h < -3.6 { return BlockId::CopperBlock; }
        if x.abs() < 1.0 && z < -23.0 && (-3.5..-1.8).contains(&h) { return BlockId::Basalt; }
    }
    BlockId::Air
}
