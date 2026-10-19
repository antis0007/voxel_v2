//! Noita-style ordered spell deck evaluator: combinable modifiers, multicasts,
//! impact triggers, and chassis stats visible on hover.
use super::*;

pub const EMPTY: u32 = 8;
pub const MAX_SLOTS: usize = 8;

/// Accumulated projectile modifiers evaluated left-to-right across a cast block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotMods {
    pub damage_mul: f32,
    pub speed_mul: f32,
    pub bounces: u8,
    pub homing: bool,
    pub orbit: bool,
    pub bore: bool,
    pub trail: bool,
}

impl Default for ShotMods {
    fn default() -> Self {
        Self {
            damage_mul: 1.0,
            speed_mul: 1.0,
            bounces: 0,
            homing: false,
            orbit: false,
            bore: false,
            trail: false,
        }
    }
}

/// Noita-style wand chassis statistics displayed on hover and used during evaluation.
#[derive(Clone, Copy, Debug)]
pub struct WandStats {
    pub name: &'static str,
    pub chassis: &'static str,
    pub shuffle: bool,
    pub spells_per_cast: u32,
    pub cast_delay: f32,
    pub recharge_time: f32,
    pub mana_max: f32,
    pub mana_charge_speed: f32,
    pub capacity: usize,
    pub spread_deg: f32,
    pub always_cast: Option<u32>,
}

/// One resolved spell invocation inside a cast block.
#[derive(Clone, Copy, Debug)]
pub struct CastAction {
    pub slot: usize,
    pub spell: u32,
    pub payload: Option<u32>,
    pub payload_slot: Option<usize>,
    pub mods: ShotMods,
}

/// Complete evaluation of the cast block starting at a given wand cursor.
#[derive(Clone, Debug)]
pub struct CastBlockEval {
    pub actions: Vec<CastAction>,
    pub modifier_slots: Vec<usize>,
    pub last_slot: usize,
    pub mana_cost: f32,
    pub cast_delay: f32,
    pub recharge_time: f32,
    pub spread_deg: f32,
    pub mods: ShotMods,
}

#[derive(Clone, Copy, Debug)]
pub struct Loadout {
    pub slots: [u32; 4],
    pub aux_slots: [u32; 4],
    pub cursor: usize,
    pub mode: u32,
}

impl Default for Loadout {
    fn default() -> Self {
        Self {
            slots: [0, 2, EMPTY, EMPTY],
            aux_slots: [EMPTY; 4],
            cursor: 0,
            mode: 0,
        }
    }
}

impl Loadout {
    pub fn slot(&self, idx: usize) -> u32 {
        if idx < 4 {
            self.slots[idx]
        } else if idx < MAX_SLOTS {
            self.aux_slots[idx - 4]
        } else {
            EMPTY
        }
    }

    pub fn set_slot(&mut self, idx: usize, spell: u32) {
        if idx < 4 {
            self.slots[idx] = spell;
        } else if idx < MAX_SLOTS {
            self.aux_slots[idx - 4] = spell;
        }
    }
}

pub fn capacity(tier: u32) -> usize {
    match tier {
        0 => 1,
        1 => 4,
        2 => 6,
        _ => 8,
    }
}

pub fn projectile(spell: u32) -> bool {
    matches!(spell, 0 | 2 | 3 | 5 | 6 | 10 | 11 | 12 | 13 | 14 | 15)
}

pub fn is_modifier(spell: u32) -> bool {
    (16..=23).contains(&spell)
}

pub fn stats(tier: u32, mode: u32, rank: u32) -> WandStats {
    let cap = capacity(tier);
    let base_recharge: f32 = match tier {
        1 => 0.65,
        2 => 0.45,
        _ => 0.30,
    };
    let base_delay: f32 = match tier {
        1 => 0.16,
        2 => 0.11,
        _ => 0.07,
    };
    let (chassis, spells_per_cast, delay_mod, recharge_mod, spread_mod) = match mode {
        1 => ("Twin Scatter", 2, 0.04_f32, 0.05_f32, 6.0_f32),
        2 => ("Trigger Conduit", 1, 0.02_f32, 0.0_f32, 0.0_f32),
        3 => ("Rapid Flux", 1, -0.06_f32, -0.08_f32, 1.5_f32),
        _ => ("Ordered Deck", 1, 0.0_f32, 0.0_f32, 0.0_f32),
    };
    WandStats {
        name: match tier {
            0 => "Unarmed",
            1 => "Lumen Twig",
            2 => "Quarry Conduit",
            _ => "Sanctum Sovereign",
        },
        chassis,
        shuffle: false,
        spells_per_cast,
        cast_delay: (base_delay + delay_mod).max(0.03),
        recharge_time: (base_recharge + recharge_mod).max(0.10),
        mana_max: 100.0 + rank as f32 * 15.0,
        mana_charge_speed: 8.0 + rank as f32 * 1.5 + tier as f32 * 4.5,
        capacity: cap,
        spread_deg: (match tier {
            1 => 2.0,
            2 => 0.8,
            _ => 0.0,
        } + spread_mod),
        always_cast: if tier >= 3 && mode == 3 {
            Some(23)
        } else {
            None
        },
    }
}

