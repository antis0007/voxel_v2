//! Timed player status effects with a level, ticked in the fixed step.

/// Effects the player controller, combat and health can express.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Kind {
    Regeneration = 0,
    Speed = 1,
    Slowness = 2,
    Strength = 3,
    Weakness = 4,
    Poison = 5,
    FireResistance = 6,
    Resistance = 7,
    Absorption = 8,
    Haste = 9,
    JumpBoost = 10,
}

impl Kind {
    pub const ALL: [Self; 11] = [
        Self::Regeneration,
        Self::Speed,
        Self::Slowness,
        Self::Strength,
        Self::Weakness,
        Self::Poison,
        Self::FireResistance,
        Self::Resistance,
        Self::Absorption,
        Self::Haste,
        Self::JumpBoost,
    ];

    pub fn from_u8(raw: u8) -> Option<Self> {
        Self::ALL.get(raw as usize).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Regeneration => "Regeneration",
            Self::Speed => "Speed",
            Self::Slowness => "Slowness",
            Self::Strength => "Strength",
            Self::Weakness => "Weakness",
            Self::Poison => "Poison",
            Self::FireResistance => "Fire Resistance",
            Self::Resistance => "Resistance",
            Self::Absorption => "Absorption",
            Self::Haste => "Haste",
            Self::JumpBoost => "Jump Boost",
        }
    }

    /// Command spelling: the name in lower case with underscores.
    pub fn parse(name: &str) -> Option<Self> {
        let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        let wanted = key(name);
        Self::ALL.into_iter().find(|k| key(k.name()) == wanted)
    }

    pub fn harmful(self) -> bool {
        matches!(self, Self::Slowness | Self::Weakness | Self::Poison)
    }
}

pub const MAX_LEVEL: u8 = 5;
pub const MAX_SECONDS: f32 = 3600.0;
/// Health per second at level 1 on the 100-point scale; each level doubles it.
const REGEN_PER_SECOND: f32 = 2.0;
const POISON_PER_SECOND: f32 = 4.0;
/// Poison never takes the player below this health.
pub const POISON_FLOOR: f32 = 5.0;
/// Absorption health per level on the 100-point scale.
pub const ABSORPTION_PER_LEVEL: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Active {
    pub kind: Kind,
    pub seconds: f32,
    pub level: u8,
    /// Duration when applied, for the HUD timer sweep.
    #[serde(default)]
    pub total: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    pub list: Vec<Active>,
}

impl Effects {
    /// Adds or refreshes `kind`; a higher level replaces a lower one, an equal level keeps the longer time.
    pub fn add(&mut self, kind: Kind, seconds: f32, level: u8) {
        let seconds = if seconds.is_finite() { seconds.clamp(0.0, MAX_SECONDS) } else { 0.0 };
        let level = level.clamp(1, MAX_LEVEL);
        if seconds <= 0.0 {
            return;
        }
        let fresh = Active { kind, seconds, level, total: seconds };
        match self.list.iter_mut().find(|a| a.kind == kind) {
            Some(a) if level > a.level => *a = fresh,
            Some(a) if level == a.level && seconds > a.seconds => *a = fresh,
            Some(_) => {}
            None => self.list.push(fresh),
        }
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }

    pub fn level(&self, kind: Kind) -> u8 {
        self.list.iter().find(|a| a.kind == kind).map_or(0, |a| a.level)
    }

    /// Advances timers by `dt` and returns the health change they cause this step.
    pub fn tick(&mut self, dt: f32, health: f32) -> f32 {
        let rate = |level: u8| f32::from(1u8 << (level.saturating_sub(1)).min(7));
        let mut delta = 0.0;
        for a in &self.list {
            let step = dt.min(a.seconds);
            match a.kind {
                Kind::Regeneration => delta += REGEN_PER_SECOND * rate(a.level) * step,
                Kind::Poison if health > POISON_FLOOR => {
                    delta -= (POISON_PER_SECOND * rate(a.level) * step).min(health - POISON_FLOOR)
                }
                _ => {}
            }
        }
        for a in &mut self.list {
            a.seconds -= dt;
        }
        self.list.retain(|a| a.seconds > 0.0);
        delta
    }

    /// Movement speed multiplier from Speed and Slowness.
    pub fn speed_factor(&self) -> f32 {
        (1.0 + 0.2 * f32::from(self.level(Kind::Speed)) - 0.15 * f32::from(self.level(Kind::Slowness))).max(0.1)
    }

    /// Melee damage multiplier from Strength and Weakness.
    pub fn melee_factor(&self) -> f32 {
        (1.0 + 0.3 * f32::from(self.level(Kind::Strength)) - 0.25 * f32::from(self.level(Kind::Weakness))).max(0.0)
    }

    pub fn fire_immune(&self) -> bool {
        self.level(Kind::FireResistance) > 0
    }

    /// Incoming damage multiplier: 20% less per Resistance level, never below 20%.
    pub fn damage_factor(&self) -> f32 {
        (1.0 - 0.2 * f32::from(self.level(Kind::Resistance))).max(0.2)
    }

    /// Mining speed multiplier from Haste.
    pub fn mining_factor(&self) -> f32 {
        1.0 + 0.25 * f32::from(self.level(Kind::Haste))
    }

    /// Jump velocity multiplier: each level adds about 40% to the jump height.
    pub fn jump_factor(&self) -> f32 {
        (1.0 + 0.4 * f32::from(self.level(Kind::JumpBoost))).sqrt()
    }

    /// Most absorption health the active Absorption level allows.
    pub fn absorption_cap(&self) -> f32 {
        ABSORPTION_PER_LEVEL * f32::from(self.level(Kind::Absorption))
    }
}

/// Food or brewed effect row as stored in data tables.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Grant {
    pub kind: Kind,
    pub seconds: f32,
    pub level: u8,
}
