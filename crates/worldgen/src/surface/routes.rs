//! Surveyed surface routes. Cached with the surface plan; no per-voxel surveys.
use super::{B, Trail, Vec3, WorldSampleConfig, sample_column};

const SHOULDER: f32 = 4.0;
pub(super) const MAX_GRADE: f32 = 0.32;
const ANCHOR_ROAD_GRADE: f32 = 0.22;
pub(super) const FOOTPATH_HALF_WIDTH: f32 = 1.0;
mod bridges;
mod survey;
pub(super) use bridges::Span;

fn ground(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let c = sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    cfg.world_y(c.height + 1)
}

fn anchor_blocked(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    let column = sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    crate::planet::is_crater_lake(cfg, &column)
        || crate::ruins::reserves(cfg, x, z, 0.0)
        || crate::surveyor::reserves(cfg, x, z)
        || crate::valley::built_up(cfg, x, z)
        || crate::amberroot::reserves(cfg, x, z)
        || crate::sites::trail_obstructed(cfg, x, z)
}

pub(super) fn anchor_road(
    cfg: &WorldSampleConfig,
    a: Vec3,
    b: Vec3,
    half_width: f32,
    survey: bool,
) -> Option<Trail> {
    let trail = if survey {
        let blocked = |x, z| {
            anchor_blocked(cfg, x, z) || bridges::water(cfg, x, z, ground(cfg, x, z)).is_some()
        };
        survey::route(cfg, a, b, &blocked, cfg.seed, half_width)?
    } else {
        Trail::polyline(vec![(a.x, a.z), (b.x, b.z)], cfg.seed, half_width)
    };
    fit(cfg, trail, Some(a.y), Some(b.y))
}

/// Space a regional road must not enter: district decks, boss courts, the
/// gatehouse that stands over a dungeon shaft, and river channels (there are
/// no bridges, so a crossing would sink the road under the water).
fn claimed(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    use crate::Profiles;
    crate::ruins::reserves(cfg, x, z, 1.0)
        || crate::sites::campus_reserved(cfg, x, z)
        || crate::dungeon::reserved(cfg, Vec3::new(x, 0.0, z))
        || crate::CachedProfiles
            .river_top(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z))
            .is_some()
}

pub(super) fn fit(
    cfg: &WorldSampleConfig,
    mut trail: Trail,
    start: Option<f32>,
    end: Option<f32>,
) -> Option<Trail> {
    if cfg.generator_version >= 62 { return bridges::fit(cfg, trail, start, end); }
    let spacing = if cfg.generator_version >= 40 { 1.0 } else { 2.0 };
    let count = (trail.length() / spacing).ceil().max(2.0) as usize;
    let mut raw = Vec::with_capacity(count + 1);
    let mut steps = Vec::with_capacity(count);
    let mut previous = trail.at(0.0);
    for i in 0..=count {
        let t = i as f32 / count as f32;
        let (x, z) = trail.at(t);
        // Routes yield to authored complexes and waterways instead of slicing them.
        if crate::valley::reserves(cfg, x, z)
            || crate::amberroot::reserves(cfg, x, z)
            || crate::sites::trail_obstructed(cfg, x, z)
            || (cfg.generator_version >= super::roads::VERSION && claimed(cfg, x, z))
        {
            return None;
        }
        let h = ground(cfg, x, z);
        if h <= 14.0 {
            return None;
        }
        let (tx, tz) = trail.tangent(t);
        for side in [-trail.half_width, trail.half_width] {
            let sx = x - tz * side;
            let sz = z + tx * side;
            let edge = ground(cfg, sx, sz);
            if (edge - h).abs() > 3.0 {
                return None;
            }
            if cfg.generator_version >= 40 && (edge <= 14.0
                || crate::valley::reserves(cfg, sx, sz)
                || crate::amberroot::reserves(cfg, sx, sz)
                || crate::sites::trail_obstructed(cfg, sx, sz)
                || (cfg.generator_version >= super::roads::VERSION && claimed(cfg, sx, sz))) {
                return None;
            }
        }
        raw.push(h);
        if i > 0 {
            steps.push((x - previous.0).hypot(z - previous.1) * MAX_GRADE);
        }
        previous = (x, z);
    }
    let mut grade = raw.clone();
    let first = start.unwrap_or(raw[0]);
    let last = end.unwrap_or(raw[count]);
    grade[0] = first;
    grade[count] = last;
    for _ in 0..8 {
        for i in 1..count {
            grade[i] = grade[i]
                .clamp(grade[i - 1] - steps[i - 1], grade[i - 1] + steps[i - 1])
                .clamp(raw[i] - 3.0, raw[i] + 2.0);
        }
        for i in (1..count).rev() {
            grade[i] = grade[i]
                .clamp(grade[i + 1] - steps[i], grade[i + 1] + steps[i])
                .clamp(raw[i] - 3.0, raw[i] + 2.0);
        }
    }
    if (0..count).any(|i| (grade[i + 1] - grade[i]).abs() > steps[i] + 0.01)
        || grade.iter().zip(&raw).any(|(a, b)| (a - b).abs() > 3.01)
    {
        return None;
    }
    trail.grade = grade;
    trail.min_x -= SHOULDER;
    trail.max_x += SHOULDER;
    trail.min_z -= SHOULDER;
    trail.max_z += SHOULDER;
    Some(trail)
}

