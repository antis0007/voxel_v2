//! A compact market settlement within the existing surveyed village footprint.
//! Common grade, inward-facing entrances, a square and a rear service circuit.
use super::*;

pub(super) fn sample(plan: &Plan, cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    let (a, b) = village_axes(plan, m.x, m.z);
    if a.abs() >= 35.0 || b.abs() >= 19.0 { return None; }
    let v = cfg.voxel_meters();
    let floor_at = |x: f32| cfg.world_y(cfg.blocks_for_height(plan.hamlet.y + x * plan.hamlet_grade));
    // The inn and workshop bookend the square; homes occupy the outer plots.
    // Staggered setbacks give each door its own forecourt without narrowing
    // the main street or changing the terrain survey's reserved footprint.
    let plots = [
        (-22.0, 10.0, 7.0, 6.0, 0), (0.0, 12.0, 9.0, 7.0, 1),
        (22.0, 10.0, 8.0, 7.0, 2), (-22.0, -10.0, 7.0, 6.0, 0),
        (0.0, -12.0, 8.0, 6.0, 3), (22.0, -10.0, 6.0, 6.0, 0),
    ];
    for (x, z, w, d, role) in plots {
        let facing = if z < 0.0 { -1.0 } else { 1.0 };
        let (lx, lz, ly) = ((a - x) * facing, (b - z) * facing, m.y - floor_at(x));
        // The shared timber sampler expects its caller to bound the parcel
        // vertically; otherwise its interior air would carve to bedrock.
        if (-6.0..14.0).contains(&ly) {
            // Keep the front threshold connected to the room: the legacy inn
            // counter and central partition can otherwise close this aisle.
            if cfg.generator_version >= 44 && lx.abs() < v.max(1.0)
                && lz >= -d * 0.5 && lz < 0.5
                && ly >= v.max(0.4) && ly < 2.6 {
                return Some(B::Air);
            }
            if let Some(block) = timber(role, lx, ly, lz, w, d, v) { return Some(block); }
        }
        // A level doorstep meets the graded approach, including coarse voxels.
        if lx.abs() < 1.5 && lz >= -d * 0.5 - 1.5 && lz < -d * 0.5 {
            if ly >= -v && ly < v { return Some(B::Cobblestone); }
            if ly >= v && ly < v + 2.8 { return Some(B::Air); }
        }
    }
    let y = m.y - floor_at(a);
    let square = a.abs() < 8.0 && b.abs() < 6.5;
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
            return Some(if square { B::StoneBricks } else if rear || ends { B::Gravel } else { B::Cobblestone });
        }
        if y >= v && y < 4.5 { return Some(B::Air); }
    }
    if cfg.generator_version >= 44 {
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
