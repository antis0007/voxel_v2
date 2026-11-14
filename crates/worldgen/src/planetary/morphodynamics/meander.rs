//! Reduced bend migration for a connected reach, followed by neck capture.
//! Curvature drives a lagged near-bank stress. A cutoff occurs when two
//! separated limbs erode into the same neck, with a downhill path between them.
use super::{river::Surface, smooth};
use crate::geo::Ground;
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::{DVec2, DVec3, IVec2};
use std::collections::{HashMap, VecDeque};

pub(in crate::planetary) struct Reach {
    pub route: Vec<DVec3>,
    pub width: f64,
    pub head: f64,
    pub tail: f64,
}

struct Frame {
    origin: DVec3,
    along: DVec3,
    across: DVec3,
}

impl Frame {
    fn coordinates(&self, p: DVec3) -> DVec2 {
        let offset = p - self.origin;
        DVec2::new(offset.dot(self.along), offset.dot(self.across))
    }

    fn direction(&self, p: DVec2) -> DVec3 {
        (self.origin + self.along * p.x + self.across * p.y).normalize()
    }
}

struct Path {
    points: Vec<DVec2>,
    length: Vec<f64>,
}

impl Path {
    fn new(points: Vec<DVec2>) -> Self {
        let mut length = Vec::with_capacity(points.len());
        length.push(0.0);
        for pair in points.windows(2) {
            length.push(length.last().unwrap() + pair[0].distance(pair[1]));
        }
        Self { points, length }
    }

    fn total(&self) -> f64 {
        *self.length.last().unwrap()
    }

    fn nearest(&self, p: DVec2) -> (f64, f64) {
        let mut best = (f64::INFINITY, 0.0);
        for (i, pair) in self.points.windows(2).enumerate() {
            let edge = pair[1] - pair[0];
            let u = ((p - pair[0]).dot(edge) / edge.length_squared().max(1e-9)).clamp(0.0, 1.0);
            let distance = p.distance_squared(pair[0] + edge * u);
            if distance < best.0 {
                best = (
                    distance,
                    self.length[i] + u * (self.length[i + 1] - self.length[i]),
                );
            }
        }
        (best.0.sqrt(), best.1)
    }

    fn regular(&self, step: f64) -> Self {
        let n = (self.total() / step).ceil().max(2.0) as usize;
        let mut i = 0;
        let points = (0..=n)
            .map(|k| {
                let along = self.total() * k as f64 / n as f64;
                while i + 2 < self.length.len() && along > self.length[i + 1] {
                    i += 1;
                }
                let t = (along - self.length[i]) / (self.length[i + 1] - self.length[i]).max(1e-9);
                self.points[i].lerp(self.points[i + 1], t)
            })
            .collect();
        Self::new(points)
    }
}

struct Capture {
    active: Path,
    loop_: Path,
    loop_heads: [f64; 2],
    contact: DVec2,
    migration: f64,
    retired: f64,
    shortcut: f64,
    episodes: usize,
    scrolls: Vec<[DVec2; 2]>,
}

fn touching(path: &Path, width: f64, fall: f64) -> Option<(usize, usize)> {
    let min_arc = (width * 12.0).max(300.0);
    let mut best = None;
    let cell = 1.72 * width;
    let mut shortest = cell;
    let bin = |p: DVec2| (p / cell).floor().as_ivec2();
    let mut bins = HashMap::<_, Vec<_>>::new();
    for (i, &p) in path.points.iter().enumerate() {
        if path.length[i].min(path.total() - path.length[i]) >= 125.0 {
            bins.entry(bin(p)).or_default().push(i);
        }
    }
    for i in 4..path.points.len().saturating_sub(8) {
        if path.length[i] < 125.0 {
            continue;
        }
        let centre = bin(path.points[i]);
        for y in -1..=1 {
            for x in -1..=1 {
                let Some(points) = bins.get(&(centre + IVec2::new(x, y))) else {
                    continue;
                };
                for &j in points.iter().filter(|&&j| j >= i + 5) {
                    let arc = path.length[j] - path.length[i];
                    if arc < min_arc || arc / path.total() * fall < 0.30 {
                        continue;
                    }
                    let gap = path.points[i].distance(path.points[j]);
                    if gap < shortest
                        && arc > gap * 3.0
                        && loop_area(&path.points[i..=j]) > width * arc
                    {
                        shortest = gap;
                        best = Some((i, j));
                    }
                }
            }
        }
    }
    best
}

