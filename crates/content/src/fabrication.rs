//! Fabricator definitions shared by the runtime and native authoring checker.
use crate::factory_resources::RESOURCE_COUNT;

pub const CAPACITY: u32 = 64 * 64;
pub const CONSUMER_BUFFER: u32 = 64;
pub const MIN_RECIPES: usize = 4;
pub const MAX_INPUTS: usize = 3;
pub const MAX_TICKS: u32 = 14_400;
pub const UNITS_PER_ITEM: u32 = 64;
pub const TICKS_PER_SECOND: u32 = 4;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fabrication {
    pub id: usize,
    pub name: String,
    pub inputs: Vec<(usize, u32)>,
    pub output: (usize, u32),
    pub ticks: u32,
    pub power: u32,
}

pub fn parse(text: &str) -> Result<Vec<Fabrication>, Vec<String>> {
    let rows: Vec<Fabrication> = ron::from_str(text).map_err(|error| vec![error.to_string()])?;
    let errors = validate(&rows);
    if errors.is_empty() { Ok(rows) } else { Err(errors) }
}

pub fn validate(rows: &[Fabrication]) -> Vec<String> {
    let mut errors = Vec::new();
    if rows.len() < MIN_RECIPES { errors.push("Keep the four original recipe IDs".into()); }
    let valid_stack = |(item, count): (usize, u32)| item < RESOURCE_COUNT && (1..=CAPACITY).contains(&count);
    for (index, row) in rows.iter().enumerate() {
        if row.id != index { errors.push(format!("Recipe {index}: IDs must remain in order")); }
        if row.name.trim().is_empty() { errors.push(format!("Recipe {index}: name is empty")); }
        if !(1..=MAX_INPUTS).contains(&row.inputs.len()) { errors.push(format!("Recipe {index}: invalid input count")); }
        for (i, &stack) in row.inputs.iter().enumerate() {
            if !valid_stack(stack) { errors.push(format!("Recipe {index}: invalid input")); }
            if row.inputs[..i].iter().any(|p| p.0 == stack.0) { errors.push(format!("Recipe {index}: duplicate ingredient")); }
        }
        if !valid_stack(row.output) { errors.push(format!("Recipe {index}: invalid output")); }
        if !(1..=MAX_TICKS).contains(&row.ticks) || !(1..=CONSUMER_BUFFER).contains(&row.power) {
            errors.push(format!("Recipe {index}: invalid duration or power"));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../../assets/fabrication.ron");
    fn rows() -> Vec<Fabrication> { parse(SOURCE).unwrap() }

    #[test]
    fn authored_rows_round_trip_and_allow_last_resource() {
        let mut rows = rows();
        rows[0].inputs = vec![(RESOURCE_COUNT - 1, CAPACITY)];
        rows[0].output = (RESOURCE_COUNT - 1, 1);
        rows[0].ticks = MAX_TICKS;
        rows[0].power = CONSUMER_BUFFER;
        assert!(validate(&rows).is_empty());
        assert_eq!(parse(&ron::to_string(&rows).unwrap()).unwrap(), rows);
    }

    #[test]
    fn invalid_boundaries_and_identity_are_rejected() {
        let checks: &[fn(&mut Fabrication)] = &[
            |r| r.id = 2, |r| r.name = " ".into(), |r| r.inputs.clear(),
            |r| r.inputs = vec![(0, 1); 4], |r| r.inputs = vec![(0, 1), (0, 2)],
            |r| r.inputs = vec![(RESOURCE_COUNT, 1)], |r| r.output = (RESOURCE_COUNT, 1),
            |r| r.output.1 = 0, |r| r.output.1 = CAPACITY + 1,
            |r| r.ticks = 0, |r| r.ticks = MAX_TICKS + 1,
            |r| r.power = 0, |r| r.power = CONSUMER_BUFFER + 1,
        ];
        for change in checks { let mut rows = rows(); change(&mut rows[0]); assert!(!validate(&rows).is_empty()); }
        assert!(!validate(&rows()[..MIN_RECIPES - 1]).is_empty());
        assert!(parse(&SOURCE.replacen("id:", "unknown:", 1)).is_err());
    }

    #[test]
    fn runtime_name_semantics_are_preserved() {
        let mut rows = rows();
        rows[1].name = rows[0].name.clone();
        assert!(validate(&rows).is_empty());
    }
}
