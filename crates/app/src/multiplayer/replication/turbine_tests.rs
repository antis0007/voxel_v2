use super::*;
use crate::world::{factory::{self, Kind, industry::turbine}, inventory::{Item, ItemStack}, industry::STEAM};
const POS: IVec3 = IVec3::new(-16, 64, -16);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }
fn command(host: &mut World, player: &Player, action: factory::Action) {
    let command = actions::Command::Factory(action);
    let bytes = encode(0, &command).unwrap();
    let decoded: actions::Command = decode(&bytes[1..]).unwrap();
    actions::apply(decoded, host, player, &DayNightCycle::default(), 2, 0, false,
        &mut actions::Tools::default(), &mut world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS),
        &mut HashSet::new(), &mut HashSet::new()).unwrap();
}
#[test]
fn turbine_commands_replicate_exact_mass_to_two_joiners_and_reconnect() {
    let mut host = world();
    let n = factory::fixture_place(&mut host, POS, Kind::GasGenerator).unwrap();
    n.exhaust[STEAM] = 1152; n.enabled = false;
    host.inventory.hotbar[0] = Some(ItemStack::new(Item::Bucket(0), 1));
    let mut player = Player::new(); player.pos = POS.as_vec3();
    command(&mut host, &player, factory::Action::SetRecipe(POS, 1));
    let day = DayNightCycle::default(); let now = std::time::Instant::now();
    let mut image = FactoryImage::default();
    let mut joiners = [(world(), Mirror::default()), (world(), Mirror::default())];
    for tick in 0..6 {
        match tick {
            1 => command(&mut host, &player, factory::Action::Toggle(POS)),
            2 => command(&mut host, &player, factory::Action::Toggle(POS)),
            3 => command(&mut host, &player, factory::Action::SetRecipe(POS, 0)),
            4 => command(&mut host, &player, factory::Action::Drain(POS)),
            _ => {},
        }
        host.factory.tick(0.25, 1);
        image.refresh(&mut host, now + std::time::Duration::from_millis(tick * 250)).unwrap();
        if tick == 2 || tick == 5 { joiners[1] = (world(), Mirror::default()); }
        for (joiner, mirror) in &mut joiners {
            let packet = capture_shared(&host, &player, &day, tick, Vec::new(), mirror, &image).unwrap();
            for _ in 0..2 {
                apply(&packet, joiner, &mut Player::new(), &mut DayNightCycle::default(), &mut HashSet::new(), &mut HashSet::new()).unwrap();
                let (a,b) = (&host.factory.nodes[&POS], &joiner.factory.nodes[&POS]);
                assert_eq!((a.exhaust, a.water, a.separator_water_mg, a.energy, a.mode, a.enabled),
                    (b.exhaust, b.water, b.separator_water_mg, b.energy, b.mode, b.enabled));
                assert_eq!(host.inventory.hotbar[0], joiner.inventory.hotbar[0]);
            }
        }
    }
    let n = &host.factory.nodes[&POS];
    assert_eq!((n.energy, n.exhaust[STEAM], turbine::condensate(n)), (12, 0, 0));
    assert_eq!(host.inventory.hotbar[0].as_ref().unwrap().item, Item::Bucket(1152));
}
