//! Host-owned player hits use the same weapon cooldown, armour and damage rules.
use super::*;

/// Slab intersection in metres; returns the first contact along a finite segment.
fn contact(start: Vec3, end: Vec3, lo: Vec3, hi: Vec3) -> Option<f32> {
    let direction = end - start;
    let (mut near, mut far) = (0.0_f32, 1.0_f32);
    for axis in 0..3 {
        if direction[axis].abs() < 1e-7 {
            if start[axis] < lo[axis] || start[axis] > hi[axis] { return None; }
        } else {
            let a = (lo[axis] - start[axis]) / direction[axis];
            let b = (hi[axis] - start[axis]) / direction[axis];
            near = near.max(a.min(b)); far = far.min(a.max(b));
            if near > far { return None; }
        }
    }
    Some(near)
}

fn target(world: &World, start: Vec3, end: Vec3, radius: f32) -> Option<(u32, Vec3, f32)> {
    if !world.rules.pvp { return None; }
    let context = world.multiplayer.as_ref()?;
    context.opponents().filter_map(|p| {
        let t = contact(start, end, p.min - Vec3::splat(radius), p.max + Vec3::splat(radius))?;
        let point = start.lerp(end, t);
        line(world, start, point).then_some((p.id, point, t))
    }).min_by(|a,b| a.2.total_cmp(&b.2))
}

fn queue(world: &mut World, id: u32, amount: f32, direction: Vec3) {
    if amount.is_finite() && amount > 0.0 && let Some(context) = world.multiplayer.as_mut()
        && context.damage.len() < 256 {
        context.damage.push(crate::multiplayer::gameplay::Hit { id, amount, direction });
    }
}

impl Adventure {
    pub(super) fn player_melee(&mut self, world: &mut World, eye: Vec3, forward: Vec3, strike: bool, rank: u32) -> bool {
        if self.is_dead() || (self.melee_buffer <= 0.0 && !strike) { return false; }
        let reach = MELEE_REACH + if self.weapon == 0 && !self.blade.armed { 0.0 } else { ARMED_REACH_BONUS };
        let Some((id, point, t)) = target(world, eye, eye + forward * reach, 0.12) else { return false; };
        // An enemy between the attacker and player receives the swing first.
        if self.enemies.iter().any(|enemy| enemy.health > 0.0 && {
            let offset = enemy.center() - eye;
            let along = offset.dot(forward);
            along > 0.0 && along < t * reach && (offset-forward*along).length() < 0.55
                && line(world, eye, enemy.center())
        }) { return false; }
        if self.melee_buffer > 0.0 && self.melee_cd <= 0.0 && !strike {
            self.melee_buffer = 0.0;
            self.combo = self.combo % 3 + 1;
            self.combo_time = 1.2;
            self.melee_cd = self.swing_duration();
            self.strike_delay = self.swing_impact_time();
            sound(&mut self.sounds, Cue::Swing, Some(eye), 0.65);
        }
        if strike {
            let finisher = self.combo == 3;
            let base = self.melee_base(world);
            let damage = (base * if finisher {1.7} else {1.0} + rank as f32 * 3.0
                + self.blade.hit_damage)
                * self.blade.damage
                * (1.0+self.resonance.stacks(0)*0.08);
            queue(world, id, damage, -forward);
            self.hit = 0.18;
            sound(&mut self.sounds, Cue::Impact, Some(point), 0.7);
        }
        true
    }

    pub(super) fn player_projectile(&mut self, world: &mut World, shot: &Projectile, end: Vec3, rank: u32) -> bool {
        if shot.farm.is_some() {return false;}
        if spell_defs::spell_base_damage(shot.spell) <= 0.0 { return false; }
        let Some((id, point, _)) = target(world, shot.pos, end, 0.10) else { return false; };
        queue(world, id, (spell_hit_damage(shot.spell)+rank as f32*4.0)*shot.mods.damage_mul, -shot.velocity);
        self.hit = 0.16;
        sound(&mut self.sounds, Cue::Impact, Some(point), 0.7);
        true
    }

    pub(super) fn player_beam_path(&mut self, world: &mut World, path: &[Vec3], damage: f32) {
        if !world.rules.pvp { return; }
        let Some(context) = world.multiplayer.as_ref() else { return; };
        let hits: Vec<_> = context.opponents().filter_map(|p| {
            path.windows(2).find_map(|segment| {
                let t = contact(segment[0], segment[1], p.min - Vec3::splat(0.12), p.max + Vec3::splat(0.12))?;
                let point = segment[0].lerp(segment[1], t);
                line(world, segment[0], point).then_some((p.id, segment[0] - segment[1]))
            })
        }).collect();
        for (id, direction) in hits {
            queue(world, id, damage, direction.normalize_or_zero());
            self.hit = 0.16;
        }
    }
    pub(super) fn player_blast(&mut self, world: &mut World, center: Vec3, radius: f32, damage: f32) {
        if !world.rules.pvp { return; }
        let Some(context)=world.multiplayer.as_ref() else { return; };
        let hits:Vec<_>=context.opponents().filter_map(|player| {
            let point=center.clamp(player.min,player.max);
            let distance=center.distance(point);
            (distance<radius&&line(world,center,point)).then_some((player.id,damage*(1.0-distance/radius),center-point))
        }).collect();
        for (id,amount,direction) in hits {queue(world,id,amount,direction);}
    }

    /// The connected water is one discharge, not one attack per wet voxel.
    pub(super) fn player_conductive_field(&mut self, world: &mut World, points: &[Vec3], damage: f32) {
        if !world.rules.pvp { return; }
        let Some(context) = world.multiplayer.as_ref() else { return; };
        let hits: Vec<_> = context.opponents().filter_map(|p| {
            points.iter().find_map(|from| {
                let to = from.clamp(p.min, p.max);
                (from.distance(to) < wands::card_rules::CONDUCTIVE_REACH && line(world, *from, to))
                    .then_some((p.id, *from, to))
            })
        }).collect();
        for (id, from, to) in hits {
            queue(world, id, damage, (from - to).normalize_or_zero());
            if self.arcs.len() < 32 { self.arcs.push(Arc { a: from, b: to, life: 0.22 }); }
            self.hit = 0.2;
        }
    }
}
