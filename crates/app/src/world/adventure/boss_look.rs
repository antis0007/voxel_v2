//! Deterministic per-site boss variants and the shared pose curves every boss
//! model reads. Variants are derived from the site hash on every load, so they
//! never touch the save format; the workshop/test site (`IVec3::ZERO`) always
//! gets the base variant so fixtures keep their exact tuning.
use super::*;
use action::{Phase, Run};
use engine_core::hash::{hash01, hash_u32};

pub(super) const VARIANTS: usize = 3;

fn key(e: &Enemy, salt: u32) -> u32 {
    let s = e.site;
    hash_u32(
        (s.x as u32).wrapping_mul(0x9e37_79b9)
            ^ hash_u32((s.y as u32) ^ hash_u32((s.z as u32) ^ e.kind.wrapping_mul(0x85eb_ca6b) ^ salt)),
    )
}

/// Which of the boss's three authored variants this site hosts.
pub(super) fn index(e: &Enemy) -> usize {
    if e.site == IVec3::ZERO {
        return 0;
    }
    key(e, 0) as usize % VARIANTS
}

/// Visual body scale within ±12%; collision keeps the shared boss radius.
pub(super) fn scale(e: &Enemy) -> f32 {
    if e.site == IVec3::ZERO {
        return 1.0;
    }
    0.88 + hash01(key(e, 0x5ca1e)) * 0.24
}

/// Committed-action pose weights. `wind` eases in through Prepare (anticipation),
/// `strike` runs 0..1 through Active, `hold` is the follow-through that decays
/// through Recover, and `weight` is the action's total influence on the body.
#[derive(Clone, Copy, Default)]
pub(super) struct Pose {
    pub wind: f32,
    pub strike: f32,
    pub hold: f32,
    pub weight: f32,
    pub active: bool,
    pub preparing: bool,
}

pub(super) fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn pose<A>(run: Option<&Run<A>>) -> Pose {
    let Some(r) = run else {
        return Pose::default();
    };
    match r.phase() {
        Phase::Prepare => {
            let w = ease(r.wind());
            Pose { wind: w, weight: w, preparing: true, ..Pose::default() }
        }
        Phase::Active => {
            let t = ((r.elapsed - r.prepare) / r.active.max(0.01)).clamp(0.0, 1.0);
            // Fast out of the coil: most of the travel happens early in the strike.
            let s = 1.0 - (1.0 - t) * (1.0 - t);
            Pose { wind: 1.0 - s, strike: s, hold: 1.0, weight: 1.0, active: true, preparing: false }
        }
        Phase::Recover => {
            let t = ((r.elapsed - r.prepare - r.active) / r.recover.max(0.01)).clamp(0.0, 1.0);
            let h = 1.0 - ease(t);
            Pose { strike: 1.0, hold: h, weight: h, ..Pose::default() }
        }
        Phase::Done => Pose::default(),
    }
}

/// Hit flash: every piece briefly blends toward warm white.
pub(super) fn tint(e: &Enemy, c: [f32; 3]) -> [f32; 3] {
    if e.flash > 0.0 {
        let k = (e.flash / 0.12).clamp(0.0, 1.0) * 0.85;
        [c[0] + (1.0 - c[0]) * k, c[1] + (0.95 - c[1]) * k, c[2] + (0.84 - c[2]) * k]
    } else {
        c
    }
}

pub(super) fn scaled(c: [f32; 3], k: f32) -> [f32; 3] {
    c.map(|v| (v * k).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_are_deterministic_and_cover_every_index() {
        let mut seen = [false; VARIANTS];
        for x in -20..20 {
            for kind in [6, 7, 14, 15] {
                let e = Enemy::new(IVec3::new(x, 0, x * 3 + 1), 0, kind, Vec3::ZERO, 0);
                let again = Enemy::new(IVec3::new(x, 0, x * 3 + 1), 0, kind, Vec3::ONE, 0);
                assert_eq!(index(&e), index(&again));
                assert_eq!(scale(&e), scale(&again));
                assert!((0.87..=1.13).contains(&scale(&e)));
                seen[index(&e)] = true;
            }
        }
        assert!(seen.iter().all(|s| *s));
        let base = Enemy::new(IVec3::ZERO, 0, 6, Vec3::ZERO, 0);
        assert_eq!((index(&base), scale(&base)), (0, 1.0));
    }

    #[test]
    fn every_boss_variant_title_is_distinct() {
        let mut names = Vec::new();
        for kind in [6, 7, 14, 15] {
            for v in 0..VARIANTS {
                names.push(bosses::variant_title(kind, v));
            }
        }
        for v in 0..VARIANTS {
            names.push(bosses::variant_title(8, v));
        }
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "{names:?}");
    }
}
