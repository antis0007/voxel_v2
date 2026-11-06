//! The creature row contract over the shipped table and over rows in the
//! single-line form the dev panel appends.
use super::*;

fn parse(rows: &'static str) -> Vec<(u32, Species)> {
    ron::from_str(rows).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn every_bestiary_row_passes_the_creature_contract() {
    let problems: Vec<String> = SPECIES.iter().flat_map(|(kind, s)| row_problems(*kind, s)).collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    let mut kinds: Vec<u32> = SPECIES.iter().map(|(kind, _)| *kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(kinds.len(), SPECIES.len(), "a kind has two rows");
}

#[test]
fn a_panel_made_animal_row_loads_and_reuses_an_authored_body() {
    let rows = parse(r#"[(1000, (name: "Tusker", role: Grazer, neutral: true, health: 40.0, speed: 1.6,
        attack_range: 2.2, windup: 0.5, cooldown: 1.8, body_width: 0.8, body_height: 1.05,
        center_height: 0.55, tall: false, flying: false, brawler: true, food: Some(Block(Bush)),
        drops: [(item: Food(RawMutton), min: 1, max: 2)], melee: Some(9.0), animal_body: Some(50)))]"#);
    let (kind, s) = rows[0];
    assert!(generated(kind));
    assert_eq!(row_problems(kind, &s), Vec::<String>::new());
}

#[test]
fn the_contract_names_each_broken_rule() {
    let rows = parse(r#"[
        (1001, (name: "Stray", role: Grazer, health: 10.0, speed: 1.0, attack_range: 0.0, windup: 0.0,
            cooldown: 0.0, body_width: 0.5, body_height: 0.6, center_height: 0.3, tall: false,
            flying: false, brawler: false, melee: Some(4.0))),
        (60, (name: "Borrowed", role: Grazer, health: 10.0, speed: 1.0, attack_range: 0.0, windup: 0.0,
            cooldown: 0.0, body_width: 0.5, body_height: 0.6, center_height: 0.3, tall: false,
            flying: false, brawler: false, animal_body: Some(49))),
        (1002, (name: "Hen", role: Skirmisher, health: 10.0, speed: 1.0, attack_range: 0.0, windup: 0.0,
            cooldown: 0.0, body_width: 0.5, body_height: 0.6, center_height: 0.3, tall: false,
            flying: false, brawler: false, lays: Some((item: Food(Egg), min: 2, max: 1)))),
    ]"#);
    let all: Vec<String> = rows.iter().flat_map(|(kind, s)| row_problems(*kind, s)).collect();
    for wanted in ["melee needs brawler", "generated grazer needs animal_body",
        "animal_body is only for generated kinds", "need role Grazer", "min <= max"] {
        assert!(all.iter().any(|p| p.contains(wanted)), "missing {wanted:?} in {all:#?}");
    }
}
