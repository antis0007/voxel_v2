use crate::{capabilities::MiningClass, parse_parts, MaterialId, ToolAssembly, PARTS, PARTS_RON};

#[test]
fn shipped_capabilities_preserve_existing_gameplay_for_every_schema() {
    for schema in &PARTS.schemas {
        let mining = schema.capabilities.mining.unwrap();
        let expected = match schema.id.as_str() {
            "axe" | "sword" | "dagger" | "broadsword" | "cleaver" | "scythe" => MiningClass::Cut,
            "shovel" | "excavator" => MiningClass::Scoop,
            _ => MiningClass::Dig,
        };
        assert_eq!(mining.class, expected, "{}", schema.id);
        assert_eq!(mining.charged, matches!(schema.id.as_str(), "hammer" | "excavator"));
        assert_eq!(mining.cuts_bamboo, matches!(schema.id.as_str(), "sword" | "broadsword"));
        assert_eq!(mining.fells_trees, schema.id == "axe");
        assert_eq!(schema.capabilities.blocking, schema.id == "shield");
        let materials = vec![MaterialId::new("iron"); schema.parts.len()];
        let assembly = ToolAssembly::new(&schema.id, &materials).unwrap();
        assert_eq!(assembly.capabilities().unwrap(), schema.capabilities);
        let mut encoded = Vec::new();
        assembly.encode(&mut encoded).unwrap();
        let (decoded, used) = ToolAssembly::decode(&encoded).unwrap();
        assert_eq!(used, encoded.len());
        assert_eq!(decoded, assembly);
        assert_eq!(decoded.capabilities().unwrap(), schema.capabilities);
    }
}

#[test]
fn unnamed_hybrid_composes_capabilities_without_a_new_item_class() {
    let mut file = parse_parts(PARTS_RON).unwrap();
    let mut schema = file.schema("axe").unwrap().clone();
    schema.id = "quarry_guard".into();
    schema.capabilities.blocking = true;
    schema.capabilities.mining.as_mut().unwrap().charged = true;
    file.schemas.push(schema.clone());
    let source = ron::ser::to_string(&file).unwrap();
    let authored = parse_parts(&source).unwrap();
    assert_eq!(authored.schema("quarry_guard").unwrap().capabilities, schema.capabilities);
    assert!(schema.capabilities.mining.unwrap().fells_trees);
}

#[test]
fn capability_configuration_is_validated_and_mining_can_be_disabled() {
    let mut file = parse_parts(PARTS_RON).unwrap();
    let axe = file.schemas.iter_mut().find(|schema| schema.id == "axe").unwrap();
    axe.capabilities.mining.as_mut().unwrap().class = MiningClass::Dig;
    let source = ron::ser::to_string(&file).unwrap();
    assert!(parse_parts(&source).unwrap_err().iter().any(|error| error.contains("axe") && error.contains("Cut")));
    file.schemas.iter_mut().find(|schema| schema.id == "axe").unwrap().capabilities.mining = None;
    assert!(parse_parts(&ron::ser::to_string(&file).unwrap()).is_ok());
}

#[test]
fn missing_capability_metadata_grants_no_implicit_native_behavior() {
    let source = "(mining: None, blocking: false)";
    let empty: crate::capabilities::Capabilities = ron::from_str(source).unwrap();
    assert_eq!(empty, Default::default());
    assert!(ron::from_str::<crate::capabilities::Capabilities>("(mining: None, blockign: true)").is_err());
}

#[test]
fn loading_an_older_durability_rating_never_repairs_or_rejects_a_valid_assembly() {
    let mut tool = ToolAssembly::new("pickaxe", &[MaterialId::new("bronze"), MaterialId::new("iron"), MaterialId::new("wood")]).unwrap();
    let maximum = tool.stats().unwrap().durability;
    // This is the state a formerly higher-durability definition can leave in a save.
    tool.durability_left = maximum + 50;
    let mut bytes = Vec::new();
    tool.encode(&mut bytes).unwrap();
    let (mut loaded, used) = ToolAssembly::decode(&bytes).unwrap();
    assert_eq!(used, bytes.len());
    loaded.reconcile_durability().unwrap();
    assert_eq!(loaded.durability_left, maximum);
    loaded.durability_left = maximum / 2;
    loaded.reconcile_durability().unwrap();
    assert_eq!(loaded.durability_left, maximum / 2);
    loaded.durability_left = 0;
    loaded.reconcile_durability().unwrap();
    assert!(loaded.is_broken());
    assert_eq!(loaded.parts, tool.parts);
    assert_eq!(loaded.schema, tool.schema);
}
