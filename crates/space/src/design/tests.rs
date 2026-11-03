use super::*;
use crate::data::parse;

fn home() -> System {
    let text = include_str!("../../../../assets/star_systems.ron");
    let def = parse(text).unwrap().systems.remove(0);
    System::compile(&def, 1.0 / 120.0).unwrap()
}

#[test]
fn rocky_density_moon_is_torn_apart() {
    let s = home();
    let home = s.id("home").unwrap();
    let rocky = MoonDesign { density_kg_m3: Some(3_300.0), ..MoonDesign::default() };
    assert!(matches!(design_moon(&s, home, &rocky), Err(Issue::InsideRoche { .. })));
}

#[test]
fn pebble_moon_has_no_room_to_orbit() {
    let s = home();
    let home = s.id("home").unwrap();
    let pebble = MoonDesign { radius_m: 1_000.0, hill_fraction: 0.36, ..MoonDesign::default() };
    assert!(matches!(design_moon(&s, home, &pebble), Err(Issue::TinySoi { .. })));
}

#[test]
fn wide_orbit_is_unstable() {
    let s = home();
    let home = s.id("home").unwrap();
    let wide = MoonDesign { hill_fraction: 0.6, ..MoonDesign::default() };
    assert!(matches!(design_moon(&s, home, &wide), Err(Issue::BeyondHill { .. })));
}

#[test]
fn star_children_have_no_hill_parent() {
    let s = home();
    let star = s.id("star").unwrap();
    assert_eq!(design_moon(&s, star, &MoonDesign::default()), Err(Issue::RootParent));
}
