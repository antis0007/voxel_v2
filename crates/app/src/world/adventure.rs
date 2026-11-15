//! Bounded local encounters, persistent victories, and player-cast magic.
use super::{World, collides_world, factory, sites, trace, undercrown};
use crate::audio::{Cue, Event as Sound};
use crate::{mesh::models::Model, player::Player};
use glam::{IVec3, Quat, Vec3};
use std::collections::HashMap;
mod bestiary;
pub use bestiary::drop_sources;
mod robots;
pub(crate) mod guard;
pub(crate) mod wildlife;
mod enemy_presentation;
mod enemy_review;
pub use enemy_review::camera as enemy_review_camera;
pub use enemy_review::install as install_enemy_review;
pub use enemy_review::arm as arm_enemy_review;
pub use enemy_review::{input as enemy_review_input, casting as enemy_review_casting, automated as enemy_review_automated, manual as enemy_review_manual};
pub use enemy_review::checkpoint as enemy_review_checkpoint;
mod bosses;
mod action;
#[cfg(feature = "multiplayer")]
mod network_combat;
mod matron;
mod conductors;
mod thunderhen;
mod habitat;
mod fauna;
mod monsters;
mod engine;
pub use engine::review::{install as install_ossuary_review, observe as observe_ossuary_review, input as ossuary_review_input, casting as ossuary_review_cast, automatic as ossuary_review_automatic, checkpoint as ossuary_review_checkpoint};
mod glassweaver;
mod boss_look;
mod tollkeeper;
mod hall_warden;
pub use tollkeeper::{install_review as install_tollkeeper_review, observe_review as observe_tollkeeper_review,
    review_cast as tollkeeper_review_cast, review_input as tollkeeper_review_input,
    review_checkpoint as tollkeeper_review_checkpoint};
mod hart;
mod hart_crown;
pub use hart::{install_review as install_hart_review, observe_review as observe_hart_review, review_camera as hart_review_camera, review_cast as hart_review_cast};
pub mod discovery;
pub mod spell_defs;
pub use spell_defs::{
    Class, spell_available, spell_base_speed, spell_cast_delay_delta,
    spell_class, spell_color, spell_cooldown, spell_cost, spell_hint, spell_hit_damage,
    spell_kind_tag, spell_name, spell_recharge_delta, spell_spread_delta,
};
pub(crate) mod growth;
pub(crate) mod hunger;
pub mod difficulty;
pub mod death;
pub mod inscriptions;
pub mod chronicle;
mod quest;
pub use quest::{QuestEntry, QuestId};
mod rare_magic;
pub mod reliquaries;
mod rewards;
pub mod resonance;
pub(crate) mod spells;
mod snapline;
pub(crate) mod magic_fx;
mod trajectory;
#[cfg(test)]
mod aim_tests;
mod water_transfer;
mod water_contact;
mod settlement_guard;
mod observatory;
mod arcana;
pub mod status;
mod tactics;
mod vault_fight;
mod navigation;
mod perception;
pub(crate) mod encounters;
pub(crate) use encounters::focused_cache;
mod encounter_placement;
mod records;
#[cfg(test)]
mod records_tests;
#[cfg(test)]
mod spell_matter_tests;
#[cfg(test)]
mod local_frame_tests;
mod modifiers;
pub mod affinity;
pub mod buffs;
mod night;
mod burrowers;
mod hurled;
mod spawn_safety;
mod field_foes;
mod food_chain;
mod mobs;
mod folklore;
mod aberrations;
pub mod tinkering;
pub mod blade;
pub mod wands;
pub mod runes;
pub mod circles;
pub mod rune_cast;
pub mod hedrons;
pub(crate) mod concentration;
pub(crate) mod enchanting;
pub(crate) mod fieldcraft;
pub mod wand_items;

#[cfg(test)]
mod interaction_tests;

pub const MAX_ENEMIES: usize = 24;
// Metres: reach along the aim ray and the widening melee sweep around it.
const MELEE_REACH: f32 = 4.2;
const ARMED_REACH_BONUS: f32 = 0.6;
const MELEE_SWEEP_BASE: f32 = 0.65;
const MELEE_SWEEP_GROWTH: f32 = 0.18;
const MELEE_SWEEP_MAX: f32 = 1.4;
const MAX_RECORDS: usize = 4096;
pub fn wand_recipe(tier: u32) -> &'static [(usize, u32)] {
    match tier {
        0 | 1 => &[(6, 64), (5, 64), (4, 128)],
        _ => &[(17, 128), (18, 256), (14, 128)],
    }
}
pub fn sword_recipe(tier: u32) -> &'static [(usize, u32)] {
    match tier {
        1 => &[(6, 128), (4, 64)],
        5 => &[(15, 192), (16, 64), (17, 64)],
        _ => &[(15, 128), (16, 64)],
    }
}
pub fn sword_name(tier: u32) -> &'static str {
    match tier {
        1 => "Copper sword",
        2 => "Steel sword",
        3 => "Cinderbrand",
        4 => "Winterglass",
        5 => "Kinetic maul",
        _ => "Unarmed",
    }
}
pub fn ration_recipe() -> &'static [(usize, u32)] {
    &[(13, 64), (4, 64)]
}
/// Highest spell id. Loadout slots are stored in 8-bit fields, so ids below 255
/// fit the existing save layout; old saves only ever hold ids 0..=63.
pub const MAX_SPELL_ID: u32 = discovery::LAST_SPELL_BIT;


/// Shared with the block swatches and the parchment interface.
/// Paper, amber, copper, olive, leaf, mana, coral, plum.
pub const PALETTE_PAPER: [f32; 3] = [0.96, 0.90, 0.79];
pub const PALETTE_AMBER: [f32; 3] = [0.90, 0.72, 0.42];
pub const PALETTE_COPPER: [f32; 3] = [0.86, 0.46, 0.24];
pub const PALETTE_OLIVE: [f32; 3] = [0.62, 0.74, 0.38];
pub const PALETTE_LEAF: [f32; 3] = [0.42, 0.58, 0.32];
pub const PALETTE_MANA: [f32; 3] = [0.55, 0.74, 0.82];
pub const PALETTE_MANA_LIT: [f32; 3] = [0.74, 0.82, 0.86];
pub const PALETTE_WATER: [f32; 3] = [0.28, 0.48, 0.55];
pub const PALETTE_CORAL: [f32; 3] = [0.86, 0.42, 0.36];
pub const PALETTE_PLUM: [f32; 3] = [0.52, 0.36, 0.52];

fn sound(queue: &mut Vec<Sound>, cue: Cue, pos: Option<Vec3>, gain: f32) {
    if queue.len() < 32 {
        queue.push(Sound {
            cue,
            pos,
            gain,
            pitch: 1.0,
        });
    }
}

/// Sound with a family-specific pitch. Spell variants use this so repeated
/// casts of different elements do not all read as the same chirp.
fn sound_pitched(queue: &mut Vec<Sound>, cue: Cue, pos: Option<Vec3>, gain: f32, pitch: f32) {
    if queue.len() < 32 {
        queue.push(Sound {
            cue,
            pos,
            gain,
            pitch,
        });
    }
}
/// Underground layer archetype at a point, if it is deep enough to have one.
fn zone_archetype(cfg: engine_worldgen::WorldSampleConfig, pos: Vec3) -> Option<crate::world::zones::Archetype> {
    let inv = cfg.inv_voxel_meters();
    let surface = cfg.world_y(
        engine_worldgen::terrain_height(&cfg, (pos.x * inv).floor() as i32, (pos.z * inv).floor() as i32),
    );
    crate::world::zones::sample(cfg.seed, pos.x, pos.z, surface - pos.y)
        .filter(|z| z.depth_weight > 0.5)
        .map(|z| z.layer.archetype)
}
pub fn enemy_name(kind: u32) -> &'static str {
    if let Some(name) = bestiary::name(kind) {
        return name;
    }
    match kind {
        2 => "Arcane Wisp",
        3 => "Burrow Beetle",
        6..=8 | tollkeeper::KIND | hall_warden::FURNACE | hall_warden::ASTRONOMER => bosses::name(kind),
        9 => "Gravelback Colossus",
        _ => "Sanctum Guardian",
    }
}
fn is_boss(kind: u32) -> bool {
    matches!(kind, 6..=8 | tollkeeper::KIND | matron::KIND | thunderhen::KIND | hall_warden::FURNACE | hall_warden::ASTRONOMER)
}
fn enemy_health(kind: u32) -> f32 {
    if let Some(species) = bestiary::species(kind) {
        return species.health;
    }
    match kind {
        3 => 54.0,
        9 => 210.0,
        6..=8 | tollkeeper::KIND => bosses::health(kind),
        _ => 35.0,
    }
}
fn tall(kind: u32) -> bool {
    bestiary::species(kind).map_or(matches!(kind, 9 | 6..=8 | tollkeeper::KIND), |s| s.tall)
}
fn flying(kind: u32) -> bool {
    bestiary::species(kind).map_or(matches!(kind, 2), |s| s.flying)
}
fn brawler(kind: u32) -> bool {
    bestiary::species(kind).map_or(matches!(kind, 3 | 9), |s| s.brawler)
}
/// Drops the component of `v` along local up.
// On the Manyfold shell up is radial, so world XZ flattening loses the player.
fn along_floor(up: Vec3, v: Vec3) -> Vec3 {
    if up.dot(Vec3::Y) > 0.999 {
        Vec3::new(v.x, 0.0, v.z)
    } else {
        v - up * v.dot(up)
    }
}
/// World-space heading for a yaw measured in the local up frame.
fn yaw_forward(up: Vec3, yaw: f32) -> Vec3 {
    let local = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    if up.dot(Vec3::Y) > 0.999 {
        local
    } else {
        Quat::from_rotation_arc(Vec3::Y, up) * local
    }
}
/// Yaw in the local up frame that faces world direction `v`.
fn local_yaw(up: Vec3, v: Vec3) -> f32 {
    let flat = if up.dot(Vec3::Y) > 0.999 { v } else { Quat::from_rotation_arc(up, Vec3::Y) * v };
    (-flat.x).atan2(-flat.z)
}
pub fn mastery(clears: usize) -> u32 {
    if clears >= 7 {
        3
    } else if clears >= 3 {
        2
    } else if clears >= 1 {
        1
    } else {
        0
    }
}
pub fn rank_name(rank: u32) -> &'static str {
    match rank {
        0 => "Wayfarer",
        1 => "Delver",
        2 => "Runekeeper",
        _ => "Arcanist",
    }
}
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Enemy {
    /// Source identity owns recurrence; these actors never enter site-clear records.
    pub(crate) habitat: Option<u32>,
    pub site: IVec3,
    pub slot: u32,
    pub kind: u32,
    pub pos: Vec3,
    pub home: Vec3,
    #[cfg(feature = "multiplayer")]
    #[serde(skip)]
    replica_target: Option<enemy_presentation::ReplicaPose>,
    #[cfg(feature = "multiplayer")]
    #[serde(skip)]
    creature_tick: u64,
    pub health: f32,
    pub slow: f32,
    pub tell: f32,
    pub phase: f32,
    yaw: f32,
    velocity: Vec3,
    /// Local up. World up until a radial field is sampled.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    up: Vec3,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    navigation: navigation::Navigation,
    gait: f32,
    recovery: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    reaction: enemy_presentation::Reaction,
    /// Lean, bank, turn follow-through and landing squash; measured from travel.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    motion: enemy_presentation::BodyMotion,
    encounter: encounters::State,
    cooldown: f32,
    sight: f32,
    visible: bool,
    last_seen: Vec3,
    alert: f32,
    /// Perception meter 0..1 (see `perception.rs`); transient, never saved.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    aware: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    investigate: Option<Vec3>,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    search: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    search_step: u8,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    spotted_flash: f32,
    /// Line of sight on the last perception check; pauses awareness decay.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    glimpsed: bool,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    provoked: bool,
    flash: f32,
    stagger: f32,
    knock: Vec3,
    charge: f32,
    lunge: Vec3,
    aim: Vec3,
    attack: u32,
    attacks: u32,
    poise: f32,
    pub menace: u8,
    pub affixes: status::Affixes,
    pub status: status::Status,
    /// Damage absorbed before health (Warded).
    pub shield: f32,
    /// Warden's carried shield. Breaking it removes the visible shield and its
    /// frontal block without touching saved encounter records.
    pub guard: bool,
    hart: hart::State,
    toll: tollkeeper::State,
    matron: matron::State,
    hen: thunderhen::State,
    engine: engine::State,
    glass: glassweaver::State,
    /// Grazer youth: 1 at birth, 0 when grown; the model scales with it.
    pub(crate) youth: f32,
    /// Segment chain of a burrower (kinds in `assets/burrowers.ron`); empty otherwise.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    burrow: burrowers::Body,
}

impl Enemy {
    /// Playback pitch of a grazer's call; `None` for every other role.
    pub(crate) fn call_pitch(&self) -> Option<f32> {
        let s = bestiary::species(self.kind).filter(|s| s.role == bestiary::Role::Grazer)?;
        // Smaller bodies and young animals call higher.
        Some((1.4 / s.body_height.max(0.3)).clamp(0.7, 1.5) * (1.0 + self.youth * 0.6))
    }
}

#[cfg(test)]
#[path = "adventure/model_material_tests.rs"]
mod model_material_tests;
impl Enemy {
    pub(super) fn hostile(&self) -> bool {
        (!wildlife::passive(self.kind) || wildlife::fights_back(self.kind))
            && (self.provoked || !bestiary::species(self.kind).is_some_and(|s| s.neutral))
    }

    pub(crate) fn preparing_stamp(&self)->bool {
        self.kind==matron::KIND && self.matron.run.as_ref().is_some_and(|r|r.kind==matron::Action::Stamp && r.phase()==action::Phase::Prepare)
    }
    pub(crate) fn new(site: IVec3, slot: u32, kind: u32, pos: Vec3, menace: u8) -> Self {
        let menace = menace.min(3);
        Self {
            site,
            habitat: None,
            slot,
            kind,
            pos,
            home: pos,
            health: vault_fight::base_health(site, slot, kind) * (1.0 + menace as f32 * 0.4),
            slow: 0.0,
            tell: 0.0,
            phase: 0.0,
            yaw: 0.0,
            velocity: Vec3::ZERO,
            up: Vec3::Y,
            navigation: navigation::Navigation::default(),
            #[cfg(feature = "multiplayer")]
            replica_target: None,
            #[cfg(feature = "multiplayer")]
            creature_tick: 0,
            gait: 0.0,
            recovery: 0.0,
            reaction: enemy_presentation::Reaction::default(),
            motion: enemy_presentation::BodyMotion::default(),
            encounter: encounters::State::new(site, slot, kind, pos),
            cooldown: 2.0,
            sight: 0.0,
            visible: false,
            last_seen: pos,
            alert: 0.0,
            aware: 0.0,
            investigate: None,
            search: 0.0,
            search_step: 0,
            spotted_flash: 0.0,
            glimpsed: false,
            provoked: false,
            flash: 0.0,
            stagger: 0.0,
            knock: Vec3::ZERO,
            charge: 0.0,
            lunge: Vec3::ZERO,
            aim: pos,
            attack: 0,
            attacks: 0,
            poise: 0.0,
            menace,
            affixes: status::Affixes::default(),
            status: status::Status::default(),
            shield: 0.0,
            guard: kind == 5,
            hart: hart::State::default(),
            toll: tollkeeper::State::default(),
            matron: matron::State::default(),
            hen: thunderhen::State::default(),
            engine: engine::State::default(),
            glass: glassweaver::State::default(),
            youth: 0.0,
            burrow: burrowers::Body::default(),
        }
    }
    fn with_affixes(mut self, affixes: status::Affixes) -> Self {
        let base = vault_fight::base_health(self.site, self.slot, self.kind) * (1.0 + self.menace as f32 * 0.4);
        self.affixes = affixes;
        self.health = base * affixes.health_mul();
        self.shield = base * affixes.shield_frac();
        self
    }
    pub fn max_health(&self) -> f32 {
        vault_fight::base_health(self.site, self.slot, self.kind) * (1.0 + self.menace as f32 * 0.4) * self.affixes.health_mul()
    }
    /// Drops the warden's shield once the body is badly hurt. Returns true only
    /// on the transition, so callers can add one shatter effect and sound.
    pub fn check_guard_break(&mut self) -> bool {
        if self.kind == 5
            && self.guard
            && self.health > 0.0
            && self.health <= self.max_health() * 0.5
        {
            self.guard = false;
            self.flash = self.flash.max(0.14);
            return true;
        }
        false
    }

    /// Damage lands on the shield first, then health.
    fn hurt(&mut self, amount: f32) {
        if amount > 0.0 { self.provoked = true; }
        if amount > 0.0 { self.flash = self.flash.max(0.16); }
        if self.kind==matron::KIND {matron::hurt(self,amount);}
        let amount = amount * if self.encounter.exposed > 0.0 { 1.25 } else { 1.0 };
        if self.kind == tollkeeper::KIND { tollkeeper::pressure(self, amount); }
        let amount = amount * if self.kind == 8 { hart::damage_taken(self) } else { 1.0 };
        let absorbed = amount.min(self.shield).max(0.0);
        self.shield -= absorbed;
        self.health -= amount - absorbed;
        self.check_guard_break();
    }
    fn windup(&self) -> f32 {
        if vault_fight::lead(self) {
            vault_fight::WINDUP
        } else if let Some(species) = bestiary::species(self.kind) {
            species.windup
        } else if self.kind == 9 {
            0.95
        } else if self.attack == 2 {
            1.0
        } else {
            0.75
        }
    }
    pub(crate) fn center(&self) -> Vec3 {
        let height = if self.kind == 8 {
            1.65
        } else if self.kind == thunderhen::KIND {
            2.0
        } else if is_boss(self.kind) {
            1.2
        } else if self.kind == 9 {
            1.35
        } else if let Some(species) = bestiary::species(self.kind) {
            species.center_height
        } else if tall(self.kind) {
            0.8
        } else {
            0.45
        };
        let up = if self.up.length_squared() > 0.5 { self.up } else { Vec3::Y };
        self.pos + up * height
    }
    pub fn model(&self, m: &mut Model, time: f32, world: &World) {
        perception::marker(self, m, time);
        if wildlife::passive(self.kind) { wildlife::draw(self, m, time, world); return; }
        if burrowers::is_kind(self.kind) { burrowers::model(self, m, time); return; }
        if field_foes::is_kind(self.kind) { field_foes::model(self, m, time); return; }
        if encounters::disguise(self, m, time) { return; }
        if is_boss(self.kind) {
            bosses::model(self, m, time);
            return;
        }
        let mark = m.mark();
        let wind = enemy_presentation::wind(self);
        let hit = if self.flash > 0.0 {
            [1.0, 0.94, 0.69]
        } else if self.slow > 0.0 {
            [0.35, 0.73, 0.96]
        } else if self.tell > 0.0 {
            [0.95, 0.37, 0.15]
        } else {
            [0.49, 0.32, 0.63]
        };
        if bestiary::has_body(self.kind) {
            bestiary::draw_body(self, m, time, wind, hit);
        } else {
            enemy_presentation::fallback(self, m, time, wind, hit);
        }
        // Body animation ends here; UI markers keep a stable size and position
        // instead of squashing, recoiling and turning with the enemy's torso.
        enemy_presentation::pose(self, m, mark, time, wind);
        if self.tell > 0.0 {
            let c = self.center() + Vec3::Y * 0.7;
            let vertices = [Vec3::new(0.0, 0.15, 0.0), Vec3::new(-0.13, -0.10, 0.0), Vec3::new(0.13, -0.10, 0.0)];
            for i in 0..3 {
                m.beam(c + vertices[i], c + vertices[(i + 1) % 3], 0.05, PALETTE_CORAL, 0.8);
            }
        }
        let maximum = self.max_health();
        if self.health < maximum {
            let y = if self.kind == 9 {
                2.30
            } else if tall(self.kind) {
                1.75
            } else {
                1.15
            };
            m.cuboid(
                self.pos + Vec3::Y * y,
                Vec3::new(0.64, 0.045, 0.035),
                Quat::IDENTITY,
                [0.18, 0.08, 0.08],
                0.0,
            );
            m.cuboid(
                self.pos + Vec3::new(-0.32 + 0.32 * self.health.max(0.0) / maximum, y, -0.024),
                Vec3::new(0.64 * self.health.max(0.0) / maximum, 0.045, 0.015),
                Quat::IDENTITY,
                PALETTE_LEAF,
                0.25,
            );
        }
        enemy_presentation::threat(self, m, wind);
        // Affix motes orbit the body in the affix colour; a ward shows as a
        // faint bubble that thins as it is broken.
        let mid = self.center();
        for (i, affix) in self.affixes.list().iter().enumerate() {
            let a = time * 0.55 + i as f32 * 2.094;
            m.glow_orb(
                mid + Vec3::new(a.cos() * 0.48, 0.08, a.sin() * 0.48),
                0.032,
                affix.color(),
                1.05,
                0.4,
            );
        }
        if self.shield > 0.0 {
            let strength = (self.shield / (self.max_health() * 0.45).max(1.0)).clamp(0.0, 1.0);
            for i in 0..6 {
                let point = |j: usize| {
                    let a = j as f32 * std::f32::consts::TAU / 6.0;
                    mid + Vec3::new(a.cos() * 0.65, a.sin() * 0.65, 0.0)
                };
                m.beam(point(i), point(i + 1), 0.025 + strength * 0.025, status::Affix::Warded.color(), 0.65);
            }
        }
    }
}
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Projectile {
    pub farm: Option<u32>,
    pub pos: Vec3,
    pub velocity: Vec3,
    pub hostile: bool,
    pub spell: u32,
    pub life: f32,
    pub payload: Option<u32>,
    /// Modifiers that wrap the payload cast, not the carrier.
    pub payload_mods: wands::ShotMods,
    pub mods: wands::ShotMods,
    pub bore_budget: u8,
    pub trail_timer: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub flight: Option<trajectory::Flight>,
    /// First surface crossing emits a splash; subsequent substeps in the same
    /// water body must not produce a ring per step.
    pub splashed: bool,
    pub follow: Option<u32>,
}
impl Projectile {
    pub(crate) fn tether(&self) -> Option<&snapline::Tether> {
        self.flight.as_ref()?.snapline.as_ref()
    }

