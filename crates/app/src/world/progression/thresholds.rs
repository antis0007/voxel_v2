//! Portal research milestones travel in the actor's progression checkpoint.
use crate::world::WorldType;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Milestone {
    ReadGuide = 24,
    LinkedCrystal,
    LitHell,
    ReachedHell,
    ReachedHall,
    ReachedManyfold,
}

impl Milestone {
    pub const ALL: [Self; 6] = [
        Self::ReadGuide, Self::LinkedCrystal, Self::LitHell,
        Self::ReachedHell, Self::ReachedHall, Self::ReachedManyfold,
    ];
    pub const MASK: u32 = 0x3f << 24;

    pub fn recorded(self, lore: u32) -> bool {
        lore & (1 << self as u8) != 0
    }

    pub fn record(self, state: &mut super::State) {
        state.lore |= 1 << self as u8;
    }

    pub fn arrival(state: &mut super::State, kind: WorldType) -> bool {
        let milestone = match kind {
            WorldType::Nether => Self::ReachedHell,
            WorldType::Hall => Self::ReachedHall,
            WorldType::Manyfold => Self::ReachedManyfold,
            _ => return false,
        };
        let first = !milestone.recorded(state.lore);
        milestone.record(state);
        first
    }

    pub fn entry(self) -> (&'static str, &'static str) {
        match self {
            Self::ReadGuide => ("Field Reader", "Read the field guide at a bookcase"),
            Self::LinkedCrystal => ("Far Side in Sight", "Link two crystal frames"),
            Self::LitHell => ("Banked Spark", "Ignite a basalt frame"),
            Self::ReachedHell => ("Across the Ash", "Cross into Hell"),
            Self::ReachedHall => ("Crimson Reception", "Cross into the Hall"),
            Self::ReachedManyfold => ("A Second Down", "Cross into the Manyfold"),
        }
    }
}
