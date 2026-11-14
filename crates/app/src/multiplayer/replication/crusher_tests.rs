use super::*;
use crate::world::{factory::{self, Kind, sides}, inventory::{Item, ItemStack}};
use crate::simulation::{Budget, residency::Residency};

const C: IVec3 = IVec3::new(-16, 64, -16);
const F: IVec3 = IVec3::new(-15, 64, -15);
const SOURCE: IVec3 = IVec3::new(-17, 64, -16);
const BRIDGE: IVec3 = IVec3::new(-15, 64, -16);
const DEST: IVec3 = IVec3::new(-14, 64, -15);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }
fn count(w: &World, p: IVec3, id: u8) -> u32 {
    w.block_data.chest(p).iter().flatten().filter(|s| s.item == Item::Resource(id)).map(|s| s.count).sum()
}
fn total(w: &World) -> u32 {
    2 * ((count(w, SOURCE, 0) + raw_source(w)) * 64 + w.factory.nodes[&C].input[0])
        + w.factory.nodes[&C].output[9] + count(w, BRIDGE, 9) * 64
        + w.factory.nodes[&F].input[9] + w.factory.nodes[&F].output[6] + count(w, DEST, 6) * 64
}
fn command(host: &mut World, player: &Player, action: factory::Action) {
    let bytes = encode(0, &actions::Command::Factory(action)).unwrap();
    let decoded: actions::Command = decode(&bytes[1..]).unwrap();
    actions::apply(decoded, host, player, &DayNightCycle::default(), 2, 0, false,
        &mut actions::Tools::default(), &mut world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS),
        &mut HashSet::new(), &mut HashSet::new()).unwrap();
}
fn raw_source(w: &World) -> u32 {
    w.block_data.chest(SOURCE).iter().flatten().filter(|s| matches!(s.item,
        Item::Block(b) if crate::world::expedition::ore_resource(b) == Some(0))).map(|s| s.count).sum()
}
#[test]
fn crusher_chests_two_joiners_duplicate_images_pause_and_reconnect() { check_two_joiners(Item::Resource(0)); }
#[test]
fn quarry_chests_two_joiners_duplicate_images_pause_and_reconnect() {
    for block in [BlockId::CopperOre, BlockId::Malachite] { check_two_joiners(Item::Block(block)); }
}
fn check_two_joiners(source: Item) {
    let mut host = world();
    for p in [SOURCE, BRIDGE, DEST] {
        host.set(p, BlockId::Chest); host.block_data.set_chest(p, vec![None; BlockId::Chest.chest_slots()]);
    }
    let mut slots = host.block_data.chest(SOURCE); slots[0] = Some(ItemStack::new(source, 2));
    host.block_data.set_chest(SOURCE, slots);
    let mut player = Player::new(); player.pos = C.as_vec3();
    for (p, kind, input, dir) in [(C, Kind::Crusher, 1, -IVec3::Z), (F, Kind::Furnace, 5, IVec3::Y)] {
        let n = factory::fixture_place(&mut host, p, kind).unwrap();
        for face in 0..6 { sides::apply_role(n, face, sides::Role::Off as u8); }
        factory::fixture_place(&mut host, p + dir, Kind::Cable).unwrap();
        let battery = factory::fixture_place(&mut host, p + dir * 2, Kind::Battery).unwrap();
        battery.upgrade = 1; battery.energy = factory::power::battery_capacity(1);
        for (face, role) in [(0, 2), (input, 1)] { command(&mut host, &player, factory::Action::SetFace(p, face, role)); }
    }
    let day = DayNightCycle::default();
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    let now = std::time::Instant::now(); let mut factory_image = FactoryImage::default();
    let mut joiners = [(world(), Mirror::default()), (world(), Mirror::default())];
    let mut paused = None;
    for tick in 0..=104 {
        if tick == 8 { command(&mut host, &player, factory::Action::Toggle(C)); paused = Some(host.factory.nodes[&C].progress); }
        if tick == 12 { command(&mut host, &player, factory::Action::Toggle(C)); }
        if tick > 0 {
            host.factory.tick(0.25, 1);
            factory::tick_world(&mut host, &player, 0.25, &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
        }
        if (8..12).contains(&tick) { assert_eq!(Some(host.factory.nodes[&C].progress), paused); }
        assert_eq!(total(&host), 256);
        factory_image.refresh(&mut host, now + std::time::Duration::from_millis(tick * 250)).unwrap();
        if tick == 9 || tick == 104 { joiners[1] = (world(), Mirror::default()); }
        for (joiner, mirror) in &mut joiners {
            let packet = capture_shared(&host, &player, &day, tick, Vec::new(), mirror, &factory_image).unwrap();
            for _ in 0..2 {
                let result = apply(&packet, joiner, &mut Player::new(), &mut DayNightCycle::default(), &mut blocks, &mut chunks).unwrap();
                assert_eq!(result.command_ack, tick);
                for p in [C, F] {
                    let (a, b) = (&host.factory.nodes[&p], &joiner.factory.nodes[&p]);
                    assert_eq!((a.input, a.output, a.progress, a.energy, a.mode, a.enabled, a.faces), (b.input, b.output, b.progress, b.energy, b.mode, b.enabled, b.faces));
                }
                for p in [SOURCE, BRIDGE, DEST] { assert_eq!(host.block_data.chest(p), joiner.block_data.chest(p)); }
                assert_eq!(total(joiner), 256);
            }
        }
    }
    assert_eq!(count(&host, DEST, 6), 4);
    assert!(joiners.iter().all(|(w, _)| count(w, DEST, 6) == 4));
}
