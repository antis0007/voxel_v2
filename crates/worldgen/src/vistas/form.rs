//! Weathered form for vista landmarks from `FORM_VERSION`: uneven beds, flank gullies, scree.
use crate::{BlockId as B, WorldSampleConfig, hash2, smoothstep, value_noise2};

/// Mean bed thickness in metres before the thickness warp.
const BED_M: f32 = 2.4;
/// Beds softer than this recede from steep faces.
const SOFT: f32 = 0.42;
/// Deepest recess of the softest bed behind the face, in metres.
const RECESS_M: f32 = 1.6;

/// Index and hardness of the bed at tilted height `yt`.
///
/// The warp keeps `t` monotonic in `yt` (slope 1 - 0.3 * 1.9 > 0), so beds never repeat.
pub(super) fn bed(key: u32, yt: f32) -> (i32, f32) {
    let phase = hash2(0, 0, key) * std::f32::consts::TAU;
    let t = yt / BED_M;
    let t = t + 0.3 * (t * 1.9 + phase).sin();
    let idx = t.floor() as i32;
    // Neighbouring beds share a slow trend so resistant runs form packages.
    let trend = value_noise2(idx as f32 / 3.0, 0.5, key.wrapping_add(17));
    (idx, hash2(idx, 7, key) * 0.65 + trend * 0.35)
}

/// Horizontal recess of a bed behind the face, zero for resistant beds.
pub(super) fn recess(hardness: f32) -> f32 {
    ((SOFT - hardness) / SOFT).max(0.0) * RECESS_M
}

/// Depth weight in 0..1 of a gully cut across `along`, `count` gullies per 2π of `along`.
pub(super) fn gully(key: u32, along: f32, count: f32) -> f32 {
    let warp = (value_noise2(along * 0.6, 0.0, key.wrapping_add(29)) - 0.5) * 1.4;
    let line = (along * count * 0.5 + warp + hash2(1, 0, key) * 3.0).sin().abs();
    (1.0 - line).powi(6)
}

/// Cap rim of a formed butte in footprint units, before joints and gullies move it.
pub(super) const CAP_R: f32 = 0.5;
/// Metres per footprint unit, for converting a radius outside the rim to metres.
pub(super) const UNIT_M: f32 = 22.0;

/// Footprint radius past which a peak has neither wall nor talus; profiles span under 40 m.
pub(super) const OUTER_R: f32 = 2.6;

thread_local! {
    static JOINTS: std::cell::RefCell<std::collections::HashMap<(u32, i32), [(f32, f32); 8]>> = Default::default();
}

/// Footprint radius cut by two near-orthogonal joint sets, so the rim breaks into flat faces.
pub(super) fn faceted(key: u32, peak: i32, dx: f32, dz: f32, r: f32) -> f32 {
    let planes = JOINTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 256 {
            cache.clear();
        }
        *cache.entry((key, peak)).or_insert_with(|| {
            let base = hash2(peak, 11, key) * std::f32::consts::FRAC_PI_2;
            std::array::from_fn(|k| {
                let k = k as i32;
                let phi = base + k as f32 * std::f32::consts::FRAC_PI_4 + (hash2(peak, 20 + k, key) - 0.5) * 0.3;
                let offset = 0.88 + 0.28 * hash2(peak, 40 + k, key);
                (phi.cos() / offset, phi.sin() / offset)
            })
        })
    });
    let poly = planes.iter().fold(0.0_f32, |m, &(c, s)| m.max(dx * c + dz * s));
    r + (poly - r) * 0.6
}

/// Azimuth sectors around a butte, each with its own bench widths and slope rates.
const SECTORS: i32 = 6;

thread_local! {
    static PROFILES: std::cell::RefCell<std::collections::HashMap<(u32, i32, i32), std::rc::Rc<[(f32, f32)]>>> =
        Default::default();
}

/// Metres below the cap top at `reach` metres outside the rim, facing `angle`.
///
/// Walking down from `top_yt` in tilted bed space, hard beds stand as near-vertical
/// cliffs, soft beds weather back as slopes, and a bench may form on top of a hard bed
/// that lies under a soft one. Neighbouring sectors are blended, so a bench narrows and
/// pinches out around the face instead of ringing the butte.
pub(super) fn stepped(key: u32, peak: i32, top_yt: f32, depth: f32, reach: f32, angle: f32) -> f32 {
    if reach <= 0.0 {
        return 0.0;
    }
    let turn = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * SECTORS as f32;
    let sector = turn.floor() as i32;
    let t = smoothstep(0.0, 1.0, turn - sector as f32);
    let a = sector_drop(key, peak, sector % SECTORS, top_yt, depth, reach);
    let b = sector_drop(key, peak, (sector + 1) % SECTORS, top_yt, depth, reach);
    a + (b - a) * t
}