pub fn evaluate_cast_block(
    wand: u32,
    rare_spells: u32,
    discoveries: u32,
    rank: u32,
    loadout: &Loadout,
    start_cursor: usize,
) -> CastBlockEval {
    let cap = capacity(wand);
    let wand_stats = stats(wand, loadout.mode, rank);
    let find_next = |after: usize| -> usize {
        (1..=cap)
            .map(|n| (after + n) % cap)
            .find(|&i| spell_available(loadout.slot(i), wand, rare_spells))
            .unwrap_or(after % cap)
    };

    let mut cur = start_cursor % cap;
    if !spell_available(loadout.slot(cur), wand, rare_spells) {
        cur = find_next(cur);
    }
    let first_slot = cur;
    let first_spell = loadout.slot(first_slot);

    // Preserve exact legacy behavior for non-modifier single slots in modes 0, 1, 2
    // so all existing acceptance & unit tests match bit-for-bit.
    if !is_modifier(first_spell) && loadout.mode <= 2 {
        let next = find_next(first_slot);
        let payload = if loadout.mode == 2
            && projectile(first_spell)
            && next > first_slot
            && projectile(loadout.slot(next))
        {
            Some(loadout.slot(next))
        } else {
            None
        };
        let twin = loadout.mode == 1 && projectile(first_spell);
        let count = if twin { 2 } else { 1 };
        let mana_cost = inscriptions::cost(first_spell, discoveries) * if twin { 1.6 } else { 1.0 }
            + payload.map_or(0.0, |p| inscriptions::cost(p, discoveries));
        let last_slot = if payload.is_some() { next } else { first_slot };
        let after_cast = find_next(last_slot);
        let wrapped = after_cast <= last_slot;
        let recharge = if wrapped {
            (wand_stats.recharge_time + spell_recharge_delta(first_spell)).max(0.08)
        } else {
            0.0
        };
        let cast_delay = (spell_cooldown(first_spell) * if twin { 1.2 } else { 1.0 }
            + spell_cast_delay_delta(first_spell))
        .max(0.04);
        let payload_slot = payload.map(|_| next);
        let mut actions = Vec::with_capacity(count);
        for _ in 0..count {
            actions.push(CastAction {
                slot: first_slot,
                spell: first_spell,
                payload,
                payload_slot,
                mods: ShotMods::default(),
            });
        }
        return CastBlockEval {
            actions,
            modifier_slots: Vec::new(),
            last_slot,
            mana_cost,
            cast_delay,
            recharge_time: recharge,
            spread_deg: if twin { 6.3 } else { wand_stats.spread_deg },
            mods: ShotMods::default(),
        };
    }

    // Full Noita left-to-right modifier, multicast, and trigger deck evaluation.
    let mut mods = ShotMods::default();
    if wand_stats.always_cast == Some(23) {
        mods.trail = true;
        mods.damage_mul *= 1.15;
    }
    let mut draws: usize = if loadout.mode == 1 { 2 } else { 1 };
    let mut attach_trigger = loadout.mode == 2;
    let mut delay_delta = if loadout.mode == 3 { -0.06 } else { 0.0 };
    let mut recharge_delta = if loadout.mode == 3 { -0.08 } else { 0.0 };
    let mut spread_deg = wand_stats.spread_deg;
    let mut mana_cost = 0.0f32;
    let mut modifier_slots = Vec::new();
    let mut actions = Vec::new();
    let mut last_slot = first_slot;
    let mut steps = 0usize;

    while draws > 0 && steps < cap {
        steps += 1;
        let s = loadout.slot(cur);
        if !spell_available(s, wand, rare_spells) {
            break;
        }
        last_slot = cur;
        if is_modifier(s) {
            modifier_slots.push(cur);
            mana_cost += inscriptions::cost(s, discoveries);
            delay_delta += spell_cast_delay_delta(s);
            recharge_delta += spell_recharge_delta(s);
            spread_deg = (spread_deg + spell_spread_delta(s)).max(0.0);
            match s {
                16 => draws = (draws + 1).min(6), // Bifurcate: -1 draw + 2 draws = +1 net
                17 => draws = (draws + 2).min(6), // Trifurcate: -1 draw + 3 draws = +2 net
                18 => attach_trigger = true,      // Trigger Bolt
                19 => mods.homing = true,         // Homing Arc
                20 => {
                    // Orbit & Ricochet
                    mods.orbit = true;
                    mods.bounces = (mods.bounces + 2).min(6);
                    mods.damage_mul *= 1.30;
                    mods.speed_mul *= 1.15;
                }
                21 => {
                    // Heavy Bore (Matter Eater + Heavy Shot)
                    mods.bore = true;
                    mods.damage_mul *= 1.65;
                    mods.speed_mul *= 0.78;
                }
                22 => {
                    // Rapid Flux (Add Mana + Chainsaw)
                    mana_cost -= 20.0;
                }
                23 => {
                    // Elemental Wake
                    mods.trail = true;
                    mods.damage_mul *= 1.20;
                }
                _ => {}
            }
            let next = find_next(cur);
            if next == first_slot && actions.is_empty() && steps >= cap {
                break;
            }
            cur = next;
        } else {
            draws -= 1;
            mana_cost += inscriptions::cost(s, discoveries);
            delay_delta += spell_cast_delay_delta(s);
            recharge_delta += spell_recharge_delta(s);
            spread_deg = (spread_deg + spell_spread_delta(s)).max(0.0);

            let carrier_slot = cur;
            let mut payload = None;
            let mut payload_slot = None;
            if attach_trigger && projectile(s) {
                let next = find_next(cur);
                let next_s = loadout.slot(next);
                if next != cur && (projectile(next_s) || next_s == 9) {
                    payload = Some(next_s);
                    payload_slot = Some(next);
                    mana_cost += inscriptions::cost(next_s, discoveries) * 0.85;
                    last_slot = next;
                    cur = next;
                }
                attach_trigger = false;
            }

            actions.push(CastAction {
                slot: carrier_slot,
                spell: s,
                payload,
                payload_slot,
                mods,
            });

            if draws > 0 {
                let next = find_next(cur);
                if next == first_slot {
                    break;
                }
                cur = next;
            }
        }
    }

    // If the deck only contained modifiers, fire a modified Spark Bolt so the cast isn't dead.
    if actions.is_empty() {
        actions.push(CastAction {
            slot: first_slot,
            spell: 0,
            payload: None,
            payload_slot: None,
            mods,
        });
        mana_cost += inscriptions::cost(0, discoveries);
    }

    let base_cd = actions
        .iter()
        .map(|a| spell_cooldown(a.spell))
        .fold(0.18f32, f32::max);
    let cast_delay = (base_cd + delay_delta).max(0.04);
    let after_cast = find_next(last_slot);
    let wrapped = after_cast <= last_slot;
    let recharge = if wrapped {
        (wand_stats.recharge_time + recharge_delta).max(0.06)
    } else {
        0.0
    };

    CastBlockEval {
        actions,
        modifier_slots,
        last_slot,
        mana_cost: mana_cost.max(0.0),
        cast_delay,
        recharge_time: recharge,
        spread_deg,
        mods,
    }
}

