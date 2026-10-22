//! Prism Hart encounter. Runtime state only; site identity and victory flags stay unchanged.
use super::*;
use crate::mesh::models::HartPose;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Attack {
    #[default]
    Charge,
    Sweep,
    Volley,
    Bloom,
    Lanes,
}

#[derive(Default)]
pub(super) struct State {
    pub crown: hart_crown::Crown,
    pub(super) active: f32,
    pub(super) recovery: f32,
    windup: f32,
    pub attack: Attack,
    pub(super) moving: f32,
    hit: bool,
    enraged: bool,
    pub(super) follow: Option<Attack>,
    passive_review: bool,
}

pub(super) fn variant(e: &Enemy) -> usize {
    (e.site.x.wrapping_mul(73_856_093)
        ^ e.site.y.wrapping_mul(19_349_663)
        ^ e.site.z.wrapping_mul(83_492_791))
    .unsigned_abs() as usize
        % 3
}
pub(super) fn color(e: &Enemy) -> [f32; 3] {
    [[0.10, 0.58, 1.0], [0.73, 0.24, 0.94], [0.08, 0.75, 0.38]][variant(e)]
}
pub(super) fn name(e: &Enemy) -> &'static str {
    const NAMES: [[&str; 3]; 3] = [
        [
            "Rime Hart · Sentinel",
            "Rime Hart · Ascendant",
            "Rime Hart · Sovereign",
        ],
        [
            "Amethyst Hart · Sentinel",
            "Amethyst Hart · Ascendant",
            "Amethyst Hart · Sovereign",
        ],
        [
            "Verdant Hart · Sentinel",
            "Verdant Hart · Ascendant",
            "Verdant Hart · Sovereign",
        ],
    ];
    NAMES[variant(e)][e.menace.min(2) as usize]
}
pub(super) fn damage_taken(e: &Enemy) -> f32 {
    if e.hart.recovery > 0.75 && e.hart.active <= 0.0 && !e.hart.passive_review {
        1.25
    } else {
        1.0
    }
}
pub(super) fn hit(e: &Enemy, point: Vec3, extra: f32) -> bool {
    let local = Quat::from_rotation_y(-e.yaw) * (point - e.pos);
    // Match the torso, neck and pointed skull, rather than a single small central sphere.
    [
        (Vec3::new(0.0, 1.65, 0.05), Vec3::new(0.55, 0.62, 1.05)),
        (Vec3::new(0.0, 2.20, -0.85), Vec3::new(0.38, 0.62, 0.45)),
        (Vec3::new(0.0, 2.59, -1.50), Vec3::new(0.31, 0.34, 0.64)),
    ]
    .into_iter()
    .any(|(center, radius)| {
        ((local - center) / (radius + Vec3::splat(extra))).length_squared() <= 1.0
    })
}
pub(super) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let wind = if e.tell > 0.0 {
        1.0 - e.tell / e.hart.windup.max(0.01)
    } else {
        0.0
    };
    let wind = wind.clamp(0.0, 1.0);
    let charging = e.hart.active > 0.0;
    let pose = HartPose {
        gait: e.phase * 4.8,
        moving: e.hart.moving,
        bow: if variant(e) == 0 && matches!(e.hart.crown.phase, hart_crown::Phase::Plant | hart_crown::Phase::Grow | hart_crown::Phase::Warn | hart_crown::Phase::Pulse) {
            if e.hart.crown.phase == hart_crown::Phase::Plant { (1.0-e.hart.crown.clock/1.6).clamp(0.0,1.0) } else { 1.0 }
        } else if e.hart.attack == Attack::Charge {
            wind * wind + if charging { 0.8 } else { 0.0 }
        } else {
            0.0
        },
        rear: if matches!(e.hart.attack, Attack::Bloom | Attack::Lanes | Attack::Sweep) {
            wind * wind
        } else {
            0.0
        },
        turn: if e.hart.attack == Attack::Sweep {
            if e.tell > 0.0 {
                -wind * 0.72
            } else {
                let t = ((1.65 - e.hart.recovery) / 0.42).clamp(0.0, 1.0);
                (-0.72 + t * 1.6) * (e.hart.recovery / 0.9).clamp(0.0, 1.0)
            }
        } else {
            0.0
        },
        recover: if variant(e)==0 && e.hart.crown.phase==hart_crown::Phase::Vent { 1.0 } else { (e.hart.recovery / 1.8).clamp(0.0, 1.0) },
        breath: (time * 1.7).sin(),
    };
    m.crystal_hart(e.pos, e.yaw, pose, color(e), e.flash > 0.0);
}

