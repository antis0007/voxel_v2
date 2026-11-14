//! A compact market settlement within the existing surveyed village footprint.
//! Common grade, inward-facing entrances, a square and a rear service circuit.
use super::*;
mod craft;
mod workshop;
mod hall;
mod facade;
mod residence;
pub(super) use residence::Prepared;
pub(super) const RESIDENCE_GENERATOR: u32 = residence::GENERATOR;

const PLOTS: [(f32, f32, f32, f32, u8); 6] = [
    (-22.0, 10.0, 7.0, 6.0, 0), (0.0, 12.0, 9.0, 7.0, 1),
    (22.0, 10.0, 8.0, 7.0, 2), (-22.0, -10.0, 7.0, 6.0, 0),
    (0.0, -12.0, 8.0, 6.0, 3), (22.0, -10.0, 6.0, 6.0, 0),
];

// A cardinal, boundary-centred frame gives every reflected cell a partner.
pub(super) fn workshop_frame(plan: &Plan, cfg: &WorldSampleConfig) -> (Vec3, Vec3, Vec3) {
    let v = cfg.voxel_meters();
    let right = Vec3::new(-plan.hamlet_fwd.z, 0.0, plan.hamlet_fwd.x);
    let z = if right.x.abs() > right.z.abs() { Vec3::X * right.x.signum() } else { Vec3::Z * right.z.signum() };
    let x = Vec3::new(z.z, 0.0, -z.x);
    let mut origin = plan.hamlet + plan.hamlet_fwd * 22.0 + right * 10.0;
    origin.x = (origin.x / v).round() * v;
    origin.z = (origin.z / v).round() * v;
    origin.y = cfg.world_y(cfg.blocks_for_height(plan.hamlet.y + 22.0 * plan.hamlet_grade));
    (origin, x, z)
}

