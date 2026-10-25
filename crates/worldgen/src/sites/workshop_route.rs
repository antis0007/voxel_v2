//! Generator 48: an early working face and a legible, lit fungal vault.
use super::*;

pub(super) fn sample(q: Vec3, v: f32) -> Option<B> {
    let depth = -q.z;
    // A side bay off the decline. The central haulage lane remains clear.
    if (-6.5..=-2.6).contains(&q.x) && (7.0..15.0).contains(&depth) {
        let floor = ((-depth * 0.45) / v).floor() * v;
        let y = q.y - floor;
        if (-v..v).contains(&y) { return Some(B::Cobblestone); }
        if (v..4.4).contains(&y) {
            if q.x < -5.0 && y < v + 1.0 && (8.0..14.0).contains(&depth) {
                return Some(match depth.floor() as i32 {
                    8 => B::CopperOre,
                    9 | 13 => B::CoalOre,
                    10 | 11 => B::GlowCap,
                    _ => B::Crystal,
                });
            }
            if q.x < -6.0 && (2.5..2.5 + v).contains(&y)
                && (depth - 7.5).abs() < v { return Some(B::Torch); }
            return Some(B::Air);
        }
        if (4.4..4.4 + v).contains(&y) { return Some(B::Limestone); }
    }
    // The decline owns its complete slope and headroom in both directions.
    // Flattening its overlap with the vault creates an unclimbable return lip.
    if q.x.abs() < 2.6 && depth <= 44.0 { return None; }
    // Dress the vault with masonry edging and supported timber, leaving the
    // middle combat floor and the resource geode walkable.
    let z = q.z + 48.0;
    let ring = (q.x / 17.0).powi(2) + (z / 19.0).powi(2);
    if ring < 0.90 && q.y >= -19.0 && q.y < -6.0 {
        if q.y < -19.0 + v && q.x.abs() < 5.0 {
            return Some(if z.abs() < 2.0 { B::StoneBricks } else { B::Cobblestone });
        }
        let pier = (q.x.abs() - 7.5).abs() < v.max(0.6) * 0.5
            && (z.abs() - 8.5).abs() < v.max(0.6) * 0.5;
        if pier && q.y < -10.0 { return Some(if q.y < -16.0 { B::WeatheredBricks } else { B::Log }); }
        if (q.x.abs() - 6.5).abs() < v * 0.5 && (z.abs() - 8.5).abs() < v * 0.5
            && (-16.0..-16.0 + v).contains(&q.y) { return Some(B::Torch); }
        // Keep the guardian floor visible. Mushroom groves remain on the
        // perimeter; the lit working bay already supplies the route's crystal.
        let arena = (q.x - 10.0).hypot(z) < 6.0;
        let aisle = (q.x.abs() < 3.0 && z.abs() < 12.0)
            || ((0.0..10.0).contains(&q.x) && z.abs() < 2.5);
        if arena || aisle {
            return Some(if q.y < -19.0 + v {
                if arena && (q.x - 10.0).hypot(z) > 4.5 { B::Gravel }
                else { B::StoneBricks }
            } else { B::Air });
        }
    }
    None
}
