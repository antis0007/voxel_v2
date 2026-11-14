//! Advancement tree loaded from `assets/advancements.ron`.
use super::{Stat, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Trigger {
    /// The statistic has reached the amount, in its stored unit.
    Stat(Stat, u64),
    /// The experience level has reached the value.
    Level(u32),
}

impl Trigger {
    pub fn met(self, state: &State, level: u32) -> bool {
        match self {
            Trigger::Stat(stat, amount) => state.get(stat) >= amount,
            Trigger::Level(target) => level >= target,
        }
    }

    /// Current and target progress in display units.
    pub fn progress(self, state: &State, level: u32) -> (u64, u64) {
        match self {
            Trigger::Stat(stat, amount) => (state.get(stat).min(amount), amount),
            Trigger::Level(target) => (u64::from(level.min(target)), u64::from(target)),
        }
    }

    /// `Blocks Mined: 1,000` style requirement line.
    pub fn requirement(self) -> String {
        match self {
            Trigger::Stat(stat, amount) => format!("{}: {}", stat.label(), stat.format(amount)),
            Trigger::Level(target) => format!("Level: {target}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Advancement {
    /// Stable key written to the sidecar; never rename one that has shipped.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub trigger: Trigger,
}

const ADVANCEMENTS_RON: &str = include_str!("../../../../../assets/advancements.ron");

/// The asset is compiled in and checked by a test; a parse failure leaves the tree empty.
static TABLE: std::sync::LazyLock<Vec<Advancement>> = std::sync::LazyLock::new(|| {
    ron::from_str(ADVANCEMENTS_RON).unwrap_or_else(|error| {
        eprintln!("[stats] assets/advancements.ron: {error}");
        Vec::new()
    })
});

pub fn table() -> &'static [Advancement] {
    &TABLE
}

pub fn index_of(id: &str) -> Option<usize> {
    TABLE.iter().position(|a| a.id == id)
}

/// Steps from the root to `index`, for indenting the tree.
pub fn depth(index: usize) -> usize {
    let mut depth = 0;
    let mut at = TABLE.get(index);
    while let Some(parent) = at.and_then(|a| a.parent.as_deref()).and_then(index_of) {
        depth += 1;
        at = TABLE.get(parent);
        if depth > TABLE.len() {
            break;
        }
    }
    depth
}