pub(super) fn sample(plan: &Plan, cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    let (a, b) = village_axes(plan, m.x, m.z);
    let reach = if plan.settlement.is_some() { 21.0 } else { 19.0 };
    if a.abs() >= 35.0 || b.abs() >= reach { return None; }
    let v = cfg.voxel_meters();
    let floor_at = |x: f32| cfg.world_y(cfg.blocks_for_height(plan.hamlet.y + x * plan.hamlet_grade));
    // The inn and workshop bookend the square; homes occupy the outer plots.
    // Staggered setbacks give each door its own forecourt without narrowing
    // the main street or changing the terrain survey's reserved footprint.
    if let Some(layout) = &plan.settlement {
        if let Some(block) = layout.sample(m, floor_at(a), v) { return Some(block); }
    } else {
    for (x, z, w, d, role) in PLOTS {
        let role = if cfg.generator_version >= 54 && role == 0 && x < 0.0 { 4 } else { role };
        let facing = if z < 0.0 { -1.0 } else { 1.0 };
        let (lx, lz, ly) = ((a - x) * facing, (b - z) * facing, m.y - floor_at(x));
        // The shared timber sampler expects its caller to bound the parcel
        // vertically; otherwise its interior air would carve to bedrock.
        if (-6.0..14.0).contains(&ly) {
            if cfg.generator_version >= 61 && role == 2 {
                let (origin, ax, az) = workshop_frame(plan, cfg);
                let q = m - origin;
                if let Some(block) = facade::sample(q.dot(ax), q.y, q.dot(az), v, ax.x.abs() > 0.5) { return Some(block); }
                let start = origin - az * 4.0;
                let right = Vec3::new(-plan.hamlet_fwd.z, 0.0, plan.hamlet_fwd.x);
                let finish = plan.hamlet + plan.hamlet_fwd * 22.0 + right * 2.0;
                let path = Vec3::new(finish.x-start.x, 0.0, finish.z-start.z);
                let delta = Vec3::new(m.x-start.x, 0.0, m.z-start.z);
                let along = (delta.dot(path) / path.length_squared().max(0.01)).clamp(0.0,1.0);
                if (delta-path*along).length_squared() < 1.5*1.5 {
                    let y = m.y - (origin.y + (floor_at(a)-origin.y)*along);
                    if y < v && y >= -2.0 { return Some(B::DressedStone); }
                    if y >= v && y < 3.0 { return Some(B::Air); }
                }
                continue;
            }
            if cfg.generator_version >= 58 && role == 2 {
                if let Some(block) = hall::sample(lx, ly, lz, v) { return Some(block); }
                continue;
            }
            if cfg.generator_version >= 57 && role == 2 {
                if let Some(block) = workshop::sample(lx, ly, lz, v) { return Some(block); }
                // This parcel has a complete new sampler; never resurrect its old roof.
                continue;
            }
            if cfg.generator_version >= 56 {
                let house = if cfg.generator_version >= 66 {
                    craft::house_versioned(role, lx, ly, lz, w, d, v, true)
                } else {
                    craft::house(role, lx, ly, lz, w, d, v)
                };
                if let Some(block) = house {
                    return Some(if cfg.generator_version >= 57 && block == B::Planks {
                        B::RoofTiles
                    } else { block });
                }
            }
            // Keep the front threshold connected to the room: the legacy inn
            // counter and central partition can otherwise close this aisle.
            if cfg.generator_version >= 44 && lx.abs() < v.max(1.0)
                && lz >= -d * 0.5 && lz < 0.5
                && ly >= v.max(0.4) && ly < 2.6 {
                return Some(B::Air);
            }
            if role == 4 && lx.abs() > 1.2 && lx.abs() < w * 0.5 - v
                && lz.abs() > d * 0.5 - v && lz.abs() <= d * 0.5
                && (4.1..5.3).contains(&ly) { return Some(B::Air); }
            if let Some(block) = timber(role, lx, ly, lz, w, d, v) {
                if cfg.generator_version >= 54 && block == B::Planks {
                    let top = if role == 1 || role == 4 { 5.8 } else { 3.2 };
                    // Pale infill and dark structural timber, with a separate
                    // roof material: houses no longer read as wooden boxes.
                    if ly >= top { return Some(B::Planks); }
                    let edge = lx.abs() > w * 0.5 - v.max(0.4)
                        || lz.abs() > d * 0.5 - v.max(0.4);
                    if edge && ly > 1.0 && ly < top - v.max(0.45) {
                        return Some(B::Limestone);
                    }
                }
                return Some(block);
            }
        }
        // A level doorstep meets the graded approach, including coarse voxels.
        if lx.abs() < 1.5 && lz >= -d * 0.5 - 1.5 && lz < -d * 0.5 {
            if ly >= -v && ly < v { return Some(B::Cobblestone); }
            if ly >= v && ly < v + 2.8 { return Some(B::Air); }
        }
    }
    }
    let y = m.y - floor_at(a);
    let square = a.abs() < 8.0 && b.abs() < 6.5;
    if cfg.generator_version >= 54 {
        // Covered well at the edge of the square. The through-street and all
        // six door spurs remain open; the basin has a solid bottom.
        let wx = a;
        let wz = b + 4.7;
        let r = wx.abs().max(wz.abs());
        if r < 1.4 {
            if (-v..v).contains(&y) { return Some(B::StoneBricks); }
            if (v..v + 0.8).contains(&y) {
                return Some(if r > 0.75 { B::Limestone } else { B::Water });
            }
            // Rotated village grids can miss the mid-side post row entirely;
            // generator 72 adds corner posts so the roof always stands on wood.
            let corner = cfg.generator_version >= crate::toll_crossing::CONNECTED_STAIRS_GENERATOR
                && wx.abs() > 1.4 - v && wz.abs() > 1.4 - v;
            let post = corner || wx.abs() > 1.4 - v && wz.abs() < v * 0.5;
            if post && (v + 0.8..3.8).contains(&y) { return Some(B::Log); }
            let roof = 3.8 + (1.4 - wx.abs()) * 0.6;
            let under = roof - if cfg.generator_version >= 66 { 0.6 * v } else { 0.0 };
            if y >= under && y < roof + v { return Some(B::Planks); }
            if cfg.generator_version >= 66 && post && y >= 3.8 && y < under {
                return Some(B::Log);
            }
        }
    }
    // Two covered stalls flank the square, leaving a four-metre through route.
    if square {
        let sx = a.abs() - 5.0;
        let sz = b.abs() - 4.5;
        if sx.abs() < 1.8 && sz.abs() < 1.2 {
            if y >= 3.2 && y < 3.2 + v { return Some(B::Planks); }
            if sx.abs() > 1.8 - v && sz > 1.2 - v && y >= v && y < 3.2 {
                return Some(B::Log);
            }
            if sz > 0.0 && y >= v && y < v + 0.8 { return Some(B::Planks); }
        }
    }
    let spur = (a + 22.0).abs().min(a.abs()).min((a - 22.0).abs()) < 1.5;
    let rear = b.abs() > 16.0 && b.abs() < 18.5 && a.abs() < 30.5;
    let ends = a.abs() > 28.0 && a.abs() < 30.5 && b.abs() < 18.5;
    let path = square || b.abs() < 2.5 || spur && b.abs() < 17.5 || rear || ends;
    if path {
        if y >= -v && y < v {
            if cfg.generator_version >= 56 {
                return Some(craft::paving(a, b, square, rear || ends));
            }
            return Some(if square { B::StoneBricks } else if rear || ends { B::Gravel } else { B::Cobblestone });
        }
        if y >= v && y < 4.5 { return Some(B::Air); }
    }
    if cfg.generator_version >= 44 {
        if cfg.generator_version >= 55 && (a - 11.5).abs() < 3.0 && (b - 10.5).abs() < 4.0 {
            // One former garden becomes the smith's service yard: fuel, ore,
            // workbench and a roofed store. Other gardens remain productive.
            let (x,z) = (a - 11.5,b - 10.5);
            if cfg.generator_version >= 57 {
                return workshop::yard_versioned(x, y, z, v, cfg.generator_version >= crate::toll_crossing::CONNECTED_STAIRS_GENERATOR);
            }
            if (-v..v).contains(&y) { return Some(B::Cobblestone); }
            if (v..2.0 * v).contains(&y) {
                if x < -1.0 && z > 1.0 { return Some(B::CoalOre); }
                if x > 1.0 && z > 1.0 { return Some(B::IronOre); }
                if x.abs() < 1.0 && z > 2.0 { return Some(B::Workbench); }
                if x > 1.0 && z < -2.0 { return Some(B::SalvageCrate); }
            }
            if x.abs() > 3.0 - v && z > 4.0 - v && (v..4.0).contains(&y) {
                return Some(B::Log);
            }
            if z > 0.5 && (3.5..3.5 + v).contains(&y) { return Some(B::Planks); }
            if (v..3.5).contains(&y) { return Some(B::Air); }
        }
        // Productive gardens occupy the gaps between parcels. Their open ends
        // face the street and service lane; edging never crosses either route.
        let garden_x = (a.abs() - 11.5).abs();
        let garden_z = (b.abs() - 10.5).abs();
        if garden_x < 3.0 && garden_z < 4.0 {
            let rim = garden_x >= 3.0 - v;
            if y >= -v && y < v {
                return Some(if rim { B::Cobblestone } else { B::Dirt });
            }
            if !rim && y >= v && y < v + 0.5
                && (a.abs() - 8.5).rem_euclid(2.0) < v {
                return Some(B::Grass);
            }
            if y >= v && y < 3.0 { return Some(B::Air); }
        }
        // Small seats at the ends of the market, outside its through street.
        if (a.abs() - 8.5).abs() < v.max(0.6) && (3.5..6.0).contains(&b.abs())
            && y >= v && y < v + 0.7 {
            return Some(B::Planks);
        }
    }
    None
}