fn loop_area(points: &[DVec2]) -> f64 {
    let origin = points[0];
    0.5 * points
        .windows(2)
        .map(|p| (p[0] - origin).perp_dot(p[1] - origin))
        .sum::<f64>()
        .abs()
}

fn migrate(
    initial: &Path,
    width: f64,
    head: f64,
    tail: f64,
    record_scrolls: bool,
    mut ground: impl FnMut(DVec2) -> f64,
) -> Option<Capture> {
    let step = (width * 0.68).clamp(8.0, 19.0);
    let mut path = initial.regular(step);
    let mut scrolls = Vec::new();
    if touching(&path, width, head - tail).is_some() {
        return None;
    }
    for episodes in 1..=380 {
        let n = path.points.len();
        let mut curvature = vec![0.0; n];
        let mut normals = vec![DVec2::ZERO; n];
        // Estimate the bend over several widths. Cell-sized turns are mesh
        // detail and cannot drive physical bank retreat.
        let span = (width * 3.5 / step).ceil() as usize;
        let lag = 1.0 - (-step / (3.5 * width)).exp();
        for i in 1..n - 1 {
            let incoming = path.points[i] - path.points[i.saturating_sub(span)];
            let outgoing = path.points[(i + span).min(n - 1)] - path.points[i];
            let normal = (incoming + outgoing).normalize_or_zero().perp();
            normals[i] = normal;
            curvature[i] = (outgoing.normalize_or_zero() - incoming.normalize_or_zero())
                .dot(normal)
                / ((incoming.length() + outgoing.length()) * 0.5).max(step);
        }
        let mut excess = 0.0;
        let mut moved = path.points.clone();
        let mut last_bank = None;
        for i in 1..n - 1 {
            let local = (curvature[i - 1] + 2.0 * curvature[i] + curvature[i + 1]) * 0.25;
            excess += (local - excess) * lag;
            let stage = head + (tail - head) * path.length[i] / path.total();
            let margin = path.length[i].min(path.total() - path.length[i]);
            let travel = (-(local * 0.35 + excess * 0.65) * width * width * 0.93).clamp(-3.8, 3.8);
            let target = path.points[i] + normals[i] * travel;
            let resistance = 1.0 - smooth(1.4, 8.0, ground(target) - stage);
            if record_scrolls && episodes % 32 == 0 {
                let migration = travel * resistance * smooth(width, width * 5.0, margin);
                if migration.abs() >= 0.55 {
                    let side = migration.signum();
                    let bank = path.points[i] - normals[i] * (side * width);
                    if let Some((previous, previous_side)) = last_bank
                        && side == previous_side
                        && bank.distance(previous) <= step * 2.0
                    {
                        scrolls.push([previous, bank]);
                    }
                    last_bank = Some((bank, side));
                } else {
                    last_bank = None;
                }
            }
            let relax = (path.points[i - 1] + path.points[i + 1] - 2.0 * path.points[i]) * 0.28;
            moved[i] +=
                (normals[i] * travel * resistance + relax) * smooth(width, width * 5.0, margin);
        }
        path = Path::new(moved).regular(step);
        if path.total() > initial.total() * 1.9 {
            return None;
        }
        if let Some((a, b)) = touching(&path, width, head - tail) {
            let retired = path.length[b] - path.length[a];
            let shortcut = path.points[a].distance(path.points[b]);
            let contact = path.points[a].lerp(path.points[b], 0.5);
            let migration = path.points[a..=b]
                .iter()
                .map(|&p| initial.nearest(p).0)
                .fold(0.0, f64::max);
            if migration < width * 0.5 {
                return None;
            }
            let stage = |i| head + (tail - head) * path.length[i] / path.total();
            let active = path.points[..=a]
                .iter()
                .chain(&path.points[b..])
                .copied()
                .collect();
            return Some(Capture {
                active: Path::new(active),
                loop_: Path::new(path.points[a..=b].to_vec()),
                loop_heads: [stage(a), stage(b)],
                contact,
                migration,
                retired,
                shortcut,
                episodes,
                scrolls,
            });
        }
    }
    None
}

pub(in crate::planetary) struct Tile {
    frame: Frame,
    min: DVec2,
    size: [usize; 2],
    spacing: f64,
    cells: Box<[[f32; 4]]>,
    pub contact: DVec3,
    pub oxbow: DVec3,
    pub migration: f64,
    pub retired: f64,
    pub shortcut: f64,
    pub episodes: usize,
    pub isolated_area: f64,
}