    fn precise_at(&self, position: Vec3) -> glam::DVec3 {
        self.flight.as_ref().map_or(position.as_dvec3(), |flight| flight.position(position))
    }

    fn next_position(&mut self, delta: Vec3) -> Vec3 {
        let velocity = self.velocity;
        self.flight.get_or_insert_with(|| trajectory::Flight::new(velocity))
            .translate(self.pos, delta)
    }

    fn correct_position(&mut self, position: Vec3) {
        self.pos = position;
        if let Some(flight) = self.flight.as_mut() { flight.rebase(position); }
    }

    fn apply_gravity(&mut self, dt: f32) {
        let base = match self.spell {
            5 => 8.5,
            12 => 6.5,
            28 => 7.5,
            _ => 0.0,
        };
        self.velocity.y -= dt * base;
        self.velocity.y -= dt * wands::card_rules::GRAVITY_ACCELERATION * self.mods.gravity;
    }

    pub fn payload_color(&self) -> Option<[f32; 3]> {
        self.payload.map(spell_color)
    }
    pub fn color(&self) -> [f32; 3] {
        if self.hostile {
            [0.92, 0.26, 0.37]
        } else {
            spell_color(self.spell)
        }
    }
}
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Remains {
    pub pos: Vec3,
    pub kind: u32,
    pub life: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub yaw: f32,
}
impl Remains {
    /// A body falls before it fades. Remains never carry collision or AI state.
    pub(crate) fn model(&self, m: &mut Model, time: f32) -> bool {
        if !bestiary::has_body(self.kind) && !matches!(self.kind, 2 | 3 | 9) { return false; }
        let center = bestiary::species(self.kind).map_or(0.7, |s| s.center_height);
        let feet = self.pos - Vec3::Y * center;
        let e = Enemy::new(IVec3::ZERO, 0, self.kind, feet, 0);
        let mark = m.mark();
        if bestiary::has_body(self.kind) {
            bestiary::draw_body(&e, m, time, 0.0, [0.42, 0.35, 0.29]);
        } else { enemy_presentation::fallback(&e, m, time, 0.0, [0.42, 0.35, 0.29]); }
        let age = (0.65 - self.life).max(0.0);
        let t = (age / 0.32).clamp(0.0, 1.0);
        let angle = t * t * (3.0 - 2.0 * t) * 1.35;
        m.rotate_since(mark, feet, Quat::from_rotation_y(self.yaw) * Quat::from_rotation_z(angle));
        // Keep the falling body on the actual floor before the brief fade.
        let scale = m.scale_meters();
        let low = m.vertices[mark..].iter().map(|v| v.pos[1] * scale).fold(feet.y, f32::min);
        let fade = (self.life / 0.18).clamp(0.0, 1.0);
        for v in &mut m.vertices[mark..] {
            v.pos[1] += (feet.y - low) / scale;
            // Contract into the floor only at the end; the fall stays full size.
            let pivot = feet / scale;
            v.pos = (pivot + (Vec3::from_array(v.pos) - pivot) * fade).to_array();
        }
        true
    }
}
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Arc {
    pub a: Vec3,
    pub b: Vec3,
    pub life: f32,
}
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Bloom {
    pub pos: Vec3,
    pub life: f32,
    pulse: f32,
}
#[derive(Clone)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Reward {
    #[cfg_attr(feature="multiplayer", serde(deserialize_with="crate::multiplayer::text::string"))]

    pub name: crate::world::interaction::Text,
    pub items: Vec<(usize, u32)>,
    pub seconds: f32,
    pub discovery: Option<u32>,
}
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Effect {
    pub pos: Vec3,
    pub color: [f32; 3],
    pub radius: f32,
    pub life: f32,
    pub total: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub(crate) impact_normal: Option<Vec3>,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub(crate) family: magic_fx::Family,
}
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyFocus {
    pub kind: u32,
#[cfg_attr(feature="multiplayer", serde(deserialize_with="crate::multiplayer::text::string"))]
    pub name: crate::world::interaction::Text,
    pub health: f32,
    pub maximum: f32,
    pub frozen: bool,
    pub affixes: status::Affixes,
    pub status: status::Status,
    pub shield: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub combat_cue: Option<CombatCue>,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub enum CombatCue {
    Rush,
    Exposed,
}

/// The selected authored objective, resolved to a world-space point for navigation.
/// Bearing uses the game's north convention: +Z is south, -Z is north, +X east.
#[derive(Clone, Debug)]
pub struct QuestMarker {
    pub pos: Vec3,
    pub label: &'static str,
    pub kind: u32,
    pub bearing_deg: f32,
    pub distance_m: f32,
    pub vertical_m: f32,
}

impl View {
    /// Placeable and castable now: right wand tier (and rare progress) and found.
    pub fn can_wield(&self, spell: u32) -> bool {
        wands::fits_instrument(spell, self.loadout.mode)
            && spell_available(spell, self.wand, self.rare_spells) && self.spell_log.grants(spell)
    }
}

#[derive(Clone, Default)]
pub struct View {
    pub hell_biome: Option<&'static str>,
    pub creative: bool,
    /// Connecting melee hits so far; the crosshair flashes when it changes.
    pub melee_landed: u32,
    pub resonance: resonance::State,
    /// Spells the player has found; unfound spells show as locked.
    pub spell_log: discovery::SpellLog,
    pub health: f32,
    pub max_health: f32,
    pub mana: f32,
    pub wand: u32,
    pub spell: u32,
    pub loadout: wands::Loadout,
    /// Every owned wand, for inventory cards.
    pub wand_bank: Vec<wand_items::WandRecord>,
    pub tool_bank: Vec<engine_tools::ToolAssembly>,
    pub active_wand: Option<u16>,
    pub cast_total: f32,
    pub cast_delay_total: f32,
    pub recharge_total: f32,
    pub mana_flash: f32,
    pub concentration_upkeep: Option<f32>,
    /// Native rune phrase bound to the held wand, in canonical text.
    pub rune_phrase: Option<String>,
    /// Rune goggles are worn: draw runes in their element colours.
    pub rune_sight: bool,
    /// The held wand's effective pool; the HUD scale must use this, not the rank formula.
    pub mana_max: f32,
    pub nearby: usize,
    pub defeated: usize,
    pub hurt: f32,
    pub damage_direction: Vec3,
    /// Attacker of the last hit when it came from a creature the HUD can name.
    pub damage_source: Option<(u32, f32)>,
    pub damage_amount: f32,
    pub hit: f32,
    pub rank: u32,
    pub cooldown: f32,
    pub ward: f32,
    pub ward_hits: u32,
    pub hunger: f32,
    pub rations: u32,
    pub weapons: u32,
    pub weapon: u32,
    /// Fitted part materials: head (damage), binding (sweep), grip (speed).
    pub tool_head: u8,
    pub tool_bind: u8,
    pub tool_grip: u8,
    /// Installed modifier bitfield.
    pub tool_mods: u8,
    pub reward: Option<Reward>,
    pub discoveries: u32,
    pub sigils: u32,
    pub heart_restored: bool,
    pub workshop_returned: bool,
    pub workshop_available: bool,
    pub workshop_cleared: bool,
    pub route: String,
    /// Active site modifier (name, one-line description) for the HUD.
    pub site_modifier: Option<(&'static str, &'static str)>,
    /// Local mana pressure; recovery has its own nonlinear scale.
    pub affinity: f32,
    pub location: Option<&'static str>,
    /// Mana poisoning glow, 0..1.
    pub mana_poison: f32,
    /// Absorption health left on top of health.
    pub absorption: f32,
    pub rare_spells: u32,
    pub quest: Option<QuestMarker>,
    pub relic_focus: Option<reliquaries::RelicFocus>,
    pub undercrown_focus: Option<super::interaction::Prompt>,
    pub boss: Option<reliquaries::BossView>,
    pub enemy_focus: Option<EnemyFocus>,
    pub respawn_point: Option<Vec3>,
    pub last_death_pos: Option<Vec3>,
    pub death: Option<death::Death>,
}
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Adventure {
    pub resonance: resonance::State,
    /// Runtime authority sets this from the current player mode each step.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub creative: bool,
    pub health: f32,
    pub mana: f32,
    pub wand: u32,
    pub spell: u32,
    pub loadout: wands::Loadout,
    /// Every wand the player has owned; items reference these by index.
    pub wand_bank: Vec<wand_items::WandRecord>,
    pub tool_bank: Vec<engine_tools::ToolAssembly>,
    pub active_wand: Option<u16>,
    /// Hotbar slot in the player's hand, set by the frame loop before `tick`.
    pub hand_slot: usize,
    pub last_cast: u32,
    /// Aiming selection only; water remains in world storage across saves.
    water_source: Option<IVec3>,
    cast_total: f32,
    cast_delay_total: f32,
    recharge_total: f32,
    mana_flash: f32,
    pub respawn_point: Option<Vec3>,
    pub last_death_pos: Option<Vec3>,
    #[cfg_attr(feature = "multiplayer", serde(default, rename = "hell_spawn"))]
    pub spawn_binding: super::spawn::Binding,
    /// Replicated death screen, including whether the host settled its drops.
    /// Ordinary world checkpoints still reconstruct death from saved health.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    death: Option<death::Death>,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    last_cause: death::Cause,
    terrain_blasts: std::collections::VecDeque<(Vec3, f32)>,
    /// Player shield state; runtime only, rebuilt from the offhand each step.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub shield_guard: guard::Guard,
    /// Set by input each frame: the block button is held with a shield in the offhand.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub shield_raised: bool,
    pub time: f32,
    /// Shared, replicated chamber phase; never mounted as a personal timer.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub manyfold_phase: f32,
    /// Modifier of the site the player stands in; re-derived each step, not saved.
    #[cfg_attr(feature = "multiplayer", serde(skip, default="network_site_modifier"))]
    pub(super) site_modifier: engine_worldgen::sites::modifiers::SiteModifier,
    /// Mana recovery multiplier where the player stands; re-derived each step.
    #[cfg_attr(feature = "multiplayer", serde(skip, default = "network_affinity"))]
    affinity: f32,
    /// Shared local ecology reading, independent of whether audio is enabled.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub location: super::ambience::State,
    /// 1 while mana poisoning bites, fading to 0 after; drives the heart glow.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    mana_poison: f32,
    /// Absorption health soaked before health; progression caps it each step.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub absorption: f32,
    /// Resistance level mirrored from progression each step.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub resist_level: u8,
    /// Effects hits applied this step, drained by progression.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub pending_effects: Vec<(crate::world::progression::effects::Kind, f32, u8)>,
    /// Review captures pin the affinity instead of sampling the field.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub review_affinity: Option<f32>,
    haunt_clock: f32,
    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub tool_debris: Vec<crate::tool_effects::Burst>,
    pub blasts: Vec<crate::tool_effects::Blast>,
    pub splashes: Vec<crate::tool_effects::Splash>,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    viscous_contact: Option<super::BlockId>,
    pub remains: Vec<Remains>,
    pub arcs: Vec<Arc>,
    pub blooms: Vec<Bloom>,
    // Audio is an event stream. Repeating it in state images replays old cues
    // and grows headless-host snapshots indefinitely.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub sounds: Vec<Sound>,
    /// Audible events for enemy hearing, drained once per enemy pass.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    noises: Vec<perception::Noise>,
    /// Countdown to the player's next body noise (steps, casting).
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    body_noise: f32,
    /// Loudest `make_noise` level since the last enemy pass, placed at the player.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pending_noise: f32,
    pub ward: f32,
    pub ward_hits: u32,
    pub hunger: f32,
    /// Session choice, not saved: a save-format field needs its own version bump.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub hunger_mode: hunger::HungerMode,
    /// Saved with the world from save version 40. Changed through `set_difficulty`.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub difficulty: difficulty::Difficulty,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    night: night::State,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    burrowers: burrowers::State,
    /// Chunks of ground held or thrown by creatures. Runtime only.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    hurled: hurled::State,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    safety: spawn_safety::State,
    /// Last unseen attacker: (bestiary kind, health fraction). Runtime only.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub damage_source: Option<(u32, f32)>,
    /// Seconds the attacker plate stays up after a named hit. Runtime only.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    damage_source_t: f32,
    pub rations: u32,
    /// Bit 0 is a copper sword; bit 1 is a steel sword.
    pub weapons: u32,
    pub weapon: u32,
    /// Tinkers-style part materials, each 0–5: head (damage), binding (sweep),
    /// grip (speed). See `adventure::tinkering` for the material table.
    pub tool_head: u8,
    pub tool_bind: u8,
    pub tool_grip: u8,
    /// Optional one-shot modifiers: jagged, flux, core, moss, luck.
    pub tool_mods: u8,
    /// Held weapon's melee factors, refreshed each step; never saved.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub blade: blade::Blade,
    /// Counts melee hits that connected; the viewmodel reads it for hit-stop and never saves it.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub melee_landed: u32,
    reward: Option<Reward>,
    pub discoveries: u32,
    discoveries_ready: bool,
    pub rare_spells: u32,
    /// Spells the player has found. Persisted from save version 19.
    pub spell_log: discovery::SpellLog,
    /// Study grants are derived from the mounted player's progression.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub(crate) study_lessons: u8,
    #[cfg(feature = "multiplayer")]
    #[serde(skip)]
    replica_projectile_lead: f32,
    /// Seed for the Chaotic chassis; changes each time the deck reloads.
    /// Not saved: a shuffle is a fresh roll after loading.
    shuffle_seed: u32,
    sigils: u32,
    route: String,
    stations: Vec<quest::Station>,
    puzzle_cd: f32,
    relic_focus: Option<reliquaries::RelicFocus>,
    undercrown_focus: Option<super::interaction::Prompt>,
    boss_view: Option<reliquaries::BossView>,
    enemy_focus: Option<EnemyFocus>,
    reliquaries: Vec<reliquaries::Reliquary>,
    // Each multiplayer player surveys their own position. Realm exchange keeps
    // this runtime state paired with that realm's discovery objects.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    relic_discovery: reliquaries::Discovery,
    wells: Vec<rare_magic::Well>,
    pub particles: Vec<spells::VoxelParticle>,
    /// Underground zone bending particle behaviour; neutral on the surface.
    #[cfg_attr(feature = "multiplayer", serde(skip, default="network_zone_fx"))]
    pub zone_fx: crate::world::zones::Fx,
    pub lasers: Vec<spells::LaserBeam>,
    pub crosses: Vec<spells::PlasmaCross>,
    pub dirty_voxels: Vec<IVec3>,
    melee_buffer: f32,
    hazards: Vec<tactics::Hazard>,
    strike_delay: f32,
    cast_pose: f32,
    // One bit per defeated guardian; bit 8 records the one-time clear reward.
    pub records: HashMap<IVec3, u32>,
    pub(crate) wildlife: wildlife::State,
    farm_mastery: usize,
    farm_shots_restored: bool,
    /// Session-only: 0 not yet seen, 1 wand not in hand when play began, 2 prompt given.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    wand_tip: u8,
    beacon_mend_cd: f32,
    tollkeepers: HashMap<IVec3, tollkeeper::Saved>,
    engines: HashMap<IVec3, engine::Saved>,
    pub companion: Option<growth::Companion>,
    #[cfg(feature = "multiplayer")]
    #[serde(skip)]
    pub(crate) remote_companions: Vec<growth::Companion>,
    #[cfg(feature = "multiplayer")]
    #[serde(skip)]
    companion_replica_target: Option<Vec3>,
    cast_cd: f32,
    queued_cast: bool,
    queued_cycle: bool,
    #[cfg_attr(feature = "multiplayer", serde(default, skip_serializing_if = "Option::is_none"))]
    concentration: Option<concentration::Channel>,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub circles: circles::State,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    rune_cast: rune_cast::State,
    /// Rune goggles are worn; refreshed each update from the head slot.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    rune_sight: bool,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    recall: Option<fieldcraft::Recall>,
    circle_cast_latched: bool,
    melee_cd: f32,
    invulnerable: f32,
    spawn_timer: f32,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    encounter_placement: encounter_placement::Placement,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    modifier_probe: modifiers::Probe,
    hurt: f32,
    damage_direction: Vec3,
    damage_amount: f32,
    pub(crate) hit: f32,
    combo: u32,
    combo_time: f32,
}
impl Default for Adventure {
    fn default() -> Self {
        Self {
            resonance: resonance::State::default(),
            creative: false,
            health: 100.0,
            mana: 100.0,
            wand: 0,
            spell: 0,
            loadout: Default::default(),
            wand_bank: Vec::new(),
            tool_bank: Vec::new(),
            active_wand: None,
            hand_slot: 0,
            last_cast: 0,
            water_source: None,
            cast_total: 0.28,
            cast_delay_total: 0.28,
            recharge_total: 0.0,
            mana_flash: 0.0,
            respawn_point: None,
            spawn_binding: Default::default(),
            last_death_pos: None,
            death: None,
            last_cause: death::Cause::Wounds,
            terrain_blasts: Default::default(),
            shield_guard: Default::default(),
            shield_raised: false,
            time: 0.0,
            manyfold_phase: 0.0,
            site_modifier: engine_worldgen::sites::modifiers::SiteModifier::None,
            affinity: 1.0,
            location: Default::default(),
            mana_poison: 0.0,
            absorption: 0.0,
            resist_level: 0,
            pending_effects: Vec::new(),
            review_affinity: None,
            haunt_clock: 0.0,
            enemies: Vec::new(),
            projectiles: Vec::new(),
            effects: Vec::new(),
            tool_debris: Vec::new(),
            blasts: Vec::new(),
            splashes: Vec::new(),
            viscous_contact: None,
            remains: Vec::new(),
            arcs: Vec::new(),
            blooms: Vec::new(),
            sounds: Vec::new(),
            noises: Vec::new(),
            body_noise: 0.0,
            pending_noise: 0.0,
            ward: 0.0,
            ward_hits: 0,
            hunger: 100.0,
            hunger_mode: hunger::HungerMode::default(),
            difficulty: difficulty::Difficulty::default(),
            night: night::State::default(),
            burrowers: burrowers::State::default(),
            hurled: hurled::State::default(),
            safety: spawn_safety::State::default(),
            damage_source: None,
            damage_source_t: 0.0,
            rations: 0,
            weapons: 0,
            weapon: 0,
            tool_head: 0,
            tool_bind: 0,
            tool_grip: 0,
            tool_mods: 0,
            blade: blade::Blade::default(),
            melee_landed: 0,
            reward: None,
            discoveries: 0,
            discoveries_ready: false,
            rare_spells: 0,
            spell_log: discovery::SpellLog::default(),
            study_lessons: 0,
            #[cfg(feature = "multiplayer")]
            replica_projectile_lead: 0.0,
            shuffle_seed: 0,
            sigils: 0,
            route: String::new(),
            stations: Vec::new(),
            puzzle_cd: 0.0,
            relic_focus: None,
            undercrown_focus: None,
            boss_view: None,
            enemy_focus: None,
            reliquaries: Vec::new(),
            relic_discovery: reliquaries::Discovery::default(),
            wells: Vec::new(),
            particles: Vec::new(),
            zone_fx: crate::world::zones::Fx::NEUTRAL,
            lasers: Vec::new(),
            crosses: Vec::new(),
            dirty_voxels: Vec::new(),
            melee_buffer: 0.0,
            hazards: Vec::new(),
            strike_delay: 0.0,
            cast_pose: 0.0,
            records: HashMap::new(),
            wildlife: wildlife::State::default(),
            farm_mastery: 0,
            farm_shots_restored: false,
            wand_tip: 0,
            beacon_mend_cd: 0.0,
            tollkeepers: HashMap::new(),
            engines: HashMap::new(),
            companion: None,
            #[cfg(feature = "multiplayer")]
            remote_companions: Vec::new(),
            #[cfg(feature = "multiplayer")]
            companion_replica_target: None,
            cast_cd: 0.0,
            queued_cast: false,
            queued_cycle: false,
            concentration: None,
            circles: circles::State::default(),
            rune_cast: rune_cast::State::default(),
            rune_sight: false,
            recall: None,
            circle_cast_latched: false,
            melee_cd: 0.0,
            invulnerable: 2.0,
            spawn_timer: 0.0,
            encounter_placement: encounter_placement::Placement::default(),
            modifier_probe: modifiers::Probe::default(),
            hurt: 0.0,
            damage_direction: Vec3::ZERO,
            damage_amount: 0.0,
            hit: 0.0,
            combo: 0,
            combo_time: 0.0,
        }
    }
}
fn free(world: &World, pos: Vec3, kind: u32) -> bool {
    let (min, max) = body_bounds(pos, kind, world.voxel_scale.meters());
    !collides_world(world, min, max)
}

fn body_bounds(pos: Vec3, kind: u32, scale: f32) -> (Vec3, Vec3) {
    let (h, width) = if kind==matron::KIND {
        (2.5,1.1)
    } else if kind == thunderhen::KIND {
        (3.4, 1.3)
    } else if kind == engine::KIND {
        (1.9, 0.95)
    } else if kind == 8 {
        (2.9, 1.0)
    } else if is_boss(kind) {
        (2.6, 0.52)
    } else if kind == 9 {
        (2.1, 0.64)
    } else if let Some(species) = bestiary::species(kind) {
        (species.body_height, species.body_width * 0.5)
    } else if tall(kind) {
        (1.6, 0.28)
    } else {
        (0.85, 0.28)
    };
    (
        (pos - Vec3::new(width, 0.0, width)) / scale,
        (pos + Vec3::new(width, h, width)) / scale,
    )
}
pub(super) fn line(world: &World, a: Vec3, b: Vec3) -> bool {
    (b - a).length_squared() <= 128.0 * 128.0 && trace::solid(world, a, b).is_none()
}

impl Adventure {
    pub fn rank(&self) -> u32 {
        mastery(self.records.values().filter(|n| **n & 256 != 0).count() + self.farm_mastery)
    }
    pub fn max_health(&self) -> f32 {
        100.0 + self.rank() as f32 * 20.0
    }
    /// The held wand's mana pool; plain crafted wands keep the rank formula.
    pub fn max_mana(&self) -> f32 {
        wand_items::stats_for(self.wand, &self.loadout, self.rank()).mana_max
    }
    pub fn swing(&self) -> f32 {
        (self.melee_cd / self.swing_duration()).clamp(0.0, 1.0)
    }
    /// Seconds from wind-up to recovery; shared by gameplay and the held model.
    pub fn swing_duration(&self) -> f32 {
        (self.stroke().duration / self.blade.speed).max(0.16)
    }
    fn stroke(&self) -> &'static crate::player::viewmodel::clips::Clip {
        crate::player::viewmodel::clips::Name::stroke(self.backswing(), self.finisher()).clip()
    }
    /// Recomputes `blade` from the hotbar hand: an assembled tool, else the legacy forge fields.
    pub(super) fn refresh_blade(&mut self, world:&World) {
        let held=match world.inventory.hotbar.get(self.hand_slot).copied().flatten().map(|s|s.item) {
            Some(super::inventory::Item::Assembled{id,..})=>self.tool_bank.get(id as usize),
            _=>None,
        };
        self.blade=match held {
            Some(tool)=>blade::Blade::assembled(tool,self.tool_mods),
            None=>blade::Blade::legacy(self.tool_head,self.tool_bind,self.tool_grip,self.tool_mods),
        };
    }
    /// Guard gear from an unbroken, blocking-capable offhand assembly.
    pub(super) fn shield_gear(&self, world: &World) -> Option<guard::Gear> {
        let Some(super::inventory::Item::Assembled { id, .. }) = world.inventory.offhand.map(|s| s.item) else { return None };
        let tool = self.tool_bank.get(id as usize)?;
        if tool.is_broken() || !tool.capabilities().ok()?.blocking { return None; }
        let stats = tool.stats().ok()?;
        Some(guard::Gear {
            block: (0.7 + 0.05 * f32::from(stats.mining_level)).min(0.9),
            thorns: engine_tools::modifiers::stat(tool, "thorns", 0.0).max(0.0),
            capacity: (stats.durability as f32 * 0.1).clamp(25.0, 120.0),
        })
    }
    /// Queues a terrain crater at `at` metres; false when the queue is full.
    pub fn queue_blast(&mut self, at: Vec3, radius: f32) -> bool {
        if self.terrain_blasts.len() >= 4 { return false; }
        self.terrain_blasts.push_back((at, radius));
        true
    }
    pub(super) fn melee_base(&self, world:&World)->f32 {
        if let Some(super::inventory::Item::Assembled{id,..})=world.inventory.hotbar.get(self.hand_slot).copied().flatten().map(|s|s.item) {
            return self.tool_bank.get(id as usize).map_or(1.0,super::tool_workshop::attack_damage);
        }
        super::inventory::weapon_damage(self.weapon)
    }
    /// Seconds from swing start to contact, at the stroke clip's impact marker.
    pub fn swing_impact_time(&self) -> f32 {
        self.swing_duration() * self.stroke().impact_fraction()
    }
    /// A loud act (mining, blasting) that `hears` hunters pick up; 0..1.
    pub fn make_noise(&mut self, level: f32) {
        self.burrowers.make_noise(level);
        self.pending_noise = self.pending_noise.max(level);
    }

    pub fn casting(&self) -> bool {
        self.cast_pose > 0.0
    }
    pub fn cast_motion(&self) -> f32 {
        self.cast_pose / 0.4
    }
    pub fn finisher(&self) -> bool {
        self.combo == 3
    }
    pub fn backswing(&self) -> bool {
        self.combo == 2
    }
    pub fn view(&self) -> View {
        View {
            hell_biome: None,
            creative: self.creative,
            melee_landed: self.melee_landed,
            resonance: self.resonance,
            health: self.health,
            max_health: self.max_health(),
            mana: self.mana,
            wand: self.wand,
            spell: self.spell,
            loadout: self.loadout,
            wand_bank: self.wand_bank.clone(),
            tool_bank: self.tool_bank.clone(),
            active_wand: self.active_wand,
            cast_total: self.cast_total,
            cast_delay_total: self.cast_delay_total,
            recharge_total: self.recharge_total,
            mana_flash: self.mana_flash,
            concentration_upkeep: self.concentration.as_ref().map(concentration::Channel::upkeep)
                .or_else(|| self.circles.held_upkeep()),
            rune_phrase: self.rune_cast.phrase(),
            rune_sight: self.rune_sight,
            mana_max: self.max_mana(),
            nearby: self.enemies.len(),
            defeated: self.records.values().filter(|n| **n & 256 != 0).count(),
            hurt: self.hurt,
            damage_direction: self.damage_direction,
            damage_source: self.damage_source.filter(|_| self.damage_source_t > 0.0),
            damage_amount: self.damage_amount,
            hit: self.hit,
            rank: self.rank(),
            cooldown: self.cast_cd,
            ward: self.ward,
            ward_hits: self.ward_hits,
            hunger: self.hunger,
            rations: self.rations,
            weapons: self.weapons,
            weapon: self.weapon,
            tool_head: self.tool_head,
            tool_bind: self.tool_bind,
            tool_grip: self.tool_grip,
            tool_mods: self.tool_mods,
            reward: self.reward.clone(),
            discoveries: self.discoveries,
            rare_spells: self.available_rare_spells(),
            spell_log: if self.creative { discovery::SpellLog::all() } else { self.effective_spell_log() },
            sigils: self.sigils,
            heart_restored: self.heart_restored(),
            workshop_returned: super::workshop::returned(self),
            workshop_available: false,
            workshop_cleared: super::workshop::cleared(self),
            route: self.route.clone(),
            site_modifier: modifiers::hud(self.site_modifier),
            affinity: self.affinity,
            location: self.location.reading().map(|s| s.place.name),
            mana_poison: self.mana_poison,
            absorption: self.absorption,
            quest: None,
            relic_focus: self.relic_focus,
            undercrown_focus: self.undercrown_focus,
            boss: self.boss_view.clone(),
            enemy_focus: self.enemy_focus,
            respawn_point: self.respawn_point,
            last_death_pos: self.last_death_pos,
            death: self.death,
        }
    }
    pub fn owns_weapon(&self, tier: u32) -> bool {
        (1..=5).contains(&tier) && self.weapons & (1 << (tier - 1)) != 0
    }
    pub fn add_weapon(&mut self, tier: u32) -> bool {
        if !(1..=5).contains(&tier) || self.owns_weapon(tier) {
            return false;
        }
        self.weapons |= 1 << (tier - 1);
        true
    }
    pub fn select_weapon(&mut self, tier: u32) -> bool {
        if tier == 0 || self.owns_weapon(tier) {
            self.weapon = tier;
            true
        } else {
            false
        }
    }
    /// Fits a material into one slot. Save bytes are validated on load, and
    /// callers only reach this after the resource cost has been paid.
    pub fn fit_part(&mut self, part: tinkering::Part, material: tinkering::Material) {
        match part {
            tinkering::Part::Head => self.tool_head = material.as_u8(),
            tinkering::Part::Binding => self.tool_bind = material.as_u8(),
            tinkering::Part::Grip => self.tool_grip = material.as_u8(),
        }
    }
    /// Adds or removes a known modifier bit. Returns the new installed state.
    pub fn toggle_modifier(&mut self, bit: u8) -> bool {
        if tinkering::modifier(bit).is_none() {
            return false;
        }
        self.tool_mods ^= bit;
        self.tool_mods &= 0x1F;
        self.tool_mods & bit != 0
    }
    /// Returns every slot to neutral copper. Installed modifiers are retained.
    pub fn clear_fittings(&mut self) {
        self.tool_head = 0;
        self.tool_bind = 0;
        self.tool_grip = 0;
    }
    pub fn has_modifier(&self, bit: u8) -> bool {
        self.tool_mods & bit != 0
    }
    fn effect(&mut self, pos: Vec3, color: [f32; 3], radius: f32) {
        if self.effects.len() < 64 {
            self.effects.push(Effect {
                pos,
                color,
                radius,
                life: 0.5,
                total: 0.5,
                impact_normal: None,
                family: magic_fx::Family::Plain,
            });
        }
    }
    /// Sets the world's difficulty and the hunger policy that goes with it.
    pub fn set_difficulty(&mut self, level: difficulty::Difficulty) {
        self.difficulty = level;
        self.hunger_mode = level.hunger_mode();
    }
    fn damage(&mut self, amount: f32, direction: Vec3) {
        if self.creative { return; }
        let amount = amount * self.resonance.incoming() * self.difficulty.incoming_damage();
        if amount <= 0.0 {
            return;
        }
        if self.invulnerable <= 0.0 {
            let blocked = self.ward > 0.0 && self.ward_hits > 0;
            if blocked {
                self.ward_hits -= 1;
                if self.ward_hits == 0 {
                    self.ward = 0.0;
                }
            }
            let taken = self.soak(amount * if blocked { 0.35 } else { 1.0 });
            self.health = (self.health - taken).max(0.0);
            self.last_cause = death::Cause::Wounds;
            sound(
                &mut self.sounds,
                if blocked { Cue::Clang } else { Cue::Hurt },
                None,
                0.85,
            );
            self.invulnerable = 0.65;
            self.hurt = 0.7;
            self.damage_direction = direction.normalize_or_zero();
            self.damage_amount = amount * if blocked { 0.35 } else { 1.0 };
        }
    }
    pub fn hell_ashfall(&self, seed: u32) -> bool {
        (self.time as u32 + seed % 40) % 40 >= 32
    }
    pub fn hell_ashfall_at(&self, world: &World, feet: Vec3) -> bool {
        let cfg = world.sample_config();
        cfg.hell_ecology()
            && engine_worldgen::nether::biome(&cfg, feet.x, feet.z)
                .is_none_or(|biome| biome == engine_worldgen::nether::Biome::AshBarrens)
            && self.hell_ashfall(world.seed)
    }
    fn update(
        &mut self,
        world: &mut World,
        player: &mut Player,
        dt: f32,
        cast: bool,
        cycle: bool,
        melee: bool,
    ) -> bool {
        // Render frames can occur between fixed simulation steps. Preserve a
        // short tap until the next step instead of silently dropping it.
        self.queued_cast |= cast;
        self.queued_cycle |= cycle;
        let dt = dt.clamp(0.0, 0.1);
        if dt <= 0.0 {
            return false;
        }
        let mut update_trace = crate::gameplay_trace::Trace::new();
        let alive = self.death.is_none();
        self.update_recall(world, alive, cast);
        let holding_cast = cast;
        if !holding_cast || !alive || cycle || self.hurt > 0.0 {
            self.concentration = None;
        }
        let cast = std::mem::take(&mut self.queued_cast) && alive;
        let cycle = std::mem::take(&mut self.queued_cycle) && alive;
        let melee = melee && alive;
        self.refresh_blade(world);
        let scale = world.voxel_scale.meters();
        self.restore_discoveries(world);
        if !self.can_wield(self.spell) {
            self.spell = 0;
        }
        self.puzzle_cd = (self.puzzle_cd - dt).max(0.0);
        #[cfg(feature="multiplayer")]
        let shared_tick=world.multiplayer.as_ref().is_none_or(|c|c.shared_tick);
        #[cfg(not(feature="multiplayer"))]
        let shared_tick=true;
        if shared_tick {
            world.undercrown.tick(dt);
            super::undercrown::announce_room(world, player.pos * world.voxel_scale.meters());
            self.manyfold_phase = (self.manyfold_phase + dt).rem_euclid(9.0);
        }
        for (item, amount) in world.factory.take_refunds() {
            world.expedition.add(item, amount);
        }
        for cell in world.factory.take_meltdowns() {
            let half = crate::world::factory::step(world) as f32 * 0.5;
            let at = (cell.as_vec3() + Vec3::splat(half)) * scale;
            let eye_m = player.eye(world.voxel_scale) * scale;
            self.reactor_meltdown(world, at, eye_m);
        }
        self.melee_buffer = (self.melee_buffer - dt).max(0.0);
        if melee && self.melee_cd <= 0.14 {
            self.melee_buffer = 0.14;
        }
        self.cast_pose = (self.cast_pose - dt).max(0.0);
        self.ward = (self.ward - dt).max(0.0);
        if self.ward == 0.0 {
            self.ward_hits = 0;
        }
        if let Some(reward) = &mut self.reward {
            reward.seconds -= dt;
            if reward.seconds <= 0.0 {
                self.reward = None;
            }
        }
        for remains in &mut self.remains {
            remains.life -= dt;
        }
        self.remains.retain(|r| r.life > 0.0);
        for arc in &mut self.arcs {
            arc.life -= dt;
        }
        self.arcs.retain(|a| a.life > 0.0);
        let strike_now = self.strike_delay > 0.0 && self.strike_delay <= dt;
        self.strike_delay = (self.strike_delay - dt).max(0.0);
        let previous_second = self.time as u32;
        self.time = (self.time + dt) % 10000.0;
        if alive && !self.creative && previous_second != self.time as u32 && world.sample_config().hell_ecology() {
            use engine_worldgen::nether::{Biome, biome};
            let feet = player.pos * scale;
            let region = biome(&world.sample_config(), feet.x, feet.z);
            let phase = (self.time as u32 + world.seed % 40) % 40;
            let ash_region = region.is_none_or(|biome| biome == Biome::AshBarrens);
            if phase == 28 && ash_region {
                world.expedition.say("Hot ash · 4 s · Take cover");
                sound(&mut self.sounds, Cue::Warning, None, 0.5);
            }
            if phase % 2 == 0 && !world.progression.effects.fire_immune() {
                if ash_region && self.hell_ashfall(world.seed)
                    && super::raycast_block(world, player.eye(world.voxel_scale), Vec3::Y, 18.0 / scale).is_none() {
                    self.environment_damage(3.0, death::Cause::Contact(super::BlockId::Ash));
                } else if region == Some(Biome::CinderRifts) {
                    let hot_bank = [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z].into_iter().any(|offset| {
                        let cell = ((feet + offset * 1.75 - Vec3::Y * 0.5) / scale).floor().as_ivec3();
                        world.get(cell) == super::BlockId::Lava
                    });
                    if hot_bank {
                        world.expedition.say("Hot bank · Back away");
                        self.environment_damage(2.0, death::Cause::Contact(super::BlockId::Lava));
                    }
                }
            }
        }
        self.cast_cd = (self.cast_cd - dt).max(0.0);
        self.mana_flash = (self.mana_flash - dt).max(0.0);
        self.melee_cd = (self.melee_cd - dt).max(0.0);
        self.invulnerable = (self.invulnerable - dt).max(0.0);
        self.hurt = (self.hurt - dt).max(0.0);
        self.damage_source_t = (self.damage_source_t - dt).max(0.0);
        let rank = self.rank();
        // Refills cap at the held wand's pool, not the plain-wand formula.
        let mana_cap = self.max_mana();
        self.hit = (self.hit - dt).max(0.0);
        self.combo_time = (self.combo_time - dt).max(0.0);
        if self.combo_time == 0.0 && !strike_now && self.melee_cd == 0.0 {
            self.combo = 0;
        }
        self.resonance_tick(player.pos * scale, dt);
        let eye_m = player.precise_eye(world.voxel_scale) * f64::from(scale);
        update_trace.mark("adventure-state");
        let shielded = super::ambience::shielded(world, player, eye_m);
        self.location.update(world, eye_m.as_vec3(), shielded);
        self.affinity_step(self.location.density(world.seed, player.pos * scale), dt);
        // One maintained spell per caster, and maintaining it pauses recovery, whether it
        // is a channel (Aegis, Plasma) or a sustained circle.
        if self.concentration.is_none() && !self.circles.sustaining() {
            self.mana = (self.mana + dt * super::ambience::mana_recovery(self.affinity) * ((8.0 + rank as f32 * 1.5) * wand_items::charge_scale(&self.loadout)
                + self.resonance.stacks(4) * 1.5 + hedrons::supply(world, player.pos * scale, None))).min(self.max_mana());
        }
        // Hunger drains slowly; adequate food restores health only when safe.
        update_trace.mark("adventure-ambience");
        world.inventory.tick(dt);
        let armor_reduction = world.inventory.armor_reduction();
        let shield = self.shield_gear(world);
        let raised = std::mem::take(&mut self.shield_raised);
        self.shield_guard.tick(shield.as_ref(), raised, dt);
        let sprinting = player.sprinting && player.vel.length_squared() > 0.25;
        let drain = hunger::drain_per_second(self.hunger_mode, sprinting, self.creative || player.flying);
        self.hunger = (self.hunger - world.progression.absorb_drain(dt * drain)).max(0.0);
        if alive && self.health > 0.0 && self.hunger >= 75.0 && self.health < self.max_health() && self.hurt <= 0.0 {
            self.health = (self.health + dt * 0.36 * super::ambience::health_recovery(self.affinity)).min(self.max_health());
        } else if self.hunger <= 0.0 && !self.creative && !player.flying {
            world.inventory.starve_timer += dt;
            if world.inventory.starve_timer >= 3.2 {
                world.inventory.starve_timer = 0.0;
                if self.health > 10.0 {
                    self.health = (self.health - 4.0).max(10.0);
                    self.hurt = self.hurt.max(0.45);
                    self.damage_direction = Vec3::ZERO;
                    self.damage_amount = 4.0;
                    sound(&mut self.sounds, Cue::Hurt, None, 0.55);
                    world
                        .expedition
                        .say("Starving — eat a fungal ration or cooked mushroom to recover.");
                }
            }
        } else {
            world.inventory.starve_timer = 0.0;
        }
        let eye = player.eye(world.voxel_scale) * scale;

        let forward = player.forward_dir();
        let antimagic = aberrations::antimagic(&self.enemies, world, eye, self.time);
        self.prepare_wand();
        if cycle && self.wand > 0 {
            self.advance_wand();
        }
        let was_concentrating = self.concentration.is_some();
        self.sustain_concentration(world, eye, forward, dt, antimagic);
        self.rune_sight = world.inventory.rune_sight();
        self.tick_circles(world, player.pos * world.voxel_scale.meters(), dt);
        self.tick_rune_cast();
        hedrons::notice(world, player.pos * world.voxel_scale.meters());
        if cast && !was_concentrating && self.cast_cd <= 0.0 && !antimagic {
            self.cast_wand(world, player, eye, forward);
        }
        // Echo step changes the current-frame collision and aiming origin.
        let eye = player.eye(world.voxel_scale) * scale;
        let feet = player.pos * scale;
        modifiers::step(self, world, player, feet, dt);
        // Drain surface-contact events once. The splash uses the exposed top of
        // the local water column, not the player's feet, so partial cells and
        // cave pools put the ring on the visible surface.
        let water_impact = std::mem::take(&mut player.water_impact);
        let water_exit = std::mem::take(&mut player.water_exit);
        if (water_impact > 0.25 || water_exit)
            && let Some(surface) = world.water_surface_at(feet + Vec3::Y * (0.35 * scale)) {
                let strength = (water_impact / 7.0).clamp(0.22, 1.0)
                    * if water_exit && water_impact <= 0.25 {
                        0.58
                    } else {
                        1.0
                    };
                self.splash(surface, strength);
                world.disturb_water_surface(surface, strength);
            }
        let liquid = world.get(player.pos.floor().as_ivec3());
        update_trace.mark("adventure-contact");
        if let Some(spray) = spells::Spray::viscous_liquid(liquid) {
            if self.viscous_contact != Some(liquid) {
                let surface = world.viscous_surface_at(feet, liquid);
                // Wait until the feet cross the inset surface of this cell.
                if surface.is_none_or(|surface| feet.y <= surface.y) {
                    self.viscous_contact = Some(liquid);
                    if let Some(surface) = surface && surface.y - feet.y <= scale * 1.5 {
                        let strength = (player.vel.length() / 7.0).clamp(0.32, 1.0);
                        self.splash_droplets(surface, strength, spray, Vec3::ZERO);
                    }
                }
            }
        } else {
            self.viscous_contact = None;
        }
        let (player_min, player_max) =
            super::player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
        let player_min = player_min * scale - Vec3::splat(0.10);
        let player_max = player_max * scale + Vec3::splat(0.10);
        // A committed swing owns the button until recovery, even if the target
        // moves away. Otherwise holding attack unexpectedly excavates terrain.
        let mut consumed = self.melee_cd > 0.0 || strike_now;
        #[cfg(feature = "multiplayer")]
        let player_strike = self.player_melee(world, eye, forward, strike_now, rank);
        #[cfg(not(feature = "multiplayer"))]
        let player_strike = false;
        consumed |= player_strike;
        if (self.melee_buffer > 0.0 || strike_now) && !player_strike {
            let target = self
                .enemies
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    let target = if e.kind == engine::KIND { engine::melee_point(e, eye, forward) } else if burrowers::is_kind(e.kind) { burrowers::melee_point(e, eye, forward) } else { e.center() };
                    let to_enemy = target - eye;
                    let aim_distance = to_enemy.dot(forward);
                    let off_aim = (to_enemy - forward * aim_distance).length();
                    let reach = MELEE_REACH
                        + if self.weapon == 0 && !self.blade.armed {
                            0.0
                        } else {
                            ARMED_REACH_BONUS
                        };
                    let sweep = (MELEE_SWEEP_BASE + aim_distance * MELEE_SWEEP_GROWTH)
                        .min(MELEE_SWEEP_MAX)
                        * self.blade.sweep;
                    (e.health > 0.0
                        && aim_distance > 0.0
                        && aim_distance < reach
                        && off_aim < sweep
                        && line(world, eye, target))
                    .then_some((i, aim_distance))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, _)) = target {
                consumed = true;
                // Resolve an overdue hit before starting another combo step.
                if self.melee_buffer > 0.0 && self.melee_cd <= 0.0 && !strike_now {
                    self.melee_buffer = 0.0;
                    self.combo = (self.combo % 3) + 1;
                    self.combo_time = 1.2;
                    let finisher = self.combo == 3;
                    self.melee_cd = self.swing_duration();
                    self.strike_delay = self.swing_impact_time();
                    sound(
                        &mut self.sounds,
                        Cue::Swing,
                        None,
                        if finisher { 0.9 } else { 0.65 },
                    );
                }
                if strike_now {
                    let finisher = self.combo == 3;
                    self.hit = 0.18;
                    let base = self.melee_base(world);
                    let e = &mut self.enemies[i];
                    let shatter = e.slow > 0.0;
                    let shield = (e.kind == 5
                        && e.guard
                        && !shatter
                        && forward.dot(Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos())) < -0.3)
                        || (!shatter && (tollkeeper::guarded(e, forward) || engine::guarded(e, forward)));
                    let damage = (if finisher { base * 1.7 } else { base }
                        + rank as f32 * 3.0
                        + if shatter { 18.0 } else { 0.0 }
                        + self.blade.hit_damage)
                        * self.blade.damage
                        * (1.0 + self.resonance.stacks(0) * 0.08)
                        * world.progression.effects.melee_factor();
                    // Sneak attack: an enemy that never noticed you takes double.
                    let sneak = perception::unaware(e);
                    e.health -= damage * if e.encounter.exposed > 0.0 { 1.25 } else { 1.0 }
                        * if sneak { 2.0 } else { 1.0 }
                        * if shield { 0.35 } else if e.kind == 8 { hart::damage_taken(e) } else { 1.0 };
                    perception::struck(e, feet);
                    if e.kind == tollkeeper::KIND { tollkeeper::pressure(e, damage); }
                    if e.kind == engine::KIND {
                        let point = engine::melee_point(e, eye, forward);
                        engine::component_hit(e, point, damage, self.weapon == 4);
                    }
                    if let Some(element) = self.blade.edge {
                        let affixes = e.affixes;
                        let reaction = e.status.apply(element, affixes);
                        // Frost reactions return their chill separately; without
                        // this the Rime edge would apply wetness but never slow.
                        e.slow = e.slow.max(reaction.chill);
                    }
                    if self.weapon == 4 && !shatter {
                        e.slow = 2.5;
                    }
                    if self.weapon == 3 && finisher {
                        e.health -= 12.0;
                        self.mana = (self.mana + 8.0).min(mana_cap);
                    }
                    e.flash = 0.16;
                    let handled = vault_fight::handles_hit(e, finisher || shatter || self.weapon == 4);
                    if self.blade.hit_heal > 0.0 {
                        self.health = (self.health + self.blade.hit_heal).min(100.0 + rank as f32 * 20.0);
                    }
                    if self.blade.hit_mana > 0.0 {
                        self.mana = (self.mana + self.blade.hit_mana).min(mana_cap);
                    }
                    if !handled {
                        e.stagger = if finisher || shatter { 0.55 } else { 0.20 }
                            + if finisher { self.blade.finisher_stagger } else { 0.0 };
                        if !is_boss(e.kind) || shatter && e.poise <= 0.0 {
                            e.tell = 0.0;
                            if is_boss(e.kind) { e.poise = 3.0; }
                        }
                        e.charge = 0.0;
                        if !is_boss(e.kind) { e.cooldown = e.cooldown.max(0.65); }
                        e.knock = forward * if finisher { 5.4 } else if self.weapon > 0 { 3.1 } else { 2.6 };
                        if sneak { e.stagger += 0.6; }
                    }
                    if shatter {
                        e.slow = 0.0;
                    }
                    let p = self.enemies[i].center();
                    self.melee_landed = self.melee_landed.wrapping_add(1);
                    self.noises.push(perception::Noise { pos: p, radius: perception::MELEE_M });
                    if sneak {
                        sound(&mut self.sounds, Cue::Shatter, Some(p), 0.75);
                        self.effect(p, [1.0, 0.86, 0.42], 0.9);
                    }
                    sound(
                        &mut self.sounds,
                        if shield {
                            Cue::Clang
                        } else if shatter {
                            Cue::Shatter
                        } else {
                            Cue::Impact
                        },
                        Some(p),
                        0.9,
                    );
                    self.effect(
                        p,
                        if let Some(c) = self.blade.color {
                            [
                                c[0] as f32 / 255.0,
                                c[1] as f32 / 255.0,
                                c[2] as f32 / 255.0,
                            ]
                        } else if self.weapon == 3 {
                            [1.0, 0.32, 0.045]
                        } else if self.weapon == 4 {
                            spell_color(2)
                        } else if self.weapon == 5 {
                            [0.56, 0.85, 0.78]
                        } else {
                            [0.9, 0.8, 0.5]
                        },
                        0.6,
                    );
                    if self.weapon == 5 {
                        self.kinetic_pulse(
                            world,
                            p,
                            if finisher { 4.2 } else { 2.4 },
                            if finisher { 8.0 } else { 3.2 },
                        );
                    }
                    if !shield {
                        self.relic_hit(world, eye, p, forward, i, finisher, shatter);
                    }
                }
            } else if self.weapon == 5
                && let Some((cell, _)) = wands::aim_cell(world, eye, forward, 3.2)
                && world.sandbox.bodies.iter().any(|b| b.occupied.iter().any(|(p, _)| *p == cell)) {
                consumed = true;
                if self.melee_buffer > 0.0 && self.melee_cd <= 0.0 && !strike_now {
                    self.melee_buffer = 0.0;
                    self.combo = (self.combo % 3) + 1;
                    self.combo_time = 1.2;
                    self.melee_cd = self.swing_duration();
                    self.strike_delay = self.swing_impact_time();
                    sound(&mut self.sounds, Cue::Swing, None, 0.9);
                }
                if strike_now && world.sandbox.strike_body(cell, forward,
                    if self.combo == 3 { 24.0 } else { 12.0 }, scale) {
                    self.hit = 0.18;
                    self.melee_landed = self.melee_landed.wrapping_add(1);
                    let point = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
                    sound(&mut self.sounds, Cue::Impact, Some(point), 0.9);
                    self.spawn_voxel_burst(point, world.get(cell).color(cell), 6, 2.0, 0.04);
                }
            }
        }
        self.spawn_timer -= dt;
        update_trace.mark("adventure-actions");
        growth::tick(self, dt);
        growth::lanterncap::follow(self, world, player, dt);
        self.publish_reliquaries(world, feet);
        update_trace.mark("adventure-reliquary-publish");
        if self.difficulty != difficulty::Difficulty::Peaceful
            && (world.rules.mob_spawning || realm_roster(world))
        {
            self.publish_encounters(world, feet);
        } else {
            self.encounter_placement = encounter_placement::Placement::default();
        }
        if self.spawn_timer <= 0.0 {
            let mut trace = crate::gameplay_trace::Trace::new();
            self.spawn_timer = 1.0;
            growth::cultivate(world);
            trace.mark("adventure-growth");
            // Awakened bosses below are the player's choice and ignore both gates.
            if world.rules.mob_spawning {
                if self.difficulty != difficulty::Difficulty::Peaceful {
                    self.request_encounters(world, feet);
                }
                trace.mark("adventure-encounters");
                wildlife::spawn(self, world, feet);
                trace.mark("adventure-wildlife");
            } else if self.difficulty != difficulty::Difficulty::Peaceful && realm_roster(world) {
                // Hall and Manyfold keep ambient spawning off. Their seeded posts still arrive.
                self.request_encounters(world, feet);
                trace.mark("adventure-encounters");
            }
            night::tick(self, world, feet, engine_core::Budget::UNBOUNDED);
            burrowers::spawn(self, world, feet);
            spawn_safety::sweep(self, world);
            trace.mark("adventure-night");
            self.update_reliquaries(world, feet);
            trace.mark("adventure-reliquaries");
            for r in self.reliquaries.clone() {
                let state = *self.records.get(&r.site).unwrap_or(&0);
                if state & reliquaries::AWAKENED != 0 && state & 8 == 0 {
                    self.spawn_boss(world, &r, eye);
                }
            }
            // Sparse drifting spores come from nearby fungal surfaces, not a
            // full-screen particle layer. They stay behind terrain depth.
            for i in 0..12 {
                let a = self.time * 1.13 + i as f32 * 2.4;
                let p = feet + Vec3::new(a.cos() * 7.0, (i % 3) as f32 - 1.0, a.sin() * 7.0);
                let cell = (p / scale).floor().as_ivec3();
                if matches!(
                    world.get(cell),
                    super::BlockId::FungalSoil | super::BlockId::GlowCap | super::BlockId::RedCap
                ) && self.effects.len() < 64
                {
                    let above = (cell.as_vec3() + Vec3::new(0.5, 1.1, 0.5)) * scale;
                    if !factory::solid_point(world, above) {
                        self.effects.push(Effect {
                            pos: above,
                            color: [0.32, 0.68, 0.44],
                            radius: 1.0,
                            life: 3.0,
                            total: 3.0,
                            impact_normal: None,
                            family: magic_fx::Family::Plain,
                        });
                    }
                }
            }
        }
        let mut incoming = 0.0;
        update_trace.mark("adventure-spawning");
        let mut incoming_direction = Vec3::ZERO;
        let mut incoming_source: Option<(u32, f32)> = None;
        let mut vault_contact = None;
        let mut toxic_hit = false;
        let mut guard_goals = settlement_guard::defend(&mut self.enemies, world, feet, dt, &mut self.sounds)
            .into_iter().flatten().peekable();
        let neighbors: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| e.health > 0.0)
            .map(|e| (e.pos, e.site))
            .collect();
        let visible_to_mobs = world.rules.mode.visible_to_mobs();
        let alerts: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| visible_to_mobs && e.health > 0.0 && e.visible && e.hostile())
            .map(|e| (e.pos, e.site, e.last_seen))
            .collect();
        let mut navigation_budget = 1;
        let mut resident_navigation_budget = 1;
        let mut blasts = Vec::new();
        if self.casting() {
            self.burrowers.make_noise(0.85);
        }
        // Perception inputs shared by every hostile this pass.
        let senses = perception::Senses {
            light: perception::light_at(world, &mut self.night, eye),
            crouched: player.crouch_t > 0.5,
            still: (player.vel * Vec3::new(1.0, 0.0, 1.0)).length() * scale < 0.3,
            sprinting: player.sprinting && player.vel.length_squared() > 0.25,
            casting: self.casting(),
            noticeable: visible_to_mobs,
        };
        self.body_noise -= dt;
        if self.body_noise <= 0.0 {
            self.body_noise = perception::BODY_NOISE_S;
            let radius = if senses.casting {
                perception::CAST_M
            } else if senses.sprinting {
                perception::SPRINT_M
            } else if !senses.still && !senses.crouched && player.on_ground {
                perception::WALK_M
            } else {
                0.0
            };
            if radius > 0.0 {
                self.noises.push(perception::Noise { pos: feet, radius });
            }
        }
        if self.pending_noise > 0.0 {
            self.noises.push(perception::Noise { pos: feet, radius: self.pending_noise * perception::LEVEL_M });
            self.pending_noise = 0.0;
        }
        let noises = std::mem::take(&mut self.noises);
        self.burrowers.settle_noise(dt);
        let ruckus = self.burrowers.ruckus;
        let mut burrow_fx = Vec::new();
        for (index, e) in self.enemies.iter_mut().enumerate() {
            let guard_goal = guard_goals.next_if(|&(guard, _)| guard == index).map(|(_, goal)| goal);
            #[cfg(feature="multiplayer")]
            if let Some(context) = world.multiplayer.as_ref() {
                // Moving into another player's vicinity during this pass must
                // not give the creature a second AI step in the same tick.
                if e.creature_tick == context.creature_tick || !context.owns(e.pos) { continue; }
                e.creature_tick = context.creature_tick;
            }
            if !visible_to_mobs {
                e.visible = false;
                e.alert = 0.0;
                e.aware = 0.0;
                e.investigate = None;
                e.search = 0.0;
                e.spotted_flash = 0.0;
                e.glimpsed = false;
            }
            if e.hostile() && !e.visible
                && let Some((_, _, seen)) = alerts
                    .iter()
                    .filter(|(p, site, _)| *site == e.site && (*p - e.pos).length_squared() < modifiers::alert_radius_sq(self.site_modifier))
                    .min_by(|(a, _, _), (b, _, _)| a.distance_squared(e.pos).total_cmp(&b.distance_squared(e.pos)))
                {
                    e.last_seen = *seen;
                    e.alert = e.alert.max(3.0);
                    // A raised alarm primes allies: they spot fast, but not instantly.
                    e.aware = e.aware.max(0.7);
                }
            if e.health <= 0.0 {
                continue;
            }
            if visible_to_mobs && e.hostile() {
                perception::hear(e, world, &noises);
                perception::decay(e, dt);
            }
            if e.check_guard_break() {
                let at = e.center();
                sound(&mut self.sounds, Cue::Clang, Some(at), 0.9);
                self.effects.push(Effect {
                    pos: at,
                    color: [0.36, 0.55, 0.72],
                    radius: 0.7,
                    life: 0.35,
                    total: 0.35,
                    impact_normal: None,
                    family: magic_fx::Family::Plain,
                });
            }
            e.reaction.advance(e.flash, e.knock, e.yaw, dt);
            enemy_presentation::motion::advance(e, dt);
            e.flash = (e.flash - dt).max(0.0);
            e.poise = (e.poise - dt).max(0.0);
            water_contact::update(e, world);
            let dot = e.status.tick(dt, e.affixes);
            if dot != 0.0 {
                e.health = (e.health - dot).min(e.max_health());
                if dot > 0.0 {
                    e.flash = e.flash.max(0.06);
                }
            }
            if burrowers::is_kind(e.kind) {
                let (bite, direction) = burrowers::tick(e, world, feet, eye, dt, self.time, visible_to_mobs, ruckus, &mut burrow_fx);
                incoming += bite;
                incoming_direction += direction;
                if bite > 0.0 { incoming_source = Some((e.kind, (e.health / e.max_health()).clamp(0.0, 1.0))); }
                continue;
            }
            if field_foes::is_kind(e.kind) {
                let (hit, direction) = field_foes::tick(e, world, feet, eye, dt, self.time, visible_to_mobs, &mut burrow_fx);
                incoming += hit;
                incoming_direction += direction;
                if hit > 0.0 { incoming_source = Some((e.kind, (e.health / e.max_health()).clamp(0.0, 1.0))); }
                continue;
            }
            let started = std::time::Instant::now();
            let hanging = e.encounter.concealed && e.encounter.mechanic() == encounters::Mechanic::CeilingDrop;
            if !is_boss(e.kind) && !hanging { navigation::gravity(e, world, dt); }
            crate::gameplay_trace::note("nav.gravity", started.elapsed().as_secs_f64() * 1000.0);
            if wildlife::vehicle::hostile_campaign(e) && visible_to_mobs {
                let offset = eye - e.center();
                let reachable = offset.length_squared() <= 2.15 * 2.15 && line(world, e.center(), eye);
                if e.tell > 0.0 {
                    e.tell = (e.tell - dt).max(0.0);
                    if e.tell == 0.0 {
                        e.cooldown = settlement_guard::STRIKE_INTERVAL;
                        if reachable && e.stagger <= 0.0 {
                            incoming += 9.0;
                            incoming_direction -= offset.normalize_or_zero() * 9.0;
                        }
                    }
                } else if reachable && e.cooldown <= 0.0 && e.stagger <= 0.0 {
                    e.yaw = (-offset.x).atan2(-offset.z);
                    e.tell = 0.34;
                    sound(&mut self.sounds, Cue::Swing, Some(e.center()), 0.55);
                }
            }
            if !e.hostile() {
                let started = std::time::Instant::now();
                wildlife::tick(e, world, feet, self.time, dt, guard_goal, &mut resident_navigation_budget);
                crate::gameplay_trace::note("nav.wildlife", started.elapsed().as_secs_f64() * 1000.0);
                continue;
            }
            if encounters::ambush(e, world, feet, eye, forward, &senses, dt, &mut self.sounds) { continue; }
            if e.status.any() || !e.affixes.is_empty() {
                // One mote when the half-second changes. The old gate was true
                // for every frame inside a 24 Hz bin, so statuses strobed.
                let tick = (self.time * 1.6) as u32;
                let prev = ((self.time - dt).max(0.0) * 1.6) as u32;
                if tick != prev && self.particles.len() < 240 {
                    let (color, rise) = if e.status.burn > 0.0 {
                        (status::Element::Fire.color(), 0.55)
                    } else if e.status.poison > 0.0 {
                        (status::Element::Toxic.color(), 0.28)
                    } else if e.status.shock > 0.0 {
                        (status::Element::Shock.color(), 0.12)
                    } else if e.status.wet > 0.0 {
                        (status::Element::Wet.color(), -0.15)
                    } else if let Some(a) = e.affixes.first() {
                        (a.color(), 0.22)
                    } else {
                        (PALETTE_AMBER, 0.15)
                    };
                    let a = e.slot as f32 * 1.7 + tick as f32 * 0.9;
                    self.particles.push(spells::VoxelParticle {
                        seed: e.slot.wrapping_mul(2654435761) ^ tick as u32,
                        pos: e.center()
                            + Vec3::new(a.cos() * 0.28, 0.05, a.sin() * 0.28),
                        vel: Vec3::new(0.0, rise, 0.0),
                        color,
                        size: 0.026,
                        glow: 1.05,
                        alpha: 0.42,
                        gravity: 0.4,
                        life: 0.9,
                        total: 0.9,
                        family: magic_fx::Family::Plain,
                    });
                }
            }
            e.stagger = (e.stagger - dt).max(0.0);
            if e.stagger > 0.0 && !is_boss(e.kind) {
                if !vault_fight::handles_hit(e, true) {
                    e.tell = 0.0;
                    e.attack = 0;
                    e.charge = 0.0;
                    e.recovery = 0.0;
                    e.cooldown = e.cooldown.max(0.5);
                }
            }
            if e.stagger > 0.0 || e.tell > 0.0 {
                e.gait *= (-dt * 10.0).exp();
            }
            e.recovery = (e.recovery - dt).max(0.0);
            if vault_fight::lead(e) && e.charge > 0.0 {
                if let Some(direction) = vault_fight::advance(e, world, feet, eye, dt, &mut self.sounds) {
                    vault_contact = Some((index, direction));
                }
                continue;
            }
            // Sweep knockback and charges; neither may teleport through thin walls.
            let charging = e.charge > 0.0;
            let impulse_start = e.pos;
            let impulse = if charging {
                e.lunge * 8.0
            } else {
                e.knock
            };
            let impulse = if is_boss(e.kind) {
                impulse.clamp_length_max(1.5)
            } else {
                impulse
            };
            let steps = (impulse.length() * dt / 0.12).ceil().max(1.0) as usize;
            for _ in 0..steps {
                if e.up.dot(Vec3::Y) > 0.999 {
                    for axis in [0, 2] {
                        let mut q = e.pos;
                        q[axis] += impulse[axis] * dt / steps as f32;
                        if free(world, q, e.kind) {
                            e.pos = q;
                        } else {
                            e.charge = 0.0;
                        }
                    }
                } else {
                    let q = e.pos + along_floor(e.up, impulse) * dt / steps as f32;
                    if navigation::clear(world, q, e.kind, e.up) {
                        e.pos = q;
                    } else {
                        e.charge = 0.0;
                    }
                }
            }
            e.knock *= (-dt * 9.0).exp();
            if charging {
                let moved = along_floor(e.up, e.pos - impulse_start);
                e.phase += moved.length();
                let v = moved / dt.max(0.001);
                e.velocity.x = v.x;
                e.velocity.z = v.z;
                e.gait += (1.0 - e.gait) * (1.0 - (-dt * 16.0).exp());
            }
            if e.charge > 0.0 {
                e.charge = (e.charge - dt).max(0.0);
                if (e.center() - eye).length() < 2.0 && line(world, e.center(), eye) {
                    incoming += 18.0;
                    incoming_direction += (e.center() - eye).normalize_or_zero() * 18.0;
                    e.charge = 0.0;
                }
            }
            e.slow = (e.slow - dt).max(0.0);
            e.cooldown -= dt * e.affixes.speed_mul();
            e.sight -= dt;
            let d = eye - e.center();
            let distance = d.length();
            if e.sight <= 0.0 {
                let interval = 0.12 + (e.slot % 4) as f32 * 0.025;
                e.sight = interval;
                let range = modifiers::sight_range(self.site_modifier);
                let los = visible_to_mobs && distance < range && line(world, e.center(), eye);
                if perception::see(e, &senses, eye, feet, distance, range, los, interval) {
                    // The "spotted" sting: the player always hears the moment it happens.
                    sound(&mut self.sounds, Cue::Warning, Some(e.center()), 0.6);
                }
            }
            if e.visible {
                e.encounter.observe(feet, dt);
            }
            e.alert = (e.alert - dt).max(0.0);
            if is_boss(e.kind) {
                let (boss_damage, boss_direction) = bosses::tick(
                    e,
                    world,
                    feet,
                    eye,
                    dt,
                    &mut self.projectiles,
                    &mut self.hazards,
                    &mut self.sounds,
                    &mut self.dirty_voxels,
                    &mut navigation_budget,
                );
                incoming += boss_damage;
                incoming_direction += boss_direction;
                continue;
            }
            if e.stagger > 0.0 {
                if aberrations::is_kind(e.kind) { e.encounter.exposed = 0.0; }
                continue;
            }
            incoming += aberrations::tether(e, world, player, dt, self.time);
            if charging { continue; }
            if e.kind == mobs::CREEPER {
                if let Some(at) = mobs::fuse(e, world, feet, eye, dt, &mut self.sounds) {
                    blasts.push(at);
                    continue;
                }
                if e.tell > 0.0 {
                    continue;
                }
            }
            if e.tell > 0.0 {
                if e.tell > e.windup() * 0.55 {
                    let target = local_yaw(e.up, e.lunge);
                    let delta = (target - e.yaw + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                    e.yaw += delta.clamp(-dt * 5.0, dt * 5.0);
                }
                e.tell -= dt;
                if e.tell <= 0.0 {
                    if vault_fight::lead(e) {
                        vault_fight::begin(e);
                        sound(&mut self.sounds, Cue::Swing, Some(e.center()), 0.85);
                        continue;
                    }
                    e.recovery = enemy_presentation::RECOVERY_S;
                    if e.attack == 3 || e.kind == 3 {
                        sound(&mut self.sounds, Cue::Swing, Some(e.center()), 0.6);
                    }
                    if let Some(damage) = aberrations::release(e, world, player, self.time) {
                        incoming += damage;
                        incoming_direction += (e.center() - eye).normalize_or_zero() * damage;
                    } else if e.kind == 3 || folklore::charges(e.kind) || e.encounter.mechanic() == encounters::Mechanic::Interceptor {
                        e.charge = if e.kind == folklore::SATYR { 0.30 } else { 0.42 };
                    } else if e.kind == 0 || (brawler(e.kind) && e.attack == 3) {
                        let toward = along_floor(e.up, eye - e.center()).normalize_or_zero();
                        if distance < 2.6 && e.visible && e.lunge.dot(toward) > 0.35 && line(world, e.center(), eye) {
                            let damage = folklore::melee_damage(e.kind) * (1.0 + e.menace as f32*0.2);
                            incoming += damage;
                            incoming_direction += (e.center() - eye).normalize_or_zero() * damage;
                            toxic_hit |= e.affixes.has(status::Affix::Toxic);
                        }
                    } else if e.attack == 2 || (e.visible && line(world, e.center(), eye)) {
                        let kiln_fire = e.kind == rewards::KILN_CRAWLER && rewards::hell_camp(world, e.site);
                        tactics::release(e, &mut self.projectiles, &mut self.hazards, kiln_fire);
                        sound(
                            &mut self.sounds,
                            if e.attack == 2 {
                                Cue::Burst
                            } else {
                                Cue::Spark
                            },
                            Some(e.center()),
                            0.55,
                        );
                    }
                    e.cooldown = bestiary::species(e.kind)
                        .map_or(if brawler(e.kind) { 1.6 } else { 2.8 }, |s| s.cooldown);
                }
                continue;
            }
            let attack_range = if vault_fight::lead(e) { vault_fight::ATTACK_RANGE }
            else if e.encounter.mechanic() == encounters::Mechanic::Hexer {9.0}
            else if e.encounter.mechanic() == encounters::Mechanic::Interceptor {6.0} else { bestiary::species(e.kind).map_or(
                if e.kind == 3 {
                    7.0
                } else if e.kind == 9 {
                    6.5
                } else {
                    18.0
                },
                |s| s.attack_range,
            ) };
            let facing_target = yaw_forward(e.up, e.yaw)
                .dot(along_floor(e.up, d).normalize_or_zero()) > 0.55;
            if e.visible && facing_target && e.cooldown <= 0.0 && e.recovery <= 0.0
                && distance < attack_range && line(world, e.center(), eye)
            {
                e.attacks = e.attacks.wrapping_add(1);
                e.attack = if e.kind == rewards::KILN_CRAWLER && e.attacks.is_multiple_of(2)
                    && rewards::hell_camp(world, e.site) { 2 }
                else if aberrations::is_kind(e.kind) { 0 }
                else if let Some(attack) = folklore::attack(e.kind, e.attacks) { attack }
                else if e.kind == rewards::KILN_CRAWLER && e.attacks.is_multiple_of(2)
                    && rewards::hell_camp(world, e.site) { 2 }
                else if e.encounter.mechanic() == encounters::Mechanic::Hexer {2}
                else if e.encounter.mechanic() == encounters::Mechanic::Suppressor {0}
                else if brawler(e.kind) && !matches!(e.kind,3|9) { 3 }
                else if (e.kind == 5 && distance < 4.5)
                    || (e.kind == 9 && distance < 6.5)
                    || (matches!(e.kind, 10..=13) && distance < 3.4)
                    || (e.kind == 4 && e.attacks % 2 == 0)
                    || (e.kind == 15 && e.attacks % 2 == 0)
                {
                    2
                } else {
                    0
                };
                e.aim = if aberrations::is_kind(e.kind) {
                    eye // Rays/tendrils are committed casts, not leading projectiles.
                } else if e.attack == 2 {
                    feet + if e.encounter.mechanic() == encounters::Mechanic::Hexer {
                        (e.encounter.motion*0.45).clamp_length_max(1.8)
                    } else {Vec3::ZERO}
                } else {
                    eye + (player.vel * scale).clamp_length_max(6.0)
                        * (distance / if e.kind == 1 { 16.0 } else { 11.0 }).clamp(0.0, 0.65)
                };
                e.tell = e.windup();
                sound(&mut self.sounds, Cue::Warning, Some(e.center()), 0.55);
                let intercept = feet + if e.encounter.mechanic() == encounters::Mechanic::Interceptor {
                    (e.encounter.motion*0.35).clamp_length_max(1.8)
                } else {Vec3::ZERO};
                e.lunge = along_floor(e.up, intercept - e.pos).normalize_or_zero();
                continue;
            }
            let leashed = (e.pos - e.home).length() < 30.0;
            // Without sight: last-seen trail, then a short search, then noises to check.
            let hunt = if leashed { perception::pursue(e, dt) } else { None };
            let pace = hunt.map_or(1.0, |(_, pace)| pace);
            let chasing = (e.visible || hunt.is_some()) && leashed;
            let goal = if chasing {
                if e.visible && e.kind == mobs::CREEPER {
                    mobs::stalk_goal(feet, player.flat_forward())
                } else if e.visible { feet } else { hunt.map_or(e.last_seen, |(at, _)| at) }
            } else {
                e.home
                    + Vec3::new(
                        (self.time * 0.22 + e.slot as f32).sin(),
                        0.0,
                        (self.time * 0.22 + e.slot as f32).cos(),
                    ) * 2.0
            };
            let tactical_direction = tactics::heading(
                e,
                goal,
                distance,
                chasing,
                &neighbors,
                &self.projectiles,
                &self.hazards,
                self.time,
            );
            // Route toward a tactical destination, preserving ranged spacing and flanks.
            let destination = if e.visible && chasing {
                e.pos + tactical_direction * (distance * 0.5).clamp(2.0, 7.0)
            } else { goal };
            let direction = e.navigation.heading(world, e.pos, destination, e.kind, dt, e.slot, &mut navigation_budget);
            let speed = pace * if aberrations::roper_like(e.kind) { 0.0 } else if e.slow > 0.0 {
                0.5
            } else {
                bestiary::species(e.kind)
                    .map_or(if brawler(e.kind) { 2.5 } else { 1.3 }, |s| s.speed)
            };
            let rooted =
                bestiary::species(e.kind).is_some_and(|s| s.role == bestiary::Role::Rooted);
            let movement =
                if e.recovery > 0.0 || (rooted && !chasing)
                    || (brawler(e.kind) && e.visible && chasing && distance < 1.55
                        && tactical_direction.dot((goal - e.pos).normalize_or_zero()) >= 0.0)
                {
                    Vec3::ZERO
                } else {
                    direction.normalize_or_zero() * speed * e.affixes.speed_mul() * dt
                };
            // A tree that turns to watch you is no disguise: tree ropers hold
            // still until the player is inside their reach.
            let facing = if aberrations::tree_roper(e.kind) && distance > 7.5 { Vec3::ZERO }
                else if e.visible { d } else { movement };
            let up = e.up;
            let flat_facing = if up.dot(Vec3::Y) > 0.999 {
                Vec3::new(facing.x, 0.0, facing.z)
            } else {
                let flat = facing - up * facing.dot(up);
                Quat::from_rotation_arc(up, Vec3::Y) * flat
            };
            if flat_facing.length_squared() > 1e-8 {
                let target = (-flat_facing.x).atan2(-flat_facing.z);
                let delta = (target - e.yaw + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                e.yaw += delta.clamp(-dt * 6.0, dt * 6.0);
            }
            let before = e.pos;
            let mut blocked = false;
            if up.dot(Vec3::Y) <= 0.999 {
                let slide = movement - up * movement.dot(up);
                let q = e.pos + slide;
                if navigation::clear(world, q, e.kind, up) && navigation::on_floor(world, q, up) {
                    e.pos = q;
                } else {
                    let raised = q + up * 1.0;
                    if navigation::clear(world, raised, e.kind, up) && navigation::on_floor(world, raised, up) {
                        e.pos = raised;
                    } else {
                        blocked = true;
                    }
                }
            }
            if up.dot(Vec3::Y) > 0.999 { for axis in [0, 2] {
                let mut q = e.pos;
                q[axis] += movement[axis];
                let supported = flying(e.kind)
                    || e.velocity.y.abs() > 0.01
                    || factory::solid_point(world, q - Vec3::Y * 0.65);
                if free(world, q, e.kind) && supported && navigation::safe_floor(world, q) {
                    e.pos = q;
                } else if !flying(e.kind)
                    && e.velocity.y.abs() < 0.01
                    && !free(world, q, e.kind)
                    && free(world, q + Vec3::Y * 1.05, e.kind)
                {
                    e.velocity.y = 5.7;
                    blocked = true;
                } else {
                    blocked = true;
                }
            } }
            // Only probe side steps after the normal collision-resolved move
            // actually meets a wall. This keeps the common AI tick cheap.
            if up.dot(Vec3::Y) > 0.999 && blocked && movement.length_squared() > 1e-8 {
                let desired = movement.normalize_or_zero();
                let advanced = ((e.pos - before) * Vec3::new(1.0, 0.0, 1.0)).length();
                let reach = (movement.length() - advanced).max(0.0);
                let mut best: Option<(Vec3, f32)> = None;
                for angle in [0.65_f32, -0.65, 1.2, -1.2, 2.2, -2.2] {
                    let candidate = Quat::from_rotation_y(angle) * desired;
                    let progress = candidate.dot(desired);
                    if progress > 0.0
                        && free(world, e.pos + candidate * reach, e.kind)
                        && navigation::safe_floor(world, e.pos + candidate * reach)
                        && (flying(e.kind)
                            || e.velocity.y.abs() > 0.01
                            || factory::solid_point(
                                world,
                                e.pos + candidate * reach - Vec3::Y * 0.65,
                            ))
                        && best.is_none_or(|(_, score)| progress > score)
                    {
                        best = Some((candidate, progress));
                    }
                }
                if let Some((heading, _)) = best {
                    let q = e.pos + heading * reach;
                    if free(world, q, e.kind) {
                        e.pos = q;
                    }
                }
            }
            if flying(e.kind) {
                let target_y = if chasing {
                    eye.y - 0.45
                } else {
                    e.home.y + 0.5
                };
                let q = e.pos + Vec3::Y * (target_y - e.pos.y).clamp(-dt, dt);
                if free(world, q, e.kind) {
                    e.pos = q;
                }
            }
            let walked = ((e.pos - before) * Vec3::new(1.0, 0.0, 1.0)).length();
            e.velocity.x = (e.pos.x - before.x) / dt.max(0.001);
            e.velocity.z = (e.pos.z - before.z) / dt.max(0.001);
            e.phase += walked;
            let moving = (walked / dt.max(0.001) / speed.max(0.1)).clamp(0.0, 1.0);
            e.gait += (moving - e.gait) * (1.0 - (-dt * 16.0).exp());
        }
        for at in blasts {
            self.burst(world, at, eye);
        }
        food_chain::hunt(&mut self.enemies, world, feet, dt, &mut burrow_fx);
        burrowers::after_tick(self, world, feet, eye, dt, burrow_fx);
        let (thrown, thrown_direction, thrower) = hurled::tick(self, world, feet, eye, dt);
        if thrown > 0.0 {
            incoming += thrown;
            incoming_direction += thrown_direction;
            if let Some(kind) = thrower {
                incoming_source = Some((kind, 1.0));
            }
        }
        update_trace.mark("adventure-navigation");
        self.tick_hazards(world, feet, eye, dt);
        if let Some((index, direction)) = vault_contact && visible_to_mobs {
            let amount = vault_fight::DAMAGE * (1.0 - armor_reduction);
            let taken = if let Some(gear) = shield {
                let outcome = self.shield_guard.absorb(&gear, amount, direction, forward);
                let e = &mut self.enemies[index];
                if outcome.blocked { sound(&mut self.sounds, Cue::Clang, Some(e.center()), 0.9); }
                if outcome.parried { vault_fight::expose(e); }
                if outcome.reflected > 0.0 { e.hurt(outcome.reflected); }
                outcome.taken
            } else { amount };
            if taken > 0.0 { self.damage(taken, direction); }
        }
        if incoming > 0.0 && world.rules.mode.visible_to_mobs() {
            let mut incoming = incoming * (1.0 - armor_reduction);
            if let Some(gear) = shield {
                let facing = player.forward_dir();
                let out = self.shield_guard.absorb(&gear, incoming, incoming_direction, facing);
                if out.blocked {
                    sound(&mut self.sounds, Cue::Clang, None, 0.9);
                    if out.reflected > 0.0 {
                        for e in self.enemies.iter_mut().filter(|e| {
                            let to = e.center() - eye;
                            to.length() < 3.2 && to.normalize_or_zero().dot(facing) > 0.34
                        }) {
                            e.hurt(out.reflected);
                        }
                    }
                }
                incoming = out.taken;
            }
            if incoming > 0.0 {
                // Read before the hit: damage starts the flinch window.
                let lands = self.invulnerable <= 0.0;
                self.damage(incoming, incoming_direction);
                if lands && incoming_source.is_some() {
                    self.damage_source = incoming_source;
                    self.damage_source_t = 4.0;
                }
                if lands && toxic_hit {
                    let (seconds, level) = buffs::TOXIC_HIT;
                    self.inflict(crate::world::progression::effects::Kind::Poison, seconds, level);
                }
            }
        }
        let shots = std::mem::take(&mut self.projectiles);
        'shot: for mut shot in shots {
            shot.life -= dt;
            if shot.life <= 0.0 {
                // On expire: a bolt that times out without hitting anything
                // still delivers its payload where it ended.
                if !shot.hostile && shot.mods.expire && shot.payload.is_some() {
                    self.release_payload(world, &mut shot);
                }
                continue;
            }
            if !shot.hostile {
                shot.apply_gravity(dt);
                if shot.mods.timer > 0.0 && shot.payload.is_some() {
                    shot.mods.timer -= dt;
                    if shot.mods.timer <= 0.0 {
                        // The payload leaves the carrier mid-flight; the carrier
                        // keeps flying and no longer holds a payload for impact.
                        self.release_payload(world, &mut shot);
                    }
                }
                if shot.spell == 3 {
                    let span = (wands::base_life(3) * shot.mods.life_mul.max(0.25)).max(0.05);
                    let spent = (1.0 - shot.life / span).clamp(0.0, 1.0);
                    let swell = 1.0 + spent * 0.55;
                    if shot.mods.radius < swell {
                        shot.mods.radius = swell;
                    }
                }
                self.arcana_flight(world, &mut shot, dt);
                if shot.mods.homing || shot.mods.seek_ally {
                    let ally = if shot.mods.seek_ally {
                        self.companion.as_ref().and_then(|c| {
                            let p = c.pos + Vec3::Y * (c.shown * 0.5);
                            let d = (p - shot.pos).length();
                            (d < 18.0 && line(world, shot.pos, p)).then_some((p, d))
                        })
                    } else {
                        None
                    };
                    let foe = self
                        .enemies
                        .iter()
                        .filter(|e| e.health > 0.0)
                        .filter_map(|e| {
                            let d = (e.center() - shot.pos).length();
                            (d < 18.0 && line(world, shot.pos, e.center()))
                                .then_some((e.center(), d))
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1));
                    if let Some((target_pos, _)) = ally.or(foe) {
                        let speed = shot.velocity.length().max(8.0);
                        let desired = (target_pos - shot.pos).normalize_or_zero();
                        let current = shot.velocity.normalize_or_zero();
                        let steered = (current + (desired - current) * (dt * 7.2).min(1.0))
                            .normalize_or_zero();
                        shot.velocity = steered * speed;
                    }
                }
                shot.trail_timer -= dt;
                if shot.trail_timer <= 0.0 {
                    // A mote only for spells that are a wake. A lance or a spark
                    // is the mesh itself; a mote on every bolt fills the air.
                    let wake = shot.mods.trail || matches!(shot.spell, 5 | 10 | 12 | 28);
                    shot.trail_timer = if shot.spell == 10 {
                        0.2
                    } else if wake {
                        0.24
                    } else {
                        0.4
                    };
                    if wake {
                        let (family, color) = magic_fx::Family::for_shot(&shot);
                        let look = family.look();
                        self.spawn_styled_particle(
                            shot.pos - shot.velocity.normalize_or_zero() * 0.08,
                            Vec3::Y * (0.08 + look.lift * 0.30),
                            color,
                            if shot.spell == 10 { 0.036 } else { 0.02 },
                            look.glow * 0.65,
                            0.35,
                            look.gravity * 0.40,
                            look.life.max(0.25),
                            family,
                        );
                    }
                    if shot.spell == 10 && shot.bore_budget > 0 {
                        // Black Hole devours solid terrain along its flight path and pulls enemies in
                        self.carve_sphere(world, shot.pos, 1.15, 4);
                        shot.bore_budget = shot.bore_budget.saturating_sub(1);
                        for e in &mut self.enemies {
                            let pull = shot.pos - e.center();
                            if e.health > 0.0 && pull.length() < 5.2 {
                                e.knock += pull.normalize_or_zero() * (dt * 18.0);
                                e.health -= dt * 16.0 * shot.mods.damage_mul;
                                e.flash = 0.08;
                            }
                        }
                    }
                    if matches!(shot.mods.element, Some(status::Element::Fire)) {
                        let cell = (shot.pos / scale).floor().as_ivec3();
                        if world.sandbox.can_admit(&[cell]) {
                            let mut tc = world.sandbox.cell(cell);
                            tc.temperature = tc.temperature.max(540.0);
                            world.sandbox.set(cell, tc);
                            world.activity.wake(cell);
                        }
                    }
                }
                // Frost ices the water it is inside now. The wake timer is too
                // slow for a lance, which can cross a pool between ticks of it.
                if shot.mods.element == Some(status::Element::Frost) {
                    let cell = (shot.pos / scale).floor().as_ivec3();
                    if world.get(cell) == super::BlockId::Water {
                        world.set(cell, super::BlockId::Ice);
                        self.mark_voxel_dirty(world, cell);
                    }
                }
            }
            let travel = if let Some(tether) = shot.flight.as_mut().and_then(|f| f.snapline.as_mut()) {
                tether.anchor = snapline::Tether::hand(eye, player);
                if tether.detached(shot.pos) {
                    continue;
                }
                tether.travel(shot.pos, &mut shot.velocity, dt)
            } else {
                shot.velocity * dt
            };
            let can_connect = shot.tether().is_none_or(|tether| !tether.connected);
            let mut spark_target = None;
            let mut crit_at: Option<Vec3> = None;
            let mut reaction_fx: Option<(Vec3, status::Element, Option<&'static str>)> = None;
            let steps = (travel.length() / (scale * 0.2).min(0.10)).ceil().max(1.0) as usize;
            let mut hit = false;
            let mut actor_hit = false;
            let mut surface_contact = None;
            // First foe a piercing bolt passed through; a rune event counts it as the hit.
            let mut rune_glance: Option<Vec3> = None;
            for _ in 0..steps {
                let delta = if !shot.hostile && shot.mods.path != 0 {
                    shot.flight.get_or_insert_with(|| trajectory::Flight::new(shot.velocity))
                        .advance(&mut shot.velocity, dt / steps as f32, shot.mods.path)
                } else {
                    travel / steps as f32
                };
                let start = shot.precise_at(shot.pos);
                let end = shot.next_position(delta);
                #[cfg(feature = "multiplayer")]
                if !shot.hostile && can_connect && self.player_projectile(world, &shot, end, rank) {
                    shot.pos = end;
                    hit = true;
                    actor_hit = true;
                    break;
                }
                let terrain = trace::solid_contact_precise(world, start, shot.precise_at(end));
                if shot.hostile
                    && let Some(contact) = self.aegis_contact(shot.pos, terrain.as_ref().map_or(end, |hit| hit.pos))
                {
                    let normal = (contact - eye).normalize_or((-shot.velocity).normalize_or(Vec3::Y));
                    self.spell_contact(contact, shot.velocity, spell_color(concentration::AEGIS),
                        magic_fx::Family::Lumen, magic_fx::Contact::Surface(normal));
                    continue 'shot;
                }
                if let Some(trace::SolidContact { pos: impact, cell, normal }) = terrain {
                    if !shot.hostile && shot.mods.phase > 0 {
                        let dir = (shot.precise_at(end) - start).normalize_or_zero();
                        let stride = f64::from(scale) * 0.5;
                        let mut probe = impact.as_dvec3();
                        let mut exit = None;
                        if dir != glam::DVec3::ZERO {
                            for _ in 0..24 {
                                probe += dir * stride;
                                if !world.solid((probe / f64::from(scale)).floor().as_ivec3()) {
                                    exit = Some(probe.as_vec3());
                                    break;
                                }
                            }
                        }
                        if let Some(exit) = exit {
                            shot.mods.phase -= 1;
                            self.spell_contact(impact, shot.velocity, shot.color(),
                                magic_fx::Family::Rift, magic_fx::Contact::Surface(normal));
                            shot.correct_position(exit);
                            self.spell_contact(exit, shot.velocity, shot.color(),
                                magic_fx::Family::Rift, magic_fx::Contact::Glance);
                            continue;
                        }
                    }
                    if !shot.hostile && shot.bore_budget > 0 && self.carve_cell(world, cell, true) {
                        shot.bore_budget -= 1;
                        shot.pos = end;
                        continue;
                    }
                    shot.correct_position(impact);
                    if !shot.hostile && matches!(shot.spell, 0 | 14) {
                        let anchor = factory::anchor(cell, factory::step(world));
                        if let Some(node) = world.factory.nodes.get_mut(&anchor) {
                            if node.kind.powered() {
                                node.energy = (node.energy + 48).min(64);
                            } else if node.kind == factory::Kind::Battery {
                                // Thaumic induction: a Spark banks 60 kJ.
                                node.energy = (node.energy + 240)
                                    .min(factory::power::battery_capacity(node.upgrade));
                                node.activity = 1.0;
                            }
                        }
                    }
                    if !shot.hostile && shot.mods.bounces > 0 && shot.spell != 3 {
                        shot.mods.bounces -= 1;
                        let (family, tint) = magic_fx::Family::for_shot(&shot);
                        self.spell_contact(impact, shot.velocity, tint, family,
                            magic_fx::Contact::Surface(normal));
                        let inward = shot.velocity.dot(normal).min(0.0);
                        shot.velocity -= 2.0 * inward * normal;
                        shot.correct_position(shot.pos + normal * 0.04);
                        self.arcana_bounce(world, &mut shot);
                        let (cue, pitch) = wands::spell_audio(shot.spell);
                        sound_pitched(&mut self.sounds, cue, Some(shot.pos), 0.35, pitch);
                        break;
                    }
                    sound(
                        &mut self.sounds,
                        if matches!(shot.spell, 3 | 12 | 29) {
                            Cue::Burst
                        } else if matches!(shot.spell, 2 | 13) {
                            Cue::Shatter
                        } else {
                            Cue::Impact
                        },
                        Some(shot.pos),
                        0.45,
                    );
                    // Keep the impact origin on the incident side of the wall;
                    // burst visibility must not start inside the blocking voxel.
                    hit = true;
                    surface_contact = Some((impact, normal));
                    // A player bolt striking stone is a lure: enemies go to look.
                    if !shot.hostile {
                        self.noises.push(perception::Noise { pos: impact, radius: perception::IMPACT_M });
                    }
                    break;
                }
                shot.pos = end;
                if !shot.splashed {
                    let liquid = world.get((shot.pos / scale).floor().as_ivec3());
                    let spray = spells::Spray::viscous_liquid(liquid);
                    let surface = match liquid {
                        super::BlockId::Water => world.water_surface_at(shot.pos),
                        _ if spray.is_some() => world.viscous_surface_at(shot.pos, liquid),
                        _ => None,
                    };
                    if let Some(surface) = surface
                        && shot.pos.y <= surface.y && surface.y - shot.pos.y <= scale * 1.5
                    {
                        shot.splashed = true;
                        let strength = (shot.velocity.length() * 0.032).clamp(0.18, 0.85);
                        if liquid == super::BlockId::Water {
                            self.splash(surface, strength);
                            world.disturb_water_surface(surface, strength);
                            if !shot.hostile && matches!(shot.spell, 14 | spells::STORM_SURGE) {
                                hit = true;
                                surface_contact = Some((surface, Vec3::Y));
                                break;
                            }
                        } else if let Some(spray) = spray {
                            self.splash_droplets(surface, strength, spray, Vec3::ZERO);
                        }
                    }
                }
                if shot.hostile {
                    if shot.pos.cmpge(player_min).all() && shot.pos.cmple(player_max).all() {
                        if self.ward > 0.0 && self.ward_hits > 0 {
                            self.ward_hits -= 1;
                            if self.ward_hits == 0 {
                                self.ward = 0.0;
                            }
                            if self.knows(2) {
                                self.mana = (self.mana + 6.0).min(self.max_mana());
                            }
                            shot.hostile = false;
                            shot.spell = 0;
                            shot.velocity = -shot.velocity * 1.8;
                            shot.life = 3.0;
                            sound(&mut self.sounds, Cue::Clang, None, 0.8);
                            self.effect(shot.pos, spell_color(4), 0.5);
                            break;
                        }
                        self.damage(
                            12.0 * shot.mods.damage_mul.max(1.0) * (1.0 - armor_reduction),
                            -shot.velocity,
                        );
                        hit = true;
                        break;
                    }
                } else if can_connect && let Some(e) = self.enemies.iter_mut().find(|e| {
                    e.health > 0.0
                        && e.slot + 1 != shot.mods.pierced
                        && shot.farm.is_none_or(|owner| {
                            world.habitats.sources.iter().any(|s| {
                                s.id == owner && emitter_can_target(e, owner, s.reserve)
                            })
                        })
                        && if e.kind == engine::KIND { engine::hit(e, shot.pos, if shot.mods.orbit { 0.24 } else { 0.0 }) }
                        else if e.kind == 8 { hart::hit(e, shot.pos, if shot.mods.orbit { 0.24 } else { 0.0 }) }
                        else if burrowers::is_kind(e.kind) { burrowers::hit(e, shot.pos, if shot.mods.orbit { 0.24 } else { 0.0 }) }
                        else if field_foes::is_kind(e.kind) { field_foes::hit(e, shot.pos, if shot.mods.orbit { 0.24 } else { 0.0 }) } else { (e.center() - shot.pos).length()
                            < if tall(e.kind) { 0.8 } else { 0.58 }
                                + if shot.mods.orbit { 0.24 } else { 0.0 } }
                }) {
                    if shot.farm.is_none() && (matches!(shot.spell, 0 | 14) || shot.mods.chain) {
                        spark_target = Some((e.site, e.slot));
                    }
                    let shatter = e.slow > 0.0 && !matches!(shot.spell, 2 | 13);
                    let shield = (e.kind == 5
                        && e.guard
                        && shot.spell == 0
                        && !shatter
                        && !shot.mods.bore
                        && shot.velocity.normalize_or_zero().dot(Vec3::new(
                            -e.yaw.sin(),
                            0.0,
                            -e.yaw.cos(),
                        )) < -0.3)
                        || (!shatter && !shot.mods.bore && (tollkeeper::guarded(e, shot.velocity) || engine::guarded(e, shot.velocity)));
                    sound(
                        &mut self.sounds,
                        if shield {
                            Cue::Clang
                        } else if shatter {
                            Cue::Shatter
                        } else {
                            Cue::Impact
                        },
                        Some(e.center()),
                        0.7,
                    );
                    let base_dmg = spell_hit_damage(shot.spell);
                    let mut dealt =
                        (base_dmg + rank as f32 * 4.0 + if shatter { 18.0 } else { 0.0 })
                            * shot.mods.damage_mul
                            * if shield { 0.3 } else { 1.0 }
                            * e.affixes.damage_taken();
                    dealt *= Adventure::arcana_hit_mul(&shot.mods, e);
                    // Sneak shot on an enemy that never noticed you; it reuses the crit spark.
                    if !shot.hostile && perception::unaware(e) {
                        dealt *= 1.5;
                        crit_at = Some(e.center());
                    }
                    if !shot.hostile {
                        perception::struck(e, feet);
                        self.noises.push(perception::Noise { pos: shot.pos, radius: perception::IMPACT_M });
                    }
                    let crit = shot.mods.crit && wands::crit_roll(self.time, e.slot);
                    if crit {
                        dealt *= wands::card_rules::CRITICAL_MULTIPLIER;
                        crit_at = Some(e.center());
                    }
                    let mut element_stagger = 0.0;
                    if let Some(element) = shot.mods.element {
                        let reaction = e.status.apply(element, e.affixes);
                        dealt *= reaction.damage_mul;
                        if reaction.name.is_some() {
                            dealt *= 1.0 + self.resonance.stacks(1) * 0.20;
                        }
                        element_stagger = reaction.stagger;
                        if reaction.chill > 0.0 {
                            e.slow = e.slow.max(reaction.chill);
                        }
                        reaction_fx = Some((e.center(), element, reaction.name));
                    }
                    e.hurt(dealt);
                    if e.kind == engine::KIND {
                        engine::component_hit(e, shot.pos, dealt, matches!(shot.spell, 2 | 13));
                    }
                    if shot.mods.mana_leech {
                        self.mana = (self.mana + wands::card_rules::MANA_LEECH).min(mana_cap);
                    }
                    if shot.mods.siphon {
                        self.health = (self.health + wands::card_rules::SIPHON_HEALTH).min(100.0 + rank as f32 * 20.0);
                    }
                    if shot.mods.blood {
                        let dealt = (base_dmg + rank as f32 * 4.0) * shot.mods.damage_mul;
                        self.health = (self.health + dealt * wands::card_rules::BLOOD_SIPHON_FRACTION).min(100.0 + rank as f32 * 20.0);
                    }
                    e.flash = 0.16;
                    if !vault_fight::handles_hit(e, shatter || matches!(shot.spell, 2 | 13) || element_stagger >= 0.4) {
                        e.stagger = (if shatter { 0.65_f32 } else { 0.24 }).max(element_stagger);
                        e.knock = shot.velocity.normalize_or_zero()
                            * if matches!(shot.spell, 3 | 14) || shot.mods.bore { 5.0 } else { 2.8 };
                        if !is_boss(e.kind) || shatter && e.poise <= 0.0 {
                            e.tell = 0.0;
                            if is_boss(e.kind) { e.poise = 3.0; }
                        }
                        e.charge = 0.0;
                        if !is_boss(e.kind) { e.cooldown = e.cooldown.max(0.6); }
                    }
                    self.hit = 0.18;
                    if matches!(shot.spell, 2 | 13) {
                        e.slow = e.slow.max(if self.discoveries & (1 << 8) != 0 || shot.spell == 13 {
                            6.0
                        } else {
                            4.0
                        });
                    } else if shatter {
                        e.slow = 0.0;
                    }
                    if shot.mods.pierce > 0 {
                        // Piercing: the bolt carries on through this foe.
                        shot.mods.pierce -= 1;
                        shot.mods.pierced = e.slot + 1;
                        if shot.mods.rune != 0 {
                            rune_glance.get_or_insert(shot.pos);
                        }
                        let (family, tint) = magic_fx::Family::for_shot(&shot);
                        self.spell_contact(shot.pos, shot.velocity, tint, family, magic_fx::Contact::Glance);
                        continue;
                    }
                    hit = true;
                    actor_hit = true;
                    break;
                }
            }
            // A native rune event runs its child once, at the first accepted hit, after
            // the bolt's own effect has landed on what it struck.
            if !shot.hostile && shot.mods.rune != 0 && (hit || rune_glance.is_some()) {
                let travel = shot.velocity.normalize_or_zero();
                let (at, normal, dir) = match (rune_glance, surface_contact) {
                    (Some(at), _) => (at, -travel, travel),
                    (None, Some((at, normal))) => (at, normal, travel - 2.0 * travel.dot(normal) * normal),
                    (None, None) => (shot.pos, -travel, travel),
                };
                let handle = std::mem::take(&mut shot.mods.rune);
                self.rune_event(world, handle, at, normal, dir);
            }
            if let Some(tether) = shot.flight.as_mut().and_then(|f| f.snapline.as_mut()) {
                if actor_hit {
                    self.mana = (self.mana + tether.connect()).min(self.max_mana());
                    self.spell_contact(shot.pos, shot.velocity, snapline::REFUND_COLOR,
                        magic_fx::Family::Thread, magic_fx::Contact::Actor);
                    sound(&mut self.sounds, Cue::Treasure, Some(shot.pos), 0.5);
                } else if hit {
                    if let Some((at, normal)) = surface_contact {
                        self.spell_contact(at, shot.velocity, tether.color(spell_color(shot.spell)),
                            magic_fx::Family::Thread, magic_fx::Contact::Surface(normal));
                    }
                    if tether.returning {
                        continue;
                    }
                    tether.returning = true;
                }
                if tether.caught(shot.pos) {
                    sound(&mut self.sounds, Cue::Swing, Some(tether.anchor), 0.25);
                } else {
                    self.projectiles.push(shot);
                }
                continue;
            }
            if hit {
                #[cfg(feature="multiplayer")]
                if !shot.hostile && (matches!(shot.spell,3|12|29)||shot.mods.volatile) {
                    self.player_blast(world,shot.pos,3.6,(34.0+rank as f32*3.0)*shot.mods.damage_mul);
                }
                if let Some(at) = crit_at {
                    self.spawn_voxel_burst(at, [1.0, 0.85, 0.35], 12, 2.8, 0.045);
                    sound(&mut self.sounds, Cue::Spark, Some(at), 0.8);
                }
                if let Some((at, element, name)) = reaction_fx {
                    self.spawn_voxel_burst(
                        at,
                        element.color(),
                        if name.is_some() { 18 } else { 7 },
                        if name.is_some() { 3.2 } else { 1.8 },
                        0.045,
                    );
                    if name.is_some() {
                        self.effect(at, element.color(), 0.9);
                        sound(&mut self.sounds, Cue::Spark, Some(at), 0.7);
                    }
                }
                self.arcana_impact(world, &shot, eye);
                if !shot.hostile && shot.mods.shrapnel {
                    self.spawn_voxel_burst(shot.pos, spell_color(62), 16, 4.5, 0.035);
                    let element = shot.mods.element;
                    for e in &mut self.enemies {
                        if e.health > 0.0 && (e.center() - shot.pos).length() < wands::card_rules::SHRAPNEL_RADIUS {
                            e.hurt(wands::card_rules::SHRAPNEL_DAMAGE * shot.mods.damage_mul * e.affixes.damage_taken());
                            if let Some(el) = element {
                                let r = e.status.apply(el, e.affixes);
                                if r.chill > 0.0 {
                                    e.slow = e.slow.max(r.chill);
                                }
                            }
                            e.flash = 0.16;
                            e.stagger = e.stagger.max(0.2);
                        }
                    }
                }
                if !shot.hostile && shot.mods.volatile {
                    self.blast_visual(
                        shot.pos,
                        1.25,
                        -shot.velocity.normalize_or_zero(),
                        [1.0, 0.42, 0.16],
                    );
                    world
                        .sandbox
                        .apply_radial_impulse(shot.pos, 2.2, 3.2, scale);
                    if self.terrain_blasts.len() < 4 {
                        self.terrain_blasts.push_back((shot.pos, 1.25));
                    }
                    for e in &mut self.enemies {
                        if e.health > 0.0 && (e.center() - shot.pos).length() < 2.0 {
                            e.health -= 10.0 * shot.mods.damage_mul;
                            e.flash = 0.16;
                            e.stagger = e.stagger.max(0.2);
                        }
                    }
                }
                if !shot.hostile && matches!(shot.spell, 6 | 10) && self.wells.len() < 4 {
                    sound(&mut self.sounds, Cue::Rift, Some(shot.pos), 0.85);
                    if shot.spell == 10 {
                        self.carve_sphere(world, shot.pos, 1.45, 10);
                    }
                    self.wells.push(rare_magic::Well {
                        pos: shot.pos,
                        life: if shot.spell == 10 { 4.8 } else { 4.0 },
                        pulse: 0.0,
                    });
                }
                if !shot.hostile && shot.spell == 12 {
                    self.thermal_bloom(world, shot.pos, 2.3, true);
                }
                if !shot.hostile && shot.spell == 28 {
                    self.thermal_bloom(world, shot.pos, 1.5, true);
                }
                if !shot.hostile && shot.spell == 29 {
                    self.blast_visual(
                        shot.pos,
                        2.6,
                        -shot.velocity.normalize_or_zero(),
                        [0.95, 0.40, 0.12],
                    );
                    world
                        .sandbox
                        .apply_radial_impulse(shot.pos, 4.2, 8.0, scale);
                    if self.terrain_blasts.len() < 4 {
                        self.terrain_blasts.push_back((shot.pos, 2.6));
                    }
                    self.thermal_bloom(world, shot.pos, 2.8, true);
                    let distance = (eye - shot.pos).length();
                    if distance < 4.0 {
                        self.damage(24.0 * (1.0 - distance / 4.0), shot.pos - eye);
                    }
                    for e in &mut self.enemies {
                        let d = (e.center() - shot.pos).length();
                        if e.health > 0.0 && d < 3.6 && line(world, shot.pos, e.center()) {
                            e.hurt(
                                (34.0 + rank as f32 * 3.0)
                                    * (1.0 - 0.5 * d / 3.6)
                                    * shot.mods.damage_mul
                                    * e.affixes.damage_taken(),
                            );
                            e.flash = 0.2;
                            e.stagger = e.stagger.max(0.5);
                            e.knock = (e.center() - shot.pos).normalize_or_zero() * 6.0;
                            self.hit = 0.18;
                        }
                    }
                    sound(&mut self.sounds, Cue::Burst, Some(shot.pos), 1.0);
                }
                if !shot.hostile && shot.mods.fork {
                    // Three weak sparks fan back out of the impact. They carry
                    // default mods, so a fork can never fork again.
                    let back = -shot.velocity.normalize_or_zero();
                    for a in wands::card_rules::FORK_ANGLES {
                        let d = (Quat::from_axis_angle(Vec3::Y, a) * back + Vec3::Y * 0.35)
                            .normalize_or_zero();
                        let mods = wands::ShotMods {
                            damage_mul: wands::card_rules::FORK_DAMAGE,
                            life_mul: 0.5,
                            ..Default::default()
                        };
                        self.emit_spell_modified(0, shot.pos + back * 0.15, d, None, mods);
                    }
                }
                if !shot.hostile && shot.spell == 72 {
                    let travel = shot.velocity;
                    self.teleport_to_impact(world, player, shot.pos, travel);
                }
                if !shot.hostile && shot.spell == 73 {
                    self.pour_water(world, shot.pos);
                }
                if !shot.hostile && shot.spell == 74 {
                    self.venom_nova(world, shot.pos, shot.mods.damage_mul);
                }
                if !shot.hostile && matches!(shot.spell, 2 | 13) {
                    self.cryo_bloom(world, shot.pos, if shot.spell == 13 { 3.2 } else { 1.35 });
                }
                if !shot.hostile && matches!(shot.spell, 14 | spells::STORM_SURGE) {
                    self.electrify_pool(world, shot.pos, rank + 1, shot.spell == spells::STORM_SURGE);
                }
                if !shot.hostile && shot.spell == 15 {
                    self.alchemical_transmute(world, shot.pos, 1.8);
                    sound(&mut self.sounds, Cue::Treasure, Some(shot.pos), 0.75);
                }
                if !shot.hostile && shot.spell == 5 && self.blooms.len() < 6 {
                    self.blooms.push(Bloom {
                        pos: shot.pos,
                        life: if self.knows(4) { 7.0 } else { 5.0 },
                        pulse: 0.0,
                    });
                    sound(&mut self.sounds, Cue::Bloom, Some(shot.pos), 0.65);
                }
                if let Some(first) = spark_target {
                    self.chain(
                        world,
                        shot.pos,
                        first,
                        rank + u32::from(shot.spell == 14) * 2,
                    );
                }
                if !shot.hostile && shot.spell == 3 {
                    self.blast_visual(
                        shot.pos,
                        2.0,
                        -shot.velocity.normalize_or_zero(),
                        [0.42, 0.38, 0.34],
                    );
                    // Resonant bursts also toss loose frames, not just terrain.
                    world
                        .sandbox
                        .apply_radial_impulse(shot.pos, 3.4, 5.5, scale);
                    if self.terrain_blasts.len() < 4 {
                        self.terrain_blasts.push_back((shot.pos, wands::card_rules::RESONANT_CRATER));
                    }
                    let distance = (eye - shot.pos).length();
                    if distance < 3.0 {
                        self.damage(28.0 * (1.0 - distance / 3.0), shot.pos - eye);
                    }
                    for e in &mut self.enemies {
                        if (e.center() - shot.pos).length() < 3.0
                            && line(world, shot.pos, e.center())
                        {
                            let shatter = e.slow > 0.0 && self.discoveries & (1 << 3) != 0;
                            e.health -=
                                (18.0 + rank as f32 * 2.0 + if shatter { 12.0 } else { 0.0 })
                                    * shot.mods.damage_mul;
                            if shatter {
                                e.slow = 0.0;
                                sound(&mut self.sounds, Cue::Shatter, Some(e.center()), 0.6);
                            }
                            e.stagger = 0.45;
                            e.knock = (e.center() - shot.pos).normalize_or_zero() * 5.0;
                            if e.kind < 6 || shatter && e.poise <= 0.0 {
                                e.tell = 0.0;
                                if is_boss(e.kind) {
                                    e.poise = 3.0;
                                }
                            }
                            self.hit = 0.18;
                        }
                    }
                }
                if let Some(payload) = shot.payload {
                    let p_dir = shot.velocity.normalize_or_zero();
                    if wands::is_effect(payload) {
                        let created = growth::resolve_effect(world, payload, shot.pos, None);
                        if let Some(follow) = shot.follow.filter(|f| wands::is_effect(*f)) {
                            growth::resolve_effect(world, follow, shot.pos, created);
                        }
                    } else if payload == 9 {
                        self.cast_luminous_drill(
                            world,
                            shot.pos - p_dir * 0.15,
                            p_dir,
                            shot.payload_mods,
                            None,
                        );
                    } else {
                        self.emit_spell_modified(payload, shot.pos, p_dir, None, shot.payload_mods);
                    }
                }
                let (burst_n, burst_speed) = match shot.spell {
                    2 | 5 | 12 | 13 => (3, 0.6),
                    0 | 14 | 83 => (4, 0.9),
                    3 | 29 => (8, 1.2),
                    _ => (5, 0.85),
                };
                if !shot.hostile && (actor_hit || surface_contact.is_some()) {
                    let (family, color) = magic_fx::Family::for_shot(&shot);
                    let (at, contact) = surface_contact.map_or((shot.pos, magic_fx::Contact::Actor),
                        |(at, normal)| (at, magic_fx::Contact::Surface(normal)));
                    self.spell_contact(at, shot.velocity, color, family, contact);
                    if matches!(shot.spell, 3 | 29) {
                        self.effect(shot.pos, shot.color(), 3.0);
                    }
                } else {
                    self.spawn_voxel_burst(shot.pos, shot.color(), burst_n, burst_speed, 0.028);
                    self.effect(shot.pos, shot.color(), if matches!(shot.spell, 3 | 29) { 3.0 } else { 0.7 });
                }
            } else {
                self.projectiles.push(shot);
            }
        }
        self.tick_blooms(world, eye, dt);
        self.tick_wells(world, dt);
        if self.hurt > 0.0 {
            self.concentration = None;
        }
        self.tick_crosses(world, dt);
        self.tick_particles(world, dt);
        modifiers::reap(&mut self.enemies);
        night::fall(self, world);
        night::reap(&mut self.enemies);
        burrowers::fall(self, world);
        self.wildlife.remember(&self.enemies);
        let mut death_blasts: Vec<Vec3> = Vec::new();
        let mut resonance_kills = Vec::new();
        // Spells found this frame: (salt, where). Applied after the loop, which
        // borrows the enemy list.
        let mut finds: Vec<(u32, Vec3, bool)> = Vec::new();
        let mut cleared_sites = Vec::new();
        let mut wand_drops = Vec::new();
        super::orbital::cargo::remember(world, &self.enemies);
        for e in &mut self.enemies {
            if e.health > 0.0 {
                continue;
            }
            if e.kind == super::orbital::cargo::ROBOT {
                sound(&mut self.sounds, Cue::Defeat, Some(e.center()), 0.7);
                continue;
            }
            if wildlife::passive(e.kind) { wildlife::fall(&mut self.wildlife, world, e); continue; }
            if let Some(id) = e.habitat {
                if settle_habitat_death(world,e,id) {
                    sound(&mut self.sounds, Cue::Defeat, Some(e.center()), 0.7);
                }
                continue;
            }
            let mask = self.records.entry(e.site).or_default();
            if *mask & (1 << e.slot) == 0 {
                if e.kind == engine::KIND {
                    engine::release_hardware(e);
                    engine::release_cargo(e, world);
                    self.engines.remove(&e.site);
                    if !e.engine.cargo.is_empty() { self.engines.insert(e.site,engine::snapshot(e)); }
                }
                if e.kind == tollkeeper::KIND {
                    tollkeeper::release_stock(e, world);
                    self.tollkeepers.remove(&e.site);
                }
                *mask |= 1 << e.slot;
                rewards::hell_camp_drop(world, e);
                world.progression.pending_xp += super::progression::xp::for_kill(e.menace, is_boss(e.kind));
                world.stats.add(super::stats::Stat::Kills, 1);
                resonance_kills.push(e.center());
                world.habitats.release(e.pos, 0, if is_boss(e.kind) { 80 } else { 8 });
                if e.affixes.has(status::Affix::Volatile) {
                    death_blasts.push(e.center());
                }
                sound(&mut self.sounds, Cue::Defeat, Some(e.center()), 0.7);
                if self.remains.len() < 24 {
                    self.remains.push(Remains {
                        pos: e.center(),
                        kind: e.kind,
                        life: 0.65,
                        yaw: e.yaw,
                    });
                }
                if self.discoveries & (1 << 7) != 0 && self.health > 0.0 {
                    self.health = (self.health + 4.0).min(100.0 + rank as f32 * 20.0);
                }
                world.expedition.add(18, 64 + e.menace as u32 * 32);
                if e.menace >= 2 {
                    world.expedition.add(16, 32);
                }
                if e.menace >= 3 {
                    world.expedition.add(17, 16);
                }
                for &(item, amount) in rewards::guardian(e.kind) {
                    world.expedition.add(item, amount);
                }
                if let Some(salt) = discovery::drop_roll(e.site, e.slot, e.menace) {
                    finds.push((salt, e.center(), false));
                }
                if let Some(drop) = wand_items::drop_roll(e.site, e.slot, e.kind, e.menace, is_boss(e.kind), rank) {
                    wand_drops.push((drop, e.center()));
                }
                if e.kind == 5 && self.weapons & 2 == 0 {
                    self.weapons |= 2;
                    world
                        .expedition
                        .say("Steel sword recovered from the warden. Equip it in Gear.");
                }
                if is_boss(e.kind) {
                    world.expedition.say(if e.kind == engine::KIND && e.engine.secured {
                        "Engine secured. F - Claim relic at the reliquary"
                    } else { "Guardian defeated. F  Claim relic at the reliquary" });
                }
                if let Some(recipe) = e.encounter.recipe {
                    for &(item, amount) in encounters::RECIPES[recipe].loot {
                        world.expedition.add(item, amount);
                    }
                }
                if is_boss(e.kind) {
                    finds.push((discovery::site_salt(e.site).wrapping_add(e.kind * 193), e.center(), true));
                }
                let all = encounters::clear_mask(e.site);
                if *mask & all == all && *mask & 256 == 0 {
                    cleared_sites.push((e.site, e.center()));
                }
            }
        }
        for (id, at) in cleared_sites { self.complete_site(world, id, at); }
        for ((seed, tier), at) in wand_drops {
            if let Some(item) = self.mint_wand(wand_items::generate(seed, tier)) {
                world.inventory.dropped.push(super::inventory::DroppedStack { pos: at, stack: super::inventory::ItemStack::new(item, 1) });
                world.expedition.say(match tier { 3 => "A masterwork wand clatters to the ground.", 2 => "A fine wand clatters to the ground.", _ => "A wand clatters to the ground." });
            }
        }
        for (salt, at, guaranteed) in finds {
            self.discover_spell(world, salt, at, guaranteed);
        }
        for at in death_blasts {
            self.blast_visual(at, 1.8, Vec3::Y, status::Affix::Volatile.color());
            self.spawn_voxel_burst(at, status::Affix::Volatile.color(), 26, 5.0, 0.05);
            sound(&mut self.sounds, Cue::Burst, Some(at), 0.9);
            let distance = (eye - at).length();
            if distance < 3.2 && line(world, at, eye) {
                self.damage(22.0 * (1.0 - distance / 3.2), eye - at);
            }
            for other in &mut self.enemies {
                if other.health > 0.0 && (other.center() - at).length() < 3.0 {
                    other.hurt(18.0);
                    other.flash = 0.16;
                }
            }
        }
        if self.rank() > rank {
            self.health = self.max_health();
            self.mana = self.max_mana();
            world.expedition.say(format!(
                "{} attained: +20 health, +15 mana, stronger strikes and spells.",
                rank_name(self.rank())
            ));
        }
        self.drain_engine_salvage(world);
        self.wildlife.remember(&self.enemies);
        for e in &self.enemies {
            wildlife::vehicle::remember_driver(world, e);
            if e.kind == engine::KIND && e.health > 0.0 && ((e.pos-feet).length() >= 90.0 || e.pos.y <= -256.0) {
                self.engines.insert(e.site, engine::snapshot(e));
            }
            if let Some(id) = e.habitat {
                // A volatile chain reaction may kill after the main death pass.
                // Settle it before saving the resident, never encode dead health.
                if e.health <= 0.0 || e.pos.y <= -256.0 {
                    settle_habitat_death(world,e,id);
                    continue;
                }
                if let Some(resident) = world.habitats.sources.iter_mut().find(|s| s.id == id).and_then(|s| s.resident.as_mut()) {
                    resident.pos = e.pos;
                    resident.health = e.health;
                    if e.kind==matron::KIND {resident.pods=e.matron.pods;resident.spores=e.matron.spores;resident.work=e.matron.work;}
                }
            }
            if e.kind == tollkeeper::KIND && e.health > 0.0 && ((e.pos-feet).length()>=90.0 || e.pos.y<=-256.0) {
                self.tollkeepers.insert(e.site, tollkeeper::snapshot(e));
            }
        }
        super::orbital::cargo::remember(world, &self.enemies);
        self.enemies.retain(|e| {
            #[cfg(feature="multiplayer")]
            let near=world.multiplayer.as_ref().map_or((e.pos-feet).length()<90.0,|c|c.players.iter().any(|p|(e.pos-p.feet).length()<90.0));
            #[cfg(not(feature="multiplayer"))]
            let near=(e.pos-feet).length()<90.0;
            if !near && e.health > 0.0 && e.pos.y > -256.0 {
                super::farming::jobs::remember_worker(world, e);
            }
            e.health>0.0 && (e.habitat.is_some() || near) && e.pos.y > -256.0
        });
        // Echo victims remain until the next death pass, so their normal loot
        // and persistent victory records settle exactly once.
        self.resonance_kills(world, &resonance_kills);
        for e in &mut self.effects {
            e.life -= dt;
        }
        self.effects.retain(|e| e.life > 0.0);
        for burst in &mut self.tool_debris {
            burst.age += dt;
        }
        self.tool_debris.retain(|burst| burst.age < 0.55);
        for blast in &mut self.blasts {
            blast.age += dt;
        }
        self.blasts.retain(|blast| blast.age < 1.2);
        for splash in &mut self.splashes {
            splash.age += dt;
        }
        self.splashes
            .retain(|splash| splash.age < splash.life && splash.life.is_finite());
        if self.health <= 0.0 && self.death.is_none() {
            self.die(world, feet);
        }
        self.refresh_focus(world, player.eye(world.voxel_scale) * scale, forward);
        world.habitats.shots=self.projectiles.iter().filter_map(|s|s.farm.map(|owner|super::habitats::FarmShot {owner,pos:s.pos,velocity:s.velocity,life:s.life})).collect();
        update_trace.mark("adventure-effects");
        consumed
    }
    pub fn take_terrain_blast(&mut self) -> Option<(Vec3, f32)> {
        self.terrain_blasts.pop_front()
    }
    /// Explosion at `centre` in metres: feedback, enemy damage, falloff damage to the player at `eye`.
    pub(crate) fn blast_damage(&mut self, centre: Vec3, radius: f32, eye: Vec3) {
        self.blast_feedback(centre, radius, Vec3::Y, [1.0, 0.55, 0.2]);
        let reach = radius * 2.0;
        let d = (eye - centre).length();
        if d < reach {
            self.damage(36.0 * (1.0 - d / reach), eye - centre);
        }
    }
    /// Dynamic point lights (position, radius, color, strength) for active spells, projectiles,
    /// black holes, plasma crosses, and sanctum altars.
    pub fn spell_lights(&self, eye: Vec3) -> Vec<(Vec3, f32, [f32; 3], f32)> {
        let mut out = Vec::with_capacity(12);
        // Reuse the bounded, occluded local-light renderer. Broken prisms stop
        // contributing immediately; no detached glow survives a destroyed node.
        for e in self.enemies.iter().filter(|e| e.kind == glassweaver::KIND && e.health > 0.0).take(2) {
            glassweaver::append_lights(e, &mut out);
        }
        // Short impact flashes and flying bolts share six existing light slots.
        let mut contacts = [(0.0, (Vec3::ZERO, 0.0, [0.0; 3], 0.0)); 6];
        let mut consider = |pos: Vec3, radius: f32, color, strength: f32| {
            let distance_sq = eye.distance_squared(pos);
            if distance_sq > 48.0 * 48.0 {
                return;
            }
            let radius_sq = radius * radius;
            let score = strength * radius_sq / (radius_sq + distance_sq);
            let index = contacts.partition_point(|&(other, _)| other >= score);
            if index < contacts.len() {
                contacts[index..].rotate_right(1);
                contacts[index] = (score, (pos, radius, color, strength));
            }
        };
        for shot in &self.projectiles {
            if let Some(tether) = shot.tether() {
                consider(shot.pos, 2.8, tether.color(shot.color()), 1.4);
            } else if shot.spell == 10 {
                consider(shot.pos, 10.5, shot.color(), 3.4);
            } else if !shot.hostile {
                let (family, tint) = magic_fx::Family::for_shot(shot);
                let radius = match family {
                    magic_fx::Family::Spore | magic_fx::Family::Acid | magic_fx::Family::Water => 4.0,
                    magic_fx::Family::Shock | magic_fx::Family::Ember => 7.5,
                    _ => 6.0,
                };
                consider(shot.pos, radius, family.color(tint, 0.84, shot.spell), family.flash_strength() * 0.60);
            } else {
                consider(shot.pos, 7.5, shot.color(), 2.4);
            }
        }
        for impact in self.effects.iter().filter(|effect| effect.impact_normal.is_some()) {
            let life = (impact.life / impact.total).clamp(0.0, 1.0);
            consider(impact.pos, 1.7 + impact.radius * 4.0,
                impact.family.color(impact.color, life, 127), impact.family.flash_strength() * life * life);
        }
        out.extend(contacts.into_iter().filter_map(|(score, light)| (score > 0.0).then_some(light)));
        for w in self.wells.iter().take(2) {
            out.push((w.pos, 11.0, spell_color(6), 3.2 * w.life.min(1.0)));
        }
        for c in self.sustained_plasma().into_iter().chain(&self.crosses).take(2) {
            out.push((c.pos, 10.0, spell_color(11), 3.0 * c.life.min(1.0)));
        }
        for l in self.lasers.iter().take(2) {
            out.push((
                l.path.get(l.path.len() / 2).copied().unwrap_or((l.a + l.b) * 0.5),
                9.5,
                l.color,
                3.6 * (l.life / l.total).clamp(0.0, 1.0),
            ));
        }
        for b in self.blooms.iter().take(2) {
            out.push((
                b.pos + Vec3::Y * 0.4,
                8.5,
                spell_color(5),
                2.1 * b.life.min(1.0),
            ));
        }
        for c in self.companions() {
            out.push((
                c.pos + Vec3::Y * (c.shown * 0.72),
                6.0,
                [0.95, 0.82, 0.35],
                1.6,
            ));
        }
        out
    }
    pub(crate) fn companions(&self) -> impl Iterator<Item = &growth::Companion> {
        #[cfg(feature = "multiplayer")]
        let remote = self.remote_companions.as_slice();
        #[cfg(not(feature = "multiplayer"))]
        let remote: &[growth::Companion] = &[];
        self.companion.iter().chain(remote)
    }
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        for v in [
            self.health.to_bits(),
            self.mana.to_bits(),
            self.wand,
            self.spell,
            self.records.len() as u32,
        ] {
            put(v);
        }
        let mut records: Vec<_> = self.records.iter().collect();
        records.sort_by_key(|(p, _)| (p.x, p.y, p.z));
        for (p, mask) in records {
            for v in p.to_array() {
                put(v as u32);
            }
            put(*mask);
        }
        put(self.hunger.to_bits());
        put(self.rations);
        put(self.weapons);
        put(self.weapon);
        for i in 0..4 {
            let primary = self.loadout.slots[i] & 0xFF;
            let aux = self.loadout.aux_slots[i];
            let aux_enc = if aux == wands::EMPTY {
                0
            } else {
                (aux + 1) & 0xFF
            };
            put(primary | (aux_enc << 8));
        }
        put(self.loadout.cursor as u32 | (self.tool_mods as u32) << 8);
        put(self.loadout.mode
            | (self.tool_head as u32) << 8
            | (self.tool_bind as u32) << 16
            | (self.tool_grip as u32) << 24);
    }
    pub fn decode(
        read: &mut impl FnMut() -> anyhow::Result<u32>,
        has_survival_equipment: bool,
        has_wand_loadout: bool,
    ) -> anyhow::Result<Self> {
        use anyhow::ensure;
        let mut a = Self::default();
        a.health = f32::from_bits(read()?);
        a.mana = f32::from_bits(read()?);
        a.wand = read()?;
        a.spell = read()?;
        let count = read()? as usize;
        ensure!(
            a.health.is_finite()
                && (0.0..=160.0).contains(&a.health)
                && a.mana.is_finite()
                && (0.0..=400.0).contains(&a.mana),
            "Invalid adventurer vitals"
        );
        ensure!(
            a.wand <= 3
                && a.spell <= MAX_SPELL_ID
                && (a.spell != wands::EMPTY || has_wand_loadout)
                && count <= records::RECORD_CEILING,
            "Invalid spell or encounter state (wand={}, spell={}, records={count})", a.wand, a.spell
        );
        // Stowing a wand can retain its last selected card in older saves.
        // Its bank still owns the deck; normalize only this inactive mirror.
        if a.wand == 0 { a.spell = 0; }
        for _ in 0..count {
            let p = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
            let mask = read()?;
            ensure!(
                p.x.unsigned_abs() < 100000
                    && p.y.unsigned_abs() < 100000
                    && (0..=5).contains(&p.z)
                    && mask
                        & !(263
                            | 8
                            | reliquaries::AWAKENED
                            | reliquaries::CLAIMED
                            | reliquaries::PHYSICAL
                            | quest::VALVES
                            | quest::BYPASS
                            | quest::RESTORED)
                        == 0,
                "Invalid encounter record"
            );
            ensure!(
                a.records.insert(p, mask).is_none(),
                "Duplicate encounter record"
            );
        }
        if has_survival_equipment {
            a.hunger = f32::from_bits(read()?);
            a.rations = read()?;
            a.weapons = read()?;
            a.weapon = read()?;
            ensure!(
                a.hunger.is_finite()
                    && (0.0..=100.0).contains(&a.hunger)
                    && a.rations <= 65535
                    && a.weapons <= 31
                    && a.weapon <= 5
                    && (a.weapon == 0 || a.owns_weapon(a.weapon)),
                "Invalid survival inventory"
            );
        }
        if has_wand_loadout {
            for i in 0..4 {
                let raw = read()?;
                let primary = raw & 0xFF;
                let aux_enc = (raw >> 8) & 0xFF;
                let aux = if aux_enc == 0 {
                    wands::EMPTY
                } else {
                    aux_enc - 1
                };
                ensure!(
                    primary <= MAX_SPELL_ID && aux <= MAX_SPELL_ID,
                    "Invalid wand spell"
                );
                a.loadout.slots[i] = primary;
                a.loadout.aux_slots[i] = aux;
            }
            let cursor_word = read()?;
            a.loadout.cursor = (cursor_word & 0xFF) as usize;
            a.tool_mods = ((cursor_word >> 8) & 0x1F) as u8;
            let mode_word = read()?;
            a.loadout.mode = mode_word & 0xFF;
            a.tool_head = ((mode_word >> 8) & 0xFF) as u8;
            a.tool_bind = ((mode_word >> 16) & 0xFF) as u8;
            a.tool_grip = ((mode_word >> 24) & 0xFF) as u8;
            let cap = wands::capacity(a.wand);
            ensure!(
                a.loadout.cursor < cap
                    && (a.loadout.mode <= 3 || a.wand == 3 && wands::is_grimoire(a.loadout.mode))
                    && a.tool_head <= 5
                    && a.tool_bind <= 5
                    && a.tool_grip <= 5,
                "Invalid wand layout"
            );
            // An explicitly emptied deck is valid; slot IDs remain checked.
        } else {
            a.loadout.slots = [a.spell, wands::EMPTY, wands::EMPTY, wands::EMPTY];
            a.loadout.aux_slots = [wands::EMPTY; 4];
        }
        ensure!(
            // The held wand's seed arrives later with the wand bank (v29), so allow
            // the largest pool any wand can roll here; the bank decode checks exactly.
            a.health <= a.max_health() && a.mana <= a.max_mana() * wand_items::MAX_MANA_SCALE,
            "Vitals exceed attained mastery"
        );
        Ok(a)
    }
}
fn realm_roster(world: &World) -> bool {
    match world.world_type {
        super::WorldType::Hall => world.generator_version >= engine_worldgen::hall::VERSION,
        super::WorldType::Manyfold => world.generator_version >= engine_worldgen::SEEDED_REALMS,
        _ => false,
    }
}

