//! Shared authored research costs for the game and development workbench.
use super::Lesson;
use crate::world::inventory::Item;
use std::sync::LazyLock;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Project {
    pub lesson: Lesson,
    pub xp: u32,
    pub requires: Option<Lesson>,
    pub samples: Vec<(Item, u32)>,
}

pub(crate) fn catalog() -> &'static [Project] {
    static ROWS: LazyLock<Vec<Project>> = LazyLock::new(|| {
        let result = serde_json::from_str::<Vec<Project>>(include_str!(
            "../../../../../../assets/field_studies.json"
        ));
        let Ok(rows) = result else {
            eprintln!("[study] Invalid field_studies.json");
            return Vec::new();
        };
        let valid = rows.len() == Lesson::ALL.len() && rows.iter().enumerate().all(|(i, row)| {
            (1..=10_000).contains(&row.xp)
                && !rows[..i].iter().any(|other| other.lesson == row.lesson)
                // Parents must precede children, ruling out cycles.
                && row.requires.is_none_or(|parent| rows[..i].iter().any(|r| r.lesson == parent))
                && (1..=4).contains(&row.samples.len())
                && row.samples.iter().enumerate().all(|(j, &(item, count))| {
                    (1..=4096).contains(&count)
                        && !row.samples[..j].iter().any(|&(other, _)| other == item)
                        && match item {
                            Item::Resource(id) => (id as usize) < crate::world::expedition::NAMES.len(),
                            Item::Food(_) => true,
                            _ => false,
                        }
                })
        });
        if valid { rows } else {
            eprintln!("[study] Invalid field study costs or dependencies");
            Vec::new()
        }
    });
    &ROWS
}
