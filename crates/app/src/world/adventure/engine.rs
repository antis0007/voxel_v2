//! The Ossuary Engine keeps manufacturing while it fights. Every module is
//! paid from finite stock bins and must ride the service arm from the cradle to
//! the mount; a heavy hit in transit knocks it loose as a real, usable item.
use super::*;
use crate::world::factory::{Kind, portable::PowerKit, power};
use crate::world::inventory::{Item, ItemStack};
use action::{Phase, Run};

pub(super) const KIND: u32 = 6;
const STOCK: u32 = 7;
const COST: u32 = 2;
const TRANSFER_SECONDS: f32 = 2.2;
/// Damage taken within one tick that shakes a module off the arm.
const KNOCK_DAMAGE: f32 = 14.0;
const PICKUP_RANGE: f32 = 1.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) enum Module {
    Brace,
    Cutter,
    Launcher,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) enum Action {
    Swing,
    Shove,
    Volley,
}
#[derive(Clone, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub installed: Option<Module>,
    /// The cradle's current job and its remaining build seconds.
    pub building: Option<(Module, f32)>,
    /// A finished module on the service arm, with transfer progress 0..1.
    pub transfer: Option<(Module, f32)>,
    pub loose: Option<(Module, Vec3)>,
    pub stock: u32,
    pub loading_damage: f32,
    pub traction_damage: f32,
    pub jam: f32,
    pub cut_cells: u32,
    pub cargo: Vec<ItemStack>,
    pub secured: bool,
    last_health: f32,
    initialized: bool,
    pub stolen: u32,
    pub knocked: u32,
}

impl Module {
    fn color(self) -> [f32; 3] {
        match self {
            Self::Brace => [0.42, 0.62, 0.86],
            Self::Cutter => [0.92, 0.56, 0.2],
            Self::Launcher => [0.86, 0.32, 0.28],
        }
    }
    /// Recovered modules are ordinary factory hardware with real operating needs.
    fn item(self) -> Item {
        match self {
            Self::Brace => Item::PowerKit(PowerKit {
                kind: Kind::Battery,
                energy: power::battery_capacity(0) / 2,
                upgrade: 0,
                mode: 0,
                limit: 0,
            }),
            Self::Cutter => Item::Block(Kind::MiningDrill.block()),
            Self::Launcher => Item::Block(Kind::Crusher.block()),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Brace => "brace cell (a charged battery)",
            Self::Cutter => "cutter head (a mining drill)",
            Self::Launcher => "launcher hopper (an ore crusher)",
        }
    }
}

pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    let s = &e.engine;
    Some(if s.secured {
        "Secured"
    } else if s.loading_damage >= DRIVE_HEALTH && s.traction_damage >= DRIVE_HEALTH {
        "Drives Disabled"
    } else if s.jam > 0.0 {
        "Frost Stalled"
    } else if let Some(r) = &s.run {
        match (r.kind, r.phase()) {
            (_, Phase::Recover) => "Recovering",
            (Action::Swing, Phase::Prepare) => "Arm Tension",
            (Action::Swing, _) => "Arm Sweep",
            (Action::Shove, Phase::Prepare) => "Bracing",
            (Action::Shove, _) => "Cutting",
            (Action::Volley, _) => "Discharging",
        }
    } else if s.loading_damage >= DRIVE_HEALTH {
        "Loading Disabled"
    } else if s.traction_damage >= DRIVE_HEALTH {
        "Traction Disabled"
    } else if s.transfer.is_some() {
        "Transferring"
    } else if s.loose.is_some() {
        "Reclaiming"
    } else if s.building.is_some() {
        "Assembling"
    } else {
        "Repositioning"
    })
}

fn flat(v: Vec3) -> Vec3 {
    v * Vec3::new(1.0, 0.0, 1.0)
}
fn facing(e: &Enemy) -> Vec3 {
    Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos())
}
fn local(e: &Enemy, offset: Vec3) -> Vec3 {
    e.pos + Quat::from_rotation_y(e.yaw) * offset
}
const CRADLE: Vec3 = Vec3::new(0.0, 1.5, 1.05);
const MOUNT: Vec3 = Vec3::new(0.95, 1.3, -0.55);
/// Where the arm holds a module at transfer progress `t`: a visible arc over the shoulder.
fn arm(e: &Enemy, t: f32) -> Vec3 {
    local(
        e,
        CRADLE.lerp(MOUNT, t) + Vec3::Y * (t * std::f32::consts::PI).sin() * 0.9,
    )
}