impl Adventure {
    pub fn set_wand_slot(&mut self, slot: u32, spell: u32) {
        let cap = capacity(self.wand);
        if self.wand == 0
            || slot as usize >= cap
            || spell > MAX_SPELL_ID
            || (spell != EMPTY && !spell_available(spell, self.wand, self.rare_spells))
        {
            return;
        }
        self.loadout.set_slot(slot as usize, spell);
        if !(0..cap).any(|i| {
            let s = self.loadout.slot(i);
            s != EMPTY && spell_available(s, self.wand, self.rare_spells)
        }) {
            self.loadout.set_slot(0, 0);
        }
        self.loadout.cursor = slot as usize;
        self.prepare_wand();
    }

    pub fn swap_wand_slots(&mut self, a: u32, b: u32) {
        let cap = capacity(self.wand);
        let (ia, ib) = (a as usize, b as usize);
        if self.wand == 0 || ia >= cap || ib >= cap || ia == ib {
            return;
        }
        let sa = self.loadout.slot(ia);
        let sb = self.loadout.slot(ib);
        self.loadout.set_slot(ia, sb);
        self.loadout.set_slot(ib, sa);
        self.loadout.cursor = ib;
        self.prepare_wand();
    }

    pub(super) fn prepare_wand(&mut self) {
        let cap = capacity(self.wand);
        self.loadout.cursor %= cap;
        for _ in 0..cap {
            let s = self.loadout.slot(self.loadout.cursor);
            if spell_available(s, self.wand, self.rare_spells) {
                self.spell = s;
                return;
            }
            self.loadout.cursor = (self.loadout.cursor + 1) % cap;
        }
        self.spell = 0;
    }

