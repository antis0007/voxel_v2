//! Shared authored spell schema. Native behavior remains in the game.
//! Runtime and authoring tools load through `parse`, not raw RON decoding.

pub use crate::spell_validation::{Diagnostic, parse, validate};

/// What a spell is, for the evaluator and the wand palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Class {
    /// Fires a bolt that flies and impacts.
    Projectile,
    /// A card that changes the next bolt, or the deck's control flow.
    Modifier,
    /// A growth or summon effect delivered by a Touch or a trigger.
    Effect,
    /// Utility casts, delivery cards and the empty slot.
    Other,
}

/// What a wand must be (or what rare magic must be found) to hold a spell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Unlock {
    /// Any wand.
    FirstWand,
    /// A second-tier wand, or any rare-magic progress.
    SecondWandOrRare,
    /// A third-tier wand, or two rare-magic discoveries.
    ThirdWandOrTwoRare,
    /// A third-tier wand.
    ThirdWand,
    /// Only the spell's own rare-magic bit (reliquary spells).
    RareBit,
    /// Never holdable: the empty slot.
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Palette {
    Bolts,
    Effects,
    Summons,
    ShotMods,
    Elements,
    Program,
    Concentration,
    Hidden,
}

impl Palette {
    pub const TABS: [(Self, &'static str); 7] = [
        (Self::Bolts, "Bolts"),
        (Self::Effects, "Effects"),
        (Self::Summons, "Summons"),
        (Self::ShotMods, "Shot mods"),
        (Self::Elements, "Elements"),
        (Self::Program, "Program"),
        (Self::Concentration, "Concentration"),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpellDef<A, Text = String> {
    pub id: u32,
    pub name: Text,
    /// Badge shown on the hover card ("PROJECTILE", "MODIFIER", ...).
    pub tag: Text,
    pub color: [f32; 3],
    pub cost: f32,
    pub cooldown: f32,
    pub cast_delay: f32,
    pub recharge: f32,
    pub spread: f32,
    pub damage: f32,
    pub speed: f32,
    pub audio: (A, f32),
    pub class: Class,
    pub unlock: Unlock,
    pub palette: Palette,
}

/// Intern only accepted runtime rows. Authoring drafts retain ordinary owned strings.
impl<A> SpellDef<A> {
    pub fn into_runtime(self) -> SpellDef<A, &'static str> {
        SpellDef {
            id: self.id,
            name: Box::leak(self.name.into_boxed_str()),
            tag: Box::leak(self.tag.into_boxed_str()),
            color: self.color,
            cost: self.cost,
            cooldown: self.cooldown,
            cast_delay: self.cast_delay,
            recharge: self.recharge,
            spread: self.spread,
            damage: self.damage,
            speed: self.speed,
            audio: self.audio,
            class: self.class,
            unlock: self.unlock,
            palette: self.palette,
        }
    }
}
