use super::*;
use engine_content::{
    BlockId,
    reactions::{self, Reactant, ReactionRow},
};

fn draft(edit: impl FnOnce(&mut ReactionRow)) -> Vec<ReactionRow> {
    let mut rows = reactions::ROWS.clone();
    edit(
        rows.iter_mut()
            .find(|row| row.batch_counts.is_some() && row.a_into == Some(BlockId::MoltenBronze))
            .unwrap(),
    );
    reactions::parse(&ron::ser::to_string(&rows).unwrap()).unwrap()
}

#[test]
fn production_bronze_and_steel_keep_mode_ids_costs_and_output() {
    let bronze = process(Kind::AlloyKiln, 0).unwrap();
    assert_eq!(bronze.inputs, [(6, 192), (7, 64), (0, 0)]);
    assert_eq!(bronze.output, (14, 256));
    assert_eq!((bronze.ticks, bronze.power), (24, 10));
    assert!(bronze.block_input.is_none());
    let steel = process(Kind::AlloyKiln, 1).unwrap();
    assert_eq!(steel.inputs, [(8, 128), (3, 64), (0, 0)]);
    assert_eq!(steel.output, (15, 128));
    assert_eq!((steel.ticks, steel.power), (32, 12));
    assert!(process(Kind::AlloyKiln, 2).is_none());
    assert_eq!(
        Kind::AlloyKiln.modes(),
        [
            "Bronze: 3 × Copper ingots + 1 × Tin ingots → 4 × Bronze ingots",
            "Steel: 2 iron + 1 coal → 2 steel",
        ]
    );
    let mut kiln = Node::new(Kind::AlloyKiln);
    assert!(kiln.accepts(6) && kiln.accepts(7));
    assert!(!kiln.accepts(8) && !kiln.accepts(14));
    kiln.input[6] = 192;
    kiln.input[7] = 63;
    assert_eq!(kiln.choose_recipe(1), None);
    kiln.input[7] += 1;
    assert_eq!(kiln.choose_recipe(1), Some(0));
}

#[test]
fn edited_batch_changes_kiln_costs_output_and_label_together() {
    let rows = draft(|row| row.batch_counts = Some((5, 2)));
    let recipe = bronze_process(&rows).unwrap();
    assert_eq!(recipe.inputs, [(6, 320), (7, 128), (0, 0)]);
    assert_eq!(recipe.output, (14, 448));
    assert_eq!((recipe.ticks, recipe.power), (24, 10));
    assert_eq!(
        bronze_label(Some(recipe)),
        "Bronze: 5 × Copper ingots + 2 × Tin ingots → 7 × Bronze ingots"
    );
    let mut kiln = Node::new(Kind::AlloyKiln);
    kiln.input[6] = 192;
    kiln.input[7] = 64;
    assert!(
        !recipe.ready(&kiln, 1),
        "the old batch no longer pays for the edited ratio"
    );
    kiln.input[6] = 320;
    kiln.input[7] = 128;
    assert!(recipe.ready(&kiln, 1));
}

#[test]
fn batch_input_order_uses_smeltery_resource_mapping() {
    let rows = draft(|row| {
        row.a = Reactant::Block(BlockId::MoltenTin);
        row.b = Some(Reactant::Block(BlockId::MoltenCopper));
        row.batch_counts = Some((1, 3));
    });
    let recipe = bronze_process(&rows).unwrap();
    assert_eq!(recipe.inputs, [(7, 64), (6, 192), (0, 0)]);
    assert_eq!(recipe.output, (14, 256));
    assert_eq!(
        bronze_label(Some(recipe)),
        "Bronze: 1 × Tin ingots + 3 × Copper ingots → 4 × Bronze ingots"
    );
}

#[test]
fn missing_or_unmapped_batch_disables_bronze_without_inventing_resources() {
    assert!(bronze_process(&[]).is_none());
    let rows = draft(|row| row.b = Some(Reactant::Block(BlockId::Water)));
    assert!(bronze_process(&rows).is_none());
    assert_eq!(bronze_label(None), "Bronze: no compatible batch reaction");
}

#[test]
fn fabrication_adapter_preserves_native_rows_and_existing_mode_ids() {
    let rows = engine_content::fabrication::parse(include_str!("../../../../../assets/fabrication.ron")).unwrap();
    assert_eq!(Kind::Fabricator.modes().len(), rows.len());
    for row in rows {
        let runtime = process(Kind::Fabricator, row.id).unwrap();
        assert_eq!(&runtime.inputs[..row.inputs.len()], row.inputs.as_slice());
        assert!(runtime.inputs[row.inputs.len()..].iter().all(|&stack| stack == (0, 0)));
        assert_eq!(runtime.output, row.output);
        assert_eq!((runtime.ticks, runtime.power), (row.ticks, row.power));
        assert_eq!(Kind::Fabricator.modes()[row.id], row.name);
        assert!(runtime.block_input.is_none());
    }
    assert_eq!(super::super::RESOURCE_COUNT, engine_content::factory_resources::NAMES.len());
    assert_eq!(super::super::FEATHERS, 24);
    assert_eq!(crate::world::expedition::NAMES[24], "Feathers");
    assert_eq!(super::super::resources_in_save(65), 24);
    assert_eq!(super::super::resources_in_save(66), 25);
}
