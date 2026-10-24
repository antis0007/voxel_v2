//! The second Noita-style spell batch (ids 80-94): what each new bolt carries
//! innately, how it flies, bounces and lands, and the new shot modifiers'
//! impact effects. The rows live in `assets/spells.ron`; the modifier and
//! trigger cards are painted in `wands.rs`.
use super::status::Element;
use super::*;

pub const MAGIC_MISSILE: u32 = 80;
pub const FREEZING_SHARD: u32 = 81;
pub const ACID_GLOB: u32 = 82;
pub const LIGHTNING_ARC: u32 = 83;
pub const LODESTONE: u32 = 84;
pub const BOUNCING_BURST: u32 = 85;
pub const DIG_BOLT: u32 = 86;
pub const SCATTER_SHOT: u32 = 87;

/// Seconds a Lodestone may exist, whatever Long life says.
pub const LODESTONE_MAX_LIFE: f32 = 3.0;
/// Pellets in one Scatter shot.
pub const SCATTER_PELLETS: usize = 5;

/// Modifiers a bolt of this spell always carries, whatever the deck says.
pub(super) fn innate_mods(spell: u32, mods: &mut wands::ShotMods) {
    match spell {
        FREEZING_SHARD => {
            mods.element.get_or_insert(Element::Frost);
        }
        ACID_GLOB => {
            mods.element.get_or_insert(Element::Toxic);
            mods.gravity = mods.gravity.max(0.55);
        }
        LIGHTNING_ARC => {
            mods.chain = true;
            mods.element.get_or_insert(Element::Shock);
        }
        BOUNCING_BURST => {
            mods.bounces = mods.bounces.max(6);
            mods.gravity = mods.gravity.max(0.35);
        }
        _ => {}
    }
}

/// Base flight time in seconds for the new bolts.
pub(super) fn base_life(spell: u32) -> Option<f32> {
    Some(match spell {
        MAGIC_MISSILE => 2.6,
        FREEZING_SHARD => 2.0,
        ACID_GLOB => 2.4,
        LIGHTNING_ARC => 0.3,
        LODESTONE => 2.5,
        BOUNCING_BURST => 4.5,
        DIG_BOLT => 1.0,
        SCATTER_SHOT => 0.45,
        _ => return None,
    })
}

/// Hard ceiling on flight time after Long life and friends.
pub(super) fn life_cap(spell: u32) -> f32 {
    if spell == LODESTONE {
        LODESTONE_MAX_LIFE
    } else {
        f32::INFINITY
    }
}

/// Directions the cast leaves in: a five-pellet cone for Scatter shot, the
/// aim itself for everything else.
pub(super) fn scatter_dirs(spell: u32, dir: Vec3) -> Vec<Vec3> {
    if spell != SCATTER_SHOT {
        return vec![dir];
    }
    let up = if dir.y.abs() > 0.98 { Vec3::X } else { Vec3::Y };
    let side = dir.cross(up).normalize_or_zero();
    let up = side.cross(dir).normalize_or_zero();
    // A fixed pattern, so the same cast always spreads the same way.
    const PATTERN: [(f32, f32); SCATTER_PELLETS] =
        [(0.0, 0.0), (0.09, 0.03), (-0.09, 0.03), (0.04, -0.08), (-0.05, 0.08)];
    PATTERN
        .iter()
        .map(|(x, y)| (dir + side * *x + up * *y).normalize_or_zero())
        .collect()
}