pub fn tick(
    world: &mut World,
    player: &mut Player,
    dt: f32,
    cast: bool,
    cycle: bool,
    melee: bool,
) -> bool {
    #[cfg(feature = "multiplayer")]
    let standalone = world.multiplayer.is_none();
    #[cfg(not(feature = "multiplayer"))]
    let standalone = true;
    if standalone { world.adventure.creative = world.rules.mode.creative_tools(); }
    if super::habitats::review::paused() {return false;}
    let mut trace = crate::gameplay_trace::Trace::new();
    let feet = player.pos * world.voxel_scale.meters();
    #[cfg(feature="multiplayer")]
    let shared_tick=world.multiplayer.as_ref().is_none_or(|c|c.shared_tick);
    #[cfg(not(feature="multiplayer"))]
    let shared_tick=true;
    if shared_tick {
        super::habitats::tick(world, feet, dt.clamp(0.0, 0.1));
        wildlife::vehicle::capture_driver_damage(world);
        super::farming::tick(world, engine_core::Budget::UNBOUNDED);
        wildlife::vehicle::sync_driver(world, feet);
        super::farming::jobs::service(world, feet);
        super::farming::flowers::service(world, feet);
        wildlife::husbandry(world, feet);
    }
    trace.mark("adventure-farming");
    let old_rank = world.adventure.rank();
    world.adventure.farm_mastery = world.habitats.mastered.count_ones() as usize;
    if world.adventure.rank() > old_rank {
        world.adventure.health = world.adventure.max_health();
        world.adventure.mana = world.adventure.max_mana();
        world.expedition.say("Matron breeding unlocked.");
    }
    // Reactivate the saved individual. An obstructed body waits; construction is
    // never erased and health/material state is never reset on re-entry.
    for source in &world.habitats.sources {
        let Some(resident) = &source.resident else { continue; };
        if world.adventure.enemies.len() >= MAX_ENEMIES+64 { break; }
        if world.adventure.enemies.iter().any(|e| e.habitat == Some(source.id))
            || !free(world, resident.pos, source.family.kind()) { continue; }
        let mut enemy = Enemy::new(IVec3::new(source.id as i32, 0, -27), 0, source.family.kind(), resident.pos, 0);
        enemy.habitat = Some(source.id);
        enemy.health = resident.health;
        enemy.matron.pods=resident.pods;enemy.matron.spores=resident.spores;enemy.matron.work=resident.work;
        enemy.home = source.pos.unwrap_or(resident.pos);
        enemy.encounter.recipe = None;
        enemy.encounter.concealed = false;
        world.adventure.enemies.push(enemy);
    }
    let mut adventure = std::mem::take(&mut world.adventure);
    if !adventure.farm_shots_restored {
        adventure.farm_shots_restored=true;
        for shot in &world.habitats.shots {
            adventure.emit_spell_modified(0,shot.pos,shot.velocity.normalize_or_zero(),None,Default::default());
            if let Some(p)=adventure.projectiles.last_mut() {p.farm=Some(shot.owner);p.velocity=shot.velocity;p.life=shot.life;}
        }
    }
    let feet = player.pos * world.voxel_scale.meters();
    let hand = adventure.hand_slot;
    adventure.sync_wands(&mut world.inventory, hand, feet);
    // First time the wand reaches the hand this session: name the cast keys once.
    // A save that loads with the wand already held stays quiet.
    match adventure.wand_tip {
        0 => adventure.wand_tip = if adventure.wand > 0 { 2 } else { 1 },
        1 if adventure.wand > 0 && !adventure.creative => {
            adventure.wand_tip = 2;
            world.expedition.say_for(format!("Wand Ready · {}", crate::player::spell_binding_hint()), 8.0);
        }
        _ => {}
    }
    adventure.mending_beacon(world, player.eye(world.voxel_scale) * world.voxel_scale.meters(), dt);
    if shared_tick {
        adventure.habitat_emitters(world);
        adventure.harvest_beacons(world);
    }
    trace.mark("adventure-reactivation");
    let consumed = adventure.update(world, player, dt, cast, cycle, melee);
    trace.mark("adventure-update");
    world.adventure = adventure;
    death::settle(world);
    enemy_review::after(world, player);
    world.reconcile_legacy_to_inventory();
    world.sync_from_inventory();
    super::progression::step(world, dt);
    super::stats::step(world, player, dt);
    consumed
}

