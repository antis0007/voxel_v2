//! Machine processes read from `assets/processes.ron`.
//!
//! One row per machine and recipe id. Recipe ids are saved in placed machines,
//! so the table is keyed by id and rows are appended, never renumbered.
use super::{Kind, blocks::Material, recipes::Process};
use engine_content::factory_resources::RESOURCE_COUNT;

const MAX_INPUTS: usize = 3;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    machine: Kind,
    recipe: usize,
    inputs: Vec<(usize, u32)>,
    #[serde(default)]
    block: Option<String>,
    output: (usize, u32),
    #[serde(default)]
    byproduct: Option<(usize, u32)>,
    ticks: u32,
    power: u32,
}

/// Parses and checks the table; every problem is reported, not only the first.
pub(super) fn build(text: &str) -> Result<Vec<(Kind, usize, Process)>, Vec<String>> {
    let rows: Vec<Row> = ron::from_str(text).map_err(|error| vec![error.to_string()])?;
    let mut errors = Vec::new();
    let mut table: Vec<(Kind, usize, Process)> = Vec::with_capacity(rows.len());
    for row in rows {
        let at = format!("{:?} recipe {}", row.machine, row.recipe);
        if table.iter().any(|(kind, recipe, _)| (*kind, *recipe) == (row.machine, row.recipe)) {
            errors.push(format!("{at}: listed twice"));
        }
        if row.inputs.len() > MAX_INPUTS {
            errors.push(format!("{at}: more than {MAX_INPUTS} inputs"));
        }
        for &(resource, count) in row.inputs.iter().chain([&row.output]).chain(&row.byproduct) {
            if resource >= RESOURCE_COUNT || count == 0 {
                errors.push(format!("{at}: resource {resource} x {count} is outside the resource table or empty"));
            }
        }
        if row.byproduct.is_some_and(|(resource, _)| resource == row.output.0) {
            errors.push(format!("{at}: byproduct repeats the output resource"));
        }
        if row.ticks == 0 {
            errors.push(format!("{at}: zero ticks"));
        }
        let block_input = match &row.block {
            None => None,
            Some(name) => {
                let found = Material::ALL.iter().copied().find(|m| format!("{m:?}") == *name);
                if found.is_none() {
                    errors.push(format!("{at}: unknown block material {name}"));
                }
                found
            }
        };
        let mut inputs = [(0, 0); MAX_INPUTS];
        for (slot, input) in inputs.iter_mut().zip(&row.inputs) {
            *slot = *input;
        }
        table.push((row.machine, row.recipe, Process { inputs, block_input, output: row.output, byproduct: row.byproduct, ticks: row.ticks, power: row.power }));
    }
    if errors.is_empty() { Ok(table) } else { Err(errors) }
}

static TABLE: std::sync::LazyLock<Vec<(Kind, usize, Process)>> = std::sync::LazyLock::new(|| {
    // The asset is compiled in and checked by a test, like fabrication.ron.
    build(super::recipes::PROCESS_DRAFT.get().copied().unwrap_or(include_str!("../../../../../assets/processes.ron")))
        .unwrap_or_else(|errors| panic!("assets/processes.ron: {}", errors.join("; ")))
});

/// Process of `kind` at a saved recipe id.
pub(super) fn get(kind: Kind, recipe: usize) -> Option<Process> {
    TABLE.iter().find(|(k, r, _)| (*k, *r) == (kind, recipe)).map(|(_, _, process)| *process)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::factory::recipes::{GOLD_RECIPE, QUARTZ_RECIPE};

    #[test]
    fn the_shipped_table_loads_and_keeps_the_ids_code_refers_to() {
        let table = build(include_str!("../../../../../assets/processes.ron")).unwrap();
        assert_eq!(table.len(), 29);
        assert_eq!(get(Kind::Furnace, GOLD_RECIPE).unwrap().output, (crate::world::factory::GOLD_INGOT, 64));
        for kind in [Kind::Crusher, Kind::Washer] {
            let quartz = get(kind, QUARTZ_RECIPE).unwrap();
            assert_eq!(quartz.block_input, Some(Material::GoldQuartz));
            assert_eq!(quartz.inputs, [(0, 0); 3]);
        }
        assert!(get(Kind::Furnace, 3).is_none());
    }

    #[test]
    fn bad_rows_are_all_reported() {
        let text = r#"[
            (machine: Washer, recipe: 0, inputs: [(0, 64)], output: (9, 192), ticks: 24, power: 12),
            (machine: Washer, recipe: 0, inputs: [(99, 64)], block: Some("Cheese"), output: (9, 0), ticks: 0, power: 12),
        ]"#;
        let errors = build(text).unwrap_err();
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert!(build("[(machine: Toaster, recipe: 0, inputs: [], output: (1, 1), ticks: 1, power: 0)]").is_err());
    }
}
