use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::WorldType;

const ALL: [Family; 4] = [Family::Foundry, Family::Toll, Family::Archive, Family::Hart];

fn kind(f: Family) -> u32 {
    match f {
        Family::Foundry => 3,
        Family::Archive => 5,
        Family::Hart => 8,
        Family::Toll => 10,
    }
}

fn find(world: &World, f: Family) -> (IVec3, Shrine) {
    let cfg = world.sample_config();
    for x in -10..=10 {
        for z in -10..=10 {
            if let Some(s) = access::site_shrine(&cfg, x, z).filter(|s| s.family == f) {
                return (IVec3::new(x, z, 0), s);
            }
        }
    }
    for x in -16..=16 {
        for z in -16..=16 {
            if let Some(s) = access::cavern_shrine(&cfg, x, z).filter(|s| s.family == f) {
                return (IVec3::new(x, z, 1), s);
            }
        }
    }
    panic!("no {f:?} shrine");
}

fn setup(f: Family) -> (World, Adventure, Shrine) {
    let world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 424242);
    assert!(world.generator_version >= access::VERSION);
    let (site, shrine) = find(&world, f);
    let mut adventure = Adventure::default();
    let state = GUARDIANS_CLEARED | PHYSICAL;
    adventure.records.insert(site, state);
    adventure.reliquaries.push(Reliquary { site, pos: shrine.relic_point(), kind: kind(f), state });
    (world, adventure, shrine)
}

fn sealed(world: &World, s: &Shrine) -> bool {
    s.cells(access::DOOR).iter().all(|p| world.get(*p) == s.family.door_block())
}

#[test]
fn each_boss_shrine_opens_only_when_its_mechanism_is_solved() {
    for f in ALL {
        let (mut world, mut adventure, shrine) = setup(f);
        tend(&mut adventure, &mut world);
        assert!(sealed(&world, &shrine), "{f:?} opened unsolved");
        let (keys, rule) = f.keys();
        for (n, k) in keys.iter().enumerate() {
            let p = shrine.cells(*k)[0];
            world.set(p, match rule {
                Rule::Fill(ok) => ok[0],
                Rule::Break(_) => BlockId::Air,
            });
            tend(&mut adventure, &mut world);
            let last = n + 1 == keys.len();
            assert_eq!(sealed(&world, &shrine), !last, "{f:?} after key {n}");
        }
        assert!(shrine.cells(access::DOOR).iter().all(|p| world.get(*p) == BlockId::Air));
    }
}

#[test]
fn foundry_valves_are_a_second_solution() {
    let (mut world, mut adventure, shrine) = setup(Family::Foundry);
    let site = adventure.reliquaries[0].site;
    *adventure.records.get_mut(&site).unwrap() |= quest::VALVES;
    tend(&mut adventure, &mut world);
    assert!(!sealed(&world, &shrine));
}

fn wands(world: &World) -> usize {
    world
        .inventory
        .dropped
        .iter()
        .filter(|d| matches!(d.stack.item, crate::world::inventory::Item::Wand { .. }))
        .count()
}

fn cells_hold(world: &World, cells: &[IVec3], b: BlockId) -> bool {
    !cells.is_empty() && cells.iter().all(|p| world.get(*p) == b)
}

#[test]
fn reward_vault_opens_once_after_the_boss_falls() {
    for f in ALL {
        let (mut world, mut adventure, shrine) = setup(f);
        let site = adventure.reliquaries[0].site;
        let vault = shrine.treasures()[2];
        assert_eq!((vault.opens, vault.tier), (access::Opens::Boss, 3));
        tend(&mut adventure, &mut world);
        let cache = shrine.cells(vault.cache);
        assert!(cells_hold(&world, &cache, BlockId::Air), "{f:?} early cache");
        assert!(cells_hold(&world, &shrine.cells(vault.seal), BlockId::VoidStone));
        assert_eq!(wands(&world), 0);
        *adventure.records.get_mut(&site).unwrap() |= BOSS_DEFEATED;
        tend(&mut adventure, &mut world);
        assert!(cells_hold(&world, &cache, f.cache_block()), "{f:?} cache");
        assert!(cells_hold(&world, &shrine.cells(vault.seal), BlockId::Air));
        assert!(cells_hold(&world, &shrine.cells(vault.pedestal), BlockId::TombStone));
        assert_eq!(wands(&world), 1, "{f:?} one found wand");
        // Taking the cache never re-grants it, nor drops another wand.
        for p in &cache {
            world.set(*p, BlockId::Air);
        }
        tend(&mut adventure, &mut world);
        tend(&mut adventure, &mut world);
        assert!(cells_hold(&world, &cache, BlockId::Air), "{f:?} re-granted");
        assert_eq!(wands(&world), 1, "{f:?} wand re-dropped");
    }
}

#[test]
fn side_treasure_rooms_open_once_by_cracked_wall_and_lever() {
    for f in ALL {
        let (mut world, mut adventure, shrine) = setup(f);
        let [hidden, lever, _] = shrine.treasures();
        tend(&mut adventure, &mut world);
        for t in [hidden, lever] {
            assert!(cells_hold(&world, &shrine.cells(t.cache), BlockId::Air), "{f:?} {} early", t.index);
        }
        assert_eq!(wands(&world), 0);
        // Break one brick of the cracked wall: only the hidden room is granted.
        world.set(shrine.cells(hidden.seal)[0], BlockId::Air);
        tend(&mut adventure, &mut world);
        assert!(cells_hold(&world, &shrine.cells(hidden.cache), f.cache_block()), "{f:?} hidden cache");
        assert!(cells_hold(&world, &shrine.cells(lever.cache), BlockId::Air), "{f:?} lever early");
        assert_eq!(wands(&world), 1);
        // Pull the lever pin: the gate opens and the lever room is granted.
        let access::Opens::Lever(pin) = lever.opens else { panic!("lever room") };
        assert!(cells_hold(&world, &shrine.cells(lever.seal), f.door_block()));
        world.set(shrine.cells(pin)[0], BlockId::Air);
        tend(&mut adventure, &mut world);
        assert!(cells_hold(&world, &shrine.cells(lever.seal), BlockId::Air), "{f:?} gate");
        assert!(cells_hold(&world, &shrine.cells(lever.cache), f.cache_block()), "{f:?} lever cache");
        assert_eq!(wands(&world), 2);
        for _ in 0..3 {
            tend(&mut adventure, &mut world);
        }
        assert_eq!(wands(&world), 2, "{f:?} side rooms re-granted");
    }
}

#[test]
fn treasure_points_name_every_cache_the_app_fills() {
    let (world, _, shrine) = setup(Family::Archive);
    let cfg = world.sample_config();
    let points = access::treasure_points(&cfg, shrine.relic_point(), 40.0);
    for t in shrine.treasures() {
        let p = points.iter().find(|p| p.index == t.index && (p.pos - shrine.relic_point()).length() < 40.0).expect("point");
        let cell = (p.pos / cfg.voxel_meters()).floor().as_ivec3();
        assert!(shrine.cells(t.cache).contains(&cell));
    }
}

#[test]
fn older_generators_have_no_shrine_mechanics() {
    let (mut world, mut adventure, shrine) = setup(Family::Toll);
    world.generator_version = 31;
    let door = shrine.cells(access::DOOR);
    let before: Vec<_> = door.iter().map(|p| world.get(*p)).collect();
    let pin = shrine.cells(Family::Toll.keys().0[0])[0];
    world.set(pin, BlockId::Air);
    tend(&mut adventure, &mut world);
    let after: Vec<_> = door.iter().map(|p| world.get(*p)).collect();
    assert_eq!(before, after);
}