fn connect(cfg: &WorldSampleConfig, a: Vec3, b: Vec3, salt: u32) -> Option<Trail> {
    // Try both sides of a rise before giving up. Never paint an impassable trail.
    let mut best: Option<(f32, Trail)> = None;
    for attempt in 0..if cfg.generator_version >= 62 { 2 } else { 5 } {
        let seed = cfg.seed.wrapping_add(salt).wrapping_add(attempt * 7919);
        let trail = Trail::new(a.x, a.z, b.x, b.z, seed, 1.6);
        let Some(trail) = fit(cfg, trail, Some(a.y), Some(b.y)) else {
            continue;
        };
        let climb: f32 = trail.grade.windows(2).map(|h| (h[1] - h[0]).abs()).sum();
        if best.as_ref().is_none_or(|(score, _)| climb < *score) {
            best = Some((climb, trail));
        }
    }
    best.map(|(_, trail)| trail)
}

pub(super) fn network(cfg: &WorldSampleConfig) -> Vec<Trail> {
    let spawn = Vec3::new(0.0, ground(cfg, 0.0, 0.0), 0.0);
    let mut points = vec![spawn];
    let mut trails = Vec::new();
    let mut edges = Vec::new();
    let mut targets = crate::sites::trail_entrances(cfg, super::REACH);
    // These two junctions leave usable building space and lead to the existing descent.
    targets.insert(0, Vec3::new(15.0, ground(cfg, 15.0, 0.0), 0.0));
    targets.insert(1, Vec3::new(-38.0, ground(cfg, -38.0, 18.0), 18.0));
    for (i, target) in targets.into_iter().take(7).enumerate() {
        let mut origins = points.clone();
        origins.sort_by(|a, b| {
            a.distance_squared(target)
                .total_cmp(&b.distance_squared(target))
        });
        for origin in origins.into_iter().take(if cfg.generator_version >= 62 { 3 } else { usize::MAX }) {
            if let Some(route) = connection(cfg, origin, target, i as u32 * 131) {
                trails.extend(route);
                edges.push((origin, target));
                points.push(target);
                break;
            }
        }
    }
    // A second way home: link the building-ground junction to the workshop.
    if cfg.generator_version >= 40 {
        let mut candidates = Vec::new();
        for (i, a) in points.iter().enumerate() {
            for b in &points[i + 1..] {
                if !edges.iter().any(|(u, v)| (u == a && v == b) || (u == b && v == a)) {
                    candidates.push((*a, *b));
                }
            }
        }
        candidates.sort_by(|(a, b), (c, d)| a.distance_squared(*b).total_cmp(&c.distance_squared(*d)));
        for (a, b) in candidates {
            if let Some(route) = connection(cfg, a, b, 40_040) {
                trails.extend(route);
                break;
            }
        }
    } else if let (Some(a), Some(b)) = (points.get(2), points.get(3))
        && let Some(route) = connection(cfg, *a, *b, 37_037)
    {
        trails.extend(route);
    }
    if cfg.generator_version >= 62 { bridges::supplement(cfg, &mut trails); }
    trails
}

