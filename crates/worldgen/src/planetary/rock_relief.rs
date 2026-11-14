//! Generator 304 rock relief: the solved beds shape the ground.
//!
//! Up to generator 300 the bedrock under a hillside decided only the colour
//! of its steep cells: hardness was a plate-boundary number, and a slope
//! threshold painted stone in contour-parallel stripes on smooth swells.
//! Here the resistance of the solved beds bends the hillside itself. Hard
//! beds stand as risers and caprock lips, soft beds retreat to benches, and
//! the cover rule then finds real bed rock on the faces.
//!
//! The remap works on height. `S(z)` is the integral of `1 / R(z)^n` over the
//! resistance profile `R`, normalised so that each [`CYCLE_M`] band of height
//! keeps its two edges fixed. The smooth hill height `u` lands at
//! `T(u) = S^-1(u)`: the ground climbs fast through hard beds and slowly
//! through soft ones. Each band starts and ends where it was, so the result
//! is continuous, bounded by the band, and the identity on uniform rock.
//!
//! Profiles are read from the history column at the corners of a coarse body
//! lattice and blended with partition-of-unity weights. A weighted sum of
//! profiles is still a profile, so mesh-cell steps in the history columns
//! cannot reach the ground as seams. The bed frame is tilted by a seeded
//! structural dip and notched down along the gullies of generator 300, so
//! risers cross the contours and break into chutes.
//!
//! All lengths are body metres and the stage fades out above a 30 m
//! footprint: the horizon and the globe pay nothing. No simulation and no
//! wall clock; every value is a function of seed, world and position.
use super::smooth;
use glam::DVec3;
use std::cell::RefCell;

/// Spacing of the lattice whose corners carry resistance profiles.
pub(super) const LATTICE_M: f64 = 48.0;
/// Height band whose edges the remap keeps fixed.
const CYCLE_M: f64 = 20.0;
/// Profile window below and above the history column's top, 1 m per entry.
const BELOW_M: f64 = 80.0;
const ENTRIES: usize = 97;
/// Resistance quantum of the stored profile.
const R_STEP: f64 = 0.0125;
/// Largest and smallest slope gain a band may give one metre of height.
const GAIN_MAX: f64 = 8.0;
const GAIN_MIN: f64 = 0.15;
/// Depth gullies lower the bed frame in their creases, metres.
const NOTCH_M: f64 = 4.0;

/// Resistance per metre of height at one lattice corner, bottom first.
#[derive(Clone, Copy)]
pub(super) struct Profile {
    base: f32,
    /// Height of the top of the crystalline basement below the beds.
    basement: f32,
    r: [u8; ENTRIES],
    valid: bool,
}

impl Profile {
    pub(super) const NONE: Self = Self { base: 0.0, basement: 0.0, r: [0; ENTRIES], valid: false };

    /// Profile of a history column whose top is `top`, sampling the bed
    /// resistance at two points per metre so a thin bed is not lost.
    /// `rock` returns the resistance at a height and whether that height is
    /// basement rather than a bed.
    pub(super) fn sample(top: f64, rock: impl Fn(f64) -> (f64, bool)) -> Self {
        let base = top - BELOW_M;
        let mut half = [0.0f64; ENTRIES * 2 + 1];
        let mut basement = base - 1.0;
        for (i, slot) in half.iter_mut().enumerate().rev() {
            let z = base + i as f64 * 0.5 - 0.5;
            let (r, crystalline) = rock(z);
            *slot = r;
            if crystalline && basement < base { basement = z; }
        }
        // A [1 2 1] filter over half metres softens bed contacts to about a
        // metre, so a riser starts with a lip and not with a single step.
        let r = std::array::from_fn(|i| {
            let j = i * 2 + 1;
            let mean = (half[j - 1] + 2.0 * half[j] + half[j + 1]) * 0.25;
            (mean / R_STEP).round().clamp(1.0, 255.0) as u8
        });
        Self { base: base as f32, basement: basement as f32, r, valid: true }
    }

