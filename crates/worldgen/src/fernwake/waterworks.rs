//! Finite water court: high reservoir, service shelf, receiving basin and a
//! breakable/rebuildable timber stop. Ordinary fluid simulation owns the water.
use crate::{BlockId as B, WorldSampleConfig};
use glam::Vec3;

pub const VERSION: u32 = 82;
pub const STOP: Vec3 = Vec3::new(320.5, -0.5, 44.5);
const LAMPS: &[(f32,f32,f32)] = &[
    (312.5,44.5,1.0),(330.5,44.5,1.0),(316.5,60.5,3.0),(329.5,60.5,5.0),
    // Above the waterline: lamps below the surface cannot light its upward face.
    (314.5,41.5,4.0),(329.5,41.5,4.0),
];

pub fn sample(cfg: &WorldSampleConfig, p: Vec3) -> Option<B> {
    if cfg.generator_version < VERSION || !super::active(cfg)
        || !(310.0..333.0).contains(&p.x) || !(34.0..63.0).contains(&p.z)
        || !(-3.0..15.0).contains(&p.y) { return None; }
    let block = local(p);
    if block == B::Torch {
        let s = cfg.voxel_meters();
        for &(x,z,y) in LAMPS {
            let centre = (Vec3::new(x,y,z)/s).floor()*s+Vec3::splat(s*0.5);
            if (p-centre).abs().max_element() < s*0.5 { return Some(B::Torch); }
        }
        return Some(B::Air);
    }
    Some(block)
}