pub(crate) fn habitat_space(world: &World, pos: Vec3, kind: u32) -> bool { free(world, pos, kind) }

/// Bestiary kind and name for `name`, ignoring case, spaces and underscores.
pub(crate) fn species_named(name: &str) -> Option<(u32, &'static str)> {
    let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let wanted = key(name);
    bestiary::SPECIES.iter().find(|(_, s)| key(s.name) == wanted).map(|(kind, s)| (*kind, s.name))
}

/// Places one creature of `kind` at `feet_m` for `/summon`.
pub(crate) fn summon(world: &mut World, kind: u32, feet_m: Vec3) -> Result<(), &'static str> {
    if world.adventure.enemies.len() >= MAX_ENEMIES + 64 {
        return Err("Creature limit reached");
    }
    if !free(world, feet_m, kind) {
        return Err("No room at that spot");
    }
    let site = (feet_m / world.voxel_scale.meters()).floor().as_ivec3();
    let slot = 1000 + world.adventure.enemies.len() as u32;
    world.adventure.enemies.push(Enemy::new(site, slot, kind, feet_m, 0));
    Ok(())
}
pub(crate) fn habitat_health(kind: u32) -> f32 { enemy_health(kind) }
fn settle_habitat_death(world: &mut World,e: &Enemy,id:u32) -> bool {
    if let Some(resident)=world.habitats.sources.iter_mut().find(|s|s.id==id).and_then(|s|s.resident.as_mut()) {
        resident.pods=e.matron.pods; resident.spores=e.matron.spores; resident.work=e.matron.work;
    }
    if let Some((resource,amount))=world.habitats.death(id,e.pos) {
        world.expedition.add(resource,amount); true
    } else {false}
}
impl Adventure {
    pub(crate) fn restore_farm_mastery(&mut self, flags: u32) {
        self.farm_mastery = flags.count_ones() as usize;
    }
}

