//! Surveyed surface routes. Cached with the surface plan; no per-voxel surveys.
use super::{B, Trail, Vec3, WorldSampleConfig, sample_column};

const SHOULDER: f32 = 4.0;
const MAX_GRADE: f32 = 0.32;
mod bridges;
pub(super) use bridges::Span;

fn ground(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let c = sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    cfg.world_y(c.height + 1)
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
                || crate::sites::trail_obstructed(cfg, sx, sz)) {
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

// Older saves retain their exact route layout. New routes can go around a
// blocked hill or building instead of merely bending the same failed chord.
fn connection(cfg: &WorldSampleConfig, a: Vec3, b: Vec3, salt: u32) -> Option<Vec<Trail>> {
    if let Some(trail) = connect(cfg, a, b, salt) {
        return Some(vec![trail]);
    }
    if cfg.generator_version < 40 { return None; }
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
    let (distance, t, trail) = best?;
    if let Some(span) = trail.bridges.iter().find(|s| t >= s.start && t <= s.end) {
        return bridges::block(cfg, trail, span, m, natural_top, distance, t);
    }
    let grade = height_at(trail, t);
    let shoulder = crate::smoothstep(trail.half_width, trail.half_width + SHOULDER, distance);
    let v = cfg.voxel_meters();
    let top = ((grade + (natural_top - grade) * shoulder) / v).round() * v;
    if m.y >= top && m.y < natural_top.max(top + 3.0) {
        return Some(B::Air);
    }
    if m.y < top && m.y >= top - v {
        return Some(if distance <= trail.half_width {
            B::Gravel
        } else {
            B::Grass
        });
    }
    if m.y < top && m.y >= natural_top.min(top - v) {
        return Some(B::Dirt);
    }
    None
}