fn local(p: Vec3) -> B {
    let x = p.x; let y = p.y; let z = p.z;
    // A shallow humpback bridge keeps the main route above a gravity-fed
    // culvert. Half-metre approach treads preserve clearance at fine scales.
    if (46.0..51.0).contains(&z) && (0.0..5.0).contains(&y) {
        let top = bridge_top(x);
        return if y < top { B::Limestone } else { B::Air };
    }
    if y < -2.0 { return B::Limestone; }
    // Continue the culvert roof from the retaining wall to the bridge. An
    // exposed cell here lets reservoir pressure wet the adjacent crossing.
    if (320.0..322.0).contains(&x) && (45.0..46.0).contains(&z)
        && (0.0..1.0).contains(&y) { return B::Limestone; }
    // A covered culvert passes below the crossing. Its ceiling is the solid
    // one-metre bridge deck; breaching the stop does not erase the route.
    if (320.0..322.0).contains(&x) && (43.0..54.0).contains(&z) && (-1.0..0.0).contains(&y) {
        return if (44.0..45.0).contains(&z) { B::Planks } else { B::Air };
    }
    let upper = (315.0..329.0).contains(&x) && (36.0..44.0).contains(&z);
    let lower = (312.0..332.0).contains(&x) && (51.0..59.0).contains(&z);
    if upper && y < 3.0 {
        // A low service alcove beneath the shelf is submerged at arrival.
        // Draining reveals its floor and the copper fitting in its back wall.
        if (319.0..324.0).contains(&x) && z < 38.0 {
            if (0.0..2.0).contains(&y) { return B::Water; }
            if y >= 2.0 { return B::Limestone; }
        }
        if (319.0..324.0).contains(&x) && (38.0..40.0).contains(&z) && y < -1.0 { return B::Limestone; }
        // Submerged shelf becomes a dry service walk after drainage. A player
        // can also swim, bridge the reservoir or use an ordinary pump.
        if z < 38.0 && y < 2.0 { return B::Limestone; }
        return B::Water;
    }
    if lower && y < 0.0 { return B::Air; }
    if y < 0.0 { return B::Limestone; }
    // Raised reservoir banks keep its initial water above the dry main route.
    if (321.0..322.0).contains(&x) && (35.0..36.0).contains(&z) && (0.0..1.0).contains(&y) { return B::CopperOre; }
    if (314.0..330.0).contains(&x) && (35.0..45.0).contains(&z)
        && !upper && y < 3.0 { return B::Limestone; }

    let roof = 11.0 + (3.0 - ((x - 321.5).abs() / 3.5).floor()).max(0.0);
    if x < 311.0 || x >= 332.0 || z < 35.0 || z >= 62.0 || y >= roof {
        return if y >= roof && y < roof + 1.0 { B::CarvedStone } else { B::Limestone };
    }
    // Both stairs rise in half-height metre pairs, not a ladder or a dead-end
    // balcony. The northern gallery looks across the reservoir to the crown.
    if x < 314.0 && z < 46.0 {
        let top = ((46.0-z)*0.5).floor().clamp(0.0,5.0);
        if y < top { return B::Limestone; }
    }
    if x >= 330.0 && z < 46.0 {
        let top = ((46.0-z)*0.5).floor().clamp(0.0,5.0);
        if y < top { return B::Limestone; }
    }
    if z < 36.0 && y < 5.0 { return B::Limestone; }
    // Service shelf (feet at 2 m) has one-metre steps to both upper landings.
    if (36.0..38.0).contains(&z) && (314.0..331.0).contains(&x) {
        let top = if x < 315.0 || x >= 329.0 { 3.0 } else { 2.0 };
        if y < top { return B::Limestone; }
    }
    for (cx,cz) in [(312.5,59.5),(330.5,59.5)] {
        let d = (x-cx).abs().max((z-cz).abs());
        if d < 0.5 || (d < 1.5 && (9.0..11.0).contains(&y)) { return B::StoneBricks; }
    }
    // Quiet southern overlook, with a broad stair and a view into the receiver.
    if z >= 59.0 {
        let top = ((x-312.0)*0.5).floor().clamp(0.0,4.0);
        if y < top { return B::Limestone; }
    }
    // Suspended distributor: copper ring, pale central hub, four stone ribs.
    // No fake moving parts: this is salvageable, dormant infrastructure.
    let dx = (x-321.5).abs(); let dz = (z-48.5).abs();
    let ring = dx.max(dz).max((dx+dz)*0.70710677);
    if (8.0..9.0).contains(&y) && (2.5..3.5).contains(&ring) {
        return B::CopperBlock;
    }
    if dx < 0.75 && dz < 0.75 && (8.0..11.0).contains(&y) { return B::CarvedStone; }
    if y >= 9.0 && ((dx < 0.5 && dz < 3.5) || (dz < 0.5 && dx < 3.5)) { return B::StoneBricks; }
    // Discrete pools of warm light; violet mineral accents mark the shelf.
    // Neither rail nor foliage occupies the central walkway.
    for &(cx,cz,base) in LAMPS {
        if (x-cx).abs()<0.5 && (z-cz).abs()<0.5 && (base-1.0..base).contains(&y) { return B::StoneBricks; }
        if (x-cx).abs()<0.5 && (z-cz).abs()<0.5 && (base..base+1.0).contains(&y) { return B::Torch; }
    }
    if (x-321.5).abs()<1.5 && (z-35.5).abs()<0.5 && (7.0..8.0).contains(&y) { return B::Crystal; }
    // Raised relief frames the mineral marker without narrowing the gallery.
    if (35.0..36.0).contains(&z) && (7.0..11.0).contains(&y) {
        if (316.0..317.0).contains(&x) || (326.0..327.0).contains(&x)
            || ((316.0..327.0).contains(&x) && y>=10.0) { return B::StoneBricks; }
        if (319.0..324.0).contains(&x) && y<9.0
            && (x<320.0 || x>=323.0 || y>=8.0) { return B::CarvedStone; }
    }
    let foliage = if y < 11.0 { 0.5 } else if y < 12.0 { 1.0 } else { 1.5 };
    if ((x-316.5).abs()<foliage || (x-327.5).abs()<foliage) && (z-40.5).abs()<foliage && (10.0..13.0).contains(&y) {
        return B::Leaves;
    }
    B::Air
}