fn sector_drop(key: u32, peak: i32, sector: i32, top_yt: f32, depth: f32, reach: f32) -> f32 {
    let points = PROFILES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.entry((key, peak, sector)).or_insert_with(|| profile(key, peak * SECTORS + sector, top_yt, depth).into()).clone()
    });
    let i = points.partition_point(|p| p.0 < reach);
    let Some(&(r1, d1)) = points.get(i) else {
        let &(r0, d0) = points.last().unwrap_or(&(0.0, 0.0));
        return d0 + (reach - r0);
    };
    let (r0, d0) = points[i.saturating_sub(1)];
    if r1 - r0 < 1e-5 { d1 } else { d0 + (d1 - d0) * (reach - r0) / (r1 - r0) }
}

/// Breakpoints (reach, drop) of one sector; beds come from the site, widths from the sector.
fn profile(key: u32, sector: i32, top_yt: f32, depth: f32) -> Vec<(f32, f32)> {
    const STEP: f32 = 0.25;
    let rate = 0.7 + 0.6 * hash2(sector, 61, key);
    let mut points = vec![(0.0, 0.0)];
    let (mut reach, mut drop, mut above_soft) = (0.0_f32, 0.0_f32, false);
    while drop < depth {
        let (idx, hardness) = bed(key, top_yt - drop - STEP * 0.5);
        let soft = hardness < SOFT;
        let width = hash2(idx, 5 + sector * 7, key);
        if above_soft && !soft && width > 0.3 {
            reach += 3.4 * (width - 0.3);
            points.push((reach, drop));
        }
        reach += STEP * if soft { rate * (0.12 + 0.3 * (SOFT - hardness) / SOFT) } else { 0.12 };
        drop += STEP;
        points.push((reach, drop));
        above_soft = soft;
    }
    points
}

/// Per-site, per-peak height factor in 0.7..1.2.
pub(super) fn jitter(key: u32, i: i32) -> f32 {
    0.7 + 0.5 * hash2(i, 3, key)
}

/// Rock for one bed: soft beds of the butte family are clay, hard caps limestone.
pub(super) fn rock(kind: u8, hardness: f32) -> B {
    match (kind, hardness) {
        (1, h) if h < SOFT => B::Clay,
        (1, h) if h < 0.82 => B::Sandstone,
        (_, h) if h < SOFT => B::Sandstone,
        _ => B::Limestone,
    }
}

/// Top voxel by slope: bare rock on faces, scree on the apron, soil cover elsewhere.
pub(super) fn cover(slope: f32, rock: B, soil: B, scree: f32) -> B {
    if slope > 1.25 {
        rock
    } else if slope > 0.55 && scree > 0.45 - 0.3 * smoothstep(0.55, 1.25, slope) {
        B::Gravel
    } else {
        soil
    }
}