/// Nearest living foe within `range` metres that the point can see.
fn nearest_foe(enemies: &[Enemy], world: &World, at: Vec3, range: f32) -> Option<Vec3> {
    enemies
        .iter()
        .filter(|e| e.health > 0.0)
        .filter_map(|e| {
            let d = (e.center() - at).length();
            (d < range && line(world, at, e.center())).then_some((e.center(), d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(p, _)| p)
}

/// Turns `velocity` toward `target` by at most `rate * dt` of the gap,
/// keeping its speed.
pub(super) fn steer(velocity: Vec3, from: Vec3, target: Vec3, rate: f32, dt: f32) -> Vec3 {
    let speed = velocity.length();
    let desired = (target - from).normalize_or_zero();
    let current = velocity.normalize_or_zero();
    if desired == Vec3::ZERO || current == Vec3::ZERO {
        return velocity;
    }
    (current + (desired - current) * (dt * rate).min(1.0)).normalize_or_zero() * speed
}

impl Adventure {
    /// Per-tick flight behaviour of the new bolts and trigger cards.
    pub(super) fn arcana_flight(&mut self, world: &mut World, shot: &mut Projectile, dt: f32) {
        match shot.spell {
            MAGIC_MISSILE => {
                // Weak homing: a third of the Homing arc's turn rate.
                if let Some(target) = nearest_foe(&self.enemies, world, shot.pos, 14.0) {
                    shot.velocity = steer(shot.velocity, shot.pos, target, 2.4, dt);
                }
            }
            LODESTONE => {
                // A slowing orb that drags foes toward itself while it lives.
                shot.velocity *= (-dt * 1.4).exp();
                for e in &mut self.enemies {
                    let pull = shot.pos - e.center();
                    let d = pull.length();
                    if e.health > 0.0 && d < 6.0 && d > 0.3 {
                        e.knock += pull / d * (dt * 16.0);
                        e.hurt(dt * 5.0 * shot.mods.damage_mul * e.affixes.damage_taken());
                        e.flash = e.flash.max(0.05);
                    }
                }
            }
            _ => {}
        }
        if shot.mods.proximity
            && shot.payload.is_some()
            && nearest_foe(&self.enemies, world, shot.pos, 3.0).is_some()
        {
            self.release_payload(world, shot);
        }
    }

    /// Called after a bolt reflects off a wall.
    pub(super) fn arcana_bounce(&mut self, world: &mut World, shot: &mut Projectile) {
        if shot.spell == BOUNCING_BURST {
            // Each ricochet adds speed and pops at foes close to the wall.
            let base = spell_base_speed(BOUNCING_BURST) * 1.6;
            let faster = shot.velocity * 1.1;
            shot.velocity = faster.clamp_length_max(base);
            for e in &mut self.enemies {
                if e.health > 0.0 && (e.center() - shot.pos).length() < 1.4 {
                    e.hurt(6.0 * shot.mods.damage_mul * e.affixes.damage_taken());
                    e.flash = 0.12;
                }
            }
        }
        if shot.mods.bounce_release && shot.payload.is_some() {
            self.release_payload(world, shot);
        }
    }

    /// Damage multiplier for a direct hit from the new modifiers.
    pub(super) fn arcana_hit_mul(mods: &wands::ShotMods, e: &Enemy) -> f32 {
        if mods.crit_wet && (e.status.wet > 0.0 || e.slow > 0.0) {
            2.0
        } else {
            1.0
        }
    }

    /// Impact effects of the new bolts and modifiers.
    pub(super) fn arcana_impact(&mut self, world: &mut World, shot: &Projectile, eye: Vec3) {
        if shot.hostile {
            return;
        }
        let scale = world.voxel_scale.meters();
        let at = shot.pos;
        match shot.spell {
            FREEZING_SHARD => self.cryo_bloom(world, at, 0.9),
            ACID_GLOB => {
                // Eats a small pocket of soft terrain and poisons the splash.
                self.carve_sphere(world, at, 0.9, 6);
                for e in &mut self.enemies {
                    if e.health > 0.0 && (e.center() - at).length() < 2.0 {
                        e.status.apply(Element::Toxic, e.affixes);
                        e.flash = 0.12;
                    }
                }
                self.spawn_voxel_burst(at, spell_color(ACID_GLOB), 14, 2.2, 0.04);
            }
            LODESTONE => self.implode(at, 4.0, 4.0),
            _ => {}
        }
        if shot.mods.explosive {
            self.blast_visual(at, 1.8, -shot.velocity.normalize_or_zero(), [1.0, 0.5, 0.2]);
            world.sandbox.apply_radial_impulse(at, 3.0, 4.5, scale);
            if self.terrain_blasts.len() < 4 {
                self.terrain_blasts.push_back((at, 1.6));
            }
            let distance = (eye - at).length();
            if distance < 2.4 {
                self.damage(14.0 * (1.0 - distance / 2.4), at - eye);
            }
            for e in &mut self.enemies {
                let d = (e.center() - at).length();
                if e.health > 0.0 && d < 2.6 && line(world, at, e.center()) {
                    e.hurt(20.0 * (1.0 - 0.5 * d / 2.6) * shot.mods.damage_mul * e.affixes.damage_taken());
                    e.flash = 0.18;
                    e.stagger = e.stagger.max(0.35);
                }
            }
            sound(&mut self.sounds, Cue::Burst, Some(at), 0.8);
        }
        if shot.mods.pull {
            self.implode(at, 4.5, 6.0);
        }
    }

    /// Yanks foes within `radius` toward `at`.
    fn implode(&mut self, at: Vec3, radius: f32, strength: f32) {
        for e in &mut self.enemies {
            let pull = at - e.center();
            if e.health > 0.0 && pull.length() < radius {
                e.knock = pull.normalize_or_zero() * strength;
                e.stagger = e.stagger.max(0.3);
            }
        }
        self.spawn_voxel_burst(at, [0.6, 0.45, 0.9], 12, 1.6, 0.035);
    }
}

#[cfg(test)]
mod tests;
