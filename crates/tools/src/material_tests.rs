use crate::traits::TraitEffect;
use crate::{MATERIALS, MATERIALS_RON, MaterialId, ToolAssembly, ToolMaterial, parse_materials};

fn bronze() -> ToolMaterial {
    parse_materials(MATERIALS_RON)
        .unwrap()
        .into_iter()
        .find(|row| row.id.as_str() == "bronze")
        .unwrap()
}

#[test]
fn material_parser_and_runtime_share_production_rows_and_assembly_properties() {
    let rows = parse_materials(MATERIALS_RON).unwrap();
    assert_eq!(rows, *MATERIALS);
    let props = |id| rows.iter().find(|row| row.id.as_str() == id).unwrap().props;
    let materials: Vec<_> = ["bronze", "stone", "wood"]
        .into_iter()
        .map(MaterialId::new)
        .collect();
    let tool = ToolAssembly::new("pickaxe", &materials).unwrap();
    let stats = tool.stats().unwrap();
    assert_eq!(
        stats.durability,
        ((props("bronze").durability + 0.25 * props("stone").durability)
            * props("wood").handle_modifier)
            .round() as u32
    );
    assert_eq!(stats.mining_speed, props("bronze").mining_speed);
    assert_eq!(stats.attack, props("bronze").attack);
    assert_eq!(tool.head_material().unwrap().props, props("bronze"));
}

#[test]
fn edited_bronze_properties_reach_the_native_trait_rules() {
    let mut rows = parse_materials(MATERIALS_RON).unwrap();
    let edited = rows
        .iter_mut()
        .find(|row| row.id.as_str() == "bronze")
        .unwrap();
    edited.props.conductivity = 0.95;
    edited.props.durability = 620.0;
    edited.props.hardness = 6.2;
    edited.props.attack = 5.5;
    edited.props.mining_speed = 8.5;
    let expected = edited.props;
    let accepted = parse_materials(&ron::ser::to_string(&rows).unwrap()).unwrap();
    let props = &accepted
        .iter()
        .find(|row| row.id.as_str() == "bronze")
        .unwrap()
        .props;
    assert_eq!(*props, expected);
    let effects: Vec<_> = crate::material_traits(props)
        .iter()
        .map(|rule| rule.effect)
        .collect();
    assert!(effects.contains(&TraitEffect::Conductive));
    assert!(effects.contains(&TraitEffect::Tempered));
    let original: Vec<_> = crate::material_traits(&bronze().props)
        .iter()
        .map(|rule| rule.effect)
        .collect();
    assert!(!original.contains(&TraitEffect::Conductive));
    assert!(!original.contains(&TraitEffect::Tempered));
}

#[test]
fn material_parser_rejects_invalid_properties_and_identity() {
    let cases: [(&str, fn(&mut ToolMaterial)); 13] = [
        ("single token", |row| row.id.0 = "bad id".into()),
        ("display_name", |row| row.display_name = "  ".into()),
        ("unknown block", |row| {
            row.block = Some("UnobtainiumBlock".into())
        }),
        ("non-finite", |row| row.props.attack = f32::NAN),
        ("durability", |row| row.props.durability = 0.0),
        ("handle_modifier", |row| row.props.handle_modifier = 0.0),
        ("density", |row| row.props.density = 0.0),
        ("mining_speed", |row| row.props.mining_speed = -1.0),
        ("attack", |row| row.props.attack = -1.0),
        ("hardness", |row| row.props.hardness = -1.0),
        ("conductivity", |row| row.props.conductivity = 1.01),
        ("conductivity", |row| row.props.conductivity = -0.01),
        ("flammability", |row| row.props.flammability = 1.01),
    ];
    for (expected, edit) in cases {
        let mut row = bronze();
        edit(&mut row);
        let source = ron::ser::to_string(&vec![row]).unwrap();
        let errors = parse_materials(&source).unwrap_err();
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{expected}: {errors:?}"
        );
    }
    let source = ron::ser::to_string(&vec![bronze(), bronze()]).unwrap();
    assert!(
        parse_materials(&source)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("appears twice"))
    );
}

#[test]
fn material_parser_rejects_field_typos_and_accepts_documented_boundaries() {
    let source = ron::ser::to_string(&vec![bronze()]).unwrap();
    for draft in [
        source.replacen("display_name:", "display_nam:", 1),
        source.replacen("hardness:", "hardnes:", 1),
    ] {
        assert!(
            parse_materials(&draft)
                .unwrap_err()
                .iter()
                .any(|error| error.contains("Unexpected field"))
        );
    }
    let mut row = bronze();
    row.props.mining_speed = 0.0;
    row.props.attack = 0.0;
    row.props.hardness = 0.0;
    row.props.conductivity = 0.0;
    row.props.flammability = 1.0;
    assert!(parse_materials(&ron::ser::to_string(&vec![row]).unwrap()).is_ok());
}
