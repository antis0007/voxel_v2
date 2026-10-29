//! Bounded local encounters, persistent victories, and player-cast magic.
use super::{World, collides_world, factory, sites, trace, undercrown};
use crate::audio::{Cue, Event as Sound};
use crate::{mesh::models::Model, player::Player};
use glam::{IVec3, Quat, Vec3};
use std::collections::HashMap;
mod bestiary;
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
mod matron;
mod engine;
pub use engine::review::{install as install_ossuary_review, observe as observe_ossuary_review, input as ossuary_review_input, casting as ossuary_review_cast, automatic as ossuary_review_automatic, checkpoint as ossuary_review_checkpoint};
mod glassweaver;
mod boss_look;
mod tollkeeper;
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
pub mod inscriptions;
mod quest;
mod rare_magic;
pub mod reliquaries;
mod rewards;
pub mod resonance;
mod spells;
mod arcana;
pub mod status;
mod tactics;
mod navigation;
mod encounters;
mod records;
#[cfg(test)]
mod records_tests;
mod modifiers;
pub mod tinkering;
pub mod wands;
pub mod wand_items;

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
pub const MAX_SPELL_ID: u32 = 95;


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
fn zone_archetype(world: &World, pos: Vec3) -> Option<crate::world::zones::Archetype> {
    let cfg = world.sample_config();
    let inv = cfg.inv_voxel_meters();
    let surface = cfg.world_y(
        world.terrain_height_at((pos.x * inv).floor() as i32, (pos.z * inv).floor() as i32),
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
        6..=8 | tollkeeper::KIND => bosses::name(kind),
        9 => "Gravelback Colossus",
        _ => "Sanctum Guardian",
    }
}
fn is_boss(kind: u32) -> bool {
    matches!(kind, 6..=8 | tollkeeper::KIND | matron::KIND)
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
pub struct Enemy {
    /// Source identity owns recurrence; these actors never enter site-clear records.
    pub(crate) habitat: Option<u32>,
    pub site: IVec3,
    pub slot: u32,
    pub kind: u32,
    pub pos: Vec3,
    pub home: Vec3,
    pub health: f32,
    pub slow: f32,
    pub tell: f32,
    pub phase: f32,
    yaw: f32,
    velocity: Vec3,
    navigation: navigation::Navigation,
    gait: f32,
    recovery: f32,
    encounter: encounters::State,
    cooldown: f32,
    sight: f32,
    visible: bool,
    last_seen: Vec3,
    alert: f32,
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
    engine: engine::State,
    glass: glassweaver::State,
    /// Grazer youth: 1 at birth, 0 when grown; the model scales with it.
    pub(crate) youth: f32,
}

#[cfg(test)]
#[path = "adventure/model_material_tests.rs"]
mod model_material_tests;
impl Enemy {
    pub(crate) fn preparing_stamp(&self)->bool {
        self.kind==matron::KIND && self.matron.run.as_ref().is_some_and(|r|r.kind==matron::Action::Stamp && r.phase()==action::Phase::Prepare)
    }
    fn new(site: IVec3, slot: u32, kind: u32, pos: Vec3, menace: u8) -> Self {
        let menace = menace.min(3);
        Self {
            site,
            habitat: None,
            slot,
            kind,
            pos,
            home: pos,
            health: enemy_health(kind) * (1.0 + menace as f32 * 0.4),
            slow: 0.0,
            tell: 0.0,
            phase: 0.0,
            yaw: 0.0,
            velocity: Vec3::ZERO,
            navigation: navigation::Navigation::default(),
            gait: 0.0,
            recovery: 0.0,
            encounter: encounters::State::new(site, slot, kind, pos),
            cooldown: 2.0,
            sight: 0.0,
            visible: false,
            last_seen: pos,
            alert: 0.0,
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
            engine: engine::State::default(),
            glass: glassweaver::State::default(),
            youth: 0.0,
        }
    }
    fn with_affixes(mut self, affixes: status::Affixes) -> Self {
        let base = enemy_health(self.kind) * (1.0 + self.menace as f32 * 0.4);
        self.affixes = affixes;
        self.health = base * affixes.health_mul();
        self.shield = base * affixes.shield_frac();
        self
    }
    pub fn max_health(&self) -> f32 {
        enemy_health(self.kind) * (1.0 + self.menace as f32 * 0.4) * self.affixes.health_mul()
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
        if let Some(species) = bestiary::species(self.kind) {
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
        self.pos + Vec3::Y * height
    }
    pub fn model(&self, m: &mut Model, time: f32) {
        if wildlife::passive(self.kind) { wildlife::draw(self, m, time); return; }
        if encounters::disguise(self, m, time) { return; }
        if is_boss(self.kind) {
            bosses::model(self, m, time);
            return;
        }
        let mark = m.mark();
        let wind = if self.tell > 0.0 {
            let t = (1.0 - self.tell / self.windup()).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        } else {
            let t = (self.recovery / 0.24).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let hit = if self.flash > 0.0 {
            [1.0, 0.94, 0.69]
        } else if self.slow > 0.0 {
            [0.35, 0.73, 0.96]
        } else if self.tell > 0.0 {
            [0.95, 0.37, 0.15]
        } else {
            [0.49, 0.32, 0.63]
        };
        if matches!(self.kind, 0 | 1 | 4 | 5 | 10..=13 | 15) {
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
    /// First surface crossing emits a splash; subsequent substeps in the same
    /// water body must not produce a ring per step.
    pub splashed: bool,
    pub follow: Option<u32>,
}
impl Projectile {
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
pub struct Remains {
    pub pos: Vec3,
    pub kind: u32,
    pub life: f32,
}
pub struct Arc {
    pub a: Vec3,
    pub b: Vec3,
    pub life: f32,
}
pub struct Bloom {
    pub pos: Vec3,
    pub life: f32,
    pulse: f32,
}
#[derive(Clone)]
pub struct Reward {
    pub name: &'static str,
    pub items: Vec<(usize, u32)>,
    pub seconds: f32,
    pub discovery: Option<u32>,
}
pub struct Effect {
    pub pos: Vec3,
    pub color: [f32; 3],
    pub radius: f32,
    pub life: f32,
    pub total: f32,
}
#[derive(Clone, Copy, Default)]
pub struct EnemyFocus {
    pub kind: u32,
    pub name: &'static str,
    pub health: f32,
    pub maximum: f32,
    pub frozen: bool,
    pub affixes: status::Affixes,
    pub status: status::Status,
    pub shield: f32,
}

/// The next authored objective, resolved to a world-space point for the compass.
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
        spell_available(spell, self.wand, self.rare_spells) && self.spell_log.grants(spell)
    }
}

#[derive(Clone, Default)]
pub struct View {
    pub resonance: resonance::State,
    /// Spells the player has found; unfound spells show as locked.
    pub spell_log: discovery::SpellLog,
    pub health: f32,
    pub mana: f32,
    pub wand: u32,
    pub spell: u32,
    pub loadout: wands::Loadout,
    /// Every owned wand, for inventory cards.
    pub wand_bank: Vec<wand_items::WandRecord>,
    pub active_wand: Option<u16>,
    pub cast_total: f32,
    pub cast_delay_total: f32,
    pub recharge_total: f32,
    pub mana_flash: f32,
    pub nearby: usize,
    pub defeated: usize,
    pub hurt: f32,
    pub damage_direction: Vec3,
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
    pub rare_spells: u32,
    pub quest: Option<QuestMarker>,
    pub relic_focus: Option<reliquaries::RelicFocus>,
    pub undercrown_focus: Option<super::interaction::Prompt>,
    pub boss: Option<reliquaries::BossView>,
    pub enemy_focus: Option<EnemyFocus>,
    pub respawn_point: Option<Vec3>,
    pub last_death_pos: Option<Vec3>,
}
pub struct Adventure {
    pub resonance: resonance::State,
    pub health: f32,
    pub mana: f32,
    pub wand: u32,
    pub spell: u32,
    pub loadout: wands::Loadout,
    /// Every wand the player has owned; items reference these by index.
    pub wand_bank: Vec<wand_items::WandRecord>,
    pub active_wand: Option<u16>,
    /// Hotbar slot in the player's hand, set by the frame loop before `tick`.
    pub hand_slot: usize,
    pub last_cast: u32,
    cast_total: f32,
    cast_delay_total: f32,
    recharge_total: f32,
    mana_flash: f32,
    pub respawn_point: Option<Vec3>,
    pub last_death_pos: Option<Vec3>,
    terrain_blasts: std::collections::VecDeque<(Vec3, f32)>,
    pub time: f32,
    /// Modifier of the site the player stands in; re-derived each step, not saved.
    pub(super) site_modifier: engine_worldgen::sites::modifiers::SiteModifier,
    haunt_clock: f32,
    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub tool_debris: Vec<crate::tool_effects::Burst>,
    pub blasts: Vec<crate::tool_effects::Blast>,
    pub splashes: Vec<crate::tool_effects::Splash>,
    pub remains: Vec<Remains>,
    pub arcs: Vec<Arc>,
    pub blooms: Vec<Bloom>,
    pub sounds: Vec<Sound>,
    pub ward: f32,
    pub ward_hits: u32,
    pub hunger: f32,
    /// Session choice, not saved: a save-format field needs its own version bump.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub hunger_mode: hunger::HungerMode,
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
    reward: Option<Reward>,
    pub discoveries: u32,
    discoveries_ready: bool,
    pub rare_spells: u32,
    /// Spells the player has found. Persisted from save version 19.
    pub spell_log: discovery::SpellLog,
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
    wells: Vec<rare_magic::Well>,
    pub particles: Vec<spells::VoxelParticle>,
    /// Underground zone bending particle behaviour; neutral on the surface.
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
    tollkeepers: HashMap<IVec3, tollkeeper::Saved>,
    engines: HashMap<IVec3, engine::Saved>,
    pub companion: Option<growth::Companion>,
    cast_cd: f32,
    queued_cast: bool,
    queued_cycle: bool,
    melee_cd: f32,
    invulnerable: f32,
    spawn_timer: f32,
    hurt: f32,
    damage_direction: Vec3,
    damage_amount: f32,
    hit: f32,
    combo: u32,
    combo_time: f32,
}
impl Default for Adventure {
    fn default() -> Self {
        Self {
            resonance: resonance::State::default(),
            health: 100.0,
            mana: 100.0,
            wand: 0,
            spell: 0,
            loadout: Default::default(),
            wand_bank: Vec::new(),
            active_wand: None,
            hand_slot: 0,
            last_cast: 0,
            cast_total: 0.28,
            cast_delay_total: 0.28,
            recharge_total: 0.0,
            mana_flash: 0.0,
            respawn_point: None,
            last_death_pos: None,
            terrain_blasts: Default::default(),
            time: 0.0,
            site_modifier: engine_worldgen::sites::modifiers::SiteModifier::None,
            haunt_clock: 0.0,
            enemies: Vec::new(),
            projectiles: Vec::new(),
            effects: Vec::new(),
            tool_debris: Vec::new(),
            blasts: Vec::new(),
            splashes: Vec::new(),
            remains: Vec::new(),
            arcs: Vec::new(),
            blooms: Vec::new(),
            sounds: Vec::new(),
            ward: 0.0,
            ward_hits: 0,
            hunger: 100.0,
            hunger_mode: hunger::HungerMode::default(),
            rations: 0,
            weapons: 0,
            weapon: 0,
            tool_head: 0,
            tool_bind: 0,
            tool_grip: 0,
            tool_mods: 0,
            reward: None,
            discoveries: 0,
            discoveries_ready: false,
            rare_spells: 0,
            spell_log: discovery::SpellLog::default(),
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
            tollkeepers: HashMap::new(),
            engines: HashMap::new(),
            companion: None,
            cast_cd: 0.0,
            queued_cast: false,
            queued_cycle: false,
            melee_cd: 0.0,
            invulnerable: 2.0,
            spawn_timer: 0.0,
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
    let s = world.voxel_scale.meters();
    let (h, width) = if kind==matron::KIND {
        (2.5,1.1)
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
    !collides_world(
        world,
        (pos - Vec3::new(width, 0.0, width)) / s,
        (pos + Vec3::new(width, h, width)) / s,
    )
}
fn line(world: &World, a: Vec3, b: Vec3) -> bool {
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
        let base = if self.finisher() { 0.46 } else { 0.30 };
        (base / tinkering::grip_multiplier(self.tool_grip)).max(0.16)
    }
    /// Seconds from swing start to contact. Keep below swing_duration().
    pub fn swing_impact_time(&self) -> f32 {
        if self.finisher() { 0.14 } else { 0.075 }
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
            resonance: self.resonance,
            health: self.health,
            mana: self.mana,
            wand: self.wand,
            spell: self.spell,
            loadout: self.loadout,
            wand_bank: self.wand_bank.clone(),
            active_wand: self.active_wand,
            cast_total: self.cast_total,
            cast_delay_total: self.cast_delay_total,
            recharge_total: self.recharge_total,
            mana_flash: self.mana_flash,
            nearby: self.enemies.len(),
            defeated: self.records.values().filter(|n| **n & 256 != 0).count(),
            hurt: self.hurt,
            damage_direction: self.damage_direction,
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
            rare_spells: self.rare_spells,
            spell_log: self.spell_log,
            sigils: self.sigils,
            heart_restored: self.heart_restored(),
            workshop_returned: super::workshop::returned(self),
            workshop_available: false,
            workshop_cleared: super::workshop::cleared(self),
            route: self.route.clone(),
            site_modifier: modifiers::hud(self.site_modifier),
            quest: None,
            relic_focus: self.relic_focus,
            undercrown_focus: self.undercrown_focus,
            boss: self.boss_view.clone(),
            enemy_focus: self.enemy_focus,
            respawn_point: self.respawn_point,
            last_death_pos: self.last_death_pos,
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
            });
        }
    }
    fn damage(&mut self, amount: f32, direction: Vec3) {
        let amount = amount * self.resonance.incoming();
        if self.invulnerable <= 0.0 {
            let blocked = self.ward > 0.0 && self.ward_hits > 0;
            if blocked {
                self.ward_hits -= 1;
                if self.ward_hits == 0 {
                    self.ward = 0.0;
                }
            }
            self.health = (self.health - amount * if blocked { 0.35 } else { 1.0 }).max(0.0);
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
    fn spawn(&mut self, world: &mut World, player: Vec3) {
        let encounters = sites::encounters(&world.sample_config(), player);
        let nearby: Vec<IVec3> = encounters.iter().map(|e| e.0).collect();
        for (id, center, kind) in encounters {
            if !self.admit_record(id, &nearby) {
                continue;
            }
            let record = {
                let record = self.records.entry(id).or_default();
                if *record & 256 == 0 {
                    *record |= reliquaries::PHYSICAL;
                }
                *record
            };
            let count = encounters::slots(id);
            if record & ((1 << count) - 1) == (1 << count) - 1 && record & 256 == 0 {
                self.complete_site(world, id, center);
            }
            for slot in 0..count {
                let variation = (id.x.wrapping_mul(31) ^ id.y.wrapping_mul(17) ^ world.seed as i32)
                    .unsigned_abs();
                let menace = (kind >> 8) as u8;
                let kind = kind & 0xFF;
                let mut kind = if slot == 0 {
                    kind
                } else {
                    // Escorts contrast the lead by role: ranged pressure or a
                    // flyer beside a brawler, a defender beside a rooted body.
                    const BY_LEAD: [[u32; 2]; 6] = [[4, 3], [5, 0], [0, 4], [0, 1], [2, 5], [1, 4]];
                    let pair = BY_LEAD[(kind as usize) % 6];
                    let pick = pair[((variation as usize) + slot as usize) % 2];
                    if pick == kind { pair[0] } else { pick }
                };
                // Deep rooms and some escorts use the broader bestiary. These
                // selections are deterministic from the site id and seed, so a
                // cleared record can never resurrect a different family on load.
                if id.z == 2 || id.z == 3 {
                    if variation.is_multiple_of(2) {
                        kind = 10; // Threshold Mimic in a cache room.
                    }
                } else if id.z >= 4 || (slot > 0 && variation.is_multiple_of(3)) {
                    const DEEP: [u32; 4] = [10, 11, 12, 13];
                    kind = DEEP[((variation / 7 + slot + id.z.unsigned_abs()) % 4) as usize];
                }
                // High-menace sites occasionally field a size-varied brute.
                // Keeping it rare preserves the existing encounter rhythm while
                // making a dangerous site read differently at a glance.
                if menace >= 2 && slot == 0 && variation.is_multiple_of(3) {
                    kind = 9;
                }
                if self.enemies.iter().filter(|e|e.habitat.is_none() && !wildlife::passive(e.kind)).count() >= MAX_ENEMIES
                    || record & (1 << slot) != 0
                    || self.enemies.iter().any(|e| e.site == id && e.slot == slot)
                {
                    continue;
                }
                let mut found = None;
                for attempt in 0..12 {
                    let angle = slot as f32 * 2.1 + attempt as f32 * 0.83;
                    let xz =
                        Vec3::new(angle.cos(), 0.0, angle.sin()) * (3.0 + attempt as f32 * 0.25);
                    for dy in [0.0, 1.0, -1.0, 2.0, -2.0] {
                        let p = center + xz + Vec3::Y * dy;
                        if (p - player).length() < 8.0 || !free(world, p, kind) {
                            continue;
                        }
                        if !flying(kind)
                            && !world.solid(
                                ((p - Vec3::Y * 0.2) / world.voxel_scale.meters())
                                    .floor()
                                    .as_ivec3(),
                            )
                        {
                            continue;
                        }
                        found = Some(p);
                        break;
                    }
                    if found.is_some() {
                        break;
                    }
                }
                if let Some(pos) = found {
                    let affixes = if is_boss(kind) {
                        status::Affixes::default()
                    } else {
                        status::roll(
                            world.sample_config().seed,
                            id.x,
                            id.y,
                            slot,
                            menace,
                            zone_archetype(world, pos),
                        )
                    };
                    let affixes = modifiers::affixes(modifiers::for_site(&world.sample_config(), id), affixes);
                    self.enemies
                        .push(Enemy::new(id, slot, kind, pos, menace).with_affixes(affixes));
                }
            }
        }
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
        let cast = std::mem::take(&mut self.queued_cast);
        let cycle = std::mem::take(&mut self.queued_cycle);
        let scale = world.voxel_scale.meters();
        self.restore_discoveries(world);
        if !self.can_wield(self.spell) {
            self.spell = 0;
        }
        self.puzzle_cd = (self.puzzle_cd - dt).max(0.0);
        world.undercrown.tick(dt);
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
        self.time = (self.time + dt) % 10000.0;
        self.cast_cd = (self.cast_cd - dt).max(0.0);
        self.mana_flash = (self.mana_flash - dt).max(0.0);
        self.melee_cd = (self.melee_cd - dt).max(0.0);
        self.invulnerable = (self.invulnerable - dt).max(0.0);
        self.hurt = (self.hurt - dt).max(0.0);
        let rank = self.rank();
        self.hit = (self.hit - dt).max(0.0);
        self.combo_time = (self.combo_time - dt).max(0.0);
        if self.combo_time == 0.0 && !strike_now && self.melee_cd == 0.0 {
            self.combo = 0;
        }
        self.resonance_tick(player.pos * scale, dt);
        self.mana = (self.mana + dt * ((8.0 + rank as f32 * 1.5) * wand_items::charge_scale(&self.loadout)
            + self.resonance.stacks(4) * 1.5)).min(self.max_mana());
        // Hunger drains slowly; adequate food restores health only when safe.
        world.inventory.tick(dt);
        let armor_reduction = world.inventory.armor_reduction();
        let sprinting = player.sprinting && player.vel.length_squared() > 0.25;
        let drain = hunger::drain_per_second(self.hunger_mode, sprinting, player.flying);
        self.hunger = (self.hunger - dt * drain).max(0.0);
        if self.hunger >= 75.0 && self.health < self.max_health() && self.hurt <= 0.0 {
            self.health = (self.health + dt * 0.36).min(self.max_health());
        } else if self.hunger <= 0.0 && !player.flying {
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
        self.prepare_wand();
        if cycle && self.wand > 0 {
            self.advance_wand();
        }
        if cast && self.cast_cd <= 0.0 {
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
        let (player_min, player_max) =
            super::player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
        let player_min = player_min * scale - Vec3::splat(0.10);
        let player_max = player_max * scale + Vec3::splat(0.10);
        // A committed swing owns the button until recovery, even if the target
        // moves away. Otherwise holding attack unexpectedly excavates terrain.
        let mut consumed = self.melee_cd > 0.0 || strike_now;
        if self.melee_buffer > 0.0 || strike_now {
            let target = self
                .enemies
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    let target = if e.kind == engine::KIND { engine::melee_point(e, eye, forward) } else { e.center() };
                    let to_enemy = target - eye;
                    let aim_distance = to_enemy.dot(forward);
                    let off_aim = (to_enemy - forward * aim_distance).length();
                    let reach = MELEE_REACH
                        + if self.weapon == 0 {
                            0.0
                        } else {
                            ARMED_REACH_BONUS
                        };
                    let sweep = (MELEE_SWEEP_BASE + aim_distance * MELEE_SWEEP_GROWTH)
                        .min(MELEE_SWEEP_MAX)
                        * tinkering::binding_multiplier(self.tool_bind);
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
                    let e = &mut self.enemies[i];
                    let shatter = e.slow > 0.0;
                    let shield = (e.kind == 5
                        && e.guard
                        && !shatter
                        && forward.dot(Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos())) < -0.3)
                        || (!shatter && (tollkeeper::guarded(e, forward) || engine::guarded(e, forward)));
                    let base = super::inventory::weapon_damage(self.weapon);
                    let damage = (if finisher { base * 1.7 } else { base }
                        + rank as f32 * 3.0
                        + if shatter { 18.0 } else { 0.0 }
                        + if self.tool_mods & 16 != 0 { 4.0 } else { 0.0 })
                        * tinkering::head_multiplier(self.tool_head)
                        * (1.0 + self.resonance.stacks(0) * 0.08)
                        * if self.tool_mods & 1 != 0 { 1.08 } else { 1.0 };
                    e.health -= damage * if e.encounter.exposed > 0.0 { 1.25 } else { 1.0 }
                        * if shield { 0.35 } else if e.kind == 8 { hart::damage_taken(e) } else { 1.0 };
                    if e.kind == tollkeeper::KIND { tollkeeper::pressure(e, damage); }
                    if e.kind == engine::KIND {
                        let point = engine::melee_point(e, eye, forward);
                        engine::component_hit(e, point, damage, self.weapon == 4);
                    }
                    if let Some(element) = tinkering::edge(self.tool_head) {
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
                        self.mana = (self.mana + 8.0).min(100.0 + rank as f32 * 15.0);
                    }
                    e.flash = 0.16;
                    e.stagger = if finisher || shatter { 0.55 } else { 0.20 }
                        + if finisher && self.tool_mods & 4 != 0 {
                            0.25
                        } else {
                            0.0
                        };
                    if self.tool_mods & 8 != 0 {
                        self.health = (self.health + 2.0).min(100.0 + rank as f32 * 20.0);
                    }
                    if self.tool_mods & 2 != 0 {
                        self.mana = (self.mana + 2.0).min(100.0 + rank as f32 * 15.0);
                    }
                    if e.kind < 6 || shatter && e.poise <= 0.0 {
                        e.tell = 0.0;
                        if is_boss(e.kind) {
                            e.poise = 3.0;
                        }
                    }
                    e.charge = 0.0;
                    if e.kind < 6 {
                        e.cooldown = e.cooldown.max(0.65);
                    }
                    e.knock = forward
                        * if finisher {
                            5.4
                        } else if self.weapon > 0 {
                            3.1
                        } else {
                            2.6
                        };
                    if shatter {
                        e.slow = 0.0;
                    }
                    let p = self.enemies[i].center();
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
                        if self.tool_head != 0 {
                            let c = tinkering::material_color(self.tool_head);
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
            }
        }
        self.spawn_timer -= dt;
        growth::tick(self, dt);
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = 1.0;
            growth::cultivate(world);
            if world.rules.mob_spawning {
                self.spawn(world, feet);
                wildlife::spawn(self, world, feet);
            }
            self.update_reliquaries(world, feet);
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
                        });
                    }
                }
            }
        }
        let mut incoming = 0.0;
        let mut incoming_direction = Vec3::ZERO;
        let neighbors: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| e.health > 0.0)
            .map(|e| (e.pos, e.site))
            .collect();
        let alerts: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| e.health > 0.0 && e.visible && !wildlife::passive(e.kind))
            .map(|e| (e.pos, e.site, e.last_seen))
            .collect();
        let mut navigation_budget = 1;
        for e in &mut self.enemies {
            if !e.visible
                && let Some((_, _, seen)) = alerts
                    .iter()
                    .find(|(p, site, _)| *site == e.site && (*p - e.pos).length_squared() < modifiers::alert_radius_sq(self.site_modifier))
                {
                    e.last_seen = *seen;
                    e.alert = e.alert.max(3.0);
                }
            if e.health <= 0.0 {
                continue;
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
                });
            }
            e.flash = (e.flash - dt).max(0.0);
            e.poise = (e.poise - dt).max(0.0);
            let dot = e.status.tick(dt, e.affixes);
            if dot != 0.0 {
                e.health = (e.health - dot).min(e.max_health());
                if dot > 0.0 {
                    e.flash = e.flash.max(0.06);
                }
            }
            if !is_boss(e.kind) { navigation::gravity(e, world, dt); }
            if wildlife::passive(e.kind) { wildlife::tick(e, world, feet, self.time, dt); continue; }
            if encounters::ambush(e, world, feet, eye, forward, dt, &mut self.sounds) { continue; }
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
                    });
                }
            }
            e.stagger = (e.stagger - dt).max(0.0);
            if e.stagger > 0.0 || e.tell > 0.0 {
                e.gait *= (-dt * 10.0).exp();
            }
            e.recovery = (e.recovery - dt).max(0.0);
            // Sweep knockback and charges; neither may teleport through thin walls.
            let impulse = if e.charge > 0.0 {
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
                for axis in [0, 2] {
                    let mut q = e.pos;
                    q[axis] += impulse[axis] * dt / steps as f32;
                    if free(world, q, e.kind) {
                        e.pos = q;
                    } else {
                        e.charge = 0.0;
                    }
                }
            }
            e.knock *= (-dt * 9.0).exp();
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
                e.sight = 0.12 + (e.slot % 4) as f32 * 0.025;
                e.visible = world.rules.mode.visible_to_mobs()
                    && distance < modifiers::sight_range(self.site_modifier)
                    && line(world, e.center(), eye);
                if e.visible {
                    e.last_seen = feet;
                    e.alert = 4.0;
                }
            }
            if e.visible {
                // Refresh remembered position each frame while sight remains clear.
                e.last_seen = feet;
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
                continue;
            }
            if e.tell > 0.0 {
                e.tell -= dt;
                if e.tell <= 0.0 {
                    e.recovery = 0.24;
                    if e.kind == 3 || e.encounter.mechanic() == encounters::Mechanic::Interceptor {
                        e.charge = 0.42;
                    } else if e.kind == 0 || (brawler(e.kind) && e.attack == 3) {
                        if distance < 2.6 && e.visible && line(world, e.center(), eye) {
                            let damage = if e.kind == 10 {18.0} else {10.0} * (1.0 + e.menace as f32*0.2);
                            incoming += damage;
                            incoming_direction += (e.center() - eye).normalize_or_zero() * damage;
                        }
                    } else if e.visible || e.attack == 2 {
                        tactics::release(e, &mut self.projectiles, &mut self.hazards);
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
            let attack_range = if e.encounter.mechanic() == encounters::Mechanic::Hexer {9.0}
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
            if e.visible && e.cooldown <= 0.0 && distance < attack_range {
                e.attacks = e.attacks.wrapping_add(1);
                e.attack = if e.encounter.mechanic() == encounters::Mechanic::Hexer {2}
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
                e.aim = if e.attack == 2 {
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
                e.lunge = ((intercept - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
                continue;
            }
            let chasing = (e.visible || e.alert > 0.0) && (e.pos - e.home).length() < 30.0;
            let goal = if chasing {
                if e.visible { feet } else { e.last_seen }
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
            let speed = if e.slow > 0.0 {
                0.5
            } else {
                bestiary::species(e.kind)
                    .map_or(if brawler(e.kind) { 2.5 } else { 1.3 }, |s| s.speed)
            };
            let rooted =
                bestiary::species(e.kind).is_some_and(|s| s.role == bestiary::Role::Rooted);
            let movement =
                if (rooted && !chasing) || (brawler(e.kind) && chasing && distance < 1.55) {
                    Vec3::ZERO
                } else {
                    direction.normalize_or_zero() * speed * e.affixes.speed_mul() * dt
                };
            let facing = if e.visible { d } else { movement };
            if facing.x * facing.x + facing.z * facing.z > 1e-8 {
                let target = (-facing.x).atan2(-facing.z);
                let delta = (target - e.yaw + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                e.yaw += delta.clamp(-dt * 6.0, dt * 6.0);
            }
            let before = e.pos;
            let mut blocked = false;
            for axis in [0, 2] {
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
            }
            // Only probe side steps after the normal collision-resolved move
            // actually meets a wall. This keeps the common AI tick cheap.
            if blocked && movement.length_squared() > 1e-8 {
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
            e.phase += walked;
            let moving = (walked / dt.max(0.001) / speed.max(0.1)).clamp(0.0, 1.0);
            e.gait += (moving - e.gait) * (1.0 - (-dt * 16.0).exp());
        }
        self.tick_hazards(world, feet, eye, dt);
        if incoming > 0.0 && world.rules.mode.visible_to_mobs() {
            self.damage(incoming * (1.0 - armor_reduction), incoming_direction);
        }
        let shots = std::mem::take(&mut self.projectiles);
        for mut shot in shots {
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
                if matches!(shot.spell, 5 | 12 | 28) {
                    shot.velocity.y -= dt
                        * match shot.spell {
                            12 => 6.5,
                            28 => 7.5,
                            _ => 8.5,
                        };
                }
                if shot.mods.gravity != 0.0 {
                    shot.velocity.y -= dt * wands::card_rules::GRAVITY_ACCELERATION * shot.mods.gravity;
                }
                if shot.mods.timer > 0.0 && shot.payload.is_some() {
                    shot.mods.timer -= dt;
                    if shot.mods.timer <= 0.0 {
                        // The payload leaves the carrier mid-flight; the carrier
                        // keeps flying and no longer holds a payload for impact.
                        self.release_payload(world, &mut shot);
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
                if shot.mods.path == 1 {
                    shot.velocity = Quat::from_rotation_y(dt * 2.2) * shot.velocity;
                } else if shot.mods.path == 2 {
                    let side = shot.velocity.cross(Vec3::Y).normalize_or_zero();
                    shot.velocity += side * (self.time * 14.0).sin() * dt * 8.0;
                } else if shot.mods.path == 3 {
                    let radial = (shot.pos - eye) * Vec3::new(1.0, 0.0, 1.0);
                    if radial.length() > 0.4 {
                        let tangent = Vec3::new(-radial.z, 0.0, radial.x).normalize_or_zero();
                        let speed = shot.velocity.length().max(6.0);
                        shot.velocity = tangent * speed;
                    }
                }
                shot.trail_timer -= dt;
                if shot.trail_timer <= 0.0 {
                    shot.trail_timer = if shot.spell == 10 { 0.16 } else { 0.18 };
                    let col = shot
                        .mods
                        .element
                        .map(|element| element.color())
                        .unwrap_or_else(|| shot.color());
                    self.spawn_voxel_particle(
                        shot.pos - shot.velocity.normalize_or_zero() * 0.1,
                        Vec3::new(
                            (shot.pos.x * 0.7).cos() * 0.08,
                            0.12,
                            (shot.pos.z * 0.7).sin() * 0.08,
                        ),
                        col,
                        if shot.spell == 10 { 0.04 } else { 0.022 },
                        1.2,
                        0.4,
                        if matches!(shot.spell, 12 | 5) {
                            2.2
                        } else {
                            -0.4
                        },
                        0.32,
                    );
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
                    if shot.mods.trail || shot.spell == 12 {
                        let cell = (shot.pos / scale).floor().as_ivec3();
                        if matches!(shot.spell, 0 | 3 | 11 | 12 | 14 | 15) {
                            if world.sandbox.can_admit(&[cell]) {
                                let mut tc = world.sandbox.cell(cell);
                                tc.temperature = tc.temperature.max(540.0);
                                world.sandbox.set(cell, tc);
                                world.activity.wake(cell);
                            }
                        } else if matches!(shot.spell, 2 | 13)
                            && world.get(cell) == super::BlockId::Water
                        {
                            world.set(cell, super::BlockId::Ice);
                            self.mark_voxel_dirty(world, cell);
                        }
                    }
                }
            }
            let travel = shot.velocity * dt;
            let mut spark_target = None;
            let mut crit_at: Option<Vec3> = None;
            let mut reaction_fx: Option<(Vec3, status::Element, Option<&'static str>)> = None;
            let steps = (travel.length() / (scale * 0.2).min(0.10)).ceil().max(1.0) as usize;
            let mut hit = false;
            for _ in 0..steps {
                let end = shot.pos + travel / steps as f32;
                if let Some((impact, cell)) = trace::solid(world, shot.pos, end) {
                    if !shot.hostile && shot.mods.phase > 0 {
                        let dir = (end - shot.pos).normalize_or_zero();
                        let stride = scale * 0.5;
                        let mut probe = impact;
                        let mut exit = None;
                        if dir != Vec3::ZERO {
                            for _ in 0..24 {
                                probe += dir * stride;
                                if !world.solid((probe / scale).floor().as_ivec3()) {
                                    exit = Some(probe);
                                    break;
                                }
                            }
                        }
                        if let Some(exit) = exit {
                            shot.mods.phase -= 1;
                            self.spawn_voxel_burst(impact, shot.color(), 6, 1.2, 0.03);
                            shot.pos = exit;
                            self.spawn_voxel_burst(exit, shot.color(), 6, 1.2, 0.03);
                            continue;
                        }
                    }
                    if !shot.hostile && shot.bore_budget > 0 && self.carve_cell(world, cell, true) {
                        shot.bore_budget -= 1;
                        shot.pos = end;
                        continue;
                    }
                    shot.pos = impact;
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
                        let cell_center = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
                        let diff = impact - cell_center;
                        let abs_d = diff.abs();
                        let normal = if abs_d.x >= abs_d.y && abs_d.x >= abs_d.z {
                            Vec3::new(diff.x.signum(), 0.0, 0.0)
                        } else if abs_d.y >= abs_d.z {
                            Vec3::new(0.0, diff.y.signum(), 0.0)
                        } else {
                            Vec3::new(0.0, 0.0, diff.z.signum())
                        };
                        shot.velocity -= 2.0 * shot.velocity.dot(normal) * normal;
                        shot.pos += normal * 0.04;
                        self.spawn_voxel_burst(shot.pos, shot.color(), 6, 1.8, 0.032);
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
                    break;
                }
                shot.pos = end;
                if !shot.splashed
                    && world.get((shot.pos / scale).floor().as_ivec3()) == super::BlockId::Water
                    && let Some(surface) = world.water_surface_at(shot.pos) {
                        shot.splashed = true;
                        let strength = (shot.velocity.length() * 0.032).clamp(0.18, 0.85);
                        self.splash(surface, strength);
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
                } else if let Some(e) = self.enemies.iter_mut().find(|e| {
                    e.health > 0.0
                        && e.slot + 1 != shot.mods.pierced
                        && if e.kind == engine::KIND { engine::hit(e, shot.pos, if shot.mods.orbit { 0.24 } else { 0.0 }) }
                        else if e.kind == 8 { hart::hit(e, shot.pos, if shot.mods.orbit { 0.24 } else { 0.0 }) } else { (e.center() - shot.pos).length()
                            < if tall(e.kind) { 0.8 } else { 0.58 }
                                + if shot.mods.orbit { 0.24 } else { 0.0 } }
                }) {
                    if matches!(shot.spell, 0 | 14) || shot.mods.chain {
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
                        self.mana = (self.mana + wands::card_rules::MANA_LEECH).min(100.0 + rank as f32 * 15.0);
                    }
                    if shot.mods.siphon {
                        self.health = (self.health + wands::card_rules::SIPHON_HEALTH).min(100.0 + rank as f32 * 20.0);
                    }
                    if shot.mods.blood {
                        let dealt = (base_dmg + rank as f32 * 4.0) * shot.mods.damage_mul;
                        self.health = (self.health + dealt * wands::card_rules::BLOOD_SIPHON_FRACTION).min(100.0 + rank as f32 * 20.0);
                    }
                    e.flash = 0.16;
                    e.stagger = (if shatter { 0.65_f32 } else { 0.24 }).max(element_stagger);
                    e.knock = shot.velocity.normalize_or_zero()
                        * if matches!(shot.spell, 3 | 14) || shot.mods.bore {
                            5.0
                        } else {
                            2.8
                        };
                    if e.kind < 6 || shatter && e.poise <= 0.0 {
                        e.tell = 0.0;
                        if is_boss(e.kind) {
                            e.poise = 3.0;
                        }
                    }
                    e.charge = 0.0;
                    if e.kind < 6 {
                        e.cooldown = e.cooldown.max(0.6);
                    }
                    self.hit = 0.18;
                    if matches!(shot.spell, 2 | 13) {
                        e.slow = if self.discoveries & (1 << 8) != 0 || shot.spell == 13 {
                            6.0
                        } else {
                            4.0
                        };
                    } else if shatter {
                        e.slow = 0.0;
                    }
                    if shot.mods.pierce > 0 {
                        // Piercing: the bolt carries on through this foe.
                        shot.mods.pierce -= 1;
                        shot.mods.pierced = e.slot + 1;
                        continue;
                    }
                    hit = true;
                    break;
                }
            }
            if hit {
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
                if !shot.hostile && shot.spell == 11 && self.crosses.len() < 4 {
                    self.crosses.push(spells::PlasmaCross {
                        pos: shot.pos,
                        normal: -shot.velocity.normalize_or_zero(),
                        life: 4.5,
                        pulse: 0.0,
                    });
                    sound(&mut self.sounds, Cue::Spark, Some(shot.pos), 0.8);
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
                if !shot.hostile && shot.spell == 14 {
                    self.electrify_pool(world, shot.pos, rank + 1);
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
                self.spawn_voxel_burst(
                    shot.pos,
                    shot.color(),
                    if matches!(shot.spell, 3 | 29) { 18 } else { 10 },
                    if matches!(shot.spell, 3 | 29) { 3.2 } else { 1.8 },
                    0.038,
                );
                self.effect(
                    shot.pos,
                    shot.color(),
                    if matches!(shot.spell, 3 | 29) { 3.0 } else { 0.7 },
                );
            } else {
                self.projectiles.push(shot);
            }
        }
        self.tick_blooms(world, eye, dt);
        self.tick_wells(world, dt);
        self.tick_crosses(world, dt);
        self.tick_particles(world, dt);
        modifiers::reap(&mut self.enemies);
        self.wildlife.remember(&self.enemies);
        let mut death_blasts: Vec<Vec3> = Vec::new();
        let mut resonance_kills = Vec::new();
        // Spells found this frame: (salt, where). Applied after the loop, which
        // borrows the enemy list.
        let mut finds: Vec<(u32, Vec3, bool)> = Vec::new();
        let mut cleared_sites = Vec::new();
        let mut wand_drops = Vec::new();
        for e in &mut self.enemies {
            if e.health > 0.0 {
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
                let all = (1 << encounters::slots(e.site)) - 1;
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
        self.enemies
            .retain(|e| e.health > 0.0 && (e.habitat.is_some() || (e.pos - feet).length() < 90.0) && e.pos.y > -256.0);
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
        if self.health <= 0.0 {
            let lost_resonance = self.resonance.active.then_some(self.resonance.salvage);
            self.resonance.end();
            self.last_death_pos = Some(feet);
            let spawn_m = self
                .respawn_point
                .unwrap_or_else(|| world.suggested_spawn() * scale);
            player.pos = spawn_m / scale;
            player.vel = Vec3::ZERO;
            self.health = self.max_health();
            self.mana = (self.max_mana() * 0.65).max(50.0);
            self.hunger = self.hunger.max(65.0);
            self.invulnerable = 5.0;
            self.projectiles.retain(|p|p.farm.is_some());
            self.hazards.clear();
            self.wells.clear();
            self.melee_buffer = 0.0;
            self.strike_delay = 0.0;
            self.melee_cd = 0.0;
            self.enemies.retain(|e|e.habitat.is_some());
            self.blooms.clear();
            self.arcs.clear();
            self.crosses.clear();
            self.lasers.clear();
            self.particles.clear();
            self.splashes.clear();
            self.remains.clear();
            self.ward = 0.0;
            self.ward_hits = 0;
            self.spawn_voxel_burst(spawn_m + Vec3::Y * 0.9, [0.98, 0.78, 0.34], 26, 2.8, 0.045);
            sound(&mut self.sounds, Cue::Mend, Some(spawn_m), 0.85);
            let death_dist = (feet - spawn_m).length();
            if self.respawn_point.is_some() {
                world.expedition.say(format!(
                    "Respawned at the reliquary. Death location: {:.0} m away. Items kept.",
                    death_dist
                ));
            } else {
                world.expedition.say(format!(
                    "Respawned at the clearing. Death location: {:.0} m away. Items kept.",
                    death_dist
                ));
            }
            if let Some(lost) = lost_resonance {
                world.expedition.message.push_str(&format!(" Resonance ended: {lost} unbanked salvage lost; calibration kept."));
            }
        }
        self.refresh_focus(world, player.eye(world.voxel_scale) * scale, forward);
        world.habitats.shots=self.projectiles.iter().filter_map(|s|s.farm.map(|owner|super::habitats::FarmShot {owner,pos:s.pos,velocity:s.velocity,life:s.life})).collect();
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
    pub fn spell_lights(&self) -> Vec<(Vec3, f32, [f32; 3], f32)> {
        let mut out = Vec::with_capacity(12);
        for shot in self.projectiles.iter().take(6) {
            out.push((
                shot.pos,
                if shot.spell == 10 { 10.5 } else { 7.5 },
                shot.color(),
                if shot.spell == 10 { 3.4 } else { 2.4 },
            ));
        }
        for w in self.wells.iter().take(2) {
            out.push((w.pos, 11.0, spell_color(6), 3.2 * w.life.min(1.0)));
        }
        for c in self.crosses.iter().take(2) {
            out.push((c.pos, 10.0, spell_color(11), 3.0 * c.life.min(1.0)));
        }
        for l in self.lasers.iter().take(2) {
            out.push((
                (l.a + l.b) * 0.5,
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
        if let Some(c) = &self.companion {
            out.push((
                c.pos + Vec3::Y * (c.shown * 0.72),
                6.0,
                [0.95, 0.82, 0.35],
                1.6,
            ));
        }
        out
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
                && a.spell != wands::EMPTY
                && (a.wand > 0 || a.spell == 0)
                && count <= records::RECORD_CEILING,
            "Invalid spell or encounter state"
        );
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
                    && a.loadout.mode <= 3
                    && a.tool_head <= 5
                    && a.tool_bind <= 5
                    && a.tool_grip <= 5,
                "Invalid wand layout"
            );
            ensure!(
                (0..cap).any(|idx| a.loadout.slot(idx) != wands::EMPTY),
                "Empty wand layout"
            );
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
pub fn tick(
    world: &mut World,
    player: &mut Player,
    dt: f32,
    cast: bool,
    cycle: bool,
    melee: bool,
) -> bool {
    if super::habitats::review::paused() {return false;}
    let feet = player.pos * world.voxel_scale.meters();
    super::habitats::tick(world, feet, dt.clamp(0.0, 0.1));
    super::farming::tick(world, engine_core::Budget::UNBOUNDED);
        wildlife::husbandry(world, feet);
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
    adventure.habitat_emitters(world);
    let consumed = adventure.update(world, player, dt, cast, cycle, melee);
    world.adventure = adventure;
    enemy_review::after(world, player);
    world.reconcile_legacy_to_inventory();
    world.sync_from_inventory();
    consumed
}

pub(crate) fn habitat_space(world: &World, pos: Vec3, kind: u32) -> bool { free(world, pos, kind) }
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

impl Adventure {
    fn habitat_emitters(&mut self, world: &mut World) {
        for index in 0..world.habitats.sources.len() {
            let source = &world.habitats.sources[index];
            if !source.emitter || source.reserve || source.emit_ticks > 0 || self.projectiles.len() >= 96 { continue; }
            let Some(p) = source.pos else { continue; };
            let Some(target) = self.enemies.iter().find(|e| e.habitat == Some(source.id) && e.health > 0.0 && e.pos.distance(p) < 6.0) else { continue; };
            let origin = p + Vec3::new(-0.7, 1.1, 0.0);
            let aim = target.center();
            // Reserve the next Matron's charging input; production buffers and
            // supplied surplus are the only fuel for a paid ordinary spell.
            let held = if source.family == super::habitats::Family::Matron { 32 } else { 0 };
            let cost = spell_cost(0).ceil() as u32;
            if source.mana + source.charge.saturating_sub(held) < cost { continue; }
            // World geometry remains authoritative for both the shot and its target.
            let blocked = trace::solid(world, origin, aim).is_some();
            if blocked { continue; }
            let source = &mut world.habitats.sources[index];
            let from_buffer = source.mana.min(cost);
            source.mana -= from_buffer;
            source.charge -= cost-from_buffer;
            source.mana_used = source.mana_used.saturating_add(cost);
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
        assert_eq!(spell_name(MAX_SPELL_ID), "Unknown spell");
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