fn connected_water(wet: &[bool], [nx, ny]: [usize; 2], start: usize) -> Vec<bool> {
    let mut seen = vec![false; wet.len()];
    let mut pending = VecDeque::from([start]);
    seen[start] = true;
    while let Some(i) = pending.pop_front() {
        let (x, y) = (i % nx, i / nx);
        for j in [
            (x > 0).then(|| i - 1),
            (x + 1 < nx).then(|| i + 1),
            (y > 0).then(|| i - nx),
            (y + 1 < ny).then(|| i + nx),
        ]
        .into_iter()
        .flatten()
        {
            if wet[j] && !seen[j] {
                seen[j] = true;
                pending.push_back(j);
            }
        }
    }
    seen
}

fn scroll_relief(
    banks: &[[DVec2; 2]],
    min: DVec2,
    [nx, ny]: [usize; 2],
    spacing: f64,
    width: f64,
) -> Option<Vec<f32>> {
    if banks.is_empty() {
        return None;
    }
    let crest = (width * 0.40).clamp(5.0, 11.0);
    let toe = crest * 2.0;
    let mut relief = vec![0.0_f32; nx * ny];
    for &[a, b] in banks {
        let lo = ((a.min(b) - min - toe) / spacing)
            .floor()
            .as_ivec2()
            .max(IVec2::ZERO);
        let hi = ((a.max(b) - min + toe) / spacing)
            .ceil()
            .as_ivec2()
            .min(IVec2::new(nx as i32 - 1, ny as i32 - 1));
        let edge = b - a;
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let p = min + DVec2::new(f64::from(x), f64::from(y)) * spacing;
                let along = ((p - a).dot(edge) / edge.length_squared().max(1e-9)).clamp(0.0, 1.0);
                let distance = p.distance(a + edge * along);
                let height = 0.8 * (1.0 - smooth(0.0, crest, distance))
                    + 0.18 * (1.0 - smooth(crest, toe, distance));
                let i = y as usize * nx + x as usize;
                relief[i] = relief[i].max(height as f32);
            }
        }
    }
    Some(relief)
}