    fn next_slot(&self, after: usize) -> usize {
        let cap = capacity(self.wand);
        (1..=cap)
            .map(|n| (after + n) % cap)
            .find(|&i| spell_available(self.loadout.slot(i), self.wand, self.rare_spells))
            .unwrap_or(after % cap)
    }

    pub(super) fn advance_wand(&mut self) {
        self.loadout.cursor = self.next_slot(self.loadout.cursor);
        self.prepare_wand();
    }

    pub(super) fn emit_spell(&mut self, spell: u32, pos: Vec3, dir: Vec3, payload: Option<u32>) {
        self.emit_spell_modified(spell, pos, dir, payload, ShotMods::default());
    }

    pub(super) fn emit_spell_modified(
        &mut self,
        spell: u32,
        pos: Vec3,
        dir: Vec3,
        payload: Option<u32>,
        mods: ShotMods,
    ) {
        if self.projectiles.len() >= 96 {
            return;
        }
        let speed = spell_base_speed(spell) * mods.speed_mul;
        let arc_lift = match spell {
            5 => Vec3::Y * 2.0,
            12 => Vec3::Y * 1.6,
            _ => Vec3::ZERO,
        };
        self.projectiles.push(Projectile {
            pos,
            velocity: dir * speed + arc_lift,
            hostile: false,
            spell,
            life: if spell == 10 { 3.6 } else { 3.0 },
            payload,
            mods,
            bore_budget: if spell == 10 {
                24
            } else if mods.bore {
                6
            } else {
                0
            },
            trail_timer: 0.0,
        });
    }

    fn room_for_spell(&self, spell: u32, count: usize) -> bool {
        let incoming = self
            .projectiles
            .iter()
            .filter(|p| !p.hostile)
            .map(|p| usize::from(p.spell == spell) + usize::from(p.payload == Some(spell)))
            .sum::<usize>();
        match spell {
            3 => self.terrain_blasts.len() + incoming + count <= 8,
            5 => self.blooms.len() + incoming + count <= 6,
            6 | 10 => self.wells.len() + incoming + count <= 4,
            11 => self.crosses.len() + incoming + count <= 4,
            _ => true,
        }
    }

