use super::*;
use crate::world::{smeltery, tool_workshop};
use engine_tools::{MATERIALS, PARTS};

fn has_entry(rows: &[&Entry], entry: &Entry) -> bool {
    rows.iter().any(|row| std::ptr::eq(*row, entry))
}

fn instantaneous(entry: &Entry) {
    assert_eq!(entry.grid, None);
    assert_eq!((entry.seconds, entry.energy, entry.yields), (0.0, 0.0, 0.0));
}

#[test]
fn forge_workshop_parts_match_authoritative_costs_at_every_scale() {
    for size in [1, 2, 4] {
        for (kind, part) in PARTS.part_kinds.iter().enumerate() {
            let kind = u8::try_from(kind).unwrap();
            for material in 0..MATERIALS.len() {
                let output = Item::Part { kind, material: u8::try_from(material).unwrap() };
                let rows: Vec<_> = producing_at(output, size).into_iter()
                    .filter(|row| row.station == Station::Utility(BlockId::Workbench)).collect();
                let cost = tool_workshop::supply(material).filter(|supply| !supply.cast)
                    .and_then(|_| tool_workshop::material_cost(
                        material, u32::from(part.material_cost) * 64, 1.0 / size as f32,
                    ));
                let Some((input, count)) = cost else {
                    assert!(rows.is_empty(), "{output:?} cannot be made at a workbench");
                    continue;
                };
                assert_eq!(rows.len(), 1, "{output:?} at scale {size}");
                let row = rows[0];
                assert_eq!(row.inputs, vec![(input, count as f32)]);
                assert_eq!(row.outputs, vec![(output, 1.0)]);
                instantaneous(row);
                assert!(has_entry(&using_at(input, size), row));
                assert!(has_entry(&using_at(Item::Block(BlockId::Workbench), size), row));
            }
        }
    }
}

#[test]
fn forge_molds_match_authoritative_costs_without_consuming_ingot_molds() {
    for size in [1, 2, 4] {
        for kind in (0..PARTS.part_kinds.len().min(255)).map(|kind| kind as u8).chain([255]) {
            let mold = Item::Cast(kind);
            let rows = producing_at(mold, size);
            assert_eq!(rows.len(), 1);
            let row = rows[0];
            assert_eq!(row.station, Station::Utility(BlockId::Workbench));
            assert_eq!(row.inputs, vec![
                (Item::Block(BlockId::Clay), tool_workshop::mold_cost(1.0 / size as f32) as f32),
            ]);
            assert_eq!(row.outputs, vec![(mold, 1.0)]);
            instantaneous(row);
            assert!(has_entry(&using_at(Item::Block(BlockId::Clay), size), row));
        }
        assert!(using_at(Item::Cast(255), size).is_empty());
    }
}

#[test]
fn forge_cast_sources_and_uses_match_cooled_collection_at_every_scale() {
    for size in [1, 2, 4] {
        for kind in 0..PARTS.part_kinds.len().min(255) {
            let kind = kind as u8;
            for metal in &smeltery::DEF.metals {
                let Some(collection) = smeltery::part_collection(kind, metal.solid, size) else { continue; };
                let rows: Vec<_> = producing_at(collection.output.item, size).into_iter()
                    .filter(|row| row.station == Station::Utility(BlockId::CastingTable))
                    .filter(|row| row.takes(Item::Block(metal.solid)) > 0.0).collect();
                assert_eq!(rows.len(), 1);
                let row = rows[0];
                assert_eq!(row.inputs, vec![
                    (collection.metal.item, collection.metal.count as f32),
                    (Item::Cast(kind), 1.0),
                ]);
                let outputs: Vec<_> = std::iter::once(collection.output).chain(collection.excess)
                    .map(|stack| (stack.item, stack.count as f32)).collect();
                assert_eq!(row.outputs, outputs);
                assert_eq!(row.note, "Cooled Metal · Mold Consumed");
                instantaneous(row);
                for (input, _) in &row.inputs {
                    assert!(has_entry(&using_at(*input, size), row));
                }
                for (output, _) in &row.outputs {
                    assert!(has_entry(&producing_at(*output, size), row));
                }
                assert!(has_entry(&using_at(Item::Block(BlockId::CastingTable), size), row));
            }
        }
    }
}

#[test]
fn forge_projection_is_bounded_and_has_no_fabricated_tools() {
    let parts = PARTS.part_kinds.len().min(256);
    let workshop_materials = (0..MATERIALS.len().min(256)).filter(|&material| {
        tool_workshop::supply(material).is_some_and(|row| !row.cast)
            && tool_workshop::material_cost(material, 64, 1.0).is_some()
    }).count();
    let cast_metals = smeltery::DEF.metals.iter()
        .filter(|metal| smeltery::material_for(metal.solid).is_some()).count();
    let expected = parts * workshop_materials + parts.min(255) * cast_metals + parts.min(255) + 1;
    for size in [1, 2, 4] {
        let rows = forge_entries(size);
        assert_eq!(rows.len(), expected);
        for row in &rows {
            instantaneous(row);
            assert!(!row.inputs.iter().any(|(item, _)| *item == Item::Cast(255)));
            for (item, _) in row.inputs.iter().chain(&row.outputs) {
                assert!(!matches!(item, Item::Assembled { .. }));
            }
            for (item, _) in &row.outputs {
                assert!(known_items().contains(item), "{item:?} missing from browser");
            }
        }
    }
}

#[test]
fn forge_collection_rejects_non_part_molds_and_non_casting_metals() {
    assert!(smeltery::part_collection(255, BlockId::CopperBlock, 1).is_none());
    assert!(smeltery::part_collection(0, BlockId::Stone, 1).is_none());
    if let Ok(kind) = u8::try_from(PARTS.part_kinds.len()) {
        assert!(smeltery::part_collection(kind, BlockId::CopperBlock, 1).is_none());
    }
}