/// A graded path around whatever blocked the direct curve.
fn surveyed(cfg: &WorldSampleConfig, a: Vec3, b: Vec3, salt: u32) -> Option<Trail> {
    if cfg.generator_version < super::TRAIL_SURVEY_VERSION {
        return None;
    }
    let blocked = |x: f32, z: f32| {
        crate::valley::built_up(cfg, x, z) || crate::amberroot::reserves(cfg, x, z) || crate::sites::trail_obstructed(cfg, x, z)
    };
    let trail = survey::route(cfg, a, b, &blocked, cfg.seed.wrapping_add(salt), 1.6)?;
    fit(cfg, trail, Some(a.y), Some(b.y))
}

/// One rounded trail through a detour point, in place of two straight legs meeting at a corner.
fn rounded_detour(cfg: &WorldSampleConfig, a: Vec3, via: Vec3, b: Vec3, seed: u32) -> Option<Trail> {
    let path = survey::round(&[(a.x, a.z), (via.x, via.z), (b.x, b.z)]);
    fit(cfg, Trail::polyline(path, seed, 1.6), Some(a.y), Some(b.y))
}

// Older saves retain their exact route layout. New routes can go around a
// blocked hill or building instead of merely bending the same failed chord.
pub(super) fn connection(cfg: &WorldSampleConfig, a: Vec3, b: Vec3, salt: u32) -> Option<Vec<Trail>> {
    if let Some(trail) = connect(cfg, a, b, salt) {
        return Some(vec![trail]);
    }
    if cfg.generator_version < 40 { return None; }
    if let Some(trail) = surveyed(cfg, a, b, salt) {
        return Some(vec![trail]);
    }
    let delta = b - a;
    let length = delta.x.hypot(delta.z);
    if length < 8.0 { return None; }
    let normal = Vec3::new(-delta.z / length, 0.0, delta.x / length);
    let mut best: Option<(f32, Vec<Trail>)> = None;
    let fractions: &[f32] = if cfg.generator_version >= 62 { &[0.5] } else { &[0.5, 0.3, 0.7] };
    let offsets: &[f32] = if cfg.generator_version >= 62 { &[-24.0, 24.0, -48.0, 48.0] } else { &[-24.0, 24.0, -48.0, 48.0, -80.0, 80.0] };
    for &fraction in fractions {
        for &offset in offsets {
            let mut via = a + delta * fraction + normal * offset;
            via.y = ground(cfg, via.x, via.z);
            if cfg.generator_version >= super::TRAIL_SURVEY_VERSION {
                let Some(trail) = rounded_detour(cfg, a, via, b, cfg.seed.wrapping_add(salt)) else { continue };
                let score = trail.length() + trail.grade.windows(2).map(|h| (h[1] - h[0]).abs() * 4.0).sum::<f32>();
                if best.as_ref().is_none_or(|(cost, _)| score < *cost) {
                    best = Some((score, vec![trail]));
                }
                continue;
            }
            // Straight legs share an exact ground-height junction. Survey both
            // completely before accepting either; no dangling half-route.
            let leg = |from: Vec3, to: Vec3, seed| {
                let mut trail = Trail::new(from.x, from.z, to.x, to.z, seed, 1.6);
                trail.cx = (from.x + to.x) * 0.5;
                trail.cz = (from.z + to.z) * 0.5;
                fit(cfg, trail, Some(from.y), Some(to.y))
            };
            let Some(first) = leg(a, via, cfg.seed.wrapping_add(salt)) else { continue; };
            let Some(second) = leg(via, b, cfg.seed.wrapping_add(salt).wrapping_add(1)) else { continue; };
            let score = first.length() + second.length()
                + first.grade.windows(2).chain(second.grade.windows(2))
                    .map(|h| (h[1] - h[0]).abs() * 4.0).sum::<f32>();
            if best.as_ref().is_none_or(|(cost, _)| score < *cost) {
                best = Some((score, vec![first, second]));
            }
        }
    }
    best.map(|(_, route)| route)
}

