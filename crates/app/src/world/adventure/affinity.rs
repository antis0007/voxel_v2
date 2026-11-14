//! Raw magical pressure in barren/hostile places. Healthy ecology supplies
//! safe density through world::ambience; recovery is not the pressure scale.
use super::{Adventure, death::Cause};
use engine_core::hash::hash01;
use glam::Vec3;

/// Lattice spacing in metres; zones are roughly one cell across.
const CELL_M: f32 = 160.0;
/// Share of lattice points that anchor a blight or an affinity zone, each.
const NODE_SHARE: f32 = 0.11;
/// `2^(charge * SPAN)`: charge -1 gives 0.41x, +1 gives 2.46x.
const SPAN: f32 = 1.3;
pub const BLIGHT_BELOW: f32 = 0.5;
pub const POISON_AT: f32 = 2.0;
/// Seconds the heart glow lingers after leaving a poisoning zone.
const POISON_FADE_S: f32 = 1.5;

fn node(seed: u32, x: i32, y: i32, z: i32) -> f32 {
    let key = seed
        ^ (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f)
        ^ 0xa11f_0001;
    let roll = hash01(key);
    if roll < NODE_SHARE {
        -1.0
    } else if roll > 1.0 - NODE_SHARE {
        1.0
    } else {
        // Ordinary ground drifts a little around 1x.
        (hash01(key ^ 0x5bd1_e995) - 0.5) * 0.3
    }
}

/// Unbuffered mana pressure at `pos` (metres).
pub fn multiplier(seed: u32, pos: Vec3) -> f32 {
    let p = pos / CELL_M;
    let base = p.floor();
    let f = p - base;
    let s = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let (bx, by, bz) = (base.x as i32, base.y as i32, base.z as i32);
    let mut charge = 0.0;
    for corner in 0..8 {
        let (dx, dy, dz) = (corner & 1, (corner >> 1) & 1, (corner >> 2) & 1);
        let w = if dx == 1 { s.x } else { 1.0 - s.x }
            * if dy == 1 { s.y } else { 1.0 - s.y }
            * if dz == 1 { s.z } else { 1.0 - s.z };
        charge += w * node(seed, bx + dx, by + dy, bz + dz);
    }
    // Sharpen so zone cores reach the thresholds instead of blurring to 1x.
    let charge = (charge * 1.6).clamp(-1.0, 1.0);
    (charge * SPAN).exp2()
}

/// Cast cooldown scale; only blight slows the wand, up to 3x.
pub fn cast_slow(m: f32) -> f32 {
    if m >= BLIGHT_BELOW { 1.0 } else { (1.0 + (BLIGHT_BELOW - m) * 22.0).min(3.0) }
}

/// Health lost per second to mana poisoning.
pub fn poison_dps(m: f32) -> f32 {
    if m < POISON_AT { 0.0 } else { 1.0 + (m - POISON_AT) * 4.0 }
}

impl Adventure {
    /// Review captures: holds the wand instrument on screen mid-recharge.
    pub fn review_cooldown(&mut self, seconds: f32) {
        self.cast_cd = seconds;
        self.cast_total = seconds;
        self.cast_delay_total = 0.0;
        self.recharge_total = seconds;
    }

    pub fn mana_density(&self) -> f32 { self.affinity }

    pub fn observe_mana_density(&mut self, density: f32) {
        // A PURGE circle holds the density at normal for the caster inside it.
        let density = if self.circles.purging { 1.0 } else { density };
        self.affinity = self.review_affinity.unwrap_or(density);
    }

    /// Ecology, orbit and habitat shielding are resolved by the shared probe.
    pub(super) fn affinity_step(&mut self, density: f32, dt: f32) {
        self.observe_mana_density(density);
        let dps = poison_dps(self.affinity);
        if dps <= 0.0 || self.creative || self.death.is_some() {
            self.mana_poison = (self.mana_poison - dt / POISON_FADE_S).max(0.0);
            return;
        }
        self.mana_poison = 1.0;
        // Spawn protection only; poisoning has no flinch, the hearts show it.
        if self.invulnerable > 1.0 {
            return;
        }
        let amount = dps * dt * self.difficulty.incoming_damage();
        self.health = (self.health - amount).max(0.0);
        self.last_cause = Cause::ManaPoisoning;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affinity_field_has_both_zones_and_is_bounded() {
        let (mut blight, mut poison, mut plain) = (0, 0, 0);
        for i in 0..4000 {
            let pos = Vec3::new(i as f32 * 37.0, (i % 7) as f32 * 11.0, (i / 50) as f32 * 53.0);
            let m = multiplier(91, pos);
            assert!((0.40..=2.47).contains(&m), "{m}");
            assert_eq!(m, multiplier(91, pos));
            blight += (m < BLIGHT_BELOW) as u32;
            poison += (m >= POISON_AT) as u32;
            plain += ((0.8..1.25).contains(&m)) as u32;
        }
        assert!(blight > 40 && poison > 40, "blight {blight} poison {poison}");
        assert!(plain > 1600, "most ground should sit near 1x: {plain}");
    }

    #[test]
    fn thresholds_shape_wands_and_health() {
        assert_eq!(cast_slow(0.5), 1.0);
        assert!(cast_slow(0.41) > 2.5);
        assert_eq!(poison_dps(1.99), 0.0);
        assert!(poison_dps(2.0) > 0.0);
    }

    #[test]
    fn poisoning_drains_health_and_fades() {
        let mut a = Adventure::default();
        a.invulnerable = 0.0;
        a.health = 50.0;
        let (seed, hot) = (0..100_000)
            .map(|i| (91u32, Vec3::new(i as f32 * 13.0, 0.0, 0.0)))
            .find(|(s, p)| multiplier(*s, *p) >= POISON_AT)
            .expect("a poisoning zone on the line");
        for _ in 0..120 {
            a.affinity_step(multiplier(seed, hot), 1.0 / 120.0);
        }
        assert!(a.health < 49.0, "{}", a.health);
        assert_eq!(a.last_cause, Cause::ManaPoisoning);
        assert_eq!(a.mana_poison, 1.0);
        let (_, calm) = (0..100_000)
            .map(|i| (91u32, Vec3::new(i as f32 * 13.0, 0.0, 0.0)))
            .find(|(s, p)| multiplier(*s, *p) < 1.2)
            .expect("plain ground");
        let hp = a.health;
        a.affinity_step(multiplier(seed, calm), 1.0);
        assert_eq!(a.health, hp);
        assert!(a.mana_poison < 0.5);
    }
}
