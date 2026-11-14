use super::*;
use crate::world::{factory::{self, Kind, industry::{boiler, turbine::condensate}}, industry::STEAM};
const POS: IVec3 = IVec3::new(-16, 64, -16);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }
fn command(host: &mut World, player: &Player, action: factory::Action) {
    let command = actions::Command::Factory(action); let bytes = encode(0, &command).unwrap();
    let decoded: actions::Command = decode(&bytes[1..]).unwrap();
    actions::apply(decoded, host, player, &DayNightCycle::default(), 2, 0, false,
        &mut actions::Tools::default(), &mut world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS),
        &mut HashSet::new(), &mut HashSet::new()).unwrap();
}
#[test]
fn fired_boiler_commands_snapshots_duplicate_delivery_and_reconnect_preserve_ledger() {
    let mut host = world(); let n = factory::fixture_place(&mut host, POS, Kind::Generator).unwrap();
    n.water = 1; n.input[3] = 16; n.enabled = false;
    let mut player = Player::new(); player.pos = POS.as_vec3();
    command(&mut host, &player, factory::Action::SetRecipe(POS, boiler::MODE));
    command(&mut host, &player, factory::Action::GasOutlet(POS, 1, (STEAM + 1) as u8));
    let day = DayNightCycle::default(); let now = std::time::Instant::now(); let mut image = FactoryImage::default();
    let mut joiners = [(world(), Mirror::default()), (world(), Mirror::default())];
    for tick in 0..8 {
        match tick {
            1 | 3 | 5 => command(&mut host, &player, factory::Action::Toggle(POS)),
            2 | 4 => command(&mut host, &player, factory::Action::SetRecipe(POS, 0)),
            _ => {},
        }
        host.factory.tick(0.25, 1);
        image.refresh(&mut host, now + std::time::Duration::from_millis(tick * 250)).unwrap();
        if tick == 4 || tick == 7 { joiners[1] = (world(), Mirror::default()); }
        for (joiner, mirror) in &mut joiners {
            let packet = capture_shared(&host, &player, &day, tick, Vec::new(), mirror, &image).unwrap();
            for _ in 0..2 {
                apply(&packet, joiner, &mut Player::new(), &mut DayNightCycle::default(), &mut HashSet::new(), &mut HashSet::new()).unwrap();
                let (a,b) = (&host.factory.nodes[&POS], &joiner.factory.nodes[&POS]);
                assert_eq!((a.exhaust, condensate(a), a.energy, a.burn, a.input, a.mode, a.enabled, a.fuel_resource),
                    (b.exhaust, condensate(b), b.energy, b.burn, b.input, b.mode, b.enabled, b.fuel_resource));
                assert_eq!((a.gas_vent_side, a.gas_vent_species), (b.gas_vent_side, b.gas_vent_species));
            }
        }
        let n = &host.factory.nodes[&POS]; assert_eq!(n.mode, boiler::MODE);
        assert_eq!(condensate(n) + u64::from(n.exhaust[STEAM]), u64::from(factory::industry::WATER_UNIT_MG));
        if tick > 0 { assert_eq!(n.burn + n.exhaust[STEAM] / 96, 400); }
    }
    assert_eq!(host.factory.nodes[&POS].energy, 0);
}