    fn at(&self, z: f64) -> f64 {
        let x = (z - f64::from(self.base)).clamp(0.0, (ENTRIES - 1) as f64);
        let i = (x as usize).min(ENTRIES - 2);
        let t = x - i as f64;
        (f64::from(self.r[i]) * (1.0 - t) + f64::from(self.r[i + 1]) * t) * R_STEP
    }
}

/// World identity and lattice coordinate of one corner.
pub(super) type CornerKey = (u64, i64, i64, i64);
const CORNER_SLOTS: usize = 1024;
const SHARED_LIMIT: usize = 1 << 15;

thread_local! {
    static CORNERS: RefCell<Vec<(CornerKey, Profile)>> = const { RefCell::new(Vec::new()) };
}
/// Profiles every terrain worker has read; neighbouring chunks share them.
static SHARED: std::sync::Mutex<Option<std::collections::HashMap<CornerKey, Profile>>> =
    std::sync::Mutex::new(None);

/// The profile at one lattice corner, computed once while cached. The tables
/// are released while it is computed, since that reads the history column.
pub(super) fn cached_profile(key: CornerKey, compute: impl FnOnce() -> Profile) -> Profile {
    let slot = (key.1.wrapping_mul(0x9E37_79B9).wrapping_add(key.2.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(key.3.wrapping_mul(0xC2B2_AE35)) ^ key.0 as i64).rem_euclid(CORNER_SLOTS as i64) as usize;
    if let Some(hit) = CORNERS.with(|table| {
        table.borrow().get(slot).filter(|(k, _)| *k == key).map(|(_, profile)| *profile)
    }) {
        return hit;
    }
    let shared = SHARED.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref().and_then(|map| map.get(&key).copied());
    let profile = shared.unwrap_or_else(|| {
        let profile = compute();
        let mut guard = SHARED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        if map.len() >= SHARED_LIMIT { map.clear(); }
        map.insert(key, profile);
        profile
    });
    CORNERS.with(|table| {
        let mut table = table.borrow_mut();
        if table.is_empty() { table.resize(CORNER_SLOTS, ((0, i64::MIN, 0, 0), Profile::NONE)); }
        table[slot] = (key, profile);
    });
    profile
}

/// What the generator 300 carve already decided at this column.
pub(super) struct Site {
    pub up: DVec3,
    /// Uphill tangent gradient of the carved surface.
    pub gradient: DVec3,
    /// Gully crease strength 0..1 (0 on ribs and plain ground).
    pub crease: f64,
    pub moisture: f64,
    /// Channel, shore and deposit fade from the carve stage.
    pub open: f64,
}

pub(super) struct Relief {
    pub top: f64,
    pub gradient: DVec3,
    /// 0..1 where the ground climbs through a hard bed faster than the hill.
    pub riser: f64,
}

/// Height offset of the bed frame: gentle folds that tighten with plate
/// collision and a slow warp, so no contact stays level. The solved beds
/// already follow the history surface; this only roughens them, and stays
/// well inside the profile window.
fn dip(seed: u32, p: DVec3, collision: f64) -> f64 {
    let folded = (super::noise(seed, p / 1280.0, 304_023) - 0.5) * (4.0 + 12.0 * collision);
    let warp = (super::noise(seed, p / 310.0, 304_037) - 0.5) * 10.0;
    folded + warp
}

/// Tangent gradient of the bed-frame dip, by forward differences.
fn dip_gradient(seed: u32, p: DVec3, up: DVec3, collision: f64, at: f64) -> DVec3 {
    const STEP: f64 = 8.0;
    let side = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let t1 = up.cross(side).normalize_or_zero();
    let t2 = up.cross(t1);
    let along = |t: DVec3| (dip(seed, p + t * STEP, collision) - at) / STEP;
    t1 * along(t1) + t2 * along(t2)
}

/// Spacing of the lattice whose corners carry a basement strike.
const STRIKE_LATTICE_M: f64 = 1536.0;
/// Thickness of one basement band; runs of one class make thicker units.
const BAND_M: f64 = 4.0;

fn hash(seed: u32, a: i64, b: i64, c: i64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (c as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ (u64::from(seed) << 32 | u64::from(seed)) ^ 0x3040_3040;
    h ^= h >> 31;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 32;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^ (h >> 32)
}

/// Foliation of the crystalline basement at one column. Bands are planes
/// `w = z cos(dip) + s sin(dip)`; `s` is a partition-of-unity blend of
/// planes through strike-lattice corners, so the strike swings smoothly
/// across kilometres and folds where neighbouring corners disagree.
struct Fabric { cos: f64, sin: f64, s: f64, contrast: f64 }

impl Fabric {
    fn new(seed: u32, p: DVec3, collision: f64) -> Self {
        // Gently dipping in old cratons, steep in collision belts.
        let dip = 8.0 + 62.0 * smooth(0.1, 0.7, collision)
            + (super::noise(seed, p / 2600.0, 304_051) - 0.5) * 20.0;
        let dip = dip.clamp(4.0, 80.0).to_radians();
        let mut s = 0.0;
        for (at, weight) in lattice(p, STRIKE_LATTICE_M) {
            if weight <= 0.0 { continue; }
            let h = hash(seed, at[0], at[1], at[2]);
            let unit = |bits: u64| (bits & 0xFFFF) as f64 / 32_768.0 - 1.0;
            let normal = DVec3::new(unit(h), unit(h >> 16), unit(h >> 32)).normalize_or(DVec3::X);
            let corner = DVec3::new(at[0] as f64, at[1] as f64, at[2] as f64) * STRIKE_LATTICE_M;
            s += weight * (p - corner).dot(normal);
        }
        Self { cos: dip.cos(), sin: dip.sin(), s, contrast: 0.7 + 0.3 * smooth(0.1, 0.7, collision) }
    }

    /// Resistance factor of the basement band through height `z`: soft
    /// schist, ordinary gneiss or a hard quartzite or amphibolite band.
    fn factor(&self, seed: u32, z: f64) -> f64 {
        let x = (z * self.cos + self.s * self.sin) / BAND_M;
        let k = x.floor();
        let class = |k: f64| {
            let h = (hash(seed, k as i64, 304_061, 0) & 0xFFFF) as f64 / 65_536.0;
            if h < 0.40 { 0.55 } else if h < 0.72 { 1.0 } else { 1.6 }
        };
        let (lower, upper) = (class(k), class(k + 1.0));
        let band = lower + (upper - lower) * smooth(0.8, 1.0, x - k);
        1.0 + self.contrast * (band - 1.0)
    }
}

/// The eight lattice corners around `p` for cells of `cell_m`: coordinate
/// and smooth trilinear weight. Weights sum to one.
fn lattice(p: DVec3, cell_m: f64) -> [([i64; 3], f64); 8] {
    let q = p / cell_m;
    let base = q.floor();
    let f = q - base;
    let fade = |t: f64| t * t * (3.0 - 2.0 * t);
    let far = DVec3::new(fade(f.x), fade(f.y), fade(f.z));
    let near = DVec3::ONE - far;
    let (bx, by, bz) = (base.x as i64, base.y as i64, base.z as i64);
    std::array::from_fn(|i| {
        let (ox, oy, oz) = ((i & 1) as i64, ((i >> 1) & 1) as i64, ((i >> 2) & 1) as i64);
        let weight = (if ox == 0 { near.x } else { far.x })
            * (if oy == 0 { near.y } else { far.y })
            * (if oz == 0 { near.z } else { far.z });
        ([bx + ox, by + oy, bz + oz], weight)
    })
}

/// Remap the carved column at body point `p` with top `top` through the
/// blended bed profile. `profile` returns the corner profile for a lattice
/// coordinate (the caller caches it). Returns `None` where nothing changes.
pub(super) fn apply(seed: u32, p: DVec3, top: f64, footprint: f64, collision: f64, site: &Site,
    profile: impl Fn(i64, i64, i64) -> Profile) -> Option<Relief>
{
    let fade = 1.0 - smooth(8.0, 30.0, footprint);
    let open = smooth(0.0, 0.6, site.open);
    if fade * open <= 0.0 { return None; }
    let mut around = [(Profile::NONE, 0.0); 8];
    let mut valid = 0.0;
    let mut basement = f64::MIN;
    for (slot, (at, weight)) in around.iter_mut().zip(lattice(p, LATTICE_M)) {
        if weight <= 0.0 { continue; }
        let corner = profile(at[0], at[1], at[2]);
        if corner.valid {
            valid += weight;
            basement = basement.max(f64::from(corner.basement));
            *slot = (corner, weight);
        }
    }
    let strength = fade * open * smooth(0.5, 0.9, valid);
    if strength <= 0.0 { return None; }
    let offset = dip(seed, p, collision) - NOTCH_M * site.crease;
    let u = top - offset;
    let a = (u / CYCLE_M).floor() * CYCLE_M;
    const SEGMENTS: usize = CYCLE_M as usize;
    // The basement is foliated: below each corner's basement top the bands
    // set its contrast. A band below every corner's basement top reads no
    // fabric; the highest one decides whether to build it.
    let fabric = (basement + 1.0 > a).then(|| Fabric::new(seed, p, collision));
    let mut corners = [(Profile::NONE, 0.0); 8];
    let mut count = 0;
    for &(corner, weight) in around.iter().filter(|(_, w)| *w > 0.0) {
        corners[count] = (corner, weight / valid);
        count += 1;
    }
    let corners = &corners[..count];
    let resistance = |z: f64| {
        let Some(fabric) = &fabric else {
            return corners.iter().map(|(c, w)| c.at(z) * w).sum::<f64>();
        };
        let mut band = None;
        corners.iter().map(|(c, w)| {
            let top = f64::from(c.basement);
            let below = smooth(top + 1.0, top - 1.0, z);
            if below <= 0.0 { return c.at(z) * w; }
            let factor = *band.get_or_insert_with(|| fabric.factor(seed, z));
            c.at(z) * w * (1.0 + below * (factor - 1.0))
        }).sum::<f64>()
    };
    // Wet ground mantles its ledges in soil; dry ground keeps them sharp.
    let n = 3.0 - 1.6 * smooth(0.35, 0.8, site.moisture);
    let mut g = [0.0f64; SEGMENTS];
    let mut lower = resistance(a);
    for (i, slot) in g.iter_mut().enumerate() {
        let upper = resistance(a + i as f64 + 1.0);
        *slot = (0.5 * (lower + upper)).max(0.05).powf(-n);
        lower = upper;
    }
    let raw = g.iter().sum::<f64>() / CYCLE_M;
    for slot in &mut g { *slot = slot.clamp(raw / GAIN_MAX, raw / GAIN_MIN); }
    let mean = g.iter().sum::<f64>() / CYCLE_M;
    // Walk the run function to the segment holding u.
    let target = u - a;
    let mut run = 0.0;
    let (mut remapped, mut gain) = (target, 1.0);
    for (i, &slot) in g.iter().enumerate() {
        let step = slot / mean;
        if run + step >= target || i == SEGMENTS - 1 {
            let t = ((target - run) / step).clamp(0.0, 1.0);
            remapped = i as f64 + t;
            gain = mean / slot;
            break;
        }
        run += step;
    }
    let delta = (remapped - target) * strength;
    if delta.abs() < 1e-4 && (gain - 1.0).abs() < 1e-4 { return None; }
    let gain = 1.0 + (gain - 1.0) * strength;
    // The tilted bed frame adds its own slope where the gain departs from one.
    let lean = 1.0 - gain;
    let gradient = if lean.abs() < 0.02 { site.gradient * gain } else {
        site.gradient * gain + dip_gradient(seed, p, site.up, collision, offset + NOTCH_M * site.crease) * lean
    };
    Some(Relief { top: top + delta, gradient, riser: smooth(2.0, 4.0, gain) })
}
