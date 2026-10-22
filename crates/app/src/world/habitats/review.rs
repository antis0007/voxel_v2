//! Disposable native evidence: real simulation, paid spells, ordinary input and
//! the production disk checkpoint service. Fixtures never alter combat rules.
use super::*;
use crate::{
    player::{Input, Player},
    world::{DayNightCycle, storage::SaveService},
};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

pub(crate) fn mode() -> &'static str {
    static MODE: OnceLock<String> = OnceLock::new();
    MODE.get_or_init(|| std::env::var("VOXEL_HABITAT_REVIEW").unwrap_or_default())
}
pub(crate) fn enabled() -> bool {
    !mode().is_empty()
}
static PAUSE: AtomicBool = AtomicBool::new(false);
pub(crate) fn paused() -> bool {
    PAUSE.load(Ordering::Relaxed)
}
fn pose(player: &mut Player, scale: f32) {
    player.pos = if mode() == "region" {
        Vec3::new(294.5, 1.0, 62.5)
    } else {
        Vec3::new(0.5, 80.02, 6.5)
    } / scale;
    player.vel = Vec3::ZERO;
    player.yaw = 0.0;
    player.pitch = -0.12;
}
pub(crate) fn camera(player: &mut Player, scale: f32) -> bool {
    static PLACED: AtomicBool = AtomicBool::new(false);
    if !PLACED.swap(true, Ordering::Relaxed) {
        pose(player, scale);
        true
    } else {
        false
    }
}
pub(crate) fn install(world: &mut World, player: &mut Player) {
    pose(player, world.voxel_scale.meters());
    if mode() == "region" {
        println!(
            "[habitat-review] natural generator={} region; no terrain fixture edits",
            world.generator_version
        );
        return;
    }
    let farm = matches!(mode(), "farm" | "matron-farm");
    let mut edits = Vec::new();
    let scale = world.voxel_scale.meters();
    let n = scale.recip().round() as i32;
    for x in -8..=9 {
        for z in -8..=10 {
            for y in 79..=84 {
                let wall = farm
                    && ((x == -2 || x == 2) && (-2..=2).contains(&z)
                        || (z == -2 || z == 2) && (-2..=2).contains(&x))
                    && (80..=83).contains(&y);
                let b = if y == 79 {
                    if x == 2 && z == 0 {
                        B::Water
                    } else {
                        B::FungalSoil
                    }
                } else if wall {
                    B::StoneBricks
                } else {
                    B::Air
                };
                let p = (Vec3::new(x as f32, y as f32, z as f32) / scale)
                    .floor()
                    .as_ivec3();
                for dx in 0..n {
                    for dy in 0..n {
                        for dz in 0..n {
                            edits.push((p + IVec3::new(dx, dy, dz), b));
                        }
                    }
                }
            }
        }
    }
    if farm {
        for x in 9..=122 {
            for z in -12..=12 {
                let p = (Vec3::new(x as f32, 79.0, z as f32) / scale)
                    .floor()
                    .as_ivec3();
                for dx in 0..n {
                    for dy in 0..n {
                        for dz in 0..n {
                            edits.push((p + IVec3::new(dx, dy, dz), B::StoneBricks));
                        }
                    }
                }
            }
        }
    }
    if farm {
        let p = cell(world, Vec3::new(2.5, 79.0, 6.5));
        for dx in 0..n {
            for dy in 0..n {
                for dz in 0..n {
                    edits.push((p + IVec3::new(dx, dy, dz), B::Water));
                }
            }
        }
    }
    world.set_many(&edits);
    if farm {
        // Real stock and output ports at both the original and relocated beds.
        // Finite fixture supplies are disclosed separately from source buffers.
        for p in [Vec3::new(-1.5, 80.0, 0.5), Vec3::new(-1.5, 80.0, 6.5)] {
            let q = cell(world, p);
            if let Some(node) =
                super::super::factory::fixture_place(world, q, super::super::factory::Kind::Storage)
            {
                node.input[13] = 512;
            }
        }
    }
    let family = if mode() == "farm" {
        Family::Nursery
    } else {
        Family::Matron
    };
    let mut s = Source::new(1, family, Some(Vec3::new(0.5, 80.0, 0.5)));
    s.feed = 4096;
    s.charge = 64;
    if farm {
        s.development = family.growth() - 4;
        s.receiver = true;
        s.automatic = true;
        s.emitter = true;
        s.reserve = true;
        s.mana = 4096;
    } else {
        s.resident = Some(Resident::new(s.pos.unwrap(), family.kind()));
        s.births = 1;
    }
    world.habitats = State {
        seeded: true,
        next_id: 2,
        sources: vec![s],
        ..Default::default()
    };
    world.adventure.enemies.clear();
    world.adventure.projectiles.clear();
    world.adventure.wand = 2;
    world.adventure.loadout.slots = [
        0,
        adventure::wands::EMPTY,
        adventure::wands::EMPTY,
        adventure::wands::EMPTY,
    ];
    world.adventure.respawn_point = Some(Vec3::new(0.5, 80.02, 8.5));
    println!(
        "[habitat-review] mode={} supplied fixture: 64 biomass, 64 charge, farm mana=4096; near-ripe initial pod; all later growth/costs/health unchanged",
        mode()
    );
}
pub(crate) fn input(world: &World, player: &mut Player, input: &mut Input) {
    if paused() || mode() == "region" {
        return;
    }
    let scale = world.voxel_scale.meters();
    let t = world.adventure.time;
    let farm = matches!(mode(), "farm" | "matron-farm");
    let target = if farm {
        if t < 34.0 {
            Vec3::new(0.5, 80.0, 6.5)
        } else if t < 70.0 {
            Vec3::new(115.5, 80.0, 6.5)
        } else {
            Vec3::new(0.5, 80.0, 6.5)
        }
    } else if mode() == "pressure" {
        world
            .adventure
            .enemies
            .iter()
            .find(|e| e.kind == 15)
            .map_or(Vec3::new(0.5, 80.0, 2.0), |e| {
                e.pos + Vec3::new((t * 0.4).sin() * 1.0, 0.0, 3.0)
            })
    } else {
        Vec3::new(
            0.5 + (t * 0.7).sin() * 5.0,
            80.0,
            4.5 + (t * 0.7).cos() * 1.5,
        )
    };
    let aim = world
        .adventure
        .enemies
        .iter()
        .find(|e| e.kind == 15)
        .map_or(Vec3::new(0.5, 81.0, 0.5), |e| e.pos + Vec3::Y * 1.2);
    let direction = (aim - player.eye(world.voxel_scale) * scale).normalize_or_zero();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    let delta = (target - player.pos * scale) * Vec3::new(1.0, 0.0, 1.0);
    let forward = player.flat_forward();
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    input.forward = delta.dot(forward) > 0.2;
    input.back = delta.dot(forward) < -0.2;
    input.right = delta.dot(right) > 0.2;
    input.left = delta.dot(right) < -0.2;
    input.jump = world.adventure.enemies.iter().any(|e| {
        e.preparing_stamp() && e.tell < 0.22 && (e.pos - player.pos * scale).length() < 3.4
    });
}
pub(crate) fn casting(world: &World) -> bool {
    mode() == "duel"
        && world.adventure.time > 3.0
        && world
            .adventure
            .enemies
            .iter()
            .any(|e| e.kind == 15 && e.health > 0.0)
}
pub(crate) fn observe(world: &mut World, player: &Player) {
    static MOVED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
    if matches!(mode(), "farm" | "matron-farm") && world.adventure.time >= 28.0 {
        if MOVED.load(Ordering::Relaxed) == 0
            && world
                .habitats
                .sources
                .first()
                .is_some_and(|s| s.resident.is_none())
        {
            action(
                world,
                player.pos * world.voxel_scale.meters(),
                Action::Carry(1),
            );
            if world.habitats.sources[0].pos.is_none() {
                MOVED.store(1, Ordering::Relaxed);
                println!("[habitat-move] carried source=1 with receiver and all buffers");
            }
        } else if MOVED.load(Ordering::Relaxed) == 1 && player.on_ground {
            action(
                world,
                player.pos * world.voxel_scale.meters(),
                Action::Place(1),
            );
            if let Some(p) = world.habitats.sources[0].pos {
                MOVED.store(2, Ordering::Relaxed);
                println!(
                    "[habitat-move] placed source=1 at={p:?} count={} births={} deaths={} mana={} xp={}",
                    world.habitats.sources.len(),
                    world.habitats.sources[0].births,
                    world.habitats.sources[0].deaths,
                    world.habitats.sources[0].mana,
                    world.habitats.sources[0].xp
                );
            }
        }
    }
    static SWITCHED: AtomicBool = AtomicBool::new(false);
    if matches!(mode(), "farm" | "matron-farm")
        && world.adventure.time >= 24.0
        && !SWITCHED.swap(true, Ordering::Relaxed)
    {
        action(
            world,
            player.pos * world.voxel_scale.meters(),
            Action::Reserve(1),
        );
        println!(
            "[habitat-review] normal local reserve control changed; later kills must consume paid real shots"
        );
    }
    static LAST: Mutex<i32> = Mutex::new(-1);
    let tick = (world.adventure.time * 0.5) as i32;
    let Ok(mut last) = LAST.lock() else {
        return;
    };
    if *last == tick {
        return;
    }
    *last = tick;
    for s in &world.habitats.sources {
        println!(
            "[habitat-live] t={:.2} source={} family={} pos={:?} distance={:.1} live_hp={:.1} pods={} spores={} development={} births={} deaths={} feed={} products={} harvest={} mana={} charge={} mana_used={} xp={} xp_made={} blocker={}",
            world.adventure.time,
            s.id,
            s.family.name(),
            s.pos,
            s.pos
                .map_or(0.0, |p| p.distance(player.pos * world.voxel_scale.meters())),
            s.resident.as_ref().map_or(0.0, |r| r.health),
            s.resident.as_ref().map_or(0, |r| r.pods),
            s.resident.as_ref().map_or(0, |r| r.spores),
            s.development,
            s.births,
            s.deaths,
            s.feed,
            s.products,
            s.harvested,
            s.mana,
            s.charge,
            s.mana_used,
            s.xp,
            s.xp_made,
            s.blocker
        );
    }
    println!(
        "[habitat-player] t={:.2} pos={:?} hp={:.1} mana={:.1} farm_shots={} mastery={}",
        world.adventure.time,
        player.pos * world.voxel_scale.meters(),
        world.adventure.health,
        world.adventure.mana,
        world.habitats.shots.len(),
        world.habitats.mastered
    );
    for (p, node) in &world.factory.nodes {
        if node.kind == super::super::factory::Kind::Storage {
            println!(
                "[habitat-crate] t={:.2} at={p:?} biomass={} fibre={}",
                world.adventure.time, node.input[13], node.input[4]
            );
        }
    }
}
struct Checkpoint {
    stage: u8,
    rounds: u8,
    expected: Vec<u8>,
    factory: Vec<u8>,
    time: f32,
    terrain: B,
    source_count: usize,
}
pub(crate) fn checkpoint(
    world: &mut World,
    player: &mut Player,
    day: &mut DayNightCycle,
    saves: &mut SaveService,
) {
    day.set_time_of_day(0.5);
    static CHECK: Mutex<Checkpoint> = Mutex::new(Checkpoint {
        stage: 0,
        rounds: 0,
        expected: Vec::new(),
        factory: Vec::new(),
        time: 0.0,
        terrain: B::Air,
        source_count: 0,
    });
    if !matches!(mode(), "farm" | "matron-farm" | "duel") {
        return;
    }
    let Some(root) = std::env::var_os("VOXEL_CAPTURE_DIR") else {
        return;
    };
    if std::env::var_os("VOXEL_SAVE_DIR").map(std::path::PathBuf::from)
        != Some(std::path::PathBuf::from(root).join("disposable-saves"))
    {
        return;
    }
    let Ok(mut c) = CHECK.lock() else {
        return;
    };
    if c.stage == 2 && c.rounds == 1 && mode() == "matron-farm" && world.habitats.mastered != 0 {
        c.stage = 0;
        c.expected.clear();
        c.factory.clear();
    }
    if c.stage == 0
        && world.adventure.time >= if mode() == "duel" { 2.5 } else { 52.0 }
        && (c.rounds > 0
            || mode() == "duel"
            || !world.habitats.shots.is_empty()
            || world.adventure.time >= 60.0)
    {
        world.habitats.encode(&mut c.expected);
        world.factory.encode_format(&mut c.factory, 10);
        c.time = world.adventure.time;
        c.source_count = world.habitats.sources.len();
        c.terrain = world.get(cell(world, Vec3::new(-1.5, 80.5, 0.5)));
        crate::world::storage::set_autosave_enabled(false);
        saves.request(true);
        c.stage = 1;
        PAUSE.store(true, Ordering::Relaxed);
        println!(
            "[habitat-disk] request sources={} farm_shots={} bytes={} round={} mastery={}",
            c.source_count,
            world.habitats.shots.len(),
            c.expected.len(),
            c.rounds + 1,
            world.habitats.mastered
        );
    } else if c.stage == 1 {
        saves.tick(Some(world), player, day);
        if saves.busy() {
            return;
        }
        c.stage = 2;
        c.rounds += 1;
        if saves.failed {
            eprintln!("[habitat-disk] FAILED save {:?}", saves.status);
            PAUSE.store(false, Ordering::Relaxed);
            return;
        }
        match crate::world::storage::load_review_checkpoint() {
            Ok(mut loaded) => {
                let mut actual = Vec::new();
                loaded.world.habitats.encode(&mut actual);
                let mut factory = Vec::new();
                loaded.world.factory.encode_format(&mut factory, 10);
                let terrain = loaded
                    .world
                    .get(cell(&loaded.world, Vec3::new(-1.5, 80.5, 0.5)))
                    == c.terrain;
                println!(
                    "[habitat-disk] loaded version={} habitats_equal={} terrain_equal={} sources_equal={} farm_shots={} duplicate_xp={} factory_equal={} round={} mastery={} max_mana={}",
                    crate::world::storage::SAVE_VERSION,
                    actual == c.expected,
                    terrain,
                    loaded.world.habitats.sources.len() == c.source_count,
                    loaded.world.habitats.shots.len(),
                    actual != c.expected,
                    factory == c.factory,
                    c.rounds,
                    loaded.world.habitats.mastered,
                    loaded.world.adventure.max_mana()
                );
                // Only preserve the review timer; gameplay state is the decoded checkpoint.
                loaded.world.adventure.time = c.time;
                *world = loaded.world;
                *player = loaded.player;
                *day = loaded.day_night;
            }
            Err(e) => eprintln!("[habitat-disk] FAILED load {e:#}"),
        }
        PAUSE.store(false, Ordering::Relaxed);
    }
}
