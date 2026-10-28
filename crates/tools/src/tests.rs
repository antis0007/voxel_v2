use crate::assembly::{ToolAssembly, ToolError};
use crate::codec::DecodeError;
use crate::materials::{MaterialId, MaterialProps, ToolMaterial};
use crate::parts::PartRole;
use crate::progression::{level_for_xp, xp_to_reach};
use crate::traits::{Stacking, TraitEffect};
use crate::{MATERIALS, MATERIALS_RON, PARTS, PARTS_RON, TRAITS, TRAITS_RON};

fn ids(list: &[&str]) -> Vec<MaterialId> {
    list.iter().map(|id| MaterialId::new(id)).collect()
}

/// Serialising the loaded table reproduces the file minus its comment header.
fn assert_round_trip<T: serde::Serialize>(rows: &T, file: &str) {
    let text = ron::ser::to_string_pretty(rows, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
    let file = file.replace('\r', "");
    let body = match file.find("\n[\n").or_else(|| file.find("\n(\n")) {
        Some(i) => &file[i + 1..],
        None => file.as_str(),
    };
    let (a, b) = (text.trim_end(), body.trim_end());
    if a != b {
        let i = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        let lo = i.saturating_sub(60);
        panic!(
            "table file differs from the loaded rows at byte {i}; rerun the ignored dump test to rewrite it.\n table: {:?}\n  file: {:?}",
            &a[lo..(i + 60).min(a.len())],
            &b[lo..(i + 60).min(b.len())]
        );
    }
}

fn dump<T: serde::Serialize>(rows: &T, source: &str, file: &str) {
    let source = source.replace('\r', "");
    let body = source.find("\n[\n").or_else(|| source.find("\n(\n")).map_or(0, |i| i + 1);
    let header = &source[..body];
    let text = ron::ser::to_string_pretty(rows, ron::ser::PrettyConfig::new()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tools").join(file);
    std::fs::write(path, format!("{header}{text}")).unwrap();
}

#[test]
fn tool_parts_ron_round_trips() {
    assert_round_trip(&*PARTS, PARTS_RON);
}

#[test]
fn tool_materials_ron_round_trips() {
    assert_round_trip(&*MATERIALS, MATERIALS_RON);
}

#[test]
fn tool_traits_ron_round_trips() {
    assert_round_trip(&*TRAITS, TRAITS_RON);
}

#[test]
#[ignore = "writes assets/tools/parts.ron"]
fn dump_tool_parts_ron() {
    dump(&*PARTS, PARTS_RON, "parts.ron");
}

#[test]
#[ignore = "writes assets/tools/materials.ron"]
fn dump_tool_materials_ron() {
    dump(&*MATERIALS, MATERIALS_RON, "materials.ron");
}

#[test]
#[ignore = "writes assets/tools/traits.ron"]
fn dump_tool_traits_ron() {
    dump(&*TRAITS, TRAITS_RON, "traits.ron");
}

#[test]
fn tables_meet_the_content_minimums() {
    assert!(PARTS.part_kinds.len() >= 6, "{} part kinds", PARTS.part_kinds.len());
    assert!(PARTS.schemas.len() >= 10, "{} schemas", PARTS.schemas.len());
    assert!(MATERIALS.len() >= 14, "{} materials", MATERIALS.len());
    assert!(TRAITS.len() >= 12, "{} trait rules", TRAITS.len());
    let blocks: Vec<&str> = MATERIALS.iter().filter_map(|m| m.block.as_deref()).collect();
    assert!(blocks.contains(&"Stone") && blocks.contains(&"CopperBlock"));
}

#[test]
fn unnamed_alloy_with_high_conductivity_is_conductive() {
    let electrum = ToolMaterial {
        id: MaterialId::new("electrum"),
        display_name: "Electrum".to_owned(),
        block: None,
        props: MaterialProps {
            durability: 240.0,
            mining_level: 2,
            mining_speed: 7.0,
            attack: 3.5,
            handle_modifier: 0.9,
            conductivity: 0.95,
            flammability: 0.0,
            density: 14.0,
            hardness: 3.0,
            magnetic: false,
            living: false,
            crystalline: false,
        },
    };
    assert!(crate::material(electrum.id.as_str()).is_none(), "electrum must not be a table row");
    assert!(!TRAITS_RON.contains("electrum"));
    let effects: Vec<TraitEffect> = crate::material_traits(&electrum.props).iter().map(|r| r.effect).collect();
    assert_eq!(effects, vec![TraitEffect::Conductive, TraitEffect::Heavy]);
}

#[test]
fn table_materials_get_expected_traits() {
    let effects = |id: &str| -> Vec<TraitEffect> {
        crate::material_traits(&crate::material(id).unwrap().props).iter().map(|r| r.effect).collect()
    };
    assert_eq!(effects("slime"), vec![TraitEffect::SelfRepair, TraitEffect::Buoyant, TraitEffect::Bouncy]);
    assert_eq!(effects("obsidian"), vec![TraitEffect::InsulatedGrip, TraitEffect::Jagged, TraitEffect::LavaProof]);
    assert_eq!(effects("flint"), vec![TraitEffect::Brittle]);
    assert_eq!(effects("steel"), vec![TraitEffect::Magnetic, TraitEffect::Tempered]);
    assert_eq!(effects("wood"), vec![TraitEffect::InsulatedGrip, TraitEffect::FlammableHandle, TraitEffect::Buoyant, TraitEffect::Lightweight]);
}

#[test]
fn aggregation_is_deterministic_and_weighted() {
    let a = ToolAssembly::new("pickaxe", &ids(&["iron", "stone", "wood"])).unwrap();
    let b = ToolAssembly::new("pickaxe", &ids(&["iron", "stone", "wood"])).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.stats(), b.stats());
    assert_eq!(a.traits(), b.traits());
    let stats = a.stats().unwrap();
    // head 250 + binding 0.25 * 120, times the wood handle modifier 1.0.
    assert_eq!(stats.durability, 280);
    assert_eq!(stats.mining_level, 2);
    assert_eq!(stats.mining_speed, 6.0);
    assert_eq!(stats.attack, 4.0);
    assert_eq!(a.durability_left, 280);

    let slime_handle = ToolAssembly::new("pickaxe", &ids(&["iron", "stone", "slime"])).unwrap();
    assert_eq!(slime_handle.stats().unwrap().durability, 420);
    let mut bytes_a = Vec::new();
    let mut bytes_b = Vec::new();
    a.encode(&mut bytes_a).unwrap();
    b.encode(&mut bytes_b).unwrap();
    assert_eq!(bytes_a, bytes_b);
}

#[test]
fn traits_stack_by_rule() {
    let hammer = ToolAssembly::new("hammer", &ids(&["iron", "copper", "copper", "wood"])).unwrap();
    let traits = hammer.traits().unwrap();
    let stacks = |effect| traits.iter().find(|t| t.effect == effect).map(|t| t.stacks);
    assert_eq!(stacks(TraitEffect::Heavy), Some(2), "PerPart counts both copper plates");
    assert_eq!(stacks(TraitEffect::Conductive), Some(1), "Once collapses two conductive plates");
    assert_eq!(stacks(TraitEffect::Magnetic), Some(1));
    assert_eq!(stacks(TraitEffect::FlammableHandle), Some(1));

    let crystal_handle = ToolAssembly::new("pickaxe", &ids(&["iron", "stone", "crystal"])).unwrap();
    let traits = crystal_handle.traits().unwrap();
    assert!(traits.iter().all(|t| t.effect != TraitEffect::Jagged), "HeadOnly ignores a crystal handle");
    let crystal_head = ToolAssembly::new("pickaxe", &ids(&["crystal", "stone", "wood"])).unwrap();
    assert!(crystal_head.traits().unwrap().iter().any(|t| t.effect == TraitEffect::Jagged));
    assert!(TRAITS.iter().any(|r| r.stacking == Stacking::HeadOnly));
}

#[test]
fn assembly_rejects_wrong_parts() {
    assert_eq!(
        ToolAssembly::new("pickaxe", &ids(&["iron", "stone"])),
        Err(ToolError::PartCount { expected: 3, found: 2 })
    );
    assert_eq!(
        ToolAssembly::new("pickaxe", &ids(&["iron", "stone", "unobtainium"])),
        Err(ToolError::UnknownMaterial("unobtainium".to_owned()))
    );
    assert_eq!(ToolAssembly::new("spork", &[]), Err(ToolError::UnknownSchema("spork".to_owned())));
    let mut swapped = ToolAssembly::new("sword", &ids(&["steel", "iron", "wood"])).unwrap();
    swapped.parts.swap(0, 1);
    assert!(matches!(swapped.stats(), Err(ToolError::PartMismatch { index: 0, .. })));
}

#[test]
fn xp_thresholds_follow_the_curve() {
    let curve = &PARTS.progression;
    assert_eq!(xp_to_reach(curve, 0), 0);
    assert_eq!(xp_to_reach(curve, 1), 100);
    assert_eq!(xp_to_reach(curve, 2), 250);
    assert_eq!(xp_to_reach(curve, 3), 450);
    assert_eq!(level_for_xp(curve, 99), 0);
    assert_eq!(level_for_xp(curve, 100), 1);
    assert_eq!(level_for_xp(curve, 449), 2);
    assert_eq!(level_for_xp(curve, u32::MAX), curve.max_level);

    let mut tool = ToolAssembly::new("axe", &ids(&["bronze", "bone", "wood"])).unwrap();
    let base_slots = tool.modifier_slots().unwrap();
    assert_eq!(tool.xp_to_next_level(), 100);
    assert_eq!(tool.gain_xp(60), 0);
    assert_eq!(tool.gain_xp(200), 2);
    assert_eq!(tool.level, 2);
    assert_eq!(tool.modifier_slots().unwrap(), base_slots + 2);
    for _ in 0..base_slots + 2 {
        tool.add_modifier("haste").unwrap();
    }
    assert_eq!(tool.add_modifier("haste"), Err(ToolError::ModifierSlotsFull { slots: base_slots + 2 }));
}

#[test]
fn repair_needs_the_head_material() {
    let mut tool = ToolAssembly::new("pickaxe", &ids(&["iron", "stone", "wood"])).unwrap();
    assert!(!tool.wear(100));
    assert_eq!(tool.durability_left, 180);
    assert_eq!(
        tool.repair(&MaterialId::new("stone"), 1),
        Err(ToolError::NotHeadMaterial { expected: "iron".to_owned(), found: "stone".to_owned() })
    );
    // One iron unit restores 250 * 0.5 = 125, capped at the 100 missing.
    assert_eq!(tool.repair(&MaterialId::new("iron"), 1), Ok(100));
    assert_eq!(tool.durability_left, 280);
    assert!(tool.wear(280));
    assert!(tool.is_broken());
    assert_eq!(tool.repair(&MaterialId::new("iron"), 1), Ok(125));
    assert!(!tool.is_broken());
    assert_eq!(tool.head_material().unwrap().id, MaterialId::new("iron"));
    assert_eq!(PARTS.part_kind("head").unwrap().role, PartRole::Head);
}

#[test]
fn encode_decode_round_trips() {
    let mut tool = ToolAssembly::new("longbow", &ids(&["wood", "slime", "sinew"])).unwrap();
    tool.gain_xp(300);
    tool.add_modifier("reinforced").unwrap();
    tool.wear(7);
    let mut bytes = vec![0xAA];
    tool.encode(&mut bytes).unwrap();
    bytes.extend_from_slice(&[1, 2, 3]);
    let (decoded, used) = ToolAssembly::decode(&bytes[1..]).unwrap();
    assert_eq!(decoded, tool);
    assert_eq!(used, bytes.len() - 4);
    assert_eq!(ToolAssembly::decode(&bytes[1..1 + used]).unwrap().0, tool);
    assert_eq!(ToolAssembly::decode(&bytes[1..used]), Err(DecodeError::Truncated));
    assert_eq!(ToolAssembly::decode(&bytes[..4]), Err(DecodeError::BadFormat(0xAA)));
}

#[test]
fn content_fingerprint_is_stable_within_a_build() {
    assert_eq!(crate::content_fingerprint(), crate::content_fingerprint());
    assert_ne!(crate::content_fingerprint(), 0);
}
