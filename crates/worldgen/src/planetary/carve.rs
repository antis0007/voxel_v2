//! Generator 300 carved surface: gullies and rills below the erosion tiles.
//!
//! The regional erosion stages resolve drainage down to 8 m cells. Finer
//! ground carried only an isotropic ripple, so a hillside read as a smooth
//! sheet. This stage cuts the missing gullies as a pure function of position
//! and the surface gradient the tiles already solved.
//!
//! Each octave lays stripes across the slope as a phasor sum over the corners
//! of a 3D body-space lattice (after the Phacelle erosion filter, Johansen
//! 2026) and then adds its own side slope to the gradient the next octave
//! follows, so finer rills run down the walls of coarser gullies and join
//! them. The coarsest octave is true phasor noise: every lattice corner draws
//! straight stripes along the gradient sampled at that corner. Reading one
//! direction at the sample and anchoring its phase a cell away turned the
//! stripes along the contours wherever the solved gradient curled within a
//! kernel, which on eroded ground is everywhere.
//!
//! All wavelengths are body metres, so cube faces share the pattern. Octaves
//! finer than the sample footprint are filtered and never change the coarser
//! ones: horizon and voxels agree. No simulation, no chunk seams.
use super::smooth;
use glam::DVec3;
use std::cell::{Cell, RefCell};
use std::f64::consts::TAU;

/// Gully spacing per octave, metres. The first sits just above the 16 m
/// Nyquist wavelength of the 8 m erosion tiles.
const WAVES_M: [f64; 5] = [22.0, 11.0, 5.5, 2.75, 1.4];
/// Lattice cell in wavelengths; longer cells keep a gully coherent for longer
/// between forks.
const CELL_WAVES: f64 = 1.25;
/// Cell of the lattice whose corners hold the drainage gradient, metres.
pub(super) const LATTICE_M: f64 = WAVES_M[0] * CELL_WAVES;
/// Side slope of a gully wall on a fully driven, bare slope, m/m.
const WALL: f64 = 0.62;
/// Wall slope ratio of each finer octave.
const GAIN: f64 = 0.74;
/// How strongly an octave's side slope steers the next one.
const STEER: f64 = 0.55;
/// Largest lean of the fall line, as a share of the hill's own slope. Gully
/// walls are often steeper than the hill; unbounded, their summed side
/// slopes turned the fine rills along the contours.
const LEAN: f64 = 0.55;
/// Ribs stand this share of an octave's depth below the uncut surface;
/// creases cut `1 + BIAS` below it. Erosion removes ground, it adds none.
const BIAS: f64 = 0.45;

thread_local! {
    /// Channel and shore fade of the last base sample on this thread.
    static OPEN: Cell<f64> = const { Cell::new(0.0) };
    /// Drainage gradient and convergence per lattice corner, direct mapped.
    static CORNERS: RefCell<Vec<(CornerKey, Corner)>> = const { RefCell::new(Vec::new()) };
}

/// World identity and lattice coordinate of one corner.
pub(super) type CornerKey = (u64, i64, i64, i64);

/// What the solved surface does at one lattice corner.
#[derive(Clone, Copy)]
pub(super) struct Corner {
    /// Uphill gradient, tangent, m/m; zero where nothing may be cut.
    pub gradient: DVec3,
    /// Divergence of that gradient, 1/m: positive in a hollow where slopes
    /// lean together and runoff gathers, negative on a spur that sheds it.
    pub convergence: f64,
}

impl Corner {
    pub(super) const NONE: Self = Self { gradient: DVec3::ZERO, convergence: 0.0 };
}

/// Spacing of the two extra gradient samples that measure convergence, metres.
pub(super) const CONVERGENCE_STEP_M: f64 = 12.0;
const CORNER_SLOTS: usize = 2048;
/// Corners every thread has computed, behind the per-thread table. Twelve
/// terrain workers building neighbouring chunks share nearly all of their
/// corners; without this each worker paid three ground samples per corner
/// again. Dropped whole when it fills, which a session reaches rarely.
static SHARED: std::sync::Mutex<Option<std::collections::HashMap<CornerKey, Corner>>> = std::sync::Mutex::new(None);
const SHARED_LIMIT: usize = 1 << 18;

