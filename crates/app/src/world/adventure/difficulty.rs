//! Difficulty presets. Each world stores its own level in the save.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    /// Hits do nothing to the player, and no hostile spawns.
    Peaceful,
    #[default]
    Normal,
    /// Hits land harder, and sprinting drains hunger.
    Hard,
}

impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Peaceful, Self::Normal, Self::Hard];

    pub fn label(self) -> &'static str {
        match self {
            Self::Peaceful => "Peaceful",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Peaceful => "Damage taken: 0%. Hostiles: none.",
            Self::Normal => "Damage taken: 100%. Hunger does not drain.",
            Self::Hard => "Damage taken: 150%. Sprinting drains hunger.",
        }
    }

    /// Multiplier on damage the player takes from enemies and hazards.
    pub fn incoming_damage(self) -> f32 {
        match self {
            Self::Peaceful => 0.0,
            Self::Normal => 1.0,
            Self::Hard => 1.5,
        }
    }

    /// Hunger policy this level starts with. Hard uses the sprint drain.
    pub fn hunger_mode(self) -> super::hunger::HungerMode {
        match self {
            Self::Hard => super::hunger::HungerMode::SprintDrain,
            Self::Peaceful | Self::Normal => super::hunger::HungerMode::FoodHeals,
        }
    }

    /// Stored as one save word. Zero is Normal, so the default needs no migration.
    pub fn code(self) -> u32 {
        match self {
            Self::Normal => 0,
            Self::Peaceful => 1,
            Self::Hard => 2,
        }
    }

    /// Unknown codes load as Normal.
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Peaceful,
            2 => Self::Hard,
            _ => Self::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_is_the_default_and_keeps_damage_unchanged() {
        assert_eq!(Difficulty::default(), Difficulty::Normal);
        assert_eq!(Difficulty::Normal.incoming_damage(), 1.0);
        assert_eq!(Difficulty::Normal.hunger_mode(), super::super::hunger::HungerMode::FoodHeals);
    }

    #[test]
    fn peaceful_takes_no_damage() {
        assert_eq!(Difficulty::Peaceful.incoming_damage(), 0.0);
    }

    #[test]
    fn hard_hurts_more_and_drains_hunger_on_sprint() {
        assert!(Difficulty::Hard.incoming_damage() > 1.0);
        assert_eq!(Difficulty::Hard.hunger_mode(), super::super::hunger::HungerMode::SprintDrain);
    }

    #[test]
    fn codes_round_trip_and_unknown_codes_are_normal() {
        for level in Difficulty::ALL {
            assert_eq!(Difficulty::from_code(level.code()), level);
        }
        assert_eq!(Difficulty::from_code(3), Difficulty::Normal);
        assert_eq!(Difficulty::from_code(u32::MAX), Difficulty::Normal);
        assert_eq!(Difficulty::Normal.code(), 0);
    }

    #[test]
    fn every_level_has_a_label_and_hint() {
        for level in Difficulty::ALL {
            assert!(!level.label().is_empty());
            assert!(level.hint().contains('%'));
        }
    }
}
