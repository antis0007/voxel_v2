use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::{BlockId as B, WorldType};

fn fixture() -> (World, State) {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let base = IVec3::new(0, engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 3, 0);
    for x in -4..=9 { for z in -4..=9 { for y in 0..=7 {
        w.set(base + IVec3::new(x, y, z), B::Air);
    } } }
    let spec = ron::from_str("(name:\"Test home\",origin:(0,0,0),size:(5,5),food_reserve:0,meals_eaten:0,path:[])").unwrap();
    let p = super::super::super::scene::plan(spec, base).unwrap();
    w.set_many(&p.cells.iter().map(|o| (o.cell(), o.block())).collect::<Vec<_>>());
    let layer = p.cells.iter().map(|o| p.work_layer(*o)).max().unwrap() + 2;
    let s = State { growth: None, version: 1, projects: vec![p], timber: vec![], timber_next: Some(0),
        phase: 1, layer: 0, project_layers: vec![layer], households: vec![], housing_overrides: vec![] };
    (w, s)
}

#[test]
fn household_geometry_requires_completed_ledger_and_clear_actual_entry() {
    let (mut w, mut s) = fixture();
    assert!(usable(&w, &s, 0));
    s.project_layers[0] -= 1;
    assert!(!usable(&w, &s, 0), "Physical blocks alone do not commit paid completion");
    s.project_layers[0] += 1;
    let home = s.projects[0].home.clone().unwrap();
    let door = (IVec3::from(home.entry) + IVec3::from(home.destination)) / 2;
    w.set(door, B::Planks);
    assert!(!usable(&w, &s, 0), "Blocked door invalidates occupancy");
    w.set(door, B::Air);
    assert!(usable(&w, &s, 0));
    w.set(IVec3::from(home.entry) - IVec3::Y, B::Air);
    assert!(!usable(&w, &s, 0), "Missing frontage invalidates occupancy");
}

#[test]
fn household_geometry_invalidates_destroyed_roof() {
    let (mut w, s) = fixture();
    let roof = s.projects[0].cells.iter().filter(|o| o.block() == B::Planks)
        .max_by_key(|o| o.cell().y).unwrap().cell();
    w.set(roof, B::Air);
    assert!(!usable(&w, &s, 0));
}

#[test]
fn household_ledger_round_trips_and_legacy_fields_default() {
    let (_, mut s) = fixture();
    s.households.push(Household { project: 0, members: vec![1, 2] });
    assert!(valid(&s));
    let encoded = ron::to_string(&s).unwrap();
    let decoded: State = ron::from_str(&encoded).unwrap();
    assert_eq!(decoded.households, s.households);
    assert_eq!(decoded.projects[0].home, s.projects[0].home);
    s.households[0].members.push(1);
    assert!(!valid(&s), "Duplicate residents must not load");
    let old: State = ron::from_str("(version:1,projects:[(name:\"Old\",cells:[],food_reserve:0,meals_eaten:0)],timber:[],phase:0,layer:0)").unwrap();
    assert!(old.households.is_empty());
    assert!(old.projects[0].home.is_none());
}