impl Tile {
    pub fn bake(reach: Reach, scrolls: bool, terrain: impl Fn(DVec3) -> Ground) -> Option<Self> {
        let first = reach.route[0];
        let last = *reach.route.last()?;
        let origin = (first + last).normalize() * RADIUS_M;
        let along = (last - first)
            .reject_from_normalized(origin.normalize())
            .normalize();
        let frame = Frame {
            origin,
            along,
            across: origin.normalize().cross(along),
        };
        let original = Path::new(
            reach
                .route
                .into_iter()
                .map(|p| frame.coordinates(p))
                .collect(),
        )
        .regular(14.0);
        let width = reach.width;
        let terrain_at = |p| terrain(frame.direction(p));
        let mut resistance = HashMap::new();
        let capture = migrate(&original, width, reach.head, reach.tail, scrolls, |p| {
            let p = p / 24.0;
            let corner = p.floor().as_ivec2();
            let uv = p - p.floor();
            let mut h = |offset: IVec2| {
                let at = corner + offset;
                *resistance
                    .entry(at)
                    .or_insert_with(|| f64::from(terrain_at(at.as_dvec2() * 24.0).top_m))
            };
            let a = h(IVec2::ZERO) * (1.0 - uv.x) + h(IVec2::X) * uv.x;
            let b = h(IVec2::Y) * (1.0 - uv.x) + h(IVec2::ONE) * uv.x;
            a * (1.0 - uv.y) + b * uv.y
        })?;
        let centre = capture.loop_.points[capture.loop_.points.len() / 2];
        // A neck at a lake margin can appear connected through the lake even
        // when the new river bed has no through-flow. Leave shorelines to the
        // standing-water solve and require dry land beside the captured bend.
        for anchor in [capture.contact, centre] {
            let mut open_water = 0;
            for i in 0..12 {
                let angle = f64::from(i) * std::f64::consts::TAU / 12.0;
                let p = anchor + DVec2::from_angle(angle) * (width * 5.0).max(90.0);
                let base = terrain_at(p);
                if base.water_m.is_some_and(|stage| stage - base.top_m > 0.35) {
                    open_water += 1;
                }
            }
            if open_water >= 4 {
                return None;
            }
        }
        let path_points = || {
            original
                .points
                .iter()
                .chain(&capture.active.points)
                .chain(&capture.loop_.points)
                .copied()
        };
        let belt = (width * 5.0 + 78.0).clamp(120.0, 300.0);
        let min = path_points().reduce(DVec2::min)? - DVec2::splat(belt);
        let max = path_points().reduce(DVec2::max)? + DVec2::splat(belt);
        let span = max - min;
        let spacing = (span.max_element() / 390.0).max(4.0);
        let size = [
            (span.x / spacing).ceil() as usize + 1,
            (span.y / spacing).ceil() as usize + 1,
        ];
        let mut cells = Vec::with_capacity(size[0] * size[1]);
        let bars = scroll_relief(&capture.scrolls, min, size, spacing, width);
        let mut wet = Vec::with_capacity(cells.capacity());
        let lake_head = capture.loop_heads[1] - 0.16;
        let depth = (0.82 + width * 0.055).min(2.6);
        let valley_start = original.points[0].x;
        let valley_run = original.points.last()?.x - valley_start;
        for y in 0..size[1] {
            for x in 0..size[0] {
                let p = min + DVec2::new(x as f64, y as f64) * spacing;
                let (old, _) = original.nearest(p);
                let (active, along) = capture.active.nearest(p);
                let (inactive, around) = capture.loop_.nearest(p);
                let distance = old.min(active.min(inactive));
                let end = along.min(capture.active.total() - along);
                let blend = (1.0 - smooth(belt * 0.60, belt, distance)) * smooth(0.0, 95.0, end);
                let base = terrain_at(p);
                let z = f64::from(base.top_m);
                let original_depth = base.water_m.map_or(0.0, |h| (f64::from(h) - z).max(0.0));
                if blend <= 0.0 {
                    cells.push([0.0; 4]);
                    wet.push(original_depth > 0.11);
                    continue;
                }
                let stage = reach.head + (reach.tail - reach.head) * along / capture.active.total();
                // The valley floor has a continuous grade across the bend.
                // A channel station can jump to the opposite limb across
                // a few metres of land, so use it only in the channel.
                let progress = ((p.x - valley_start) / valley_run).clamp(0.0, 1.0);
                let plain_stage = reach.head + (reach.tail - reach.head) * progress;
                let bank = plain_stage + 0.8 + 0.008 * active.min(belt);
                let alluvial = (1.0 - smooth(0.08, 0.43, f64::from(base.slope))) * blend;
                let mut bed = z - (z - bank).clamp(0.0, 11.0) * alluvial;
                // Fine sediment settles in the retired entrances. Leave a
                // remnant low channel between the two floodplain plugs.
                if old < width * 1.35 || inactive < width * 1.25 {
                    let blanket = 1.0 - smooth(width * 0.74, width * 1.35, old.min(inactive));
                    bed += (plain_stage + 0.45 - bed).clamp(0.0, 5.0) * blanket;
                }
                let plug = smooth(
                    width * 2.1,
                    width * 3.3,
                    around.min(capture.loop_.total() - around),
                );
                let relic_width = width * 0.74 * plug;
                let mut water = f64::NEG_INFINITY;
                if inactive < relic_width && active > width * 1.32 {
                    let former_stage = capture.loop_heads[0]
                        + (capture.loop_heads[1] - capture.loop_heads[0]) * around
                            / capture.loop_.total();
                    let depth = 1.12 * plug * (1.0 - (inactive / relic_width).powi(2));
                    bed = bed.min(former_stage.min(lake_head + 0.72) - depth);
                    water = lake_head;
                }
                if active < width {
                    bed = bed.min(stage - depth * (1.0 - (active / width).powi(3)));
                    water = stage;
                } else if active < width * 2.7 {
                    let bank_top = stage + 0.95 * smooth(width, width * 1.7, active);
                    let shoulder = 1.0 - smooth(width, width * 2.7, active);
                    bed -= (bed - bank_top).max(0.0) * shoulder;
                }
                if let Some(bars) = &bars
                    && water == f64::NEG_INFINITY
                {
                    // Older point bars remain as low ridges across the plain.
                    // Floods cannot lay down these bars on high or steep banks.
                    let clear = smooth(width * 1.25, width * 2.0, active.min(inactive));
                    let low = 1.0 - smooth(1.6, 4.0, bed - plain_stage);
                    bed += f64::from(bars[y * size[0] + x]) * clear * low * alluvial;
                }
                let depth = (water - bed).max(0.0);
                wet.push(original_depth + (depth - original_depth) * blend > 0.11);
                cells.push([bed as f32, depth as f32, (bed - z) as f32, blend as f32]);
            }
        }
        let point =
            |i: usize| min + DVec2::new((i % size[0]) as f64, (i / size[0]) as f64) * spacing;
        let nearest = |p: DVec2, candidates: &[bool]| {
            candidates
                .iter()
                .enumerate()
                .filter(|&(_, &wet)| wet)
                .map(|(i, _)| (i, point(i).distance_squared(p)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|&(_, d)| d < (width * 1.65).powi(2))
                .map(|(i, _)| i)
        };
        let live = connected_water(&wet, size, nearest(capture.contact, &wet)?);
        if nearest(
            capture.active.points[capture.active.points.len() / 8],
            &live,
        )
        .is_none()
            || nearest(
                capture.active.points[capture.active.points.len() * 7 / 8],
                &live,
            )
            .is_none()
        {
            return None;
        }
        let remnant: Vec<_> = wet
            .iter()
            .zip(&live)
            .map(|(&wet, &live)| wet && !live)
            .collect();
        let pond = connected_water(&remnant, size, nearest(centre, &remnant)?);
        let isolated_area = pond.iter().filter(|&&wet| wet).count() as f64 * spacing * spacing;
        if isolated_area < width * width * 1.3 {
            return None;
        }
        Some(Self {
            contact: frame.direction(capture.contact) * RADIUS_M,
            oxbow: frame.direction(centre) * RADIUS_M,
            frame,
            min,
            size,
            spacing,
            cells: cells.into_boxed_slice(),
            migration: capture.migration,
            retired: capture.retired,
            shortcut: capture.shortcut,
            episodes: capture.episodes,
            isolated_area,
        })
    }

    pub fn origin(&self) -> DVec3 {
        self.frame.origin
    }

    pub fn radius(&self) -> f64 {
        self.min.length() + self.spacing * (self.size[0] as f64).hypot(self.size[1] as f64)
    }

    pub fn overlaps(&self, p: DVec3, margin: f64) -> bool {
        let max = self.min + DVec2::new(self.size[0] as f64, self.size[1] as f64) * self.spacing;
        let offset = self.frame.coordinates(p);
        // Tangent coordinates alone fold the opposite hemisphere onto the
        // same patch. Check the spherical side before using the rectangle.
        p.dot(self.frame.origin) > 0.0 && offset.distance(offset.clamp(self.min, max)) <= margin
    }

    pub fn checksum(&self, mut hash: u64) -> u64 {
        for cell in &self.cells {
            for &v in cell {
                hash = (hash ^ u64::from(v.to_bits())).wrapping_mul(0x100000001b3);
            }
        }
        hash
    }

    pub fn sample(&self, p: DVec3, footprint: f32) -> Option<Surface> {
        let xy = (self.frame.coordinates(p) - self.min) / self.spacing;
        let [nx, ny] = self.size;
        if xy.x < 0.0 || xy.y < 0.0 || xy.x >= (nx - 1) as f64 || xy.y >= (ny - 1) as f64 {
            return None;
        }
        let x = xy.x as usize;
        let y = xy.y as usize;
        let u = xy.x.fract();
        let v = xy.y.fract();
        let i = y * nx + x;
        let mut cell = [0.0; 4];
        for (i, weight) in [
            (i, (1.0 - u) * (1.0 - v)),
            (i + 1, u * (1.0 - v)),
            (i + nx, (1.0 - u) * v),
            (i + nx + 1, u * v),
        ] {
            let weighted = f64::from(self.cells[i][3]) * weight;
            for (k, value) in cell[..3].iter_mut().enumerate() {
                *value += f64::from(self.cells[i][k]) * weighted;
            }
            cell[3] += weighted;
        }
        if cell[3] <= 0.0 {
            return None;
        }
        let blend = cell[3] * (1.0 - smooth(28.0, 140.0, f64::from(footprint)));
        let shift = cell[2] / cell[3];
        Some(Surface {
            bed: cell[0] / cell[3],
            depth: cell[1] / cell[3],
            shift,
            deposited: shift,
            blend,
            rock_exposure: 0.0,
        })
    }
}