    pub(super) fn cast_wand(
        &mut self,
        world: &mut World,
        player: &mut Player,
        eye: Vec3,
        forward: Vec3,
    ) {
        if self.wand == 0 {
            self.cast_cd = 0.7;
            world.expedition.say(
                "Craft a lumen wand at a workbench. Arrange its spells in Gear; hold R to cast.",
            );
            return;
        }
        self.prepare_wand();
        let rank = self.rank();
        let eval = evaluate_cast_block(
            self.wand,
            self.rare_spells,
            self.discoveries,
            rank,
            &self.loadout,
            self.loadout.cursor,
        );

        if eval.actions.len() == 1 && eval.actions[0].spell == 1 && self.health >= self.max_health()
        {
            self.loadout.cursor = eval.last_slot;
            self.advance_wand();
            self.cast_cd = 0.12;
            return;
        }
        if self.mana < eval.mana_cost {
            self.cast_cd = 0.12;
            self.mana_flash = 0.45;
            return;
        }

        let proj_count = eval
            .actions
            .iter()
            .filter(|a| projectile(a.spell))
            .count();
        if proj_count > 0 && self.projectiles.len() + proj_count > 96 {
            self.cast_cd = 0.12;
            return;
        }
        for act in &eval.actions {
            if projectile(act.spell) {
                let same_count = eval
                    .actions
                    .iter()
                    .filter(|a| a.spell == act.spell)
                    .count();
                if !self.room_for_spell(act.spell, same_count)
                    || act
                        .payload
                        .is_some_and(|p| !self.room_for_spell(p, if p == act.spell { 2 } else { 1 }))
                {
                    self.cast_cd = 0.12;
                    return;
                }
            }
        }
        if eval.actions.len() == 1 && eval.actions[0].spell == 7 && !self.echo_step(world, player) {
            self.cast_cd = 0.12;
            return;
        }

        self.mana = (self.mana - eval.mana_cost).clamp(0.0, self.max_mana());
        self.last_cast = eval.actions[0].spell;
        self.cast_pose = 0.4;

        let total_actions = eval.actions.len();
        let up_axis = if forward.y.abs() > 0.98 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let mut played_cue = None;

        for (idx, act) in eval.actions.iter().enumerate() {
            let spell = act.spell;
            match spell {
                1 => {
                    self.health = (self.health + 30.0 + rank as f32 * 5.0).min(self.max_health());
                    if self.knows(6) {
                        self.hunger = (self.hunger + 12.0).min(100.0);
                    }
                    self.effect(eye, spell_color(1), 1.0);
                    self.spawn_voxel_burst(eye + forward * 0.6, spell_color(1), 18, 1.4, 0.045);
                    played_cue.get_or_insert(Cue::Mend);
                }
                4 => {
                    self.ward = if self.knows(9) { 9.0 } else { 6.0 };
                    self.ward_hits = 3;
                    self.effect(eye + forward * 0.7, spell_color(4), 0.7);
                    self.spawn_voxel_burst(eye + forward * 0.7, spell_color(4), 16, 1.1, 0.04);
                    played_cue.get_or_insert(Cue::Ward);
                }
                7 => {
                    if total_actions > 1 {
                        let _ = self.echo_step(world, player);
                    }
                }
                9 => {
                    // Luminous Drill: instant 3D laser beam that carves rock/ore and slices enemies
                    let angle = if total_actions > 1 {
                        let centered = idx as f32 - (total_actions - 1) as f32 * 0.5;
                        centered * eval.spread_deg.to_radians().max(0.045)
                    } else {
                        0.0
                    };
                    let dir = Quat::from_axis_angle(up_axis, angle) * forward;
                    self.cast_luminous_drill(world, eye, dir, act.mods, act.payload);
                    played_cue.get_or_insert(Cue::Spark);
                }
                _ => {
                    let dir = if total_actions == 2 && self.loadout.mode == 1 && eval.modifier_slots.is_empty() {
                        Quat::from_axis_angle(up_axis, if idx == 0 { -0.055 } else { 0.055 })
                            * forward
                    } else if total_actions > 1 {
                        let step_rad = (eval.spread_deg.to_radians() / (total_actions as f32 - 1.0).max(1.0)).clamp(0.045, 0.14);
                        let centered = idx as f32 - (total_actions - 1) as f32 * 0.5;
                        Quat::from_axis_angle(up_axis, centered * step_rad) * forward
                    } else {
                        forward
                    };
                    self.emit_spell_modified(spell, eye, dir, act.payload, act.mods);
                    if spell == 14 && !player.flying {
                        player.vel -= dir * 1.4;
                    }
                    self.effect(eye + forward * 0.5, spell_color(spell), 0.14);
                    self.spawn_voxel_burst(eye + forward * 0.55, spell_color(spell), 8, 1.2, 0.032);
                    played_cue.get_or_insert(match spell {
                        2 | 13 => Cue::Frost,
                        3 | 12 => Cue::Burst,
                        5 => Cue::Bloom,
                        6 | 10 => Cue::Rift,
                        _ => Cue::Spark,
                    });
                }
            }
        }

        if let Some(cue) = played_cue {
            sound(&mut self.sounds, cue, None, 0.7);
        }

        self.loadout.cursor = eval.last_slot;
        self.advance_wand();
        self.cast_delay_total = eval.cast_delay;
        self.recharge_total = eval.recharge_time;
        self.cast_cd = eval.cast_delay + eval.recharge_time;
        self.cast_total = self.cast_cd;
    }

    pub fn cancel_cast_input(&mut self) {
        self.queued_cast = false;
        self.queued_cycle = false;
    }

    pub fn blast_visual(&mut self, pos: Vec3, radius: f32, normal: Vec3, color: [f32; 3]) {
        if self.blasts.len() >= 8 {
            self.blasts.remove(0);
        }
        self.blasts.push(crate::tool_effects::Blast {
            pos,
            normal,
            radius,
            color,
            age: 0.0,
        });
    }

