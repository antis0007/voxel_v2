use super::*;
use crate::world::{factory::{self, Kind}, inventory::{Item, ItemStack, all_recipes}};
use crate::simulation::{Budget, residency::Residency};

const POS: IVec3 = IVec3::new(-16, 64, -16);

fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }

#[test]
fn assembler_chests_replicate_intake_progress_and_output_to_two_joiners() {
    replicate_mode(0);
}

#[test]
fn assembler_single_batch_replicates_pause_and_mode_to_two_joiners() {
    replicate_mode(factory::assembler::SINGLE_BATCH);
}

fn replicate_mode(mode: u32) {
    let mut host = world();
    let p = POS + IVec3::X;
    host.set(p, BlockId::Chest);
    let mut slots = vec![None; BlockId::Chest.chest_slots()];
    slots[0] = Some(ItemStack::new(Item::Resource(6), 2));
    slots[1] = Some(ItemStack::new(Item::Resource(4), 1));
    host.block_data.set_chest(p, slots);
    let n = factory::fixture_place(&mut host, POS, Kind::Assembler).unwrap();
    n.target = all_recipes().iter().position(|r| r.name == "Hand-wound induction coil").unwrap() as u32 + 1;
    n.energy = 64;
    n.mode = mode;
    let player = Player::new();
    let day = DayNightCycle::default();
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    let now = std::time::Instant::now();
    let mut factory_image = FactoryImage::default();
    let mut joiners = [(world(), Mirror::default()), (world(), Mirror::default())];
    for tick in 0..=9 {
        if tick > 0 {
            host.factory.tick(0.25, 1);
            factory::tick_world(&mut host, &player, 0.25, &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
        }
        factory_image.refresh(&mut host, now + std::time::Duration::from_millis(tick * 250)).unwrap();
        if tick == 9 { joiners[1] = (world(), Mirror::default()); }
        for (joiner, mirror) in &mut joiners {
            let packet = capture_shared(&host, &player, &day, tick, Vec::new(), mirror, &factory_image).unwrap();
            for _ in 0..2 {
                let result = apply(&packet, joiner, &mut Player::new(), &mut DayNightCycle::default(), &mut blocks, &mut chunks).unwrap();
                assert_eq!(result.command_ack, tick);
                let (a, b) = (&host.factory.nodes[&POS], &joiner.factory.nodes[&POS]);
                assert_eq!((a.input, a.output, a.stock, a.progress, a.energy, a.mode, a.enabled), (b.input, b.output, b.stock, b.progress, b.energy, b.mode, b.enabled));
                assert_eq!(host.block_data.chest(p), joiner.block_data.chest(p));
            }
        }
    }
    let count = |w: &World| w.block_data.chest(p).iter().flatten().filter(|s| s.item == Item::Resource(16)).map(|s| s.count).sum::<u32>();
    assert_eq!(host.factory.nodes[&POS].enabled, mode != factory::assembler::SINGLE_BATCH);
    assert_eq!(count(&host), 1);
    assert!(joiners.iter().all(|(w, _)| count(w) == 1));
    assert_eq!(host.factory.nodes[&POS].energy, 64 - factory::assembler::POWER * factory::assembler::CRAFT_TICKS);
}