fn emitter_can_target(enemy: &Enemy, owner: u32, defense: bool) -> bool {
    if defense {
        enemy.habitat.is_none() && enemy.hostile()
    } else {
        enemy.habitat == Some(owner) && !wildlife::passive(enemy.kind)
    }
}

impl Adventure {
    fn mending_beacon(&mut self, world: &mut World, eye: Vec3, dt: f32) {
        use super::habitats::{BEACON_RANGE, Beacon, MEND_HEALTH, MEND_MANA};

        if dt <= 0.0 {
            return;
        }
        self.beacon_mend_cd = (self.beacon_mend_cd - dt.min(0.1)).max(0.0);
        let cap = self.max_health();
        if self.beacon_mend_cd > 0.0 || self.is_dead() || self.health <= 0.0 || self.health >= cap {
            return;
        }
        let Some((index, origin)) = world.habitats.sources.iter().enumerate()
            .filter(|(_, s)| s.receiver && s.beacon == Beacon::Mending && s.usable_mana() >= MEND_MANA)
            .filter_map(|(i, s)| {
                let origin = s.pos? + Vec3::new(-0.7, 1.1, 0.0);
                let distance = origin.distance_squared(eye);
                (distance <= BEACON_RANGE * BEACON_RANGE && trace::solid(world, origin, eye).is_none())
                    .then_some((i, origin, distance, s.id))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.3.cmp(&b.3)))
            .map(|(i, origin, _, _)| (i, origin))
        else {
            return;
        };
        world.habitats.sources[index].spend_mana(MEND_MANA);
        self.health = (self.health + MEND_HEALTH).min(cap);
        self.beacon_mend_cd = 1.0;
        self.effect(eye - Vec3::Y * 0.5, Beacon::Mending.color(), 0.8);
        sound(&mut self.sounds, Cue::Mend, Some(origin), 0.55);
    }