/// Rock below a formed landmark surface: soft beds recede from steep faces by up to 1.6 m.
///
/// A voxel `depth` below its column top on slope `s` lies about `depth / s` behind the face.
/// Recesses shallower than most of a voxel are skipped, so finer grids show more ledges.
#[allow(clippy::too_many_arguments)]
pub(super) fn bedded(cfg:&WorldSampleConfig,s:super::Site,a:f32,b:f32,y:f32,my:f32,top:f32,column:crate::ColumnSample)->B{
    let v=cfg.voxel_meters();
    let yt=y+s.tilt(a,b);
    let (_,hardness)=bed(s.key,yt);
    let rock=rock(s.kind,hardness);
    let depth=top-my;
    let slope=||{
        let d=v.max(0.5);
        let dx=(s.relief(a+d,b)-s.relief(a-d,b))/(2.0*d);
        let dz=(s.relief(a,b+d)-s.relief(a,b-d))/(2.0*d);
        dx.hypot(dz)
    };
    if depth<v*1.1 {
        if b.abs()<1.8&&(-76.0..77.0).contains(&a) {return B::Gravel;}
        if s.relief(a,b)>=43.0 {return rock;}
        let scree=value_noise2((s.x+a)/5.0,(s.z+b)/5.0,s.key.wrapping_add(41));
        return cover(slope(),rock,column.surface,scree);
    }
    let recess=recess(hardness);
    if recess>=v*0.75&&y>2.0&&depth<recess*4.0 {
        let face=slope();
        if face>0.9&&depth/face<recess {return B::Air;}
    }
    rock
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beds_vary_in_hardness_and_thickness() {
        let mut idx = Vec::new();
        let (mut soft, mut hard) = (0, 0);
        let mut last = i32::MIN;
        for k in 0..2000 {
            let (i, h) = bed(91, k as f32 * 0.05);
            assert!(i >= last, "bed index went back at {k}");
            if i != last {
                idx.push(k);
                last = i;
                if h < SOFT { soft += 1 } else { hard += 1 }
            }
        }
        let gaps: Vec<i32> = idx.windows(2).map(|w| w[1] - w[0]).collect();
        let (lo, hi) = (gaps.iter().min().copied().unwrap_or(0), gaps.iter().max().copied().unwrap_or(0));
        assert!(hi - lo >= 20, "bed thickness {lo}..{hi} samples is near uniform");
        assert!(soft > 3 && hard > 3, "{soft} soft, {hard} hard beds");
    }

    fn hills(version: u32, voxel_scale: engine_core::VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig { planet: None, generator_version: version, seed: 424242, world_type: crate::WorldType::Hills, voxel_scale }
    }

    #[test]
    fn formed_sites_are_rarer_and_older_versions_keep_theirs() {
        let one = engine_core::VoxelScalePreset::OneMeter;
        let count = |cfg: &WorldSampleConfig| {
            (-8..8).flat_map(|z| (-8..8).map(move |x| (x, z))).filter(|&(x, z)| super::super::site(cfg, x, z).is_some()).count()
        };
        let (old, new) = (count(&hills(super::super::FORM_VERSION - 1, one)), count(&hills(super::super::FORM_VERSION, one)));
        assert!(old >= 12 && new * 2 < old, "{old} sites before, {new} after");
    }

    #[test]
    fn butte_walls_step_back_bed_by_bed() {
        let mut cliffs = 0;
        let mut benches: Vec<f32> = Vec::new();
        for sector in 0..SECTORS {
            let angle = sector as f32 / SECTORS as f32 * std::f32::consts::TAU;
            let drop = |reach: f32| stepped(91, 0, 60.0, 68.0, reach, angle);
            for k in 0..320 {
                let r = k as f32 * 0.25;
                let grade = (drop(r + 0.25) - drop(r)) / 0.25;
                if grade > 3.0 { cliffs += 1 }
                if grade < 0.05 && drop(r) > 0.5 && benches.iter().all(|b| (b - drop(r)).abs() > 1.0) {
                    benches.push(drop(r));
                }
            }
        }
        benches.sort_by(f32::total_cmp);
        let gaps: Vec<f32> = benches.windows(2).map(|w| w[1] - w[0]).collect();
        let spread = gaps.iter().cloned().fold(0.0, f32::max) - gaps.iter().cloned().fold(f32::MAX, f32::min);
        assert!(cliffs >= 24 && benches.len() >= 4, "{cliffs} cliff samples, bench levels {benches:?}");
        assert!(spread > 2.0, "bench spacing {gaps:?} is near uniform");
        let drop = |reach: f32| stepped(91, 0, 60.0, 68.0, reach, 0.0);
        assert!(drop(400.0) >= 68.0, "the profile reaches the foot");
        for sector in 0..SECTORS {
            let angle = sector as f32 / SECTORS as f32 * std::f32::consts::TAU;
            // The early-out in `relief` skips peaks past OUTER_R; the wall must be down by then.
            let reach = (OUTER_R * 0.85 - CAP_R) * UNIT_M;
            assert!(stepped(91, 0, 60.0, 68.0, reach, angle) >= 68.0, "sector {sector} still stands at {reach} m");
        }
        let apart = (0..160).map(|k| k as f32 * 0.25)
            .map(|r| (stepped(91, 0, 60.0, 68.0, r, 0.3) - stepped(91, 0, 60.0, 68.0, r, 3.4)).abs())
            .fold(0.0, f32::max);
        assert!(apart > 1.5, "opposite faces share one profile ({apart} m apart): benches ring the butte");
    }

    #[test]
    fn joints_facet_the_rim() {
        let radius = |t: f32| faceted(91, 1, t.cos(), t.sin(), 1.0);
        let rs: Vec<f32> = (0..360).map(|d| radius((d as f32).to_radians())).collect();
        let (lo, hi) = rs.iter().fold((f32::MAX, 0.0_f32), |(l, h), &r| (l.min(r), h.max(r)));
        assert!(hi - lo > 0.06, "rim radius {lo}..{hi} is still a circle");
    }

    /// Metres of air enclosed under rock in columns across a landmark flank.
    fn notch_metres(scale: engine_core::VoxelScalePreset) -> f32 {
        let cfg = hills(super::super::FORM_VERSION, scale);
        let near = (-12..=12).flat_map(|z| (-12..=12).map(move |x| (x, z)));
        let Some(s) = near.filter_map(|(x, z)| super::super::site(&cfg, x, z)).next() else { return 0.0 };
        let v = cfg.voxel_meters();
        let mut air = 0.0;
        // Out from the lookout platform across its cap edge and foot, clear of the road.
        for k in 0..240 {
            let p = s.point(82.0, 0.0, 12.0 + k as f32 * 0.25);
            let (x, z) = ((p.x / v).floor() as i32, (p.z / v).floor() as i32);
            let top = crate::sample_column(&cfg, x, z).height;
            let mut under_rock = false;
            for y in (top - (60.0 / v) as i32..=top).rev() {
                let solid = crate::sample_block(&cfg, glam::IVec3::new(x, y, z)).is_solid();
                if solid { under_rock = true } else if under_rock { air += v }
            }
        }
        air
    }

    #[test]
    fn finer_grids_resolve_more_recessed_beds() {
        let coarse = notch_metres(engine_core::VoxelScalePreset::OneMeter);
        let fine = notch_metres(engine_core::VoxelScalePreset::QuarterMeter);
        assert!(fine > coarse && fine > 0.0, "recess air {coarse} m at 1 m, {fine} m at 0.25 m");
    }
}