fn bridge_top(x: f32) -> f32 {
    (((x-310.0).min(333.0-x)/2.0).floor()*0.5).clamp(0.0,2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldType;
    use engine_core::VoxelScalePreset;

    #[test]
    fn waterworks_preserves_routes_and_older_generators() {
        for seed in [1,31,9187] {
            let mut cfg = WorldSampleConfig {
    planet: None, seed, generator_version: VERSION,
                world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter };
            assert_eq!(sample(&cfg, STOP), Some(B::Planks));
            for x in 310..333 { for z in 46..51 { for y in 0..3 {
                let xx=x as f32+0.5;
                assert_eq!(sample(&cfg, Vec3::new(xx,y as f32+0.5+bridge_top(xx),z as f32+0.5)), Some(B::Air));
            }}}
            for z in 45..54 {
                assert_eq!(local(Vec3::new(320.5,-0.5,z as f32+0.5)), B::Air);
            }
            assert_eq!(local(Vec3::new(320.5,0.5,48.5)), B::Limestone);
            assert_eq!(local(Vec3::new(320.5,0.5,45.5)), B::Limestone);
            assert_eq!(local(Vec3::new(317.5,2.5,37.5)), B::Water);
            assert_eq!(local(Vec3::new(321.5,2.5,37.5)), B::Limestone);
            assert_eq!(local(Vec3::new(321.5,1.5,37.5)), B::Water);
            assert_eq!(local(Vec3::new(321.5,-0.5,37.5)), B::Limestone);
            assert_eq!(local(Vec3::new(321.5,0.5,35.5)), B::CopperOre);
            assert_eq!(local(Vec3::new(321.5,-1.5,54.5)), B::Air);
            let mut water = 0;
            let mut below_route_capacity = 0;
            for x in 310..333 { for z in 34..63 { for y in -2..3 {
                let b = local(Vec3::new(x as f32+0.5,y as f32+0.5,z as f32+0.5));
                water += usize::from(b == B::Water);
                below_route_capacity += usize::from(y < 0 && matches!(b, B::Water | B::Air | B::Planks));
            }}}
            assert!(water <= below_route_capacity, "receiver capacity {below_route_capacity} must contain {water} water cells below the main route");
            // Sweep a normal standing body around both stairs and the linking
            // gallery. Checking only the tread centre misses wall/head snags.
            let mut route = Vec::new();
            for z in 35..46 {
                let top = ((46.0-(z as f32+0.5))*0.5).floor().clamp(0.0,5.0);
                for x in [313.5,331.5] { route.push(Vec3::new(x,top,z as f32+0.5)); }
            }
            for x in 313..332 { route.push(Vec3::new(x as f32+0.5,5.0,35.5)); }
            for feet in route {
                assert_ne!(local(feet-Vec3::Y*0.1), B::Air, "unsupported tread {feet:?}");
                for dx in [-0.35,0.35] { for dz in [-0.35,0.35] { for y in [0.1,0.9,1.8] {
                    assert_eq!(local(feet+Vec3::new(dx,y,dz)), B::Air, "blocked route at {feet:?}");
                }}}
            }
            for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::HalfMeter,VoxelScalePreset::QuarterMeter] {
                cfg.voxel_scale=scale;
                let s=cfg.voxel_meters(); let n=(1.0/s) as i32;
                for &(x,z,y) in LAMPS {
                    let mut torches=0;
                    for dx in 0..n { for dy in 0..n { for dz in 0..n {
                        let p=Vec3::new(x-0.5+(dx as f32+0.5)*s,y+(dy as f32+0.5)*s,z-0.5+(dz as f32+0.5)*s);
                        torches+=usize::from(sample(&cfg,p)==Some(B::Torch));
                    }}}
                    assert_eq!(torches,1,"one torch per fixture at {s} metres");
                }
            }
            cfg.generator_version = VERSION-1;
            assert_eq!(sample(&cfg,STOP), None);
        }
    }
}
