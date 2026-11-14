//! Creature size classes shared by generation (cistern gauges) and gameplay.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum SizeClass {
    Small = 0,
    Normal = 1,
    Large = 2,
}

impl SizeClass {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Small),
            1 => Some(Self::Normal),
            2 => Some(Self::Large),
            _ => None,
        }
    }
    pub fn height(self) -> f32 {
        match self {
            Self::Small => 0.7,
            Self::Normal => 1.3,
            Self::Large => 2.4,
        }
    }
    pub fn half_width(self) -> f32 {
        match self {
            Self::Small => 0.25,
            Self::Normal => 0.45,
            Self::Large => 0.70,
        }
    }
    pub fn weight(self) -> u8 {
        match self {
            Self::Small => 0,
            Self::Normal => 1,
            Self::Large => 2,
        }
    }
    pub fn carry(self) -> u8 {
        match self {
            Self::Small => 1,
            Self::Normal => 2,
            Self::Large => 3,
        }
    }
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Small => Some(Self::Normal),
            Self::Normal => Some(Self::Large),
            Self::Large => None,
        }
    }
    pub fn prev(self) -> Option<Self> {
        match self {
            Self::Small => None,
            Self::Normal => Some(Self::Small),
            Self::Large => Some(Self::Normal),
        }
    }
}
