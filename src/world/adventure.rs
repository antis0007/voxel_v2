//! Bounded local encounters, persistent victories, and player-cast magic.
use super::{World, collides_world, factory, sites};
use crate::audio::{Cue, Event as Sound};
use crate::{mesh::models::Model, player::Player};
use glam::{IVec3, Quat, Vec3};
use std::collections::HashMap;
mod bosses;
pub mod inscriptions;
mod rare_magic;
mod quest;
pub mod reliquaries;
mod rewards;
mod spells;
mod tactics;
mod trace;
pub mod wands;

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
        0 => &[(6, 64), (5, 64), (4, 128)],
        1 => &[(15, 128), (18, 128), (16, 64)],
        _ => &[(17, 128), (18, 256), (14, 128)],
    }
}
pub fn sword_recipe(tier: u32) -> &'static [(usize, u32)] {
    match tier {
        1 => &[(6, 128), (4, 64)],
        _ => &[(15, 128), (16, 64)],
    }
}
pub fn sword_name(tier: u32) -> &'static str {
    match tier {
        1 => "Copper sword",
        2 => "Steel sword",
        3 => "Cinderbrand",
        4 => "Winterglass",
        _ => "Unarmed",
    }
}
pub fn ration_recipe() -> &'static [(usize, u32)] {
    &[(13, 64), (4, 64)]
}
pub const MAX_SPELL_ID: u32 = 23;
pub const ALL_SPELLS: [u32; 23] = [
    0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

pub fn spell_name(spell: u32) -> &'static str {
    match spell {
        0 => "Spark",
        1 => "Mend",
        2 => "Frost lance",
        3 => "Resonant burst",
        4 => "Mirror ward",
        5 => "Spore bloom",
        6 => "Rift well",
        7 => "Echo step",
        8 => "Empty",
        9 => "Lumen drill",
        10 => "Black hole",
        11 => "Plasma cross",
        12 => "Magma glob",
        13 => "Cryo nova",
        14 => "Thunder charge",
        15 => "Touch of gold",
        16 => "Bifurcate",
        17 => "Trifurcate",
        18 => "Trigger bolt",
        19 => "Homing arc",
        20 => "Orbit & ricochet",
        21 => "Heavy bore",
        22 => "Rapid flux",
        23 => "Elemental wake",
        _ => "Echo step",
    }
}
pub fn spell_count(tier: u32) -> u32 {
    if tier == 0 { 0 } else { 6 }
}

