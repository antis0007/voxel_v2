//! Goal rows loaded from `assets/goals.ron`, in the order they are offered.
use super::super::BlockId;
use super::super::factory::Kind;
use super::super::inventory::Item;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tier {
    Home,
    Survival,
    Base,
    Machine,
    Power,
    Magic,
    Guardian,
    Portal,
    Orbit,
}

impl Tier {
    pub const ALL: [Tier; 9] = [
        Tier::Home,
        Tier::Survival,
        Tier::Base,
        Tier::Machine,
        Tier::Power,
        Tier::Magic,
        Tier::Guardian,
        Tier::Portal,
        Tier::Orbit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tier::Home => "Workshop",
            Tier::Survival => "Survival",
            Tier::Base => "Base",
            Tier::Machine => "First Machine",
            Tier::Power => "Power",
            Tier::Magic => "Magic",
            Tier::Guardian => "Guardian",
            Tier::Portal => "Portal",
            Tier::Orbit => "Orbit",
        }
    }
}

/// One-shot signals latched by gameplay hooks for the current session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    Ate,
    Slept,
    NightSurvived,
    GateAttuned,
    GatesLinked,
    HedronCaptured,
    CircleDrawn,
    /// The player stood inside the starter workshop. Worlds without one count it as met.
    WorkshopEntered,
    SpellCast,
    LiftBoarded,
    /// An express-lift ride docked at the crown anchorage.
    CrownReached,
}

impl Event {
    pub fn bit(self) -> u32 {
        1 << self as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Condition {
    /// Hotbar and pack together hold at least `count`.
    HasItem(Item, u32),
    /// The player placed this block during the session.
    Placed(BlockId),
    /// The factory holds a node of this kind.
    Machine(Kind),
    Event(Event),
    /// A node of the first kind exists and a node of the second kind holds items.
    Stocked(Kind, Kind),
    /// At least one guardian has been defeated.
    GuardianDefeated,
}

impl Condition {
    /// The item the HUD lists ingredients for, with the count the goal needs.
    pub fn target(self) -> Option<(Item, u32)> {
        match self {
            Condition::HasItem(item, count) => Some((item, count)),
            Condition::Placed(block) => Some((Item::Block(block), 1)),
            Condition::Machine(kind) => Some((Item::Block(kind.block()), 1)),
            Condition::Stocked(source, _) => Some((Item::Block(source.block()), 1)),
            Condition::Event(_) | Condition::GuardianDefeated => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Goal {
    pub id: String,
    pub tier: Tier,
    pub title: String,
    pub action: String,
    pub condition: Condition,
}

pub(super) const GOALS_RON: &str = include_str!("../../../../../assets/goals.ron");

/// The asset is compiled in and checked by a test; a parse failure leaves the track empty.
pub static GOALS: std::sync::LazyLock<Vec<Goal>> = std::sync::LazyLock::new(|| {
    ron::from_str(GOALS_RON).unwrap_or_else(|error| {
        eprintln!("[goals] assets/goals.ron: {error}");
        Vec::new()
    })
});