/// Frontal hits glance off an installed brace except while it commits an attack.
pub(super) fn guarded(e: &Enemy, incoming: Vec3) -> bool {
    e.kind == KIND
        && e.engine.installed == Some(Module::Brace)
        && incoming.normalize_or_zero().dot(facing(e)) < -0.35
        && e.engine
            .run
            .as_ref()
            .is_none_or(|r| r.phase() == Phase::Prepare)
}

fn move_ground(e: &mut Enemy, world: &World, direction: Vec3, distance: f32) {
    if e.engine.traction_damage >= DRIVE_HEALTH {
        return;
    }
    let steps = (distance / 0.1).ceil().max(1.0) as u32;
    for _ in 0..steps {
        let q = e.pos + direction * distance / steps as f32;
        if free(world, q, KIND) && factory::solid_point(world, q - Vec3::Y * 0.15) {
            e.phase += q.distance(e.pos);
            e.pos = q;
        } else {
            break;
        }
    }
}

fn ground_below(world: &World, mut p: Vec3) -> Vec3 {
    for _ in 0..48 {
        if factory::solid_point(world, p - Vec3::Y * 0.1) {
            break;
        }
        p.y -= 0.1;
    }
    p
}

/// The next affordable plan: counter the player's spacing with a tool it lacks.
fn choose(e: &Enemy, distance: f32) -> Option<Module> {
    if e.engine.stock < COST {
        return None;
    }
    let order = if distance > 7.0 {
        [Module::Launcher, Module::Brace, Module::Cutter]
    } else {
        [Module::Cutter, Module::Brace, Module::Launcher]
    };
    order.into_iter().find(|m| Some(*m) != e.engine.installed)
}

/// Production, transfer, knock-loose and pickup. Runs whether or not it attacks.
fn manufacture(e: &mut Enemy, world: &mut World, feet: Vec3, dt: f32, sounds: &mut Vec<Sound>) {
    if e.engine.loading_damage >= DRIVE_HEALTH
        || e.engine.jam > 0.0
        || e.engine
            .run
            .as_ref()
            .is_some_and(|r| r.kind == Action::Swing)
    {
        e.engine.last_health = e.health;
        return;
    }
    let enraged = e.health < e.max_health() * 0.5;
    let taken = e.engine.last_health - e.health;
    e.engine.last_health = e.health;
    if let Some((module, t)) = e.engine.transfer {
        if taken >= KNOCK_DAMAGE {
            let at = ground_below(world, arm(e, t));
            e.engine.transfer = None;
            e.engine.loose = Some((module, at));
            e.engine.knocked += 1;
            e.stagger = e.stagger.max(0.4);
            sound(sounds, Cue::Clang, Some(at), 0.9);
            world
                .expedition
                .say("The module tears free of the service arm. Grab it before the Engine does.");
        } else if t + dt / TRANSFER_SECONDS >= 1.0 {
            // A replaced tool goes back into the bins as scrap, never as a free item.
            if e.engine.installed.replace(module).is_some() {
                e.engine.stock += 1;
            }
            e.engine.transfer = None;
            sound(sounds, Cue::ForgeSlam, Some(e.center()), 0.7);
        } else {
            e.engine.transfer = Some((module, t + dt / TRANSFER_SECONDS));
        }
    }
    if let Some((module, at)) = e.engine.loose
        && flat(feet - at).length() < PICKUP_RANGE
        && (feet.y - at.y).abs() < 1.6
    {
        let (inserted, _) = world.inventory.try_insert(ItemStack::new(module.item(), 1));
        if inserted == 1 {
            if let Item::Block(b) = module.item()
                && let Some(kind) = Kind::from_block(b)
            {
                world.factory.kits[kind as usize] = world.inventory.count_item(module.item());
            }
            e.engine.loose = None;
            e.engine.stolen += 1;
            world
                .expedition
                .say(format!("Salvaged the Engine's {}.", module.label()));
            sound(sounds, Cue::Treasure, Some(at), 0.8);
        }
    }
    if let Some((module, left)) = e.engine.building {
        let left = left - dt;
        if left <= 0.0 && e.engine.transfer.is_none() {
            e.engine.building = None;
            e.engine.transfer = Some((module, 0.0));
            sound(sounds, Cue::ForgeWindup, Some(local(e, CRADLE)), 0.6);
        } else {
            e.engine.building = Some((module, left.max(0.0)));
        }
    } else if e.engine.transfer.is_none()
        && e.engine.loose.is_none()
        && let Some(next) = choose(e, flat(feet - e.pos).length())
    {
        e.engine.stock -= COST;
        e.engine.building = Some((next, if enraged { 3.5 } else { 5.0 }));
    }
}

pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    sounds: &mut Vec<Sound>,
    dirty: &mut Vec<IVec3>,
    navigation_budget: &mut usize,
) -> (f32, Vec3) {
    if !e.engine.initialized {
        e.engine.initialized = true;
        // Foundry starts cutting, Bastion braced with spare stock, Siege with its launcher.
        let variant = boss_look::index(e);
        e.engine.stock = [STOCK, STOCK + 1, STOCK - 1][variant];
        e.engine.installed = Some([Module::Cutter, Module::Brace, Module::Launcher][variant]);
        e.engine.last_health = e.health;
    }
    navigation::gravity(e, world, dt);
    e.engine.jam = (e.engine.jam - dt).max(0.0);
    if e.engine.jam > 0.0 || e.engine.secured {
        e.tell = 0.0;
        return (0.0, Vec3::ZERO);
    }
    // Disengaging pauses the fight; it never heals or refunds the bins.
    if (feet - e.home).length() > 48.0 {
        e.engine.run = None;
        e.tell = 0.0;
        e.engine.last_health = e.health;
        return (0.0, Vec3::ZERO);
    }
    manufacture(e, world, feet, dt, sounds);
    let mut damage = 0.0;
    if let Some(mut run) = e.engine.run.take() {
        if (e.stagger > 0.3 && run.phase() == Phase::Prepare)
            || (matches!(run.phase(), Phase::Prepare | Phase::Active)
                && run.kind == Action::Swing
                && e.engine.loading_damage >= DRIVE_HEALTH)
            || (matches!(run.phase(), Phase::Prepare | Phase::Active)
                && run.kind == Action::Shove
                && e.engine.traction_damage >= DRIVE_HEALTH)
        {
            run.interrupt();
        }
        let previous_time = run.elapsed;
        let release = run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare {
            run.prepare - run.elapsed
        } else {
            0.0
        };
        if release && run.kind == Action::Volley && e.engine.stock > 0 && shots.len() <= 90 {
            e.engine.stock -= 1;
            let aim = (e.aim - e.center()).normalize_or_zero();
            for i in 0..3 {
                shots.push(Projectile {
                    farm: None,
                    pos: local(e, MOUNT),
                    velocity: Quat::from_rotation_y((i as f32 - 1.0) * 0.14) * aim * 10.0,
                    hostile: true,
                    payload: None,
                    payload_mods: Default::default(),
                    spell: 0,
                    life: 3.0,
                    mods: wands::ShotMods {
                        damage_mul: 1.2,
                        ..Default::default()
                    },
                    bore_budget: 0,
                    trail_timer: 0.0,
                    flight: None,
                    splashed: false,
                    follow: None,
                });
            }
        }
        if release {
            sound(sounds, Cue::ForgeSlam, Some(e.center()), 0.8);
        }
        let active_dt =
            (run.elapsed.min(run.prepare + run.active) - previous_time.max(run.prepare)).max(0.0);
        if active_dt > 0.0 {
            if run.kind == Action::Shove {
                cut_advance(e, world, active_dt, dirty);
            }
            let d = flat(feet - e.pos);
            let contact = match run.kind {
                Action::Swing => {
                    let progress = ((run.elapsed - run.prepare) / run.active).clamp(0.0, 1.0);
                    let previous = ((previous_time - run.prepare) / run.active).clamp(0.0, 1.0);
                    let n = d.normalize_or_zero();
                    let angle = e.lunge.cross(n).y.atan2(e.lunge.dot(n));
                    (1.5..3.6).contains(&d.length())
                        && angle >= -1.6 + previous * 3.2 - 0.2
                        && angle <= -1.6 + progress * 3.2 + 0.2
                }
                Action::Shove => {
                    let along = d.dot(e.lunge);
                    along > 0.0 && along < 2.4 && d.cross(e.lunge).y.abs() < 1.15
                }
                Action::Volley => false,
            };
            if !run.hit && contact && (feet.y - e.pos.y).abs() < 2.1 && line(world, e.center(), eye)
            {
                damage = if run.kind == Action::Swing {
                    22.0
                } else {
                    12.0
                };
                run.hit = true;
            }
        }
        if run.phase() != Phase::Done {
            e.engine.run = Some(run);
        } else {
            e.cooldown = 0.5;
        }
        return (damage, (e.center() - eye).normalize_or_zero() * damage);
    }
    e.tell = 0.0;
    if e.cooldown > 0.0 {
        return (0.0, Vec3::ZERO);
    }
    // An unclaimed part is worth fetching when the player is not standing on it.
    if let Some((_, at)) = e.engine.loose
        && e.engine.loading_damage < DRIVE_HEALTH
        && e.engine.traction_damage < DRIVE_HEALTH
        && flat(feet - at).length() > 3.0
    {
        if flat(at - e.pos).length() < 1.3 {
            let (module, _) = e.engine.loose.take().unwrap_or((Module::Brace, at));
            // Recovered parts need no new stock, but must ride the arm again.
            e.engine.building = Some((module, 0.6));
            e.cooldown = 0.8;
        } else {
            let direction =
                e.navigation
                    .heading(world, e.pos, at, KIND, dt, e.slot, navigation_budget);
            move_ground(e, world, direction, dt * 1.6);
            e.yaw = (-direction.x).atan2(-direction.z);
        }
        return (0.0, Vec3::ZERO);
    }
    let distance = flat(feet - e.pos).length();
    if (!e.visible && e.alert <= 0.0) || distance > 26.0 {
        return (0.0, Vec3::ZERO);
    }
    let target = if e.visible { feet } else { e.last_seen };
    let direction = flat(target - e.pos).normalize_or_zero();
    let yaw = (-direction.x).atan2(-direction.z);
    let turn = (yaw - e.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    // The scoop covers a broad arc: demanding exact facing here lets a
    // circling target suppress every attack by keeping the carriage turning.
    let alignment = if distance < 4.0 && e.engine.loading_damage < DRIVE_HEALTH {
        0.85
    } else {
        0.25
    };
    if turn.abs() > alignment && e.engine.traction_damage < DRIVE_HEALTH {
        e.yaw += turn.clamp(-dt * 0.85, dt * 0.85);
        return (0.0, Vec3::ZERO);
    }
    let installed = e.engine.installed;
    let kind = if distance < 4.0
        && e.engine.loading_damage < DRIVE_HEALTH
        && e.engine.transfer.is_none()
    {
        Action::Swing
    } else if distance < 13.0
        && e.engine.traction_damage < DRIVE_HEALTH
        && e.engine.cut_cells < CUT_CAPACITY
    {
        Action::Shove
    } else if distance > 6.0
        && installed == Some(Module::Launcher)
        && e.engine.stock > 0
        && e.engine.loading_damage < DRIVE_HEALTH
    {
        Action::Volley
    } else {
        if e.pos.distance(e.home) < 24.0 {
            let heading =
                e.navigation
                    .heading(world, e.pos, feet, KIND, dt, e.slot, navigation_budget);
            move_ground(
                e,
                world,
                heading,
                dt * if installed == Some(Module::Brace) {
                    0.8
                } else {
                    1.2
                },
            );
        }
        return (0.0, Vec3::ZERO);
    };
    // Direction is fixed here: the committed attack never tracks a sidestep.
    e.lunge = facing(e);
    e.aim = eye;
    let (prepare, active, recover) = match kind {
        Action::Swing => (1.15, 0.8, 1.25),
        Action::Shove => (1.4, 2.2, 1.5),
        Action::Volley => (1.1, 0.1, 1.0),
    };
    e.attacks = e.attacks.wrapping_add(1);
    e.engine.run = Some(Run::new(kind, e.attacks, prepare, active, recover));
    e.tell = prepare;
    sound(sounds, Cue::ForgeWindup, Some(e.center()), 0.8);
    (0.0, Vec3::ZERO)
}

mod mechanisms;
pub mod review;
use mechanisms::{CUT_CAPACITY, DRIVE_HEALTH, cut_advance};
pub(super) use mechanisms::{
    component_hit, focus, hit, melee_point, release_cargo, release_hardware, secure,
};
mod save;
pub(super) use save::{Saved, restore, snapshot};
mod model;
pub(super) use model::model;

#[cfg(test)]
mod tests;