    pub fn blast_feedback(&mut self, pos: Vec3, radius: f32, normal: Vec3, color: [f32; 3]) {
        sound(&mut self.sounds, Cue::Burst, Some(pos), 1.0);
        self.blast_visual(pos, radius, normal, color);
        for e in &mut self.enemies {
            let d = e.center() - pos;
            if d.length() < radius {
                e.health -= 48.0 * (1.0 - d.length() / radius);
                e.knock = d.normalize_or_zero() * 6.0;
                e.flash = 0.2;
            }
        }
    }
}

#[cfg(test)]
mod acceptance {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::WorldType};
    #[test]
    fn wand_burst_detonates_at_point_blank_walls_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut p = Player::new();
            p.pos = Vec3::new(-0.05, 150.0, scale.meters() * 0.5) / scale.meters();
            p.flying = true;
            let eye = p.eye(scale) * scale.meters();
            let cell = IVec3::new(0, (eye.y / scale.meters()).floor() as i32, 0);
            w.set(cell, crate::world::BlockId::Stone);
            let mut a = Adventure::default();
            a.wand = 3;
            a.invulnerable = 0.0;
            a.emit_spell(3, eye, Vec3::X, None);
            w.adventure = a;
            super::super::tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
            assert!(w.adventure.projectiles.is_empty(), "{scale:?}");
            let (point, radius) = w
                .adventure
                .take_terrain_blast()
                .expect("burst crater queued");
            assert!(point.x <= 0.0 && point.x >= -0.05);
            assert_eq!(radius, 2.0);
            assert!(!w.adventure.blasts.is_empty());
            assert!(w.adventure.health < 100.0);
        }
    }
    #[test]
    fn wand_taps_survive_zero_step_frames_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut p = Player::new();
            p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
            p.flying = true;
            w.adventure.wand = 3;
            w.adventure.loadout.slots = [0, 2, EMPTY, EMPTY];
            super::super::tick(&mut w, &mut p, 0.0, true, false, false);
            assert!(w.adventure.projectiles.is_empty());
            super::super::tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
            assert_eq!(w.adventure.projectiles.len(), 1, "{scale:?}");
            assert!(w.adventure.mana < 90.0);
            assert_eq!(w.adventure.loadout.cursor, 1);
            super::super::tick(&mut w, &mut p, 0.0, false, true, false);
            super::super::tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
            assert_eq!(w.adventure.loadout.cursor, 0);
        }
    }
    #[test]
    fn wand_patterns_prepaid_payload_and_twin_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut p = Player::new();
            p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
            let eye = p.eye(scale) * scale.meters();
            let mut a = Adventure::default();
            a.wand = 3;
            a.loadout.slots = [0, 3, EMPTY, EMPTY];
            a.loadout.mode = 2;
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert_eq!(a.projectiles.len(), 1);
            assert_eq!(a.projectiles[0].payload, Some(3));
            assert_eq!(a.mana, 58.0);
            assert_eq!(a.loadout.cursor, 0);
            a.projectiles.clear();
            a.mana = 100.0;
            a.loadout.mode = 1;
            a.cast_wand(&mut w, &mut p, eye, Vec3::Y);
            assert_eq!(a.projectiles.len(), 2);
            assert!(
                a.projectiles[0]
                    .velocity
                    .distance(a.projectiles[1].velocity)
                    > 1.0
            );
            assert!((a.mana - 80.8).abs() < 0.01);
        }
    }
    #[test]
    fn noita_modifier_chains_combine_multicast_trigger_and_rapid_flux() {
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 17);
        let mut p = Player::new();
        p.pos = Vec3::new(0.0, 150.0, 0.0) / w.voxel_scale.meters();
        let eye = p.eye(w.voxel_scale) * w.voxel_scale.meters();
        let mut a = Adventure::default();
        a.wand = 3;
        // Deck: [Rapid Flux (22)] -> [Bifurcate (16)] -> [Homing (19)] -> [Trigger (18)] -> [Spark (0)] -> [Burst (3)] -> [Frost (2)]
        a.loadout.slots = [22, 16, 19, 18];
        a.loadout.aux_slots = [0, 3, 2, EMPTY];
        a.loadout.cursor = 0;
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        // Bifurcate draws 2 projectiles: Spark (with Burst payload) and Frost Lance, both with Homing!
        assert_eq!(a.projectiles.len(), 2);
        assert_eq!(a.projectiles[0].spell, 0);
        assert_eq!(a.projectiles[0].payload, Some(3));
        assert!(a.projectiles[0].mods.homing);
        assert_eq!(a.projectiles[1].spell, 2);
        assert!(a.projectiles[1].mods.homing);
        // Cursor wrapped past slot 6 back to 0!
        assert_eq!(a.loadout.cursor, 0);
    }

    #[test]
    fn noita_world_interactive_spells_and_8_slot_save_roundtrip() {
        use crate::world::BlockId as B;
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
        let center = IVec3::new(10, 80, 10);
        w.set(center, B::Water);
        w.set(center + IVec3::X, B::Stone);
        w.set(center + IVec3::Z, B::Stone);

        let mut a = Adventure::default();
        a.wand = 3;
        a.rare_spells = !0;
        let pos = (center.as_vec3() + Vec3::splat(0.5)) * w.voxel_scale.meters();

        // 1. Cryo Nova freezes Water into walkable Ice and emits 3D translucent voxel particles
        a.cryo_bloom(&mut w, pos, 1.5);
        assert_eq!(w.get(center), B::Ice);
        assert!(!a.particles.is_empty());

        // 2. Magma Glob thermal bloom melts Ice back to Water / Steam and heats thermal cells
        a.thermal_bloom(&mut w, pos, 1.8, true);
        assert!(matches!(w.get(center), B::Water | B::Steam | B::Fire));

        // 3. Touch of Gold transmutes Stone into Crystal / Ore / GlowCap
        a.alchemical_transmute(&mut w, pos, 2.0);
        assert!(matches!(
            w.get(center + IVec3::X),
            B::Crystal | B::CopperOre | B::IronOre | B::GlowCap
        ));

        // 4. Lumen Drill carves solid blocks along its beam and spawns laser + voxel sparks
        w.set(center + IVec3::Z, B::Stone);
        let drill_origin = (center.as_vec3() + Vec3::new(0.5, 0.5, -1.0)) * w.voxel_scale.meters();
        a.cast_luminous_drill(&mut w, drill_origin, Vec3::Z, ShotMods::default(), None);
        assert_eq!(w.get(center + IVec3::Z), B::Air);
        assert!(!a.lasers.is_empty());

        // 5. 8-slot wand deck save/decode roundtrip preserves all 8 slots and mode 3
        a.spell = 10;
        a.loadout.slots = [16, 19, 10, 11];
        a.loadout.aux_slots = [22, 18, 12, 15];
        a.loadout.cursor = 5;
        a.loadout.mode = 3;
        let mut bytes = Vec::new();
        a.encode(&mut bytes);
        let mut idx = 0usize;
        let decoded = Adventure::decode(
            &mut || {
                let word = u32::from_le_bytes(bytes[idx..idx + 4].try_into().unwrap());
                idx += 4;
                Ok(word)
            },
            true,
            true,
        )
        .expect("decode 8-slot wand");
        assert_eq!(idx, bytes.len());
        assert_eq!(decoded.spell, 10);
        assert_eq!(decoded.loadout.slots, [16, 19, 10, 11]);
        assert_eq!(decoded.loadout.aux_slots, [22, 18, 12, 15]);
        assert_eq!(decoded.loadout.cursor, 5);
        assert_eq!(decoded.loadout.mode, 3);

        // 6. Direct drag-and-drop wand slot swapping & trigger-payload slot tracking
        let mut b = Adventure::default();
        b.wand = 3;
        b.rare_spells = !0;
        b.loadout.slots = [18, 10, 11, 17];
        b.loadout.aux_slots = [12, 0, 0, 0];
        b.swap_wand_slots(0, 4);
        assert_eq!(b.loadout.slot(0), 12);
        assert_eq!(b.loadout.slot(4), 18);
        b.swap_wand_slots(0, 4);
        let eval = evaluate_cast_block(b.wand, b.rank(), b.discoveries, b.rare_spells, &b.loadout, 0);
        assert_eq!(eval.actions.len(), 1);
        assert_eq!(eval.actions[0].slot, 1);
        assert_eq!(eval.actions[0].spell, 10);
        assert_eq!(eval.actions[0].payload, Some(11));
        assert_eq!(eval.actions[0].payload_slot, Some(2));
    }
}