    fn harvest_beacons(&mut self, world: &mut World) {
        use super::habitats::{BEACON_RANGE, Beacon, HARVEST_DAMAGE, HARVEST_MANA};

        for index in 0..world.habitats.sources.len() {
            let source = &world.habitats.sources[index];
            if !source.receiver || source.beacon != Beacon::Harvest || source.emit_ticks != 0 {
                continue;
            }
            let limit = (source.usable_mana() / HARVEST_MANA).min(4) as usize;
            let Some(p) = source.pos.filter(|_| limit > 0) else { continue; };
            let origin = p + Vec3::new(-0.7, 1.1, 0.0);
            let mut targets: Vec<_> = self.enemies.iter().enumerate()
                .filter(|(_, e)| e.health > 0.0 && !wildlife::passive(e.kind))
                .filter_map(|(i, e)| {
                    let owner = e.habitat?;
                    let center = e.center();
                    let distance = origin.distance_squared(center);
                    (distance <= BEACON_RANGE * BEACON_RANGE
                        && world.habitats.sources.iter().any(|s| s.id == owner && !s.reserve)
                        && trace::solid(world, origin, center).is_none())
                        .then_some((i, distance, owner))
                })
                .collect();
            targets.sort_unstable_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
            let hits = targets.len().min(limit);
            if hits == 0 {
                continue;
            }
            let source = &mut world.habitats.sources[index];
            source.spend_mana(HARVEST_MANA * hits as u32);
            source.emit_ticks = 4;
            for (i, _, _) in targets.into_iter().take(hits) {
                let enemy = &mut self.enemies[i];
                enemy.health = (enemy.health - HARVEST_DAMAGE).max(0.0);
                let center = enemy.center();
                self.effect(center, Beacon::Harvest.color(), 0.7);
            }
            self.effect(origin, Beacon::Harvest.color(), 1.25);
        }
    }

