use super::{B, FULL, WHEAT, command};
use crate::{
    player::{Input, Player},
    simulation::fixed,
    world::{DayNightCycle, World, WorldType, inventory::ItemStack, rules::GameMode, storage},
};
use anyhow::{Context, Result, ensure};
use glam::{IVec3, Vec3};
use std::time::{Duration, Instant};

pub(crate) fn run() -> Result<()> {
    let directory =
        std::env::var_os("VOXEL_SAVE_DIR").context("Provide an empty scratch VOXEL_SAVE_DIR")?;
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    ensure!(
        directory.read_dir()?.next().is_none(),
        "Review needs an empty save directory"
    );
    let mut world = World::with_seed(
        WorldType::Planet,
        engine_core::VoxelScalePreset::OneMeter,
        7,
    );
    let mut rules = world.rules;
    rules.mode = GameMode::Creative;
    world.set_rules(rules);
    let mut day = DayNightCycle::default();
    let mut player = Player::new();
    let spawn = world.suggested_spawn();
    let x = spawn.x.floor() as i32;
    let z = spawn.z.floor() as i32;
    println!("[mill-review] home spawn={spawn:?}");
    let height = (x..=x + 13)
        .map(|x| world.terrain_height_at(x, z))
        .max()
        .unwrap();
    let mut home = Vec3::new(
        x as f32 + 0.5,
        (height as f32 + 5.0).max(spawn.y + 5.0),
        z as f32 + 0.5,
    );
    player.pos = home;
    player.precise_pos = home.as_dvec3();
    player.flying = true;
    player.flight.speed = 64.0;
    player.flight.boost = 4.0;
    for rise in [0.0, 4.0, 8.0, 16.0, 32.0] {
        player.pos = home + Vec3::Y * rise;
        player.precise_pos = player.pos.as_dvec3();
        println!("[mill-review] {}", command(&mut world, &player, "build"));
        if world.mill.is_some() {
            home = player.pos;
            break;
        }
    }
    let mill = world.mill.context("Commissioning failed")?;
    let wheel = mill.wheel();
    let high = mill.high();
    let low = mill.low();
    let marker = mill.origin() + IVec3::Z * 2;
    player.pos = wheel.as_vec3() + Vec3::new(0.5, 1.1, 0.5);
    player.precise_pos = player.pos.as_dvec3();
    let (loaded, _) = world.inventory.try_insert(ItemStack::new(WHEAT, 24));
    ensure!(loaded == 24, "Could not provision review wheat");
    println!("[mill-review] {}", command(&mut world, &player, "load"));
    ensure!(
        world.mill.context("Mill disappeared")?.wheat == 24
            && world.inventory.count_item(WHEAT) == 0,
        "Wheat did not transfer"
    );
    world.set(high, B::Air);
    world.set(marker, B::CopperBlock);
    ensure!(
        world.get(high) == B::Air
            && world.get(low) == B::IronBlock
            && world.get(marker) == B::CopperBlock,
        "Setup edits did not commit"
    );

    let step =
        |world: &mut World, player: &mut Player, day: &mut DayNightCycle, mut input: Input| {
            fixed::gameplay(world, player, &mut input, true, |_, _, _, _| {
                fixed::Actions::default()
            });
            day.advance(crate::simulation::STEP_SECONDS);
            world.orbital.planet_tick = crate::planet::tick_of(day);
        };
    let mut ascent = Input::default();
    ascent.jump = true;
    ascent.sprint = true;
    let mut descent = Input::default();
    descent.sneak = true;
    for _ in 0..14_400 {
        step(&mut world, &mut player, &mut day, ascent.clone());
        if player.pos.y >= 7_000.0 {
            break;
        }
    }
    ensure!(
        player.pos.y >= 7_000.0 && player.anchor.is_none(),
        "Normal flight did not reach space: {:?}",
        player.pos
    );
    // Stop thrust and allow one or more background releases before checkpointing.
    for _ in 0..1_500 {
        step(&mut world, &mut player, &mut day, Input::default());
    }
    let before = world.mill.context("Missing airborne mill state")?;
    ensure!(
        before.passed > before.published && before.passed < FULL / 2,
        "Expected partially processed background state"
    );
    println!(
        "[mill-review] airborne height={:.1} passed={} published={} flour={}",
        player.pos.y, before.passed, before.published, before.flour
    );

    let mut save = storage::SaveService::new();
    save.request(true);
    let deadline = Instant::now() + Duration::from_secs(30);
    while save.busy() && Instant::now() < deadline {
        save.tick(Some(&mut world), &player, &day);
        std::thread::sleep(Duration::from_millis(2));
    }
    ensure!(
        !save.busy() && !save.failed,
        "Save failed: {:?}",
        save.status
    );
    drop(save);
    let id = world.regions.view.session_name().to_owned();
    let saved = storage::load_named(&id)?.context("Saved planet missing from the world library")?;
    ensure!(
        saved.recovery_notice.is_none(),
        "The world library did not reopen the newest checkpoint"
    );
    ensure!(
        saved.player.pos == player.pos && saved.player.vel == player.vel && saved.player.flying,
        "Flight checkpoint changed player state"
    );
    world = saved.world;
    player = saved.player;
    day = saved.day_night;
    let after = world.mill.context("Mill did not reload")?;
    ensure!(
        ron::to_string(&before)? == ron::to_string(&after)?,
        "Mill state changed on reload"
    );
    ensure!(
        world.get(high) == B::Air
            && world.get(low) == B::IronBlock
            && world.get(marker) == B::CopperBlock,
        "Voxel edits changed on reload"
    );
    println!("[mill-review] reloaded in space height={:.1}", player.pos.y);
    for _ in 0..6_000 {
        step(&mut world, &mut player, &mut day, Input::default());
    }
    ensure!(
        world.mill.context("Missing background mill")?.passed == FULL / 2,
        "Upper spillway did not stop at half"
    );
    for _ in 0..14_400 {
        step(&mut world, &mut player, &mut day, descent.clone());
        if player.pos.y <= home.y + 5.0 {
            break;
        }
    }
    ensure!(
        player.pos.y <= home.y + 5.0 && player.pos.y >= home.y - 10.0,
        "Normal flight did not return: {:?}",
        player.pos
    );
    let mill = world.mill.context("Missing returned mill")?;
    let half = world.environment.quantum() * u64::from(FULL / 2);
    ensure!(
        world.get(high) == B::Air
            && world.get(low) == B::IronBlock
            && world.get(marker) == B::CopperBlock,
        "Voxel edits changed on return"
    );
    ensure!(
        mill.published == FULL / 2
            && world.water_microlitres(mill.head()) == half
            && world.water_microlitres(mill.tail()) == half,
        "Water did not materialize: passed={} published={} head={} tail={}",
        mill.passed,
        mill.published,
        world.water_microlitres(mill.head()),
        world.water_microlitres(mill.tail())
    );
    ensure!(
        mill.flour == 12
            && mill.wheat == 12
            && mill.last_return
                == (
                    FULL / 2 - before.published,
                    12 - u32::from(before.published) * 3
                ),
        "Mill production did not reconcile"
    );
    println!(
        "[mill-review] PASS edits=3 head_l={} tail_l={} flour={} wheat={}",
        half / 1_000_000,
        half / 1_000_000,
        mill.flour,
        mill.wheat
    );
    Ok(())
}
