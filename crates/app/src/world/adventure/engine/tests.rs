use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::WorldType;

fn fixture() -> (World, Enemy) {
    let world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    let mut y = 80.0;
    while y > -40.0 && !factory::solid_point(&world, Vec3::new(4.5, y - 0.1, 4.5)) {
        y -= 0.1;
    }
    let mut e = Enemy::new(IVec3::ZERO, 3, KIND, Vec3::new(4.5, y, 4.5), 0);
    e.engine.initialized = true;
    e.engine.stock = STOCK;
    e.engine.installed = Some(Module::Cutter);
    e.engine.last_health = e.health;
    (world, e)
}

#[test]
fn heavy_hit_in_transit_drops_a_real_item_the_player_can_take() {
    let (mut world, mut e) = fixture();
    let far = e.pos + Vec3::new(20.0, 0.0, 0.0);
    let mut sounds = Vec::new();
    e.engine.transfer = Some((Module::Launcher, 0.5));
    e.health -= KNOCK_DAMAGE + 1.0;
    manufacture(&mut e, &mut world, far, 0.01, &mut sounds);
    let (module, at) = e.engine.loose.expect("module knocked loose");
    assert_eq!(module, Module::Launcher);
    assert!(e.engine.transfer.is_none() && e.engine.installed == Some(Module::Cutter));
    let before = world.inventory.count_item(Module::Launcher.item());
    manufacture(&mut e, &mut world, at, 0.01, &mut sounds);
    assert_eq!(world.inventory.count_item(Module::Launcher.item()), before + 1);
    assert!(e.engine.loose.is_none());
    assert_eq!(e.engine.stolen, 1);
}

#[test]
fn light_hits_never_knock_a_module_loose() {
    let (mut world, mut e) = fixture();
    let far = e.pos + Vec3::new(20.0, 0.0, 0.0);
    e.engine.transfer = Some((Module::Brace, 0.2));
    e.health -= KNOCK_DAMAGE * 0.5;
    manufacture(&mut e, &mut world, far, 0.01, &mut Vec::new());
    assert!(e.engine.loose.is_none() && e.engine.transfer.is_some());
}

#[test]
fn production_stops_when_the_bins_are_empty() {
    let (mut world, mut e) = fixture();
    let near = e.pos + Vec3::new(3.0, 0.0, 0.0);
    let mut installs = 0;
    let mut last = e.engine.installed;
    for _ in 0..4000 {
        manufacture(&mut e, &mut world, near, 0.05, &mut Vec::new());
        if e.engine.installed != last {
            installs += 1;
            last = e.engine.installed;
        }
    }
    // Seven stock plus one unit of scrap per replaced tool: a bounded run.
    assert!((3..=6).contains(&installs), "installs {installs}");
    assert!(e.engine.stock < COST && e.engine.building.is_none() && e.engine.transfer.is_none());
}

#[test]
fn the_brace_blocks_only_frontal_hits() {
    let (_, mut e) = fixture();
    e.yaw = 0.0;
    e.engine.installed = Some(Module::Brace);
    // Facing is -Z; a shot travelling +Z hits the front.
    assert!(guarded(&e, Vec3::Z));
    assert!(!guarded(&e, Vec3::NEG_Z));
    assert!(!guarded(&e, Vec3::X));
    e.engine.installed = Some(Module::Cutter);
    assert!(!guarded(&e, Vec3::Z));
}

#[test]
fn a_committed_swing_keeps_its_direction() {
    let (mut world, mut e) = fixture();
    e.visible = true;
    e.cooldown = 0.0;
    let mut feet = e.pos + Vec3::new(0.0, 0.0, -2.5);
    let (mut shots, mut sounds, mut budget) = (Vec::new(), Vec::new(), 64usize);
    tick(&mut e, &mut world, feet, feet + Vec3::Y * 1.6, 0.01, &mut shots, &mut sounds, &mut budget);
    assert_eq!(e.engine.run.as_ref().map(|r| r.kind), Some(Action::Swing));
    let committed = e.lunge;
    feet += Vec3::new(2.5, 0.0, 0.0);
    for _ in 0..40 {
        tick(&mut e, &mut world, feet, feet + Vec3::Y * 1.6, 0.01, &mut shots, &mut sounds, &mut budget);
    }
    assert_eq!(e.lunge, committed);
}
