//! Semantic checks shared by runtime loading and offline authoring.
use crate::spells::SpellDef;

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub id: Option<u32>,
    pub field: &'static str,
    pub message: String,
}

/// Parse the real spell schema (including the caller's native audio cue type)
/// and reject invalid tables before binary-search lookups can observe them.
pub fn parse<A: serde::de::DeserializeOwned>(
    source: &str,
) -> Result<Vec<SpellDef<A>>, Vec<Diagnostic>> {
    let rows: Vec<SpellDef<A>> = ron::from_str(source).map_err(|error| {
        vec![Diagnostic {
            id: None,
            field: "$",
            message: error.to_string(),
        }]
    })?;
    let errors = validate(&rows);
    if errors.is_empty() {
        Ok(rows)
    } else {
        Err(errors)
    }
}

/// Returns all semantic errors, with stable spell IDs and field keys suitable
/// for an inspector to focus. Sparse IDs are valid; row order stays untouched.
pub fn validate<A>(rows: &[SpellDef<A>]) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    if rows.is_empty() {
        errors.push(Diagnostic {
            id: None,
            field: "$",
            message: "Spell table is empty".into(),
        });
    }
    for (index, row) in rows.iter().enumerate() {
        let mut fail = |field, message: &str| {
            errors.push(Diagnostic {
                id: Some(row.id),
                field,
                message: message.into(),
            })
        };
        if index > 0 && rows[index - 1].id >= row.id {
            fail("id", "Spell IDs must be strictly increasing and unique");
        }
        if row.name.trim().is_empty() {
            fail("name", "Name must not be blank");
        }
        if row.tag.trim().is_empty() {
            fail("tag", "Tag must not be blank");
        }
        for (field, value) in [
            ("cost", row.cost),
            ("cooldown", row.cooldown),
            ("damage", row.damage),
        ] {
            if !value.is_finite() || value < 0.0 {
                fail(field, "Must be finite and nonnegative");
            }
        }
        for (field, value) in [("speed", row.speed), ("audio", row.audio.1)] {
            if !value.is_finite() || value <= 0.0 {
                fail(field, "Must be finite and positive");
            }
        }
        // Negative timing and spread deltas are intentional modifier behavior.
        for (field, value) in [
            ("cast_delay", row.cast_delay),
            ("recharge", row.recharge),
            ("spread", row.spread),
        ] {
            if !value.is_finite() {
                fail(field, "Must be finite");
            }
        }
        if !row
            .color
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            fail("color", "Channels must be finite and between zero and one");
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::{Class, Palette, Unlock};
    use engine_audio::Cue;

    const SOURCE: &str = include_str!("../../../assets/spells.ron");

    #[test]
    fn production_catalog_round_trips_through_the_shared_schema() {
        let rows = parse::<Cue>(SOURCE).unwrap();
        assert!(rows.len() > 80);
        assert_eq!(rows[0].name, "Spark");
        let serialized = ron::ser::to_string_pretty(&rows, ron::ser::PrettyConfig::new())
            .unwrap()
            .replace('\r', "");
        // Authored tables may contain comments and noncanonical whitespace.
        // Compare actual values, rather than requiring authors to reformat.
        assert_eq!(parse::<Cue>(&serialized).unwrap(), rows);
    }

    #[test]
    fn diagnostics_collect_fields_and_stable_ids_in_one_pass() {
        let mut rows = parse::<Cue>(SOURCE).unwrap();
        rows[0].cost = -1.0;
        rows[0].speed = 0.0;
        rows[0].cast_delay = f32::NAN;
        rows[1].id = rows[0].id;
        let errors = validate(&rows);
        for field in ["cost", "speed", "cast_delay", "id"] {
            assert!(
                errors
                    .iter()
                    .any(|error| error.id == Some(0) && error.field == field)
            );
        }
    }

    #[test]
    fn future_rows_use_existing_schema_without_another_registration() {
        let mut rows = parse::<Cue>(SOURCE).unwrap();
        let mut row = rows[0].clone();
        row.id = rows.last().unwrap().id + 3;
        row.name = "Authored future spell".into();
        row.cast_delay = -0.1;
        row.recharge = -0.2;
        row.spread = -2.0;
        row.class = Class::Modifier;
        row.unlock = Unlock::FirstWand;
        row.palette = Palette::ShotMods;
        let id = row.id;
        rows.push(row);
        let source = ron::ser::to_string(&rows).unwrap();
        assert_eq!(parse::<Cue>(&source).unwrap().last().unwrap().id, id);
    }

    #[test]
    fn rejects_unknown_native_cues_and_malformed_tables() {
        assert!(parse::<Cue>("[]").is_err());
        assert!(
            parse::<Cue>(&SOURCE.replacen("audio: (Spark,", "audio: (ImaginaryCue,", 1)).is_err()
        );
        assert!(parse::<Cue>(&SOURCE.replacen("cost: 12.0", "cost: -12.0", 1)).is_err());
        assert!(parse::<Cue>(&SOURCE.replacen("id: 1,", "id: 0,", 1)).is_err());
    }

    #[test]
    fn accepted_owned_rows_preserve_the_copyable_runtime_layout_and_values() {
        let rows = parse::<Cue>(SOURCE).unwrap();
        let encoded = ron::ser::to_string(&rows).unwrap();
        let runtime: Vec<_> = rows.into_iter().map(SpellDef::into_runtime).collect();
        fn copyable<T: Copy>(_: T) {}
        copyable(runtime[0]);
        assert_eq!(ron::ser::to_string(&runtime).unwrap(), encoded);
        assert_eq!(runtime.binary_search_by_key(&0, |row| row.id), Ok(0));
    }

    #[test]
    fn rejects_every_nonfinite_number_without_restricting_valid_modifiers() {
        let rows = parse::<Cue>(SOURCE).unwrap();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut row = rows[0].clone();
            row.cost = value;
            row.cooldown = value;
            row.damage = value;
            row.speed = value;
            row.audio.1 = value;
            row.cast_delay = value;
            row.recharge = value;
            row.spread = value;
            row.color[0] = value;
            assert_eq!(validate(&[row]).len(), 9);
        }
    }
}