    fn habitat_emitters(&mut self, world: &mut World) {
        for index in 0..world.habitats.sources.len() {
            let source = &world.habitats.sources[index];
            if !source.emitter || !source.receiver || source.emit_ticks > 0 || self.projectiles.len() >= 96 { continue; }
            let Some(p) = source.pos else { continue; };
            let origin = p + Vec3::new(-0.7, 1.1, 0.0);
            let cost = spell_cost(0).ceil() as u32;
            if source.usable_mana() < cost {
                continue;
            }
            let Some(target) = self.enemies.iter()
                .filter(|e| e.health > 0.0 && emitter_can_target(e, source.id, source.reserve))
                .filter(|e| e.pos.distance_squared(p) < 6.0 * 6.0)
                .filter(|e| trace::solid(world, origin, e.center()).is_none())
                .min_by(|a, b| a.pos.distance_squared(p).total_cmp(&b.pos.distance_squared(p))
                    .then(a.site.to_array().cmp(&b.site.to_array())).then(a.slot.cmp(&b.slot)))
            else {
                continue;
            };
            let center = target.center();
            let flight_time = origin.distance(center) / spell_base_speed(0);
            let lead = center + target.velocity * flight_time.min(0.3);
            let aim = if trace::solid(world, origin, lead).is_none() { lead } else { center };
            let source = &mut world.habitats.sources[index];
            source.spend_mana(cost);
            source.emit_ticks = (spell_cooldown(0)*4.0).ceil().max(1.0) as u32;
            self.emit_spell_modified(0,origin,(aim-origin).normalize_or_zero(),None,Default::default());
            if let Some(shot)=self.projectiles.last_mut() {shot.farm=Some(source.id);}
        }
    }
}

