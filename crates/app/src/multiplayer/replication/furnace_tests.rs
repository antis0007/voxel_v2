use super::*;
use crate::world::{factory::{self, Kind}, inventory::{Item, ItemStack}};
use crate::simulation::{Budget, residency::Residency};

const POS: IVec3 = IVec3::new(-16, 64, -16);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }

#[test]
fn furnace_chests_replicate_powered_work_to_two_joiners_and_reconnect() {
    let mut host = world();
    let source = POS - IVec3::X;
    let dest = POS + IVec3::X;
    for at in [source, dest] {
        host.set(at, BlockId::Chest);
        host.block_data.set_chest(at, vec![None; BlockId::Chest.chest_slots()]);
    }
    let mut slots = host.block_data.chest(source);
    slots[0] = Some(ItemStack::new(Item::Resource(9), 2));
    host.block_data.set_chest(source, slots);
    factory::fixture_place(&mut host, POS, Kind::Furnace).unwrap();
    factory::fixture_place(&mut host, POS - IVec3::Z, Kind::Cable).unwrap();
    let battery = factory::fixture_place(&mut host, POS - IVec3::Z * 2, Kind::Battery).unwrap();
    battery.upgrade = 1; battery.energy = factory::power::battery_capacity(1);
    let mut player = Player::new(); player.pos = POS.as_vec3();
    for (face, role) in [(0, 2), (1, 1)] {
        let command = actions::Command::Factory(factory::Action::SetFace(POS, face, role));
        let bytes = encode(0, &command).unwrap();
        let decoded: actions::Command = decode(&bytes[1..]).unwrap();
        actions::apply(decoded, &mut host, &player, &DayNightCycle::default(), 2, 0, false,
            &mut actions::Tools::default(), &mut world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS),
            &mut HashSet::new(), &mut HashSet::new()).unwrap();
    }
    let day = DayNightCycle::default();
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    let now = std::time::Instant::now();
    let mut factory_image = FactoryImage::default();
    let mut joiners = [(world(), Mirror::default()), (world(), Mirror::default())];
    for tick in 0..=28 {
        if tick > 0 {
            host.factory.tick(0.25, 1);
            factory::tick_world(&mut host, &player, 0.25, &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
        }
        factory_image.refresh(&mut host, now + std::time::Duration::from_millis(tick * 250)).unwrap();
        if tick == 7 || tick == 28 { joiners[1] = (world(), Mirror::default()); }
        for (joiner, mirror) in &mut joiners {
            let packet = capture_shared(&host, &player, &day, tick, Vec::new(), mirror, &factory_image).unwrap();
            for _ in 0..2 {
                let result = apply(&packet, joiner, &mut Player::new(), &mut DayNightCycle::default(), &mut blocks, &mut chunks).unwrap();
                assert_eq!(result.command_ack, tick);
                let (a, b) = (&host.factory.nodes[&POS], &joiner.factory.nodes[&POS]);
                assert_eq!((a.input, a.output, a.progress, a.energy, a.mode, a.enabled, a.faces), (b.input, b.output, b.progress, b.energy, b.mode, b.enabled, b.faces));
                assert_eq!(host.block_data.chest(source), joiner.block_data.chest(source));
                assert_eq!(host.block_data.chest(dest), joiner.block_data.chest(dest));
            }
        }
    }
    let count = |w: &World| w.block_data.chest(dest).iter().flatten().filter(|s| s.item == Item::Resource(6)).map(|s| s.count).sum::<u32>();
    assert_eq!(count(&host), 2);
    assert!(joiners.iter().all(|(w, _)| count(w) == 2));
    assert_eq!(host.factory.nodes[&POS].input[9] + host.factory.nodes[&POS].output[6], 0);
}