/// A regional link: two direct tries, then four short detours. Cheaper than
/// `connection`, since a region owns up to a dozen of these.
pub(super) fn link(cfg: &WorldSampleConfig, a: Vec3, b: Vec3, salt: u32) -> Option<Vec<Trail>> {
    for attempt in 0..2u32 {
        let seed = cfg.seed.wrapping_add(salt).wrapping_add(attempt * 7919);
        if let Some(trail) = fit(cfg, Trail::new(a.x, a.z, b.x, b.z, seed, 1.6), Some(a.y), Some(b.y)) {
            return Some(vec![trail]);
        }
    }
    if let Some(trail) = surveyed(cfg, a, b, salt) {
        return Some(vec![trail]);
    }
    let delta = b - a;
    let length = delta.x.hypot(delta.z);
    if length < 8.0 {
        return None;
    }
    let normal = Vec3::new(-delta.z / length, 0.0, delta.x / length);
    for offset in [-24.0, 24.0, -48.0, 48.0] {
        let mut via = a + delta * 0.5 + normal * offset;
        via.y = ground(cfg, via.x, via.z);
        if cfg.generator_version >= super::TRAIL_SURVEY_VERSION {
            if let Some(trail) = rounded_detour(cfg, a, via, b, cfg.seed.wrapping_add(salt)) {
                return Some(vec![trail]);
            }
            continue;
        }
        let leg = |from: Vec3, to: Vec3, seed| {
            let mut trail = Trail::new(from.x, from.z, to.x, to.z, seed, 1.6);
            trail.cx = (from.x + to.x) * 0.5;
            trail.cz = (from.z + to.z) * 0.5;
            fit(cfg, trail, Some(from.y), Some(to.y))
        };
        let Some(first) = leg(a, via, cfg.seed.wrapping_add(salt)) else { continue };
        let Some(second) = leg(via, b, cfg.seed.wrapping_add(salt).wrapping_add(1)) else { continue };
        return Some(vec![first, second]);
    }
    None
}

pub(super) fn height_at(trail: &Trail, t: f32) -> f32 {
    let pos = t.clamp(0.0, 1.0) * (trail.grade.len() - 1) as f32;
    let i = (pos.floor() as usize).min(trail.grade.len() - 2);
    trail.grade[i] + (trail.grade[i + 1] - trail.grade[i]) * (pos - i as f32)
}

pub(super) fn block(
    cfg: &WorldSampleConfig,
    trails: &[Trail],
    m: Vec3,
    natural_top: f32,
) -> Option<B> {
    let best = nearest(trails.iter(), m)?;
    paint(cfg, best, m, natural_top)
}

/// The closest surveyed route whose shoulder reaches `m`.
pub(super) fn nearest<'a>(trails: impl Iterator<Item = &'a Trail>, m: Vec3) -> Option<(f32, f32, &'a Trail)> {
    let mut best = None;
    for trail in trails {
        if trail.grade.is_empty()
            || m.x < trail.min_x
            || m.x > trail.max_x
            || m.z < trail.min_z
            || m.z > trail.max_z
        {
            continue;
        }
        let (distance, t) = trail.nearest(m.x, m.z);
        if distance > trail.half_width + SHOULDER {
            continue;
        }
        if best.is_none_or(|(d, _, _)| distance < d) {
            best = Some((distance, t, trail));
        }
    }
    best
}

pub(super) fn paint(cfg: &WorldSampleConfig, (distance, t, trail): (f32, f32, &Trail), m: Vec3, natural_top: f32) -> Option<B> {
    if let Some(span) = trail.bridges.iter().find(|s| t >= s.start && t <= s.end) {
        return bridges::block(cfg, trail, span, m, natural_top, distance, t);
    }
    let grade = height_at(trail, t);
    let verge = if super::planet_roads::active(cfg) { 2.0 } else { SHOULDER };
    if distance > trail.half_width + verge {
        return None;
    }
    let shoulder = crate::smoothstep(trail.half_width, trail.half_width + verge, distance);
    let v = cfg.voxel_meters();
    let top = ((grade + (natural_top - grade) * shoulder) / v).round() * v;
    if m.y >= top && m.y < natural_top.max(top + 3.0) {
        return Some(B::Air);
    }
    if m.y < top && m.y >= top - v {
        return Some(bed(cfg, trail.half_width, m, distance));
    }
    if m.y < top && m.y >= natural_top.min(top - v) {
        return Some(B::Dirt);
    }
    None
}