/// The base ground publishes how far the sample is from channels, lakes and
/// the sea (0 beside them, 1 on open slopes).
pub(super) fn publish_open(open: f64) { OPEN.with(|cell| cell.set(open)); }

/// Read it once, before any nested ground query can overwrite it.
pub(super) fn take_open() -> f64 { OPEN.with(|cell| cell.replace(0.0)) }

/// The solved surface at a lattice corner, computed once per world while it
/// stays cached. A chunk touches a dozen corners for a thousand columns.
pub(super) fn cached_corner(key: CornerKey, compute: impl FnOnce() -> Corner) -> Corner {
    let slot = (key.1.wrapping_mul(0x9E37_79B9).wrapping_add(key.2.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(key.3.wrapping_mul(0xC2B2_AE35)) ^ key.0 as i64).rem_euclid(CORNER_SLOTS as i64) as usize;
    if let Some(hit) = CORNERS.with(|table| {
        table.borrow().get(slot).filter(|(k, _)| *k == key).map(|(_, corner)| *corner)
    }) {
        return hit;
    }
    // Both tables are released while a corner is computed: it samples the
    // ground, which may itself build erosion tiles.
    let shared = SHARED.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref().and_then(|map| map.get(&key).copied());
    let corner = shared.unwrap_or_else(|| {
        let corner = compute();
        let mut guard = SHARED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        if map.len() >= SHARED_LIMIT { map.clear(); }
        map.insert(key, corner);
        corner
    });
    CORNERS.with(|table| {
        let mut table = table.borrow_mut();
        if table.is_empty() { table.resize(CORNER_SLOTS, ((0, i64::MIN, 0, 0), Corner::NONE)); }
        table[slot] = (key, corner);
    });
    corner
}

pub(super) struct Site {
    /// Unit radial direction.
    pub up: DVec3,
    /// Resistant bedrock near the surface, 0..1.
    pub hard: f64,
    /// Climate wetness 0..1; wet slopes carry a soil mantle that creeps smooth.
    pub wet: f64,
    /// 0 beside channels, lakes and the sea, 1 on open slopes.
    pub open: f64,
}

pub(super) struct Carved {
    /// Height change, metres; mostly negative.
    pub delta_m: f64,
    /// +1 on a rib crest, -1 in a crease, from the two coarsest octaves.
    pub rib: f64,
    /// Side slope the gullies add to the surface gradient, tangent, m/m.
    pub steer: DVec3,
    /// Share of the full pattern that was cut here, 0..1.
    pub strength: f64,
}

fn filtered(wavelength: f64, footprint: f64) -> f64 {
    1.0 - smooth(wavelength * 0.25, wavelength * 1.5, footprint)
}

/// Four independent values in -0.5..0.5 for one lattice corner: an anchor
/// offset and a spacing factor.
pub(super) fn corner(seed: u32, x: i64, y: i64, z: i64) -> (DVec3, f64) {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ (u64::from(seed) << 32 | u64::from(seed));
    h ^= h >> 31;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 32;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 32;
    let unit = |bits: u64| (bits & 0xFFFF) as f64 / 65_536.0 - 0.5;
    (DVec3::new(unit(h), unit(h >> 16), unit(h >> 32)), unit(h >> 48))
}

/// The eight lattice corners around `p` for cells of `cell_m`: lattice
/// coordinate, offset from the cell origin, partition-of-unity weight.
pub(super) fn corners(p: DVec3, cell_m: f64) -> (DVec3, [([i64; 3], DVec3, f64); 8]) {
    let q = p / cell_m;
    let base = q.floor();
    let f = q - base;
    let fade = |t: f64| t * t * (3.0 - 2.0 * t);
    let far = DVec3::new(fade(f.x), fade(f.y), fade(f.z));
    let near = DVec3::ONE - far;
    let (bx, by, bz) = (base.x as i64, base.y as i64, base.z as i64);
    (f, std::array::from_fn(|i| {
        let (ox, oy, oz) = ((i & 1) as i64, ((i >> 1) & 1) as i64, ((i >> 2) & 1) as i64);
        let weight = (if ox == 0 { near.x } else { far.x })
            * (if oy == 0 { near.y } else { far.y })
            * (if oz == 0 { near.z } else { far.z });
        ([bx + ox, by + oy, bz + oz], DVec3::new(ox as f64, oy as f64, oz as f64), weight)
    }))
}

/// One phasor kernel: stripes of the corner's own spacing varying along
/// `across`, anchored at the jittered corner.
fn kernel(seed: u32, at: [i64; 3], offset: DVec3, f: DVec3, across: DVec3) -> (f64, f64) {
    let (jitter, spacing) = corner(seed, at[0], at[1], at[2]);
    // Each corner draws its stripes 0.72 to 1.28 times the nominal spacing;
    // where two disagree a gully forks or dies, so spacing is irregular the
    // way drainage is, not a ruled washboard.
    let (sin, cos) = (TAU * CELL_WAVES * (1.0 + 0.56 * spacing) * (f - offset - jitter * 0.8).dot(across)).sin_cos();
    (cos, sin)
}

/// Soft normalisation: full amplitude where corners agree, a quiet fork
/// where they cancel, never a division by nothing.
fn normalised(c: f64, s: f64) -> (f64, f64) {
    let scale = 1.0 / ((c * c + s * s).sqrt() + 0.14);
    (c * scale, s * scale)
}

/// Stripes of `wavelength` varying along one unit vector `across`, as the
/// cosine and sine of a phasor summed over the surrounding lattice corners.
pub(super) fn stripes(seed: u32, p: DVec3, across: DVec3, wavelength: f64) -> (f64, f64) {
    let (f, around) = corners(p, wavelength * CELL_WAVES);
    let (mut c, mut s) = (0.0, 0.0);
    for (at, offset, weight) in around {
        // The surface is a sheet through the lattice: most corners are far
        // from it and add nothing a voxel can see.
        if weight <= 0.004 { continue; }
        let (cos, sin) = kernel(seed, at, offset, f, across);
        c += weight * cos;
        s += weight * sin;
    }
    normalised(c, s)
}

/// Cut gullies into a slope at body point `p`. `lattice(x, y, z)` is the
/// solved surface at lattice corner `(x, y, z) * LATTICE_M`.
pub(super) fn carve(seed: u32, p: DVec3, footprint: f64, site: &Site, lattice: impl Fn(i64, i64, i64) -> Corner) -> Carved {
    let mut out = Carved { delta_m: 0.0, rib: 0.0, steer: DVec3::ZERO, strength: 0.0 };
    let resolved = filtered(WAVES_M[0], footprint);
    if site.open <= 0.0 || resolved <= 0.0 { return out; }
    let tangent = |g: DVec3| g - site.up * g.dot(site.up);
    // The hillside's gradient, interpolated from the corners, and the
    // coarsest stripes, one straight kernel per corner.
    let (f, around) = corners(p, LATTICE_M);
    let mut gradient = DVec3::ZERO;
    let mut convergence = 0.0;
    let (mut c, mut s) = (0.0, 0.0);
    for (at, offset, weight) in around {
        if weight <= 0.004 { continue; }
        let corner = lattice(at[0], at[1], at[2]);
        let g = tangent(corner.gradient);
        gradient += g * weight;
        convergence += corner.convergence * weight;
        let across = site.up.cross(g).normalize_or_zero();
        if across == DVec3::ZERO { continue; }
        let (cos, sin) = kernel(seed.wrapping_add(300_011), at, offset, f, across);
        c += weight * cos;
        s += weight * sin;
    }
    let slope = gradient.length();
    // Runoff needs a grade to concentrate; flats keep their sheet flow.
    let drive = smooth(0.05, 0.38, slope) * site.open;
    if drive <= 0.0 { return out; }
    // Drainage density is lowest in the middle: a turf or woodland mantle on
    // moderately watered ground creeps and heals rills. Bare dry slopes
    // gully under rare storms, and ground under daily rain is dissected into
    // ridge and ravine however thick the forest. Hard rock on a steep face
    // holds sharper chutes.
    let steep_rock = site.hard * smooth(0.3, 0.8, slope);
    let mantle = smooth(0.22, 0.40, site.wet) * (1.0 - smooth(0.56, 0.72, site.wet)) * (1.0 - 0.6 * steep_rock);
    // Gullying is patchy: one hillside is scored from crest to foot and its
    // neighbour, on stronger or better-drained ground, is nearly smooth.
    let patch = smooth(0.30, 0.62, super::noise(seed, p / 150.0, 300_041) * 0.65
        + super::noise(seed, p / 47.0, 300_043) * 0.35);
    // Channels begin where topography gathers runoff (the area-slope
    // threshold of channel initiation, read here from planform convergence):
    // hollows are scored, spurs and noses shed their water and stay whole.
    // A gully wall is also no steeper than the hill can feed.
    let gathered = smooth(-0.004, 0.009, convergence);
    let mut wall = (WALL * (0.20 + 0.80 * (0.6 * gathered + 0.4 * patch)) * (1.0 - 0.5 * mantle) * (0.8 + 0.45 * steep_rock))
        .min(0.12 + 0.95 * slope) * drive;
    out.strength = wall / WALL;
    // Broad rounded ribs, a sharp V in the crease.
    let profile = |cos: f64| 2.0 * (0.5 + 0.5 * cos).powf(0.7) - 1.0;
    let (cos, sin) = normalised(c, s);
    let side_slope = wall * resolved;
    out.delta_m = WAVES_M[0] * side_slope / TAU * (profile(cos) - BIAS);
    let lean = LEAN * slope;
    out.steer = (-site.up.cross(gradient / slope) * (sin * side_slope * STEER)).clamp_length_max(lean);
    let (mut rib, mut rib_weight) = (cos * resolved, resolved);
    for (octave, &wavelength) in WAVES_M.iter().enumerate().skip(1) {
        wall *= GAIN;
        let resolved = filtered(wavelength, footprint);
        if resolved <= 0.0 { break; }
        let running = gradient + out.steer;
        let across = site.up.cross(running).normalize_or_zero();
        if across == DVec3::ZERO { break; }
        let (cos, sin) = stripes(seed.wrapping_add(300_011 + octave as u32 * 7919), p, across, wavelength);
        let side_slope = wall * resolved;
        out.delta_m += wavelength * side_slope / TAU * (profile(cos) - BIAS);
        out.steer = (out.steer - across * (sin * side_slope * STEER)).clamp_length_max(lean);
        if octave == 1 {
            rib += cos * resolved * 0.5;
            rib_weight += resolved * 0.5;
        }
    }
    out.rib = rib / rib_weight.max(1.0);
    out
}

/// Prevailing surface wind at `up`: unit, tangent. Same zonal belts as the
/// solve's rain, abrasion and wave climate, turned a little by weather-scale
/// meanders so neighbouring dune fields do not line up like a ruled page.
pub(super) fn wind(seed: u32, up: DVec3) -> DVec3 {
    let east = DVec3::Y.cross(up).normalize_or_zero();
    let north = up.cross(east);
    // Westerlies in the mid latitudes, easterlies elsewhere; no hard flip.
    let lat = up.y.abs();
    let westerly = smooth(0.38, 0.46, lat) * (1.0 - smooth(0.78, 0.86, lat));
    let zonal = 2.0 * westerly - 1.0;
    let meander = (super::noise(seed, up * 30_558.0 / 1900.0, 300_201) - 0.5) * 1.3;
    (east * zonal + north * (meander + 0.25 * (1.0 - zonal.abs()))).normalize_or_zero()
}

/// Transverse dunes on loose sand: crests across the prevailing wind, a
/// gentle windward back and a steeper slip face, forking where the lattice
/// phases disagree as real dune crests do. Metres, never negative; the
/// caller weights it by where sand lies. `erg` (0..1) adds the draa of a
/// desert sand sea: ridges a hundred metres apart that carry the dunes.
pub(super) fn dunes(seed: u32, p: DVec3, footprint: f64, up: DVec3, erg: f64) -> f64 {
    let downwind = wind(seed, up);
    if downwind == DVec3::ZERO { return 0.0; }
    let mut height = 0.0;
    for (wavelength, crest_m, salt) in [(130.0, 8.0 * erg, 300_227_u32), (38.0, 2.6, 300_211), (11.0, 0.55, 300_223)] {
        if crest_m <= 0.0 { continue; }
        let resolved = filtered(wavelength, footprint);
        if resolved <= 0.0 { continue; }
        let (cos, sin) = stripes(seed.wrapping_add(salt), p, downwind, wavelength);
        // cos(phase - k sin(phase)): the crest leans downwind.
        let skew = 0.55 * sin;
        let wave = cos * skew.cos() + sin * skew.sin();
        // Flat interdune floors between the ridges.
        height += crest_m * resolved * (wave * 0.5 + 0.5).powf(1.6);
    }
    height
}

/// Patchiness of a forest floor, 0..1 each for standing-wet ground and for
/// well-drained rises. Two warped octaves break the single-wavelength bands
/// that drew equal-width loops across a whole forest.
pub(super) fn floor_patches(seed: u32, p: DVec3) -> (f64, f64) {
    let warp = DVec3::new(
        super::noise(seed, p / 37.0, 300_301) - 0.5,
        super::noise(seed, p / 37.0, 300_307) - 0.5,
        super::noise(seed, p / 37.0, 300_311) - 0.5) * 30.0;
    let q = p + warp;
    let broad = super::noise(seed, q / 83.0, 300_313);
    let mid = super::noise(seed, q / 23.0, 300_317);
    let fine = super::noise(seed, p / 6.5, 300_331);
    let wet = smooth(0.60, 0.74, broad * 0.55 + mid * 0.30 + fine * 0.15);
    let rise = smooth(0.50, 0.66, (1.0 - broad) * 0.40 + mid * 0.25 + fine * 0.35);
    (wet, rise * (1.0 - wet))
}

/// Sheep-track steps on a grassed slope: treads and risers along the
/// contour, zero mean against the plain slope. They are a local, quiet
/// texture: a few patches per hillside, resolved only below half a metre.
/// Laid on every moist slope at full strength they ruled contour lines
/// across whole ranges and drowned the gullies.
pub(super) fn terracette(seed: u32, p: DVec3, footprint: f64, top: f64, slope: f64, soil: f64) -> f64 {
    let resolved = 1.0 - smooth(0.30, 0.60, footprint);
    if resolved <= 0.0 { return 0.0; }
    let band = smooth(0.22, 0.32, slope) * (1.0 - smooth(0.42, 0.55, slope)) * soil * resolved;
    if band <= 0.0 { return 0.0; }
    let patch = smooth(0.62, 0.78, super::noise(seed, p / 70.0, 300_103));
    if patch <= 0.0 { return 0.0; }
    const RISE_M: f64 = 1.1;
    let warp = (super::noise(seed, p / 11.0, 300_101) - 0.5) * 2.2;
    let s = ((top + warp) / RISE_M).rem_euclid(1.0);
    (smooth(0.18, 0.82, s) - s) * RISE_M * 0.42 * band * patch
}
