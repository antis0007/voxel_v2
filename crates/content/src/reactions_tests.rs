use super::*;
use crate::BlockId as B;

fn row(a: Reactant, b: Option<Reactant>) -> ReactionRow {
    ReactionRow { why: String::new(), a, b, a_above: None, a_below: None, rate: 1.0, a_into: Some(B::Ash), b_into: None, a_temp: None, b_temp: None }
}

fn compile_one(r: ReactionRow) -> Result<ReactionTable, Vec<String>> {
    let text = ron::ser::to_string_pretty(&vec![r.clone()], ron::ser::PrettyConfig::new()).unwrap();
    compile(&[r], &text, &crate::SIM_TABLE)
}

#[test]
fn the_shipped_reactions_compile() {
    let rows: Vec<ReactionRow> = ron::from_str(REACTIONS_RON).unwrap();
    let table = compile(&rows, REACTIONS_RON, &crate::SIM_TABLE).unwrap_or_else(|e| panic!("{}", e.join("\n")));
    let lava_water = table.pair(B::Lava as u8, B::Water as u8).expect("lava meets water");
    assert_eq!((lava_water.a_into.0, lava_water.b_into.0), (B::Basalt as u8, B::Steam as u8));
    assert!(table.pair(B::Water as u8, B::Lava as u8).is_some(), "found from either side");
}

#[test]
fn burnable_expands_to_every_material_with_an_ignition_point() {
    let mut r = row(Reactant::Burnable, Some(Reactant::Block(B::Air)));
    r.a_above = Some(Threshold::Ignition);
    r.b_into = Some(B::Fire);
    let table = compile_one(r).unwrap();
    for block in [B::Log, B::Planks, B::Leaves, B::PineLeaves, B::Bush, B::MushroomStem] {
        let flame = table.pair(block as u8, B::Air as u8).unwrap_or_else(|| panic!("{block:?} lights the air"));
        assert_eq!(flame.a_above, block.sim().ignite, "{block:?} at its own ignition point");
        assert_eq!(flame.b_into.0, B::Fire as u8);
    }
    assert!(table.pair(B::Stone as u8, B::Air as u8).is_none());
}

#[test]
fn errors_cite_the_line_of_the_row() {
    let mut r = row(Reactant::Block(B::Lava), Some(Reactant::Block(B::Water)));
    r.rate = 1.5;
    let errors = compile_one(r).unwrap_err();
    assert_eq!(errors, ["assets/reactions.ron:2: rate must be in (0, 1], not 1.5"]);
}

#[test]
fn mistakes_are_refused() {
    let lava_water = || row(Reactant::Block(B::Lava), Some(Reactant::Block(B::Water)));
    let cases: [(ReactionRow, &str); 9] = [
        (ReactionRow { rate: 1e-12, ..lava_water() }, "rounds to a chance of zero"),
        (ReactionRow { a_into: None, b_into: None, a_temp: Some(500.0), ..lava_water() }, "changes no material"),
        (row(Reactant::Block(B::Water), Some(Reactant::Block(B::Water))), "a self-pair"),
        (row(Reactant::Block(B::Air), None), "empty space does nothing on its own"),
        (ReactionRow { a_above: Some(Threshold::Ignition), ..lava_water() }, "Ignition needs a material with an ignition point"),
        (ReactionRow { b_into: Some(B::Steam), ..row(Reactant::Block(B::Fire), None) }, "a single-cell row has no b_into"),
        (ReactionRow { a_temp: Some(0.0), ..lava_water() }, "a_temp must be in"),
        (row(Reactant::Block(B::Lava), Some(Reactant::Burnable)), "Burnable is only allowed as a"),
        (ReactionRow { a_above: Some(Threshold::K(400.0)), a_below: Some(Threshold::K(300.0)), ..lava_water() }, "a_above is above a_below"),
    ];
    for (r, expect) in cases {
        let errors = compile_one(r).unwrap_err();
        assert!(errors.iter().any(|e| e.contains(expect)), "{expect}: {errors:?}");
    }
}

#[test]
fn an_error_in_a_burnable_row_is_reported_once() {
    let mut r = row(Reactant::Burnable, Some(Reactant::Block(B::Air)));
    r.a_above = Some(Threshold::K(-1.0));
    let errors = compile_one(r).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
}

#[test]
fn a_duplicate_pair_is_refused_with_both_orders_counted() {
    let rows = vec![
        row(Reactant::Block(B::Lava), Some(Reactant::Block(B::Water))),
        row(Reactant::Block(B::Water), Some(Reactant::Block(B::Lava))),
    ];
    let text = ron::ser::to_string_pretty(&rows, ron::ser::PrettyConfig::new()).unwrap();
    let errors = compile(&rows, &text, &crate::SIM_TABLE).unwrap_err();
    assert!(errors.iter().any(|e| e.contains("Water + Lava already has a row")), "{errors:?}");
}

#[test]
fn reactions_ron_round_trips() {
    let rows: Vec<ReactionRow> = ron::from_str(REACTIONS_RON).unwrap();
    let text = ron::ser::to_string_pretty(&rows, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
    let file = REACTIONS_RON.replace('\r', "");
    let body = file.find("\n[\n").map_or(file.as_str(), |i| &file[i + 1..]);
    assert_eq!(text.trim_end(), body.trim_end(), "run `cargo test -p engine_content dump_reactions_ron -- --ignored`");
}

/// Rewrites assets/reactions.ron from its parsed rows (keeps the header).
#[test]
#[ignore = "writes assets/reactions.ron"]
fn dump_reactions_ron() {
    let rows: Vec<ReactionRow> = ron::from_str(REACTIONS_RON).unwrap();
    let header = REACTIONS_RON.split("\n[\n").next().unwrap_or("");
    let text = ron::ser::to_string_pretty(&rows, ron::ser::PrettyConfig::new()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/reactions.ron");
    std::fs::write(path, format!("{header}\n{text}\n")).unwrap();
}
