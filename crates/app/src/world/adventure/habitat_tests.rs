use super::*;
use super::super::{bestiary, wildlife};
use engine_core::VoxelScalePreset;

fn ctx(realm: Realm, biome: Option<Biome>, night: bool) -> Context {
    Context { biome, realm, night }
}

#[test]
fn spawns_ron_round_trips_and_names_real_biomes_and_kinds() {
    for (kind, h) in ROWS.iter() {
        assert!(bestiary::species(*kind).is_some(), "kind {kind} has no bestiary row");
        assert!(h.weight > 0 && !h.realms.is_empty(), "kind {kind}: empty row");
        for name in &h.biomes {
            assert!(Biome::from_name(name).is_some(), "kind {kind}: unknown biome {name}");
        }
    }
    let text = ron::ser::to_string_pretty(&*ROWS, ron::ser::PrettyConfig::default()).expect("serialise");
    let back: Vec<(u32, Habitat)> = ron::from_str(&text).expect("re-parse");
    assert_eq!(&back, &*ROWS);
}

#[test]
fn all_is_complete() {
    for (i, b) in Biome::ALL.into_iter().enumerate() {
        assert_eq!(b.index(), i, "{b:?} is out of order in Biome::ALL");
        assert_eq!(Biome::from_name(&format!("{b:?}")), Some(b));
    }
}

#[test]
fn every_night_weight_in_the_bestiary_has_a_spawn_row() {
    for (kind, s) in bestiary::SPECIES.iter() {
        if s.night > 0 {
            assert!(listed(*kind), "kind {kind} ({}) rises at night but has no spawns.ron row", s.name);
        }
    }
}

#[test]
fn the_roll_walks_rows_in_order_and_respects_the_context() {
    let hostile = |k: u32| !wildlife::passive(k) && !bestiary::species(k).is_some_and(|s| s.neutral);
    let forest_night = ctx(Realm::Surface, Some(Biome::Forest), true);
    let total: u32 = ROWS.iter().filter(|(k, h)| hostile(*k) && h.admits(&forest_night)).map(|(_, h)| u32::from(h.weight)).sum();
    assert!(total > 0);
    let mut seen = std::collections::BTreeSet::new();
    for roll in 0..total {
        let kind = pick(&forest_night, roll, hostile).expect("a kind for every roll");
        assert_eq!(pick(&forest_night, roll + total, hostile), Some(kind), "roll wraps on the total");
        seen.insert(kind);
    }
    assert!(seen.contains(&53), "owlbears rise in a forest at night");
    assert!(!seen.contains(&54), "gnolls stay out of forests");
    assert!(!seen.contains(&57), "hell hounds stay in the Nether");
    let nether = ctx(Realm::Nether, None, false);
    let kinds: std::collections::BTreeSet<u32> = (0..64).filter_map(|r| pick(&nether, r, hostile)).collect();
    assert!(kinds.contains(&57) && !kinds.contains(&0), "the Nether has hounds and no skitterers");
    let day_surface = ctx(Realm::Surface, Some(Biome::Meadow), false);
    assert_eq!(pick(&day_surface, 7, hostile), None, "nothing hostile rises on an open meadow by day");
    let cave_day = ctx(Realm::Underground, Some(Biome::Meadow), false);
    assert!(pick(&cave_day, 3, hostile).is_some(), "caves spawn by day");
}

#[test]
fn grazers_follow_the_biome() {
    let meadow = ctx(Realm::Surface, Some(Biome::Meadow), false);
    let marsh = ctx(Realm::Surface, Some(Biome::Marsh), false);
    let meadow_kinds: std::collections::BTreeSet<u32> = (0..32).filter_map(|r| pick(&meadow, r, wildlife::passive)).collect();
    let marsh_kinds: std::collections::BTreeSet<u32> = (0..32).filter_map(|r| pick(&marsh, r, wildlife::passive)).collect();
    assert!(meadow_kinds.contains(&49) && meadow_kinds.contains(&16));
    assert_eq!(marsh_kinds, [52].into_iter().collect());
}

#[test]
fn context_classifies_depth_and_realm() {
    use crate::world::{World, WorldType};
    let w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let top = w.terrain_height_at(0, 0);
    let surface = Context::at(&w, glam::IVec3::new(0, top + 1, 0), true, false);
    assert_eq!(surface.realm, Realm::Surface);
    assert!(surface.biome.is_some());
    let deep = Context::at(&w, glam::IVec3::new(0, top - 10, 0), false, true);
    assert_eq!(deep.realm, Realm::Underground);
    let shallow_roof = Context::at(&w, glam::IVec3::new(0, top - 2, 0), false, true);
    assert_eq!(shallow_roof.realm, Realm::Surface, "a shed roof is not a cave");
    let nether = World::with_seed(WorldType::Nether, VoxelScalePreset::OneMeter, 7);
    assert_eq!(Context::at(&nether, glam::IVec3::ZERO, false, false).realm, Realm::Nether);
}