pub fn spell_color(spell: u32) -> [f32; 3] {
    match spell {
        1 => [0.33, 0.91, 0.49],
        2 => [0.32, 0.77, 1.0],
        3 => [0.77, 0.44, 1.0],
        4 => [0.55, 0.88, 1.0],
        5 => [0.65, 0.87, 0.25],
        6 => [0.66, 0.28, 1.0],
        7 => [0.48, 0.9, 1.0],
        9 => [0.22, 0.98, 0.68],
        10 => [0.48, 0.16, 0.92],
        11 => [0.28, 0.84, 1.0],
        12 => [1.0, 0.38, 0.08],
        13 => [0.52, 0.92, 1.0],
        14 => [0.45, 0.94, 1.0],
        15 => [1.0, 0.84, 0.24],
        16 => [0.42, 0.82, 0.98],
        17 => [0.56, 0.68, 1.0],
        18 => [1.0, 0.56, 0.22],
        19 => [0.38, 0.96, 0.78],
        20 => [0.88, 0.52, 1.0],
        21 => [0.94, 0.46, 0.26],
        22 => [0.36, 0.98, 0.54],
        23 => [1.0, 0.66, 0.28],
        _ => [1.0, 0.73, 0.25],
    }
}
pub fn spell_cost(spell: u32) -> f32 {
    match spell {
        1 | 3 => 30.0,
        2 => 18.0,
        4 => 26.0,
        5 => 38.0,
        6 => 42.0,
        7 => 24.0,
        9 => 10.0,
        10 => 46.0,
        11 => 34.0,
        12 => 22.0,
        13 => 26.0,
        14 => 36.0,
        15 => 32.0,
        16 => 2.0,
        17 => 4.0,
        18 => 6.0,
        19 => 8.0,
        20 => 6.0,
        21 => 10.0,
        22 => 0.0,
        23 => 5.0,
        _ => 12.0,
    }
}
pub fn spell_cooldown(spell: u32) -> f32 {
    match spell {
        1 => 2.5,
        3 => 0.85,
        4 => 0.8,
        5 => 1.2,
        6 => 2.5,
        7 => 2.0,
        9 => 0.08,
        10 => 1.8,
        11 => 0.75,
        12 => 0.42,
        13 => 0.55,
        14 => 0.68,
        15 => 0.60,
        16..=23 => 0.0,
        _ => 0.28,
    }
}
pub fn spell_cast_delay_delta(spell: u32) -> f32 {
    match spell {
        9 => -0.16,
        10 => 0.35,
        14 => 0.14,
        18 => -0.04,
        19 => 0.02,
        21 => 0.08,
        22 => -0.14,
        _ => 0.0,
    }
}
pub fn spell_recharge_delta(spell: u32) -> f32 {
    match spell {
        9 => -0.10,
        10 => 0.20,
        21 => 0.06,
        22 => -0.12,
        _ => 0.0,
    }
}
pub fn spell_spread_delta(spell: u32) -> f32 {
    match spell {
        9 => -4.0,
        14 => 2.5,
        16 => 4.5,
        17 => 8.5,
        19 => -3.5,
        _ => 0.0,
    }
}
pub fn spell_base_damage(spell: u32) -> f32 {
    match spell {
        0 | 2 => 22.0,
        3 => 34.0,
        5 => 30.0,
        6 => 28.0,
        9 => 26.0,
        10 => 40.0,
        11 => 48.0,
        12 => 32.0,
        13 => 26.0,
        14 => 42.0,
        15 => 20.0,
        _ => 0.0,
    }
}
pub fn spell_base_speed(spell: u32) -> f32 {
    match spell {
        2 => 34.0,
        5 => 14.0,
        9 => 65.0,
        10 => 10.5,
        11 => 22.0,
        12 => 19.0,
        13 => 25.0,
        14 => 38.0,
        15 => 28.0,
        _ => 26.0,
    }
}
pub fn spell_kind_tag(spell: u32) -> &'static str {
    match spell {
        1 | 4 | 7 => "UTILITY",
        6 | 10 | 11 => "STATIC FIELD",
        9 => "LASER DRILL",
        16 | 17 => "MULTICAST",
        18 => "TRIGGER MOD",
        19..=23 => "MODIFIER",
        _ => "PROJECTILE",
    }
}
pub fn spell_hint(spell: u32) -> &'static str {
    match spell {
        0 => "Chains to nearby enemies. Shatters frost and charges factory machines.",
        1 => "Circle of Vigour: restores health immediately and spawns sanctum motes.",
        2 => "Cryo lance: freezes guardians and solidifies water into walkable ice.",
        3 => "Explosive bolt: carves a 2 m crater and shatters frozen targets.",
        4 => "Orbiting crystal runes reflect three enemy bolts into lightning.",
        5 => "Bio-spore nebula harms guardians, heals you, and seeds fungal soil.",
        6 => "Gravity singularity pulls guardians inward. Found in archives.",
        7 => "Teleport blink along your aim ray onto safe ground. Found in cathedrals.",
        9 => "Pinpoint 6.5 m laser that carves solid rock/ore and slashes cast delay.",
        10 => "Slow void singularity that devours terrain along its path and pulls foes in.",
        11 => "Deploys a spinning 4-blade 3D plasma cross that slices foes and melts ice.",
        12 => "Molten fireball that heats the thermal lattice, ignites fire, and boils water.",
        13 => "Sub-zero sphere that freezes water into ice bridges and deep-freezes foes.",
        14 => "High-voltage bolt that arcs across enemies and electrifies water & cables.",
        15 => "Alchemical transmutation: turns stone and dirt into crystal, ore, and glowcaps.",
        16 => "Draws and casts the next 2 spells in the deck simultaneously.",
        17 => "Draws and casts the next 3 spells in a 3D triad formation.",
        18 => "Attaches the next spell in the deck as a 3D impact payload.",
        19 => "Curves projectile trajectories in 3D toward nearby guardians.",
        20 => "Adds orbiting satellite bolts, +30% damage, and 2 wall ricochet bounces.",
        21 => "Matter Eater + Heavy Shot: 1.65x damage and bores through solid rock in flight.",
        22 => "Add Mana + Chainsaw: generates +20 mana and cuts cast delay & recharge time.",
        23 => "Projectiles leave a 3D thermal/hydro wake and glowing micro-voxel trail.",
        _ => "Empty wand slot.",
    }
}
pub fn spell_available(spell: u32, wand: u32, rare: u32) -> bool {
    if wand == 0 || spell == wands::EMPTY || spell > MAX_SPELL_ID {
        return false;
    }
    match spell {
        0..=5 => true,
        6 | 7 => rare & (1 << spell) != 0,
        9 | 12 | 13 | 16 | 18 | 20 | 22 | 23 => wand >= 1,
        11 | 14 | 17 | 19 | 21 => wand >= 2 || rare != 0,
        10 | 15 => wand >= 3 || rare.count_ones() >= 2,
        _ => false,
    }
}
fn sound(queue: &mut Vec<Sound>, cue: Cue, pos: Option<Vec3>, gain: f32) {
    if queue.len() < 32 {
        queue.push(Sound { cue, pos, gain });
    }
}
pub fn enemy_name(kind: u32) -> &'static str {
    match kind {
        0 => "Ruin Skitterer",
        1 => "Spore Sentinel",
        2 => "Arcane Wisp",
        3 => "Burrow Beetle",
        4 => "Aether Moth",
        5 => "Sanctum Warden",
        6..=8 => bosses::name(kind),
        _ => "Sanctum Guardian",
    }
}
fn enemy_health(kind: u32) -> f32 {
    match kind {
        1 => 65.0,
        3 => 54.0,
        4 => 42.0,
        5 => 110.0,
        6..=8 => bosses::health(kind),
        _ => 35.0,
    }
}
fn tall(kind: u32) -> bool {
    matches!(kind, 1 | 5 | 6..=8)
}
fn flying(kind: u32) -> bool {
    matches!(kind, 2 | 4)
}
fn brawler(kind: u32) -> bool {
    matches!(kind, 0 | 3)
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
}
impl Enemy {
    fn new(site: IVec3, slot: u32, kind: u32, pos: Vec3) -> Self {
        Self {
            site,
            slot,
            kind,
            pos,
            home: pos,
            health: enemy_health(kind),
            slow: 0.0,
            tell: 0.0,
            phase: 0.0,
            yaw: 0.0,
            velocity: Vec3::ZERO,
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
        }
    }
    fn windup(&self) -> f32 {
        if self.kind == 0 {
            0.45
        } else if self.attack == 2 {
            1.0
        } else {
            0.75
        }
    }
    fn center(&self) -> Vec3 {
        self.pos
            + Vec3::Y
                * if self.kind >= 6 {
                    1.2
                } else if tall(self.kind) {
                    0.8
                } else {
                    0.45
                }
    }
    pub fn model(&self, m: &mut Model, time: f32) {
        if self.kind >= 6 {
            bosses::model(self, m, time);
            return;
        }
        let mark = m.mark();
        let wind = (1.0 - self.tell / self.windup()).clamp(0.0, 1.0)
            * if self.tell > 0.0 { 1.0 } else { 0.0 };
        let hit = if self.flash > 0.0 {
            [1.0, 0.94, 0.69]
        } else if self.slow > 0.0 {
            [0.35, 0.73, 0.96]
        } else if self.tell > 0.0 {
            [0.95, 0.37, 0.15]
        } else {
            [0.49, 0.32, 0.63]
        };
        if self.kind == 3 {
            // Low, armoured burrow beetle with six skittering legs.
            m.cuboid(
                self.pos + Vec3::Y * 0.36,
                Vec3::new(0.64, 0.44, 0.70),
                Quat::IDENTITY,
                [0.28, 0.17, 0.12],
                0.0,
            );
            m.cuboid(
                self.pos + Vec3::Y * 0.60,
                Vec3::new(0.54, 0.12, 0.65),
                Quat::IDENTITY,
                hit,
                0.2,
            );
            for x in [-0.27, 0.27] {
                for j in 0..3 {
                    let z = j as f32 * 0.23 - 0.23;
                    m.beam(
                        self.pos + Vec3::new(x, 0.3, z),
                        self.pos
                            + Vec3::new(
                                x * 1.75,
                                0.06,
                                z + (self.phase * 12.0 + j as f32).sin() * 0.06,
                            ),
                        0.045,
                        [0.41, 0.30, 0.16],
                        0.0,
                    );
                }
            }
            for x in [-0.15, 0.15] {
                m.orb(self.pos + Vec3::new(x, 0.4, -0.38), 0.07, hit, 2.0);
            }
        } else if self.kind == 0 {
            let bob = (self.phase * 8.0).sin().abs() * 0.05;
            m.beam(
                self.pos + Vec3::Y * 0.08,
                self.pos + Vec3::Y * (0.58 + bob),
                0.15,
                [0.64, 0.61, 0.42],
                0.0,
            );
            m.cuboid(
                self.pos + Vec3::Y * (0.61 + bob),
                Vec3::new(0.76, 0.22, 0.68),
                Quat::from_rotation_z((self.phase * 5.0).sin() * 0.08),
                hit,
                0.08,
            );
            for x in [-0.16, 0.16] {
                m.orb(
                    self.pos + Vec3::new(x, 0.49 + bob, -0.21),
                    0.055,
                    [0.55, 0.98, 0.42],
                    1.8,
                );
                m.cuboid(
                    self.pos + Vec3::new(x, 0.08, (self.phase * 9.0 + x * 5.0).sin() * 0.12),
                    Vec3::new(0.16, 0.15, 0.28),
                    Quat::IDENTITY,
                    [0.27, 0.32, 0.20],
                    0.0,
                );
            }
        } else if tall(self.kind) {
            m.cuboid(
                self.pos + Vec3::Y * 0.82,
                Vec3::new(0.66, 0.70, 0.46),
                Quat::IDENTITY,
                [0.33, 0.39, 0.43],
                0.0,
            );
            m.cuboid(
                self.pos + Vec3::Y * 1.34,
                Vec3::new(0.50, 0.35, 0.44),
                Quat::from_rotation_y((time * 0.7).sin() * 0.2),
                [0.48, 0.41, 0.30],
                0.0,
            );
            m.orb(self.pos + Vec3::new(0.0, 1.34, -0.23), 0.105, hit, 2.0);
            for x in [-0.23, 0.23] {
                m.cuboid(
                    self.pos + Vec3::new(x, 0.25, 0.0),
                    Vec3::new(0.19, 0.5, 0.22),
                    Quat::from_rotation_x((self.phase * 6.0 + x * 6.0).sin() * 0.24),
                    [0.22, 0.27, 0.29],
                    0.0,
                );
                m.beam(
                    self.pos + Vec3::new(x * 1.9, 0.5 + wind * 0.42, -wind * 0.42),
                    self.pos + Vec3::new(x * 1.9, 1.03, 0.0),
                    0.11,
                    [0.40, 0.31, 0.19],
                    0.0,
                );
            }
            if self.kind == 5 {
                m.cuboid(
                    self.pos + Vec3::new(-0.42, 0.82, -0.28),
                    Vec3::new(0.40, 0.84, 0.14),
                    Quat::IDENTITY,
                    [0.20, 0.31, 0.36],
                    0.0,
                );
                m.beam(
                    self.pos + Vec3::new(-0.42, 0.55, -0.37),
                    self.pos + Vec3::new(-0.42, 1.1, -0.37),
                    0.035,
                    hit,
                    1.0,
                );
            }
        } else {
            let c = self.center() + Vec3::Y * (time * 2.0 + self.slot as f32).sin() * 0.12;
            m.orb(c, 0.26, hit, 1.4);
            if self.kind == 4 {
                for x in [-1.0, 1.0] {
                    m.cuboid(
                        c + Vec3::new(x * 0.35, 0.1, 0.0),
                        Vec3::new(0.55, 0.055, 0.44),
                        Quat::from_rotation_z(x * (time * 12.0).sin() * 0.55),
                        [0.50, 0.61, 0.29],
                        0.4,
                    );
                }
            }
            for i in 0..4 {
                let a = time * 1.8 + i as f32 * 1.57;
                m.orb(
                    c + Vec3::new(a.cos(), (a * 1.5).sin() * 0.5, a.sin()) * 0.41,
                    0.08,
                    [0.28, 0.86, 0.73],
                    1.2,
                );
            }
        }
        if self.tell > 0.0 {
            m.orb(
                self.center() + Vec3::Y * 0.7,
                0.10 + 0.08 * (time * 14.0).sin().abs(),
                [1.0, 0.35, 0.12],
                3.0,
            );
        }
        let maximum = enemy_health(self.kind);
        if self.health < maximum {
            let y = if tall(self.kind) { 1.75 } else { 1.15 };
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
                [0.3, 0.8, 0.32],
                0.7,
            );
        }
        let crouch = if brawler(self.kind) { wind * 0.17 } else { 0.0 };
        m.scale_since(
            mark,
            self.pos,
            Vec3::new(1.0 + crouch * 0.4, 1.0 - crouch, 1.0),
        );
        m.rotate_since(
            mark,
            self.pos,
            Quat::from_rotation_x(-wind * 0.10 + self.flash * 0.9),
        );
        m.rotate_since(mark, self.pos, Quat::from_rotation_y(self.yaw));
    }
}
pub struct Projectile {
    pub pos: Vec3,
    pub velocity: Vec3,
    pub hostile: bool,
    pub spell: u32,
    pub life: f32,
    pub payload: Option<u32>,
    pub mods: wands::ShotMods,
    pub bore_budget: u8,
    pub trail_timer: f32,
}
impl Projectile {
    pub fn payload_color(&self)->Option<[f32;3]> {self.payload.map(spell_color)}
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
}
#[derive(Clone, Default)]
pub struct View {
    pub health: f32,
    pub mana: f32,
    pub wand: u32,
    pub spell: u32,
    pub loadout: wands::Loadout,
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
    pub reward: Option<Reward>,
    pub discoveries: u32,
    pub sigils: u32,
    pub heart_restored: bool,
    pub route: String,
    pub rare_spells: u32,
    pub relic_focus: Option<reliquaries::RelicFocus>,
    pub boss: Option<reliquaries::BossView>,
    pub enemy_focus: Option<EnemyFocus>,
}
pub struct Adventure {
    pub health: f32,
    pub mana: f32,
    pub wand: u32,
    pub spell: u32,
    pub loadout: wands::Loadout,
    pub last_cast: u32,
    cast_total: f32,
    cast_delay_total: f32,
    recharge_total: f32,
    mana_flash: f32,
    terrain_blasts: std::collections::VecDeque<(Vec3,f32)>,
    pub time: f32,
    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub tool_debris: Vec<crate::tool_effects::Burst>,
    pub blasts: Vec<crate::tool_effects::Blast>,
    pub remains: Vec<Remains>,
    pub arcs: Vec<Arc>,
    pub blooms: Vec<Bloom>,
    pub sounds: Vec<Sound>,
    pub ward: f32,
    pub ward_hits: u32,
    pub hunger: f32,
    pub rations: u32,
    /// Bit 0 is a copper sword; bit 1 is a steel sword.
    pub weapons: u32,
    pub weapon: u32,
    reward: Option<Reward>,
    pub discoveries: u32,
    discoveries_ready: bool,
    pub rare_spells: u32,
    sigils: u32,
    route: String,
    stations: Vec<quest::Station>,
    puzzle_cd: f32,
    relic_focus: Option<reliquaries::RelicFocus>,
    boss_view: Option<reliquaries::BossView>,
    enemy_focus: Option<EnemyFocus>,
    reliquaries: Vec<reliquaries::Reliquary>,
    wells: Vec<rare_magic::Well>,
    pub particles: Vec<spells::VoxelParticle>,
    pub lasers: Vec<spells::LaserBeam>,
    pub crosses: Vec<spells::PlasmaCross>,
    pub dirty_voxels: Vec<IVec3>,
    melee_buffer: f32,
    hazards: Vec<tactics::Hazard>,
    strike_delay: f32,
    cast_pose: f32,
    // One bit per defeated guardian; bit 8 records the one-time clear reward.
    pub records: HashMap<IVec3, u32>,
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
            health: 100.0,
            mana: 100.0,
            wand: 0,
            spell: 0,
            loadout: Default::default(),
            last_cast: 0,
            cast_total: 0.28,
            cast_delay_total: 0.28,
            recharge_total: 0.0,
            mana_flash: 0.0,
            terrain_blasts: Default::default(),
            time: 0.0,
            enemies: Vec::new(),
            projectiles: Vec::new(),
            effects: Vec::new(),
            tool_debris: Vec::new(),
            blasts: Vec::new(),
            remains: Vec::new(),
            arcs: Vec::new(),
            blooms: Vec::new(),
            sounds: Vec::new(),
            ward: 0.0,
            ward_hits: 0,
            hunger: 100.0,
            rations: 0,
            weapons: 0,
            weapon: 0,
            reward: None,
            discoveries: 0,
            discoveries_ready: false,
            rare_spells: 0,
            sigils: 0,
            route: String::new(),
            stations: Vec::new(),
            puzzle_cd: 0.0,
            relic_focus: None,
            boss_view: None,
            enemy_focus: None,
            reliquaries: Vec::new(),
            wells: Vec::new(),
            particles: Vec::new(),
            lasers: Vec::new(),
            crosses: Vec::new(),
            dirty_voxels: Vec::new(),
            melee_buffer: 0.0,
            hazards: Vec::new(),
            strike_delay: 0.0,
            cast_pose: 0.0,
            records: HashMap::new(),
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
    let h = if kind >= 6 {
        2.6
    } else if tall(kind) {
        1.6
    } else {
        0.85
    };
    let width = if kind >= 6 { 0.52 } else { 0.28 };
    !collides_world(
        world,
        (pos - Vec3::new(width, 0.0, width)) / s,
        (pos + Vec3::new(width, h, width)) / s,
    )
}
fn line(world: &World, a: Vec3, b: Vec3) -> bool {
    (b-a).length_squared()<=128.0*128.0 && trace::solid(world,a,b).is_none()
}