#[cfg(test)]
mod flavor_tests {
    use super::*;

    /// Every id that has a name. Unnamed ids fall through to "Unknown spell".
    fn defined() -> Vec<u32> {
        (0..=MAX_SPELL_ID)
            .filter(|id| *id != wands::EMPTY && spell_name(*id) != "Unknown spell")
            .collect()
    }

    #[test]
    fn every_defined_spell_is_complete_and_unique() {
        let ids = defined();
        assert!(ids.len() >= 56, "spell table shrank to {}", ids.len());
        let mut names = std::collections::HashSet::new();
        for id in ids {
            let name = spell_name(id);
            assert!(names.insert(name), "spell {id} duplicates the name {name}");
            assert_ne!(spell_hint(id), "Empty wand slot.", "spell {id} ({name}) has no hint");
            assert!(spell_cost(id) >= 0.0 && spell_cost(id) <= 60.0, "{name} cost");
            assert!(spell_cooldown(id) >= 0.0, "{name} cooldown");
            let c = spell_color(id);
            assert!(c.iter().all(|v| (0.0..=1.0).contains(v)), "{name} colour");
            assert!(spell_available(id, 3, !0), "{name} must be reachable on the top wand");
            let tag = spell_kind_tag(id);
            // The evaluator's classification must agree with the UI tag, so a
            // modifier can never be tagged as a bolt (or the reverse).
            assert_eq!(
                wands::is_modifier(id),
                matches!(tag, "MODIFIER" | "MULTICAST" | "TRIGGER MOD" | "ELEMENT"),
                "{name} ({id}) tag {tag} disagrees with is_modifier"
            );
        }
    }

    #[test]
    fn unnamed_ids_are_visible_instead_of_posing_as_a_real_spell() {
        assert_eq!(spell_name(33), "Unknown spell");
        assert_eq!(spell_name(MAX_SPELL_ID + 1), "Unknown spell");
        assert!(!spell_available(33, 3, !0));
    }

    #[test]
    fn new_spells_unlock_in_tiers() {
        // Cheap tools arrive with the first wand; heavy ones need the top wand
        // or deep rare-magic progress.
        assert!(spell_available(28, 1, 0) && spell_available(32, 1, 0));
        assert!(!spell_available(30, 1, 0) && spell_available(30, 2, 0));
        assert!(!spell_available(29, 2, 0) && spell_available(29, 3, 0));
        assert!(spell_available(29, 2, (1 << 6) | (1 << 7)));
    }
}

#[cfg(feature="multiplayer")]
fn network_affinity() -> f32 { 1.0 }
fn network_site_modifier()->engine_worldgen::sites::modifiers::SiteModifier {engine_worldgen::sites::modifiers::SiteModifier::None}
#[cfg(feature="multiplayer")]
fn network_zone_fx()->crate::world::zones::Fx {crate::world::zones::Fx::NEUTRAL}

impl Adventure {
    /// Danger tier (0-3, `assets/danger.ron`) at `at_m` metres in `biome`.
    pub fn danger_at(&self, at_m: glam::Vec3, biome: Option<engine_worldgen::landscape::Biome>) -> u8 {
        let from_spawn = self.respawn_point.map_or(0.0, |p| glam::Vec2::new(at_m.x - p.x, at_m.z - p.z).length());
        habitat::danger(from_spawn, biome)
    }

    pub fn realm_spawn(&self) -> super::spawn::RealmSpawn {
        super::spawn::RealmSpawn { point: self.respawn_point, binding: self.spawn_binding }
    }

    pub fn set_realm_spawn(&mut self, spawn: super::spawn::RealmSpawn) {
        self.respawn_point = spawn.point;
        self.spawn_binding = spawn.binding;
    }

    /// These objects and coordinates belong to a realm, not to its visitor.
    pub(crate) fn swap_realm_shared(&mut self, other: &mut Self) {
        macro_rules! exchange {($($field:ident),*)=>{$(std::mem::swap(&mut self.$field,&mut other.$field);)*};}
        exchange!(enemies, records, wildlife, reliquaries, relic_discovery, tollkeepers, engines, sigils,
            route, stations, respawn_point, spawn_binding, last_death_pos, projectiles, effects,
            tool_debris, blasts, splashes, remains, arcs, blooms, sounds, wells,
            particles, lasers, crosses, dirty_voxels, hazards, terrain_blasts,
            water_source, relic_focus, undercrown_focus, boss_view, enemy_focus, night);
    }
}

#[cfg(feature="multiplayer")]
impl Adventure {
    pub(crate) fn swap_multiplayer_shared(&mut self,other:&mut Self) {
        macro_rules! exchange {($($field:ident),*)=>{$(std::mem::swap(&mut self.$field,&mut other.$field);)*};}
        exchange!(manyfold_phase,tool_bank,enemies,records,wildlife,reliquaries,tollkeepers,engines,sigils,route,stations,
            discoveries,discoveries_ready,rare_spells,spell_log);
    }

    /// Move only the displayed shots between host images. A late image can
    /// leave the shots paused, but cannot predict damage or flight through walls.
    pub(crate) fn advance_multiplayer_projectiles(world: &mut World, dt: f32) {
        const MAX_LEAD: f32 = 0.10;
        let dt = dt.min(MAX_LEAD - world.adventure.replica_projectile_lead);
        if dt <= 0.0 || !dt.is_finite() { return; }
        world.adventure.replica_projectile_lead += dt;
        let mut shots = std::mem::take(&mut world.adventure.projectiles);
        shots.retain_mut(|shot| {
            shot.life -= dt;
            if shot.life <= 0.0 { return false; }
            let start = shot.precise_at(shot.pos);
            if let Some(tether) = shot.flight.as_mut().and_then(|f| f.snapline.as_mut()) {
                let delta = tether.travel(shot.pos, &mut shot.velocity, dt);
                let returning = tether.returning;
                let end = shot.next_position(delta);
                if delta != Vec3::ZERO && let Some(contact) = trace::solid_contact_precise(world, start, shot.precise_at(end)) {
                    if returning {
                        return false;
                    }
                    if let Some(tether) = shot.flight.as_mut().and_then(|f| f.snapline.as_mut()) {
                        tether.returning = true;
                    }
                    shot.correct_position(contact.pos);
                } else {
                    shot.pos = end;
                }
                return !shot.tether().is_some_and(|tether| tether.caught(shot.pos));
            }
            if !shot.hostile { shot.apply_gravity(dt); }
            let delta = if !shot.hostile && shot.mods.path != 0 {
                shot.flight.get_or_insert_with(|| trajectory::Flight::new(shot.velocity))
                    .advance(&mut shot.velocity, dt, shot.mods.path)
            } else {
                shot.velocity * dt
            };
            let end = shot.next_position(delta);
            if delta != Vec3::ZERO && end.is_finite() {
                if let Some(contact) = trace::solid_contact_precise(world, start, shot.precise_at(end)) {
                    shot.correct_position(contact.pos);
                } else {
                    shot.pos = end;
                }
            }
            true
        });
        world.adventure.projectiles = shots;
    }
    pub(crate) fn smooth_multiplayer_actors(&mut self, previous: &Self) {
        let key = |enemy: &Enemy| (enemy.site, enemy.slot, enemy.kind, enemy.habitat);
        let poses: std::collections::HashMap<_, _> = previous.enemies.iter()
            .map(|enemy| (key(enemy), enemy_presentation::replica_pose(enemy))).collect();
        for enemy in &mut self.enemies {
            let target = enemy_presentation::replica_pose(enemy);
            // New actors, teleports and reset spawn identities appear immediately.
            if let Some(&shown) = poses.get(&key(enemy))
                && target.can_smooth(shown, wildlife::vehicle::driver_id(enemy).is_some())
            {
                enemy.replica_target = Some(target);
                enemy_presentation::apply_replica_pose(enemy, shown);
            }
        }
        if let (Some(cap), Some(old)) = (&mut self.companion, &previous.companion)
            && cap.is_rescued() && cap.id == old.id && old.pos.distance_squared(cap.pos) < 4.0
        {
            self.companion_replica_target = Some(cap.pos);
            cap.pos = old.pos;
        }
    }

    /// Advance replica presentation without running combat, AI, or world writes.
    pub(crate) fn advance_multiplayer_visuals(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 { return; }
        let blend = 1.0 - (-20.0 * dt).exp();
        for enemy in &mut self.enemies {
            if let Some(target) = enemy.replica_target {
                let shown = target.blend_from(enemy_presentation::replica_pose(enemy), dt);
                enemy_presentation::apply_replica_pose(enemy, shown);
            }
            enemy_presentation::advance_replica_pose(enemy, dt);
        }
        if let (Some(cap), Some(target)) = (&mut self.companion, self.companion_replica_target) {
            cap.pos = cap.pos.lerp(target, blend);
        }
        self.manyfold_phase = (self.manyfold_phase + dt).rem_euclid(9.0);
        self.time = (self.time + dt) % 10000.0;
        for timer in [&mut self.cast_pose, &mut self.melee_cd, &mut self.cast_cd,
            &mut self.mana_flash, &mut self.hurt, &mut self.hit] {
            *timer = (*timer - dt).max(0.0);
        }
        for effect in &mut self.effects { effect.life -= dt; }
        self.effects.retain(|effect| effect.life > 0.0);
        for arc in &mut self.arcs { arc.life -= dt; }
        self.arcs.retain(|arc| arc.life > 0.0);
        for remains in &mut self.remains { remains.life -= dt; }
        self.remains.retain(|remains| remains.life > 0.0);
        for beam in &mut self.lasers { beam.life -= dt; }
        self.lasers.retain(|beam| beam.life > 0.0);
        for bloom in &mut self.blooms { bloom.life -= dt; }
        self.blooms.retain(|bloom| bloom.life > 0.0);
        for cross in &mut self.crosses { cross.life -= dt; }
        self.crosses.retain(|cross| cross.life > 0.0);
        for particle in &mut self.particles {
            particle.life -= dt;
            if particle.family == magic_fx::Family::Plain {
                particle.vel.y -= particle.gravity * dt;
            } else {
                particle.vel = particle.family.velocity(particle.pos, particle.vel, particle.gravity, self.time, dt);
            }
            particle.pos += particle.vel * dt;
        }
        self.particles.retain(|particle| particle.life > 0.0);
        for burst in &mut self.tool_debris { burst.age += dt; }
        self.tool_debris.retain(|burst| burst.age < 0.55);
        for blast in &mut self.blasts { blast.age += dt; }
        self.blasts.retain(|blast| blast.age < 1.2);
        for splash in &mut self.splashes { splash.age += dt; }
        self.splashes.retain(|splash| splash.age < splash.life && splash.life.is_finite());
    }

    pub(crate) fn merge_multiplayer_visuals(&mut self,mut other:Self) {
        if let Some(companion) = other.companion.take().filter(growth::Companion::is_rescued) {
            self.remote_companions.push(companion);
        }
        macro_rules! append {($($field:ident),*)=>{$(self.$field.append(&mut other.$field);)*};}
        append!(projectiles,effects,tool_debris,blasts,remains,arcs,blooms,particles,lasers,crosses,wells,sounds);
        self.particles.truncate(480);
        self.splashes.append(&mut other.splashes);
        self.splashes.sort_by(|a,b|a.age.total_cmp(&b.age));
        self.splashes.dedup_by(|a,b| {
            a.pos.distance_squared(b.pos)<0.01 && (a.age-b.age).abs()<0.08
        });
        self.splashes.truncate(spells::MAX_SPLASHES);
        // Foreign actors are presentation only. Their resources and active
        // spell timers never overwrite this client's own character.
    }
    pub(crate) fn multiplayer_damage(&mut self,amount:f32,direction:Vec3,armor:f32) {
        if self.death.is_none() && amount.is_finite() && amount>0.0 {self.damage(amount*(1.0-armor.clamp(0.0,0.9)),direction);}
    }
}

/// Camera tremor for one frame: volcanic quakes plus burrowers grinding nearby.
pub fn ground_shake(time_s: f32) -> glam::Vec3 {
    super::volcanism::shake(time_s) + burrowers::shake(time_s)
}