/// Top cell of a trail bed `distance` metres from its centre line. From 133 a
/// worn earth band with a wandering edge replaces the one-voxel gravel/turf
/// switch. Both terms are metric, so every voxel scale draws the same edge;
/// world-fixed noise lets crossing trails agree.
fn bed(cfg: &WorldSampleConfig, half_width: f32, m: Vec3, distance: f32) -> B {
    if cfg.generator_version < super::MEADOW_GROUND_VERSION {
        return if distance <= half_width { B::Gravel } else { B::Grass };
    }
    let seed = cfg.seed ^ 0x51ed_0133;
    let wobble = (crate::value_noise2(m.x / 3.2, m.z / 3.2, seed) - 0.5) * 0.9;
    let cell = cfg.voxel_meters();
    let fray = crate::hash2((m.x / cell).floor() as i32, (m.z / cell).floor() as i32, seed ^ 0x2f17);
    let d = distance + wobble;
    if super::planet_roads::active(cfg) && half_width == FOOTPATH_HALF_WIDTH {
        return if d <= half_width + 0.45 { B::Dirt } else { B::Grass };
    }
    if d <= half_width - 0.35 {
        B::Gravel
    } else if d <= half_width + 0.55 || (d <= half_width + 1.4 && fray > 0.72) {
        B::Dirt
    } else {
        B::Grass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::MEADOW_GROUND_VERSION;
    use engine_core::VoxelScalePreset as S;
    use engine_voxel::WorldType;

    fn cfg(generator_version: u32, voxel_scale: S) -> WorldSampleConfig {
        WorldSampleConfig { planet: None, generator_version, seed: crate::test_world::seed(), world_type: WorldType::Hills, voxel_scale }
    }

    #[test]
    fn older_generators_keep_the_one_voxel_gravel_edge() {
        let c = cfg(MEADOW_GROUND_VERSION - 1, S::OneMeter);
        for i in 0..80 {
            let m = Vec3::new(i as f32 * 0.7 - 20.0, 30.0, -i as f32 * 1.3);
            for d in [0.0, 1.5, 1.6, 1.7, 3.0, 5.0] {
                assert_eq!(bed(&c, 1.6, m, d), if d <= 1.6 { B::Gravel } else { B::Grass });
            }
        }
    }

    #[test]
    fn verges_fray_from_gravel_through_earth_to_turf_with_a_wandering_edge() {
        let c = cfg(MEADOW_GROUND_VERSION, S::OneMeter);
        let mut edges = Vec::new();
        for i in 0..200 {
            let m = Vec3::new(i as f32 * 0.5 - 50.0, 30.0, 12.0);
            assert_eq!(bed(&c, 1.6, m, 0.0), B::Gravel);
            assert_eq!(bed(&c, 1.6, m, 4.0), B::Grass);
            let earth = (0..40).map(|k| k as f32 * 0.1).find(|&d| bed(&c, 1.6, m, d) == B::Dirt);
            edges.push(earth.expect("every cross-section has an earth band"));
        }
        let (lo, hi) = edges.iter().fold((f32::MAX, f32::MIN), |(a, b), &e| (a.min(e), b.max(e)));
        assert!(hi - lo > 0.4, "edge must wander: {lo}..{hi}");
    }

    #[test]
    fn the_edge_is_metric_across_voxel_scales() {
        // Fray flecks differ by cell; the main boundary does not.
        let (one, quarter) = (cfg(MEADOW_GROUND_VERSION, S::OneMeter), cfg(MEADOW_GROUND_VERSION, S::QuarterMeter));
        let mut same = 0;
        for i in 0..400 {
            let m = Vec3::new(-300.0 + i as f32 * 1.7, 30.0, 40.0 - i as f32 * 0.9);
            for d in [0.5, 1.0, 1.9, 2.4, 3.6] {
                same += usize::from(bed(&one, 1.6, m, d) == bed(&quarter, 1.6, m, d));
            }
        }
        assert!(same > 1800, "{same} of 2000");
    }

    /// Surface block of each column in a meadow box that dry swales cross on
    /// the review seed (they drew straight mud ribbons before 133).
    fn surfaces(c: &WorldSampleConfig) -> Vec<B> {
        let mut out = Vec::new();
        for z in (-200..-100).step_by(4) {
            for x in (-130..-30).step_by(4) {
                let h = crate::sample_column(c, x, z).height;
                out.push((h - 2..=h + 2).rev().map(|y| crate::sample_block(c, glam::IVec3::new(x, y, z))).find(|&b| b != B::Air).unwrap_or(B::Air));
            }
        }
        out
    }

    #[test]
    fn dry_swales_keep_their_turf_from_the_gate() {
        let mud = |v: u32| surfaces(&cfg(v, S::OneMeter)).iter().filter(|&&b| b == B::Mud).count();
        let (old, new) = (mud(126), mud(MEADOW_GROUND_VERSION));
        assert!(old > 40, "fixture must cross dry swales: {old}");
        assert!(new * 3 < old, "{old} -> {new}");
    }
}