impl Adventure {
    pub fn rank(&self) -> u32 {
        mastery(self.records.values().filter(|n| **n & 256 != 0).count())
    }
    pub fn max_health(&self) -> f32 {
        100.0 + self.rank() as f32 * 20.0
    }
    pub fn max_mana(&self) -> f32 {
        100.0 + self.rank() as f32 * 15.0
    }
    pub fn swing(&self) -> f32 {
        (self.melee_cd / self.swing_duration()).clamp(0.0, 1.0)
    }
    /// Seconds from wind-up to recovery; shared by gameplay and the held model.
    pub fn swing_duration(&self) -> f32 {
        if self.finisher() { 0.46 } else { 0.30 }
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
            health: self.health,
            mana: self.mana,
            wand: self.wand,
            spell: self.spell,
            loadout: self.loadout,
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
            reward: self.reward.clone(),
            discoveries: self.discoveries,
            rare_spells: self.rare_spells,
            sigils: self.sigils,
            heart_restored: self.heart_restored(),
            route: self.route.clone(),
            relic_focus: self.relic_focus.clone(),
            boss: self.boss_view.clone(),
            enemy_focus: self.enemy_focus.clone(),
        }
    }
    pub fn owns_weapon(&self, tier: u32) -> bool {
        (1..=4).contains(&tier) && self.weapons & (1 << (tier - 1)) != 0
    }
    pub fn add_weapon(&mut self, tier: u32) -> bool {
        if !(1..=4).contains(&tier) || self.owns_weapon(tier) {
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
    pub fn eat_ration(&mut self) -> bool {
        if self.rations == 0 || self.hunger >= 99.0 {
            return false;
        }
        self.rations -= 1;
        self.hunger = (self.hunger + 38.0).min(100.0);
        self.health = (self.health + 8.0).min(self.max_health());
        true
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
    fn spawn(&mut self, world: &World, player: Vec3) {
        for (id, center, kind) in sites::encounters(&world.sample_config(), player) {
            if self.records.len() >= MAX_RECORDS && !self.records.contains_key(&id) {
                continue;
            }
            let record = {
                let record = self.records.entry(id).or_default();
                if *record & 256 == 0 {
                    *record |= reliquaries::PHYSICAL;
                }
                *record
            };
            let count = if id == IVec3::ZERO { 2 } else { 3 };
            for slot in 0..count {
                let variation = (id.x.wrapping_mul(31) ^ id.y.wrapping_mul(17) ^ world.seed as i32)
                    .unsigned_abs();
                let kind = if id == IVec3::ZERO || slot == 0 {
                    kind
                } else {
                    const ESCORTS: [[u32; 2]; 4] = [[3, 4], [1, 2], [0, 5], [2, 4]];
                    ESCORTS[variation as usize % 4][(slot - 1) as usize]
                };
                if self.enemies.len() >= MAX_ENEMIES
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
                    self.enemies.push(Enemy::new(id, slot, kind, pos));
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
        if !spell_available(self.spell, self.wand, self.rare_spells) {
            self.spell = 0;
        }
        self.puzzle_cd = (self.puzzle_cd - dt).max(0.0);
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
        self.mana = (self.mana + dt * (8.0 + rank as f32 * 1.5)).min(self.max_mana());
        // Hunger drains slowly; adequate food restores health only when safe.
        world.inventory.tick(dt);
        let armor_reduction = world.inventory.armor_reduction();
        if !player.flying {
            let sprint_factor = if player.sprinting && player.vel.length_squared() > 0.25 {
                2.2
            } else {
                1.0
            };
            self.hunger = (self.hunger - dt * 0.04 * sprint_factor).max(0.0);
        }
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
        if cycle && self.wand > 0 { self.advance_wand(); }
        if cast && self.cast_cd <= 0.0 { self.cast_wand(world,player,eye,forward); }
        // Echo step changes the current-frame collision and aiming origin.
        let eye = player.eye(world.voxel_scale) * scale;
        let feet = player.pos * scale;
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
                    let to_enemy = e.center() - eye;
                    let aim_distance = to_enemy.dot(forward);
                    let off_aim = (to_enemy - forward * aim_distance).length();
                    let reach = MELEE_REACH
                        + if self.weapon == 0 {
                            0.0
                        } else {
                            ARMED_REACH_BONUS
                        };
                    let sweep =
                        (MELEE_SWEEP_BASE + aim_distance * MELEE_SWEEP_GROWTH).min(MELEE_SWEEP_MAX);
                    (e.health > 0.0
                        && aim_distance > 0.0
                        && aim_distance < reach
                        && off_aim < sweep
                        && line(world, eye, e.center()))
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
                    let shield = e.kind == 5
                        && !shatter
                        && forward.dot(Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos())) < -0.3;
                    let base = match self.weapon {
                        1 => 22.0,
                        2 => 32.0,
                        3 => 38.0,
                        4 => 28.0,
                        _ => 14.0,
                    };
                    let damage = if finisher { base * 1.7 } else { base }
                        + rank as f32 * 3.0
                        + if shatter { 18.0 } else { 0.0 };
                    e.health -= damage * if shield { 0.35 } else { 1.0 };
                    if self.weapon == 4 && !shatter {
                        e.slow = 2.5;
                    }
                    if self.weapon == 3 && finisher {
                        e.health -= 12.0;
                        self.mana = (self.mana + 8.0).min(100.0 + rank as f32 * 15.0);
                    }
                    e.flash = 0.16;
                    e.stagger = if finisher || shatter { 0.55 } else { 0.20 };
                    if e.kind < 6 || shatter && e.poise <= 0.0 {
                        e.tell = 0.0;
                        if e.kind >= 6 {
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
                    self.effect(p, if self.weapon==3 {[1.0,0.32,0.045]} else if self.weapon==4 {spell_color(2)} else {[0.9,0.8,0.5]}, 0.6);
                    if !shield {self.relic_hit(world,eye,p,forward,i,finisher,shatter);}
                }
            }
        }
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = 1.0;
            self.spawn(world, feet);
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
            .filter(|e| e.health > 0.0 && e.visible)
            .map(|e| (e.pos, e.site, e.last_seen))
            .collect();
        for e in &mut self.enemies {
            if !e.visible {
                if let Some((_, _, seen)) = alerts
                    .iter()
                    .find(|(p, site, _)| *site == e.site && (*p - e.pos).length_squared() < 144.0)
                {
                    e.last_seen = *seen;
                    e.alert = e.alert.max(3.0);
                }
            }
            if e.health <= 0.0 {
                continue;
            }
            e.flash = (e.flash - dt).max(0.0);
            e.poise = (e.poise - dt).max(0.0);
            e.stagger = (e.stagger - dt).max(0.0);
            // Sweep knockback and charges; neither may teleport through thin walls.
            let impulse = if e.charge > 0.0 {
                e.lunge * 8.0
            } else {
                e.knock
            };
            let impulse = if e.kind >= 6 {
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
            e.cooldown -= dt;
            e.sight -= dt;
            let d = eye - e.center();
            let distance = d.length();
            if e.sight <= 0.0 {
                e.sight = 0.3;
                e.visible = distance < 27.0 && line(world, e.center(), eye);
                if e.visible {
                    e.last_seen = feet;
                    e.alert = 4.0;
                }
            }
            e.alert = (e.alert - dt).max(0.0);
            if e.kind >= 6 {
                bosses::tick(
                    e,
                    world,
                    feet,
                    eye,
                    dt,
                    &mut self.projectiles,
                    &mut self.hazards,
                    &mut self.sounds,
                );
                continue;
            }
            if e.stagger > 0.0 {
                continue;
            }
            if e.tell > 0.0 {
                e.tell -= dt;
                if e.tell <= 0.0 {
                    if e.kind == 3 {
                        e.charge = 0.42;
                    } else if e.kind == 0 {
                        if distance < 2.2 && e.visible && line(world, e.center(), eye) {
                            incoming += 10.0;
                            incoming_direction += (e.center() - eye).normalize_or_zero() * 10.0;
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
                    e.cooldown = if brawler(e.kind) { 1.6 } else { 2.8 };
                }
                continue;
            }
            if e.visible
                && e.cooldown <= 0.0
                && distance
                    < if e.kind == 3 {
                        7.0
                    } else if e.kind == 0 {
                        1.9
                    } else {
                        18.0
                    }
            {
                e.attacks = e.attacks.wrapping_add(1);
                e.attack = if e.kind == 5 && distance < 4.5 || e.kind == 4 && e.attacks % 2 == 0 {
                    2
                } else {
                    0
                };
                e.aim = if e.attack == 2 {
                    feet
                } else {
                    eye + (player.vel * scale).clamp_length_max(6.0)
                        * if e.kind == 1 { 0.3 } else { 0.0 }
                };
                e.tell = e.windup();
                sound(&mut self.sounds, Cue::Warning, Some(e.center()), 0.55);
                e.lunge = ((feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
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
            let direction = tactics::heading(
                e,
                goal,
                distance,
                chasing,
                &neighbors,
                &self.projectiles,
                self.time,
            );
            let speed = if e.slow > 0.0 {
                0.5
            } else if brawler(e.kind) {
                2.5
            } else {
                1.3
            };
            let movement = if brawler(e.kind) && chasing && distance < 1.55 {
                Vec3::ZERO
            } else {
                direction.normalize_or_zero() * speed * dt
            };
            if movement.length_squared() > 1e-8 {
                e.yaw = (-movement.x).atan2(-movement.z);
            }
            if e.visible {
                e.yaw = (-d.x).atan2(-d.z);
            }
            let before = e.pos;
            let mut blocked = false;
            for axis in [0, 2] {
                let mut q = e.pos;
                q[axis] += movement[axis];
                let supported = flying(e.kind)
                    || e.velocity.y.abs() > 0.01
                    || factory::solid_point(world, q - Vec3::Y * 0.65);
                if free(world, q, e.kind) && supported {
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
                let reach = movement.length();
                let mut best: Option<(Vec3, f32)> = None;
                for angle in [0.65_f32, -0.65, 1.2, -1.2, 2.2, -2.2] {
                    let candidate = Quat::from_rotation_y(angle) * desired;
                    let progress = candidate.dot(desired);
                    if progress > 0.0
                        && free(world, e.pos + candidate * reach, e.kind)
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
            } else {
                e.velocity.y = (e.velocity.y - dt * 14.0).max(-14.0);
                let dy = e.velocity.y * dt;
                let steps = (dy.abs() / 0.18).ceil().max(1.0) as usize;
                for _ in 0..steps {
                    let q = e.pos + Vec3::Y * (dy / steps as f32);
                    if free(world, q, e.kind) {
                        e.pos = q;
                    } else {
                        e.velocity.y = 0.0;
                        break;
                    }
                }
            }
            e.phase += (e.pos - before).length();
        }
        self.tick_hazards(world, feet, eye, dt);
        if incoming > 0.0 {
            self.damage(incoming * (1.0 - armor_reduction), incoming_direction);
        }
        let shots = std::mem::take(&mut self.projectiles);
        for mut shot in shots {
            shot.life -= dt;
            if shot.life <= 0.0 {
                continue;
            }
            if !shot.hostile {
                if matches!(shot.spell, 5 | 12) {
                    shot.velocity.y -= dt * if shot.spell == 12 { 6.5 } else { 8.5 };
                }
                if shot.mods.homing {
                    if let Some(target_pos) = self
                        .enemies
                        .iter()
                        .filter(|e| e.health > 0.0)
                        .filter_map(|e| {
                            let d = (e.center() - shot.pos).length();
                            (d < 18.0 && line(world, shot.pos, e.center())).then_some((e.center(), d))
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(p, _)| p)
                    {
                        let speed = shot.velocity.length().max(8.0);
                        let desired = (target_pos - shot.pos).normalize_or_zero();
                        let current = shot.velocity.normalize_or_zero();
                        let steered = (current + (desired - current) * (dt * 7.2).min(1.0)).normalize_or_zero();
                        shot.velocity = steered * speed;
                    }
                }
                shot.trail_timer -= dt;
                if shot.trail_timer <= 0.0 {
                    shot.trail_timer = if shot.spell == 10 { 0.07 } else { 0.055 };
                    let col = shot.color();
                    self.spawn_voxel_particle(
                        shot.pos - shot.velocity.normalize_or_zero() * 0.14,
                        Vec3::new(
                            (self.time * 13.1 + shot.pos.x).cos() * 0.45,
                            0.35,
                            (self.time * 13.1 + shot.pos.z).sin() * 0.45,
                        ),
                        col,
                        if shot.spell == 10 { 0.048 } else { 0.030 },
                        3.4,
                        0.74,
                        if matches!(shot.spell, 12 | 5) { 2.2 } else { -0.4 },
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
            let steps = (travel.length() / (scale * 0.2).min(0.10)).ceil().max(1.0) as usize;
            let mut hit = false;
            for _ in 0..steps {
                let end = shot.pos + travel / steps as f32;
                if let Some((impact, cell)) = trace::solid(world, shot.pos, end) {
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
                        sound(&mut self.sounds, Cue::Spark, Some(shot.pos), 0.35);
                        break;
                    }
                    sound(
                        &mut self.sounds,
                        if matches!(shot.spell, 3 | 12) {
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
                        self.damage(12.0 * (1.0 - armor_reduction), -shot.velocity);
                        hit = true;
                        break;
                    }
                } else if let Some(e) = self.enemies.iter_mut().find(|e| {
                    e.health > 0.0
                        && (e.center() - shot.pos).length()
                            < if tall(e.kind) { 0.8 } else { 0.58 }
                                + if shot.mods.orbit { 0.24 } else { 0.0 }
                }) {
                    if matches!(shot.spell, 0 | 14) {
                        spark_target = Some((e.site, e.slot));
                    }
                    let shatter = e.slow > 0.0 && !matches!(shot.spell, 2 | 13);
                    let shield = e.kind == 5
                        && shot.spell == 0
                        && !shatter
                        && !shot.mods.bore
                        && shot.velocity.normalize_or_zero().dot(Vec3::new(
                            -e.yaw.sin(),
                            0.0,
                            -e.yaw.cos(),
                        )) < -0.3;
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
                    let base_dmg = spell_base_damage(shot.spell).max(22.0);
                    e.health -= (base_dmg + rank as f32 * 4.0 + if shatter { 18.0 } else { 0.0 })
                        * shot.mods.damage_mul
                        * if shield { 0.3 } else { 1.0 };
                    e.flash = 0.16;
                    e.stagger = if shatter { 0.65 } else { 0.24 };
                    e.knock = shot.velocity.normalize_or_zero()
                        * if matches!(shot.spell, 3 | 14) || shot.mods.bore {
                            5.0
                        } else {
                            2.8
                        };
                    if e.kind < 6 || shatter && e.poise <= 0.0 {
                        e.tell = 0.0;
                        if e.kind >= 6 {
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
                    hit = true;
                    break;
                }
            }
            if hit {
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
                if !shot.hostile && matches!(shot.spell, 2 | 13) {
                    self.cryo_bloom(
                        world,
                        shot.pos,
                        if shot.spell == 13 { 3.2 } else { 1.35 },
                    );
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
                    if self.terrain_blasts.len() < 8 {
                        self.terrain_blasts.push_back((shot.pos, 2.0));
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
                            e.health -= (18.0 + rank as f32 * 2.0 + if shatter { 12.0 } else { 0.0 })
                                * shot.mods.damage_mul;
                            if shatter {
                                e.slow = 0.0;
                                sound(&mut self.sounds, Cue::Shatter, Some(e.center()), 0.6);
                            }
                            e.stagger = 0.45;
                            e.knock = (e.center() - shot.pos).normalize_or_zero() * 5.0;
                            if e.kind < 6 || shatter && e.poise <= 0.0 {
                                e.tell = 0.0;
                                if e.kind >= 6 {
                                    e.poise = 3.0;
                                }
                            }
                            self.hit = 0.18;
                        }
                    }
                }
                if let Some(payload) = shot.payload {
                    let p_dir = shot.velocity.normalize_or_zero();
                    if payload == 9 {
                        self.cast_luminous_drill(world, shot.pos - p_dir * 0.15, p_dir, shot.mods, None);
                    } else {
                        self.emit_spell_modified(payload, shot.pos, p_dir, None, shot.mods);
                    }
                }
                self.spawn_voxel_burst(
                    shot.pos,
                    shot.color(),
                    if shot.spell == 3 { 18 } else { 10 },
                    if shot.spell == 3 { 3.2 } else { 1.8 },
                    0.038,
                );
                self.effect(
                    shot.pos,
                    shot.color(),
                    if shot.spell == 3 { 3.0 } else { 0.7 },
                );
            } else {
                self.projectiles.push(shot);
            }
        }
        self.tick_blooms(world, eye, dt);
        self.tick_wells(world, dt);
        self.tick_crosses(world, dt);
        self.tick_particles(world, dt);
        for e in &self.enemies {
            if e.health > 0.0 {
                continue;
            }
            let mask = self.records.entry(e.site).or_default();
            if *mask & (1 << e.slot) == 0 {
                *mask |= 1 << e.slot;
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
                world.expedition.add(18, 64);
                for &(item, amount) in rewards::guardian(e.kind) {
                    world.expedition.add(item, amount);
                }
                if e.kind == 5 && self.weapons & 2 == 0 {
                    self.weapons |= 2;
                    world
                        .expedition
                        .say("Steel sword recovered from the warden. Equip it in Gear.");
                }
                if e.kind >= 6 {
                    world.expedition.say("The guardian has fallen. Return to the reliquary and claim its relic with F.");
                }
                let all = if e.site == IVec3::ZERO { 3 } else { 7 };
                if *mask & all == all && *mask & 256 == 0 {
                    *mask |= 256;
                    let reward = rewards::sanctum(&world.sample_config(), e.site);
                    world
                        .expedition
                        .say("The reliquary is unsealed. Approach its floating relic and press F.");
                    for &(i, n) in &reward.items {
                        world.expedition.add(i, n);
                    }
                    self.reward = Some(reward);
                    sound(&mut self.sounds, Cue::Treasure, None, 0.75);
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
        self.enemies
            .retain(|e| e.health > 0.0 && (e.pos - feet).length() < 90.0 && e.pos.y > -256.0);
        for e in &mut self.effects {
            e.life -= dt;
        }
        self.effects.retain(|e| e.life > 0.0);
        for burst in &mut self.tool_debris { burst.age += dt; }
        self.tool_debris.retain(|burst| burst.age < 0.55);
        for blast in &mut self.blasts { blast.age += dt; }
        self.blasts.retain(|blast| blast.age < 1.2);
        if self.health <= 0.0 {
            player.pos = world.suggested_spawn();
            player.vel = Vec3::ZERO;
            self.health = self.max_health();
            self.mana = 50.0;
            self.invulnerable = 5.0;
            self.projectiles.clear();
            self.hazards.clear();
            self.wells.clear();
            self.melee_buffer = 0.0;
            self.strike_delay = 0.0;
            self.melee_cd = 0.0;
            self.enemies.clear();
            self.blooms.clear();
            self.arcs.clear();
            self.crosses.clear();
            self.lasers.clear();
            self.particles.clear();
            self.remains.clear();
            self.ward = 0.0;
            self.ward_hits = 0;
            world.expedition.say("Returned to the inland clearing. Supplies retained; your mana needs time to recover.");
        }
        self.refresh_focus(world, player.eye(world.voxel_scale) * scale, forward);
        consumed
    }
    pub fn take_terrain_blast(&mut self)->Option<(Vec3,f32)> {self.terrain_blasts.pop_front()}
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
            out.push((b.pos + Vec3::Y * 0.4, 8.5, spell_color(5), 2.1 * b.life.min(1.0)));
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
            let aux_enc = if aux == wands::EMPTY { 0 } else { (aux + 1) & 0xFF };
            put(primary | (aux_enc << 8));
        }
        put(self.loadout.cursor as u32);
        put(self.loadout.mode);
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
                && (0.0..=145.0).contains(&a.mana),
            "Invalid adventurer vitals"
        );
        ensure!(
            a.wand <= 3
                && a.spell <= MAX_SPELL_ID
                && a.spell != wands::EMPTY
                && (a.wand > 0 || a.spell == 0)
                && count <= MAX_RECORDS,
            "Invalid spell or encounter state"
        );
        for _ in 0..count {
            let p = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
            let mask = read()?;
            ensure!(
                p.x.unsigned_abs() < 100000
                    && p.y.unsigned_abs() < 100000
                    && (0..=1).contains(&p.z)
                    && mask
                        & !(263
                            | 8
                            | reliquaries::AWAKENED
                            | reliquaries::CLAIMED
                            | reliquaries::PHYSICAL
                            | quest::VALVES | quest::BYPASS | quest::RESTORED)
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
                    && a.weapons <= 15
                    && a.weapon <= 4
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
            a.loadout.cursor = read()? as usize;
            a.loadout.mode = read()?;
            let cap = wands::capacity(a.wand);
            ensure!(
                a.loadout.cursor < cap && a.loadout.mode <= 3,
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
            a.health <= a.max_health() && a.mana <= a.max_mana(),
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
    let mut adventure = std::mem::take(&mut world.adventure);
    let consumed = adventure.update(world, player, dt, cast, cycle, melee);
    world.adventure = adventure;
    world.reconcile_legacy_to_inventory();
    world.sync_from_inventory();
    consumed
}