fn plan(e: &Enemy, distance: f32) -> Attack {
    // The Rime family contests its editable crown with physical actions. Its identity
    // no longer depends on shard fans, frost circles, or a damage-permission window.
    if variant(e) == 0 { return if distance < 4.0 { Attack::Sweep } else { Attack::Charge }; }
    if distance < 3.3 && !e.attacks.is_multiple_of(3) {
        return Attack::Sweep;
    }
    let patterns = match (variant(e), e.menace.min(2), e.hart.enraged) {
        (0, 0, false) => &[
            Attack::Charge,
            Attack::Volley,
            Attack::Bloom,
            Attack::Charge,
        ][..],
        (1, 0, false) => &[Attack::Bloom, Attack::Volley, Attack::Charge, Attack::Bloom][..],
        (2, 0, false) => &[Attack::Lanes, Attack::Charge, Attack::Bloom, Attack::Volley][..],
        (0, _, _) => &[
            Attack::Volley,
            Attack::Charge,
            Attack::Sweep,
            Attack::Lanes,
            Attack::Charge,
        ][..],
        (1, _, _) => &[
            Attack::Bloom,
            Attack::Volley,
            Attack::Lanes,
            Attack::Charge,
            Attack::Bloom,
        ][..],
        _ => &[
            Attack::Lanes,
            Attack::Bloom,
            Attack::Charge,
            Attack::Sweep,
            Attack::Volley,
        ][..],
    };
    patterns[e.attacks as usize % patterns.len()]
}
fn ground(world: &World, mut p: Vec3, reference: f32) -> Option<Vec3> {
    let scale = world.voxel_scale.meters();
    // A bounded local ground query. Ceiling tops cannot become targets through the floor.
    for dy in [0.0, -scale, scale, -2.0 * scale, 2.0 * scale] {
        p.y = reference + dy;
        if !factory::solid_point(world, p + Vec3::Y * 0.12)
            && factory::solid_point(world, p - Vec3::Y * 0.18)
        {
            return Some(p);
        }
    }
    None
}
fn hazard(
    world: &World,
    out: &mut Vec<tactics::Hazard>,
    p: Vec3,
    floor: f32,
    radius: f32,
    delay: f32,
    damage: f32,
) {
    if out.len() >= 16 {
        return;
    }
    if let Some(pos) = ground(world, p, floor) {
        out.push(tactics::Hazard {
            pos,
            radius,
            delay,
            life: 0.22,
            pulse: 0.0,
            damage,
            spores: false,
        });
    }
}
fn release(
    e: &mut Enemy,
    world: &World,
    shots: &mut Vec<Projectile>,
    hazards: &mut Vec<tactics::Hazard>,
) {
    let tier = e.menace.min(2) as f32;
    let damage = 16.0 + tier * 4.0 + if e.hart.enraged { 3.0 } else { 0.0 };
    match e.hart.attack {
        Attack::Charge => {
            e.hart.active = 0.85;
            e.hart.hit = false;
        }
        Attack::Volley => {
            let count = if e.menace > 0 { 7 } else { 5 };
            let origin = e.center() + e.lunge * 1.25 + Vec3::Y * 0.65;
            for i in 0..count {
                if shots.len() >= 96 {
                    break;
                }
                let direction = Quat::from_rotation_y((i as f32 - (count - 1) as f32 * 0.5) * 0.17)
                    * (e.aim - origin).normalize_or_zero();
                shots.push(Projectile {
                    farm: None,
                    pos: origin,
                    velocity: direction * (8.0 + tier),
                    hostile: true,
                    payload: None,
                    payload_mods: Default::default(),
                    spell: [1, 5, 4][variant(e)],
                    life: 3.0,
                    mods: wands::ShotMods {
                        damage_mul: damage / 12.0,
                        ..Default::default()
                    },
                    bore_budget: 0,
                    trail_timer: 0.0,
                    splashed: false,
                    follow: None,
                });
            }
        }
        Attack::Bloom => {
            // A targeted opening strike, followed by a ring with a clear centre.
            hazard(world, hazards, e.aim, e.pos.y, 1.5, 0.65, damage);
            let count = if e.menace > 0 { 6 } else { 4 };
            for i in 0..count {
                let a = i as f32 * std::f32::consts::TAU / count as f32;
                hazard(
                    world,
                    hazards,
                    e.aim + Vec3::new(a.cos(), 0.0, a.sin()) * 3.7,
                    e.pos.y,
                    1.25,
                    1.25,
                    damage,
                );
            }
        }
        Attack::Lanes => {
            // Parallel, staggered lanes leave permanent two-metre dodge channels.
            let side = Vec3::new(-e.lunge.z, 0.0, e.lunge.x);
            for lane in [-1.0_f32, 1.0] {
                for i in 0..4 {
                    let p = e.pos + side * lane * 3.2 + e.lunge * (2.5 + i as f32 * 2.7);
                    hazard(
                        world,
                        hazards,
                        p,
                        e.pos.y,
                        1.15,
                        0.7 + i as f32 * 0.22,
                        damage,
                    );
                }
            }
        }
        Attack::Sweep => {}
    }
}

pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    hazards: &mut Vec<tactics::Hazard>,
    sounds: &mut Vec<Sound>,
    dirty: &mut Vec<IVec3>,
) -> (f32, Vec3) {
    let mut damage = 0.0;
    if e.hart.passive_review {
        return (0.0, Vec3::ZERO);
    }
    let before = e.pos;
    if (feet - e.home).length() > 43.0 {
        e.tell = 0.0;
        e.cooldown = 2.0;
        // Disengage without undoing damage, terrain, or finite water consumption.
        e.hart.crown.phase = hart_crown::Phase::Seek;
        e.hart.crown.clock = 3.0;
        e.hart.active = 0.0;
        return (0.0, Vec3::ZERO);
    }
    if variant(e) == 0 {
        let (planted, pulse_damage) = hart_crown::tick(e, world, feet, eye, dt, dirty);
        damage += pulse_damage;
        if planted { return (damage, (e.center()-eye).normalize_or_zero()*damage); }
    }
    if e.health < e.max_health() * 0.5 && !e.hart.enraged {
        e.hart.enraged = true;
        e.hart.follow = None;
        e.hart.recovery = 1.7;
        e.hart.active = 0.0;
        e.tell = 0.0;
        e.cooldown = 1.7;
        sound(sounds, Cue::PrismWindup, Some(e.center()), 0.9);
    }
    e.hart.recovery = (e.hart.recovery - dt).max(0.0);
    if e.stagger > 0.3 && e.poise > 2.65 {
        e.hart.active = 0.0;
        e.hart.follow = None;
        e.tell = 0.0;
        e.hart.recovery = 1.25;
        e.cooldown = 1.25;
    }
    if e.hart.active > 0.0 {
        let duration = dt.min(e.hart.active);
        e.hart.active -= duration;
        let movement = e.lunge
            * (11.5 + e.menace.min(2) as f32 * 0.8)
            * duration
            * if e.slow > 0.0 { 0.75 } else { 1.0 };
        let steps = (movement.length() / 0.10).ceil().max(1.0) as usize;
        for _ in 0..steps {
            let q = e.pos + movement / steps as f32;
            if free(world, q, 8) && factory::solid_point(world, q - Vec3::Y * 0.2) {
                e.pos = q;
                let delta = (feet - e.pos - e.lunge * 1.3) * Vec3::new(1.0, 0.0, 1.0);
                if !e.hart.hit
                    && delta.length() < 1.1
                    && (feet.y - e.pos.y).abs() < 2.2
                    && line(world, e.center(), eye)
                {
                    damage = 23.0 + e.menace.min(2) as f32 * 4.0;
                    e.hart.hit = true;
                }
            } else {
                e.hart.active = 0.0;
                break;
            }
        }
        if e.hart.active <= 0.0 {
            e.hart.recovery = if e.hart.follow.is_some() { 0.55 } else { 1.85 };
            e.cooldown = e.hart.recovery;
        }
    } else if e.tell > 0.0 {
        let desired = (-e.lunge.x).atan2(-e.lunge.z);
        let angle = (desired - e.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        e.yaw += angle.clamp(-dt * 3.5, dt * 3.5);
        e.tell -= dt;
        if e.tell <= 0.0 {
            if e.hart.attack == Attack::Sweep {
                let delta = (feet - e.pos) * Vec3::new(1.0, 0.0, 1.0);
                if delta.length() < 3.6
                    && delta.normalize_or_zero().dot(e.lunge) > -0.05
                    && (feet.y - e.pos.y).abs() < 1.25
                    && line(world, e.center(), eye)
                {
                    damage = 19.0 + e.menace.min(2) as f32 * 4.0;
                }
            }
            release(e, world, shots, hazards);
            sound(sounds, Cue::PrismRelease, Some(e.center()), 0.85);
            if e.hart.attack != Attack::Charge {
                e.hart.recovery = if e.hart.follow.is_some() { 0.65 } else { 1.65 };
                e.cooldown = e.hart.recovery;
            }
        }
    } else if e.hart.recovery <= 0.0 && e.cooldown <= 0.0 && e.visible {
        let distance = ((feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)).length();
        let chained = e.hart.follow;
        let attack = chained.unwrap_or_else(|| plan(e, distance));
        // Admission happens before committing a telegraph. Existing accepted hazards are retained.
        let needed = if attack == Attack::Lanes {
            8
        } else if attack == Attack::Bloom {
            7
        } else {
            0
        };
        if hazards.len() + needed > 16 || (attack == Attack::Volley && shots.len() + 7 > 96) {
            e.cooldown = 0.2;
        } else {
            e.hart.follow = if chained.is_some() {
                None
            } else if e.menace >= 2 {
                match (variant(e), attack) {
                    (0, Attack::Charge) => Some(Attack::Volley),
                    (1, Attack::Bloom) => Some(Attack::Volley),
                    (2, Attack::Lanes) => Some(Attack::Charge),
                    _ => None,
                }
            } else if attack == Attack::Charge && e.menace == 1 && e.hart.enraged {
                Some(Attack::Sweep)
            } else {
                None
            };
            e.hart.attack = attack;
            e.aim = if matches!(attack, Attack::Volley) {
                eye
            } else {
                feet
            };
            e.lunge = ((e.aim - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
            e.hart.windup = match attack {
                Attack::Charge => 1.15,
                Attack::Sweep => 0.95,
                _ => 1.1,
            };
            e.tell = e.hart.windup;
            e.attacks = e.attacks.wrapping_add(1);
            e.attack = 8; // Custom telegraphs below own the exact area; skip generic boss circles.
            sound(sounds, Cue::PrismWindup, Some(e.center()), 0.75);
        }
    } else if e.hart.recovery <= 0.0 && e.visible {
        let delta = (feet - e.pos) * Vec3::new(1.0, 0.0, 1.0);
        if delta.length() > 6.0 {
            let forward = delta.normalize_or_zero();
            for angle in [0.0_f32, 0.75, -0.75, 1.25, -1.25] {
                let direction = Quat::from_rotation_y(angle) * forward;
                let q = e.pos + direction * dt * 2.1;
                if free(world, q, 8) && factory::solid_point(world, q - Vec3::Y * 0.2) {
                    e.pos = q;
                    let desired = (-direction.x).atan2(-direction.z);
                    let turn = (desired - e.yaw + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    e.yaw += turn.clamp(-dt * 3.5, dt * 3.5);
                    break;
                }
            }
        }
    }
    let walked = ((e.pos - before) * Vec3::new(1.0, 0.0, 1.0)).length();
    e.phase += walked;
    let target = if walked > 0.001 { 1.0 } else { 0.0 };
    e.hart.moving += (target - e.hart.moving) * (1.0 - (-dt * 12.0).exp());
    // Swept floor following; decorative antlers do not form an invisible four-metre box.
    e.velocity.y = (e.velocity.y - dt * 14.0).max(-12.0);
    let vertical = e.velocity.y * dt;
    let steps = (vertical.abs() / 0.12).ceil().max(1.0) as usize;
    for _ in 0..steps {
        let q = e.pos + Vec3::Y * vertical / steps as f32;
        if free(world, q, 8) {
            e.pos = q;
        } else {
            e.velocity.y = 0.0;
            break;
        }
    }
    (damage, (e.center() - eye).normalize_or_zero() * damage)
}

pub(super) fn telegraph(e: &Enemy, m: &mut Model) {
    if e.health > 0.0 && variant(e) == 0 { hart_crown::draw(e, m); }
    if e.tell <= 0.0 {
        return;
    }
    let c = color(e);
    let p = e.pos + Vec3::Y * 0.10;
    let side = Vec3::new(-e.lunge.z, 0.0, e.lunge.x);
    match e.hart.attack {
        Attack::Charge => {
            let reach = (11.5 + e.menace.min(2) as f32 * 0.8) * 0.85 + 2.4;
            for sign in [-1.0_f32, 1.0] {
                m.beam(
                    p + side * sign * 1.3,
                    p + e.lunge * reach + side * sign * 1.3,
                    0.022,
                    c,
                    0.75,
                );
            }
        }
        Attack::Sweep => {
            for i in 0..18 {
                let a = -1.62 + i as f32 * 3.24 / 18.0;
                let b = -1.62 + (i + 1) as f32 * 3.24 / 18.0;
                m.beam(
                    p + Quat::from_rotation_y(a) * e.lunge * 3.6,
                    p + Quat::from_rotation_y(b) * e.lunge * 3.6,
                    0.024,
                    c,
                    0.75,
                );
            }
        }
        Attack::Volley => {
            for a in [-0.52_f32, 0.0, 0.52] {
                m.beam(
                    e.center() + Vec3::Y * 0.5,
                    e.center() + Quat::from_rotation_y(a) * e.lunge * 3.0,
                    0.016,
                    c,
                    0.6,
                );
            }
        }
        _ => {
            // Ground strikes reveal their complete exact footprint through admitted Hazard rings.
            m.orb(
                e.pos + Vec3::Y * 3.0,
                0.09 + 0.05 * (1.0 - e.tell / e.hart.windup),
                c,
                0.8,
            );
        }
    }
}

/// Disposable native review uses the same sampler, Enemy, tick, damage and renderer as play.
pub fn install_review(world: &mut World, player: &mut Player, variant: u8, difficulty: u8) {
    let scale = world.voxel_scale.meters();
    let base = Vec3::new(0.0, 80.0, 0.0);
    let mut edits = Vec::new();
    let n = scale.recip() as i32;
    for y in -2 * n..=12 * n {
        for z in -19 * n..=19 * n {
            for x in -19 * n..=19 * n {
                let cell = IVec3::new(x, 80 * n + y, z);
                let p = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) * scale;
                if let Some(block) = super::super::prism_sanctum::block(p, scale) {
                    edits.push((cell, block));
                }
            }
        }
    }
    world.set_many(&edits);
    if std::env::var_os("VOXEL_HART_CROWN_REVIEW").is_some() {
        // Disposable fixture: finite wet contact, a two-metre trench, and a torch
        // beyond it. These are normal editable blocks, not simulated boss resources.
        let mut archive = Vec::new();
        for z in -11*n..=-3*n { for x in -4*n..=4*n {
            for y in 77*n..80*n {
                let b = if z >= -5*n && y >= 79*n { super::super::BlockId::Water }
                    else if (-9*n..=-7*n).contains(&z) { super::super::BlockId::Air }
                    else { super::super::BlockId::StoneBricks };
                archive.push((IVec3::new(x,y,z),b));
            }
        }}
        archive.push((IVec3::new(0,80*n,-10*n),super::super::BlockId::Torch));
        world.set_many(&archive);
    }
    let site = (0..12)
        .map(|x| IVec3::new(x, 0, 1))
        .find(|p| {
            let e = Enemy::new(*p, 3, 8, base, difficulty.min(2));
            self::variant(&e) == variant as usize % 3
        })
        .unwrap_or(IVec3::new(0, 0, 1));
    let mut enemy = Enemy::new(
        site,
        3,
        8,
        base + Vec3::new(0.0, 0.02, -2.0),
        difficulty.min(2),
    );
    enemy.cooldown = if std::env::var_os("VOXEL_HART_SHOWCASE").is_some() {
        999.0
    } else {
        2.5
    };
    enemy.hart.passive_review = std::env::var_os("VOXEL_HART_SHOWCASE").is_some();
    if std::env::var_os("VOXEL_HART_ENRAGED").is_some() {
        enemy.health *= 0.45;
    }
    enemy.yaw = -2.0;
    if std::env::var_os("VOXEL_HART_CROWN_REVIEW").is_some() { enemy.yaw=0.0; }
    world.adventure.enemies.clear();
    world.adventure.enemies.push(enemy);
    world.adventure.wand = 2;
    if std::env::var_os("VOXEL_HART_MANUAL").is_some() {
        world.inventory.hotbar[5]=Some(super::super::inventory::ItemStack::new(super::super::inventory::Item::Tool(1),1));
    }
    world.adventure.hunger = 50.0; // No regeneration hides the damage received in this review.
    player.pos = (base
        + if world.adventure.enemies[0].hart.passive_review {
            Vec3::new(4.8, 0.04, 5.3)
        } else {
            Vec3::new(6.2, 0.04, 7.8)
        })
        / scale;
    player.precise_pos = player.pos.as_dvec3();
    player.vel = Vec3::ZERO;
    let direction =
        (base + Vec3::new(0.0, 2.0, -2.0) - player.eye(world.voxel_scale) * scale).normalize();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    println!(
        "[hart-review] installed variant={variant} difficulty={difficulty} initial_health={:.1}",
        world.adventure.health
    );
}

pub fn observe_review(world: &World) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static LAST: AtomicU32 = AtomicU32::new(u32::MAX);
    let key = (world.adventure.time / 3.0) as u32;
    if LAST.swap(key, Ordering::Relaxed) == key {
        return;
    }
    if std::env::var_os("VOXEL_HART_CROWN_REVIEW").is_some() {
        let n=world.voxel_scale.meters().recip() as i32;
        let mut water_units=0u32;
        let mut ice=0u32;
        for z in -11*n..=-3*n { for x in -4*n..=4*n { for y in 77*n..=80*n {
            let p=IVec3::new(x,y,z);
            water_units+=world.water_volume(p) as u32;
            ice+=u32::from(world.get(p)==super::super::BlockId::Ice);
        }}}
        println!("[hart-crown-review] water_units={water_units} ice_voxels={ice} material_units={}",water_units+8*ice);
    }
    if let Some(e) = world.adventure.enemies.iter().find(|e| e.kind == 8) {
        println!(
            "[hart-review] t={:.2} player_health={:.1} boss_health={:.1}/{:.1} attack={:?} tells={:.2} active={:.2} recovery={:.2} attacks={} pos={:?} crown={:?} grown={} cut_fronts={}",
            world.adventure.time,
            world.adventure.health,
            e.health,
            e.max_health(),
            e.hart.attack,
            e.tell,
            e.hart.active,
            e.hart.recovery,
            e.attacks,
            e.pos, e.hart.crown.phase, e.hart.crown.grown, e.hart.crown.severed
        );
    }
}

pub fn review_camera(player: &mut Player, scale: f32, time: f32) {
    if std::env::var_os("VOXEL_HART_CROWN_REVIEW").is_some() {
        let feet=Vec3::new(5.0,80.02,-0.5);
        player.pos=feet/scale; player.precise_pos=player.pos.as_dvec3(); player.vel=Vec3::ZERO;
        let d=(Vec3::new(0.0,81.4,-5.0)-(feet+Vec3::Y*1.62)).normalize();
        player.yaw=d.x.atan2(-d.z); player.pitch=d.y.asin(); return;
    }
    let a = time * 0.22;
    let feet = Vec3::new(a.cos() * 10.0, 80.02, a.sin() * 10.0);
    player.pos = feet / scale;
    player.precise_pos = player.pos.as_dvec3();
    player.vel = Vec3::ZERO;
    let direction = (Vec3::new(0.0, 82.0, 0.0) - (feet + Vec3::Y * 1.62)).normalize();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
}

/// Opt-in duel review fires the real equipped wand, with its normal mana and cooldown.
pub fn review_cast(world: &mut World, player: &mut Player) -> bool {
    if std::env::var_os("VOXEL_HART_CROWN_RETARGET").is_some() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static MOVED: AtomicBool=AtomicBool::new(false);
        let ready=world.adventure.enemies.iter().any(|e| e.kind==8 && e.hart.crown.cycles==1
            && e.hart.crown.grown>0 && e.hart.crown.phase==hart_crown::Phase::Vent && e.hart.crown.clock<2.0);
        if ready && !MOVED.swap(true,Ordering::Relaxed) {
            let n=world.voxel_scale.meters().recip() as i32;
            let from=IVec3::new(0,80*n,-10*n);
            let to=IVec3::new(4*n,80*n,-10*n);
            use super::super::BlockId as B;
            if world.get(from)==B::Torch && world.get(to)==B::Air {
                world.set_many(&[(from,B::Air),(to,B::Torch)]);
                if world.get(to)!=B::Torch { world.set(from,B::Torch); }
                world.adventure.dirty_voxels.extend([from,to]);
                println!("[hart-crown-review] SCRIPTED one-torch transfer from={from:?} to={to:?} destination={:?}",world.get(to));
            }
        }
    }
    if std::env::var_os("VOXEL_HART_CROWN_CUT").is_some() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static CUT: AtomicBool = AtomicBool::new(false);
        let junction = world.adventure.enemies.iter().find(|e| e.kind==8)
            .filter(|e| e.hart.crown.phase==hart_crown::Phase::Warn && e.hart.crown.clock<0.7)
            .map(|e| (e.hart.crown.root,e.hart.crown.target));
        if let Some((root,target))=junction
            && !CUT.swap(true,Ordering::Relaxed) {
                let s=world.voxel_scale.meters();
                let delta=target-(root.as_vec3()+Vec3::splat(0.5))*s;
                let across=if delta.x.abs()>=delta.z.abs(){IVec3::Z}else{IVec3::X};
                let width=(1.0/s).ceil() as i32;
                let mut adventure=std::mem::take(&mut world.adventure);
                let mut removed=0;
                for w in -(width/2)..=(width-1)/2 {
                    let q=root+IVec3::Y+across*w;
                    if world.get(q)==super::super::BlockId::Ice && adventure.carve_cell(world,q,false) { removed+=1; }
                }
                world.adventure=adventure;
                println!("[hart-crown-review] SCRIPTED utility cut removed={removed} root={root:?}; remaining ice is ordinary world material");
            }
    }
    if std::env::var_os("VOXEL_HART_CAST").is_none() {
        return false;
    }
    if let Some(enemy) = world
        .adventure
        .enemies
        .iter()
        .find(|e| e.kind == 8 && e.health > 0.0)
    {
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        let direction = (enemy.center() - eye).normalize_or_zero();
        player.yaw = direction.x.atan2(-direction.z);
        player.pitch = direction.y.asin();
        return true;
    }
    false
}
