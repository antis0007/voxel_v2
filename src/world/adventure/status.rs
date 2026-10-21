//! Elemental status effects and enemy affixes. Noita's depth comes from
//! materials interacting (wet + shock, fire + ice, poison stacking), and from
//! the same creature appearing in many elemental variants. This module is the
//! pure rule set; `adventure.rs` applies it to live enemies.
use crate::world::zones::Archetype;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Fire,
    Frost,
    Shock,
    Toxic,
    Wet,
}

impl Element {

    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Fire => super::PALETTE_COPPER,
            Self::Frost => super::PALETTE_MANA,
            Self::Shock => super::PALETTE_MANA_LIT,
            Self::Toxic => super::PALETTE_OLIVE,
            Self::Wet => super::PALETTE_WATER,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affix {
    Fiery,
    Frostbound,
    Static,
    Toxic,
    Volatile,
    Armored,
    Regenerating,
    Swift,
    Warded,
}

pub const AFFIXES: [Affix; 9] = [
    Affix::Fiery,
    Affix::Frostbound,
    Affix::Static,
    Affix::Toxic,
    Affix::Volatile,
    Affix::Armored,
    Affix::Regenerating,
    Affix::Swift,
    Affix::Warded,
];

impl Affix {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fiery => "Fiery",
            Self::Frostbound => "Frostbound",
            Self::Static => "Static",
            Self::Toxic => "Toxic",
            Self::Volatile => "Volatile",
            Self::Armored => "Armored",
            Self::Regenerating => "Regenerating",
            Self::Swift => "Swift",
            Self::Warded => "Warded",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Fiery => "Immune to fire. Water puts it out; frost hurts.",
            Self::Frostbound => "Immune to frost. Fire and shock cut deep.",
            Self::Static => "Immune to shock. Wet makes it worse for everyone.",
            Self::Toxic => "Immune to poison. Burns it clean.",
            Self::Volatile => "Detonates when it dies. Back away.",
            Self::Armored => "Takes less from every hit, and has more to lose.",
            Self::Regenerating => "Mends itself. Burst it down or burn the mending out.",
            Self::Swift => "Moves and strikes faster than its kind.",
            Self::Warded => "Starts behind a shield that must be broken first.",
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Fiery => Element::Fire.color(),
            Self::Frostbound => Element::Frost.color(),
            Self::Static => Element::Shock.color(),
            Self::Toxic => Element::Toxic.color(),
            Self::Volatile => [1.0, 0.82, 0.25],
            Self::Armored => [0.62, 0.66, 0.74],
            Self::Regenerating => [0.45, 1.0, 0.55],
            Self::Swift => [1.0, 0.95, 0.55],
            Self::Warded => [0.80, 0.55, 1.0],
        }
    }

    fn bit(self) -> u16 {
        1 << AFFIXES.iter().position(|a| *a == self).unwrap_or(0)
    }

    fn immune_to(self) -> Option<Element> {
        match self {
            Self::Fiery => Some(Element::Fire),
            Self::Frostbound => Some(Element::Frost),
            Self::Static => Some(Element::Shock),
            Self::Toxic => Some(Element::Toxic),
            _ => None,
        }
    }

    /// An element this affix is especially weak to.
    fn weak_to(self) -> &'static [Element] {
        match self {
            Self::Fiery => &[Element::Frost, Element::Wet],
            Self::Frostbound => &[Element::Fire, Element::Shock],
            Self::Static => &[Element::Frost],
            Self::Toxic => &[Element::Fire],
            _ => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Affixes(u16);

impl Affixes {
    pub fn has(self, affix: Affix) -> bool {
        self.0 & affix.bit() != 0
    }

    pub fn with(self, affix: Affix) -> Self {
        Self(self.0 | affix.bit())
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn list(self) -> Vec<Affix> {
        AFFIXES.iter().copied().filter(|a| self.has(*a)).collect()
    }

    pub fn first(self) -> Option<Affix> {
        AFFIXES.iter().copied().find(|a| self.has(*a))
    }

    /// Multiplier on maximum health.
    pub fn health_mul(self) -> f32 {
        if self.has(Affix::Armored) { 1.35 } else { 1.0 }
    }

    /// Multiplier on every hit taken, before element reactions.
    pub fn damage_taken(self) -> f32 {
        if self.has(Affix::Armored) { 0.78 } else { 1.0 }
    }

    pub fn speed_mul(self) -> f32 {
        if self.has(Affix::Swift) { 1.45 } else { 1.0 }
    }

    /// Starting shield, as a fraction of base health.
    pub fn shield_frac(self) -> f32 {
        if self.has(Affix::Warded) { 0.45 } else { 0.0 }
    }

    pub fn immune(self, element: Element) -> bool {
        self.list().iter().any(|a| a.immune_to() == Some(element))
    }

    fn weakness(self, element: Element) -> bool {
        self.list().iter().any(|a| a.weak_to().contains(&element))
    }

    /// Short comma-separated label, for example "Fiery, Swift".
    pub fn label(self) -> String {
        self.list()
            .iter()
            .map(|a| a.name())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Live status on one enemy. Timers are seconds remaining.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Status {
    pub burn: f32,
    pub poison: f32,
    pub shock: f32,
    pub wet: f32,
    /// Poison stacks, deepening damage per second.
    pub stacks: u8,
}

/// What a hit did beyond its raw damage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reaction {
    pub damage_mul: f32,
    pub name: Option<&'static str>,
    /// Seconds of chill to apply (frost reactions); 0 for none.
    pub chill: f32,
    /// Seconds of stagger to add.
    pub stagger: f32,
    /// Status was extinguished or cleansed by the hit.
    pub quenched: bool,
}

impl Reaction {
    fn plain() -> Self {
        Self {
            damage_mul: 1.0,
            ..Default::default()
        }
    }
}

impl Status {
    pub fn any(&self) -> bool {
        self.burn > 0.0 || self.poison > 0.0 || self.shock > 0.0 || self.wet > 0.0
    }

    /// Apply `element` from a hit and report the reaction. Immunities negate
    /// the status entirely; weaknesses amplify the hit.
    pub fn apply(&mut self, element: Element, affixes: Affixes) -> Reaction {
        if affixes.immune(element) {
            return Reaction {
                damage_mul: 0.85,
                name: Some("Resisted"),
                ..Reaction::plain()
            };
        }
        let mut r = Reaction::plain();
        if affixes.weakness(element) {
            r.damage_mul *= 1.4;
            r.name = Some("Exposed");
        }
        match element {
            Element::Fire => {
                if self.wet > 0.0 {
                    // Steam: the fire is spent flash-boiling the water.
                    self.wet = 0.0;
                    r.name = Some("Steam");
                    r.damage_mul *= 1.15;
                    r.quenched = true;
                } else {
                    self.burn = (self.burn + 3.0).min(8.0);
                }
            }
            Element::Frost => {
                if self.burn > 0.0 {
                    self.burn = 0.0;
                    r.quenched = true;
                    r.name = Some("Doused");
                }
                r.chill = if self.wet > 0.0 { 6.5 } else { 3.5 };
                if self.wet > 0.0 {
                    r.name = Some("Flash freeze");
                    r.damage_mul *= 1.25;
                }
            }
            Element::Shock => {
                self.shock = 0.6;
                r.stagger = 0.35;
                if self.wet > 0.0 {
                    r.name = Some("Conducted");
                    r.damage_mul *= 1.8;
                    r.stagger = 0.7;
                }
            }
            Element::Toxic => {
                self.poison = (self.poison + 4.0).min(10.0);
                self.stacks = (self.stacks + 1).min(5);
            }
            Element::Wet => {
                self.wet = (self.wet + 6.0).min(10.0);
                if self.burn > 0.0 {
                    self.burn = 0.0;
                    r.quenched = true;
                    r.name = Some("Doused");
                }
            }
        }
        r
    }

    /// Advance timers. Returns damage dealt this step.
    pub fn tick(&mut self, dt: f32, affixes: Affixes) -> f32 {
        let mut dmg = 0.0;
        if self.burn > 0.0 {
            self.burn = (self.burn - dt).max(0.0);
            dmg += dt * 7.0;
        }
        if self.poison > 0.0 {
            self.poison = (self.poison - dt).max(0.0);
            dmg += dt * 2.5 * self.stacks.max(1) as f32;
            if self.poison <= 0.0 {
                self.stacks = 0;
            }
        }
        self.shock = (self.shock - dt).max(0.0);
        self.wet = (self.wet - dt).max(0.0);
        if affixes.has(Affix::Regenerating) && self.burn <= 0.0 {
            dmg -= dt * 3.2;
        }
        dmg
    }
}

/// Affix weights for a zone layer: embers breed fiery things, ice breeds
/// frostbound things, and so on.
fn zone_bias(zone: Option<Archetype>, affix: Affix) -> u32 {
    let Some(z) = zone else { return 1 };
    let favoured = match z {
        Archetype::Ember => &[Affix::Fiery, Affix::Volatile][..],
        Archetype::Flooded => &[Affix::Toxic, Affix::Regenerating][..],
        Archetype::Crystal => &[Affix::Warded, Affix::Static][..],
        Archetype::Rot => &[Affix::Toxic, Affix::Regenerating][..],
        Archetype::Frost => &[Affix::Frostbound, Affix::Armored][..],
        Archetype::Void => &[Affix::Swift, Affix::Warded][..],
        Archetype::Storm => &[Affix::Static, Affix::Swift][..],
        Archetype::Gilded => &[Affix::Armored, Affix::Warded][..],
    };
    if favoured.contains(&affix) { 5 } else { 1 }
}

fn mix(a: u32, b: u32) -> u32 {
    let mut h = a ^ b.wrapping_mul(0x9e37_79b1);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

/// Deterministic roll for one spawn. `menace` is 0..=3. Bosses never roll.
pub fn roll(
    seed: u32,
    site_x: i32,
    site_y: i32,
    slot: u32,
    menace: u8,
    zone: Option<Archetype>,
) -> Affixes {
    let base = mix(
        mix(seed, site_x as u32),
        mix(site_y as u32, slot.wrapping_add(0x51ed)),
    );
    // Even the gentlest encounter sometimes fields a variant.
    let chance = [28u32, 55, 75, 90][menace.min(3) as usize];
    if base % 100 >= chance {
        return Affixes::default();
    }
    let count = if menace >= 3 || (menace >= 2 && mix(base, 7) % 4 == 0) {
        2
    } else {
        1
    };
    let mut out = Affixes::default();
    for pick in 0..count {
        let total: u32 = AFFIXES.iter().map(|a| zone_bias(zone, *a)).sum();
        let mut roll = mix(base, 100 + pick) % total;
        for a in AFFIXES {
            let w = zone_bias(zone, a);
            if roll < w {
                // Opposed elements cancel; keep one.
                let opposed = (a == Affix::Fiery && out.has(Affix::Frostbound))
                    || (a == Affix::Frostbound && out.has(Affix::Fiery));
                if !opposed {
                    out = out.with(a);
                }
                break;
            }
            roll -= w;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wet_then_shock_conducts() {
        let mut s = Status::default();
        s.apply(Element::Wet, Affixes::default());
        let r = s.apply(Element::Shock, Affixes::default());
        assert_eq!(r.name, Some("Conducted"));
        assert!(r.damage_mul > 1.7);
    }

    #[test]
    fn dry_shock_is_plain() {
        let mut s = Status::default();
        let r = s.apply(Element::Shock, Affixes::default());
        assert_eq!(r.damage_mul, 1.0);
        assert!(r.name.is_none());
    }

    #[test]
    fn water_douses_fire_and_fire_boils_water() {
        let mut s = Status::default();
        s.apply(Element::Fire, Affixes::default());
        assert!(s.burn > 0.0);
        let r = s.apply(Element::Wet, Affixes::default());
        assert!(r.quenched && s.burn == 0.0);
        let r = s.apply(Element::Fire, Affixes::default());
        assert_eq!(r.name, Some("Steam"));
        assert_eq!(s.burn, 0.0, "fire spent boiling the water");
    }

    #[test]
    fn wet_frost_freezes_longer() {
        let mut dry = Status::default();
        let mut wet = Status::default();
        wet.apply(Element::Wet, Affixes::default());
        let a = dry.apply(Element::Frost, Affixes::default());
        let b = wet.apply(Element::Frost, Affixes::default());
        assert!(b.chill > a.chill && b.damage_mul > a.damage_mul);
    }

    #[test]
    fn poison_stacks_and_deepens() {
        let mut s = Status::default();
        for _ in 0..3 {
            s.apply(Element::Toxic, Affixes::default());
        }
        assert_eq!(s.stacks, 3);
        let one = {
            let mut t = Status::default();
            t.apply(Element::Toxic, Affixes::default());
            t.tick(1.0, Affixes::default())
        };
        assert!(s.tick(1.0, Affixes::default()) > one * 2.5);
    }

    #[test]
    fn immunity_and_weakness() {
        let fiery = Affixes::default().with(Affix::Fiery);
        let mut s = Status::default();
        let r = s.apply(Element::Fire, fiery);
        assert_eq!(s.burn, 0.0);
        assert!(r.damage_mul < 1.0);
        let r = s.apply(Element::Frost, fiery);
        assert!(r.damage_mul > 1.3);
    }

    #[test]
    fn regenerating_heals_unless_burning() {
        let regen = Affixes::default().with(Affix::Regenerating);
        let mut s = Status::default();
        assert!(s.tick(1.0, regen) < 0.0);
        s.apply(Element::Fire, regen);
        assert!(s.tick(1.0, regen) > 0.0);
    }

    #[test]
    fn rolls_are_deterministic_and_varied() {
        assert_eq!(roll(9, 3, 4, 1, 2, None), roll(9, 3, 4, 1, 2, None));
        let mut seen = std::collections::HashSet::new();
        let mut with_affix = 0;
        for i in 0..400 {
            let a = roll(7, i, i * 3, (i % 3) as u32, 2, None);
            if !a.is_empty() {
                with_affix += 1;
            }
            seen.insert(a);
        }
        assert!(seen.len() >= 9, "only {} distinct combos", seen.len());
        assert!(with_affix > 200 && with_affix < 360, "{with_affix}/400");
    }

    #[test]
    fn menace_scales_variants() {
        let count = |m: u8| {
            (0..500)
                .filter(|i| !roll(1, *i, 2, 0, m, None).is_empty())
                .count()
        };
        assert!(count(0) < count(1) && count(1) < count(3));
    }

    #[test]
    fn zones_bias_affixes() {
        let fiery = |z| {
            (0..600)
                .filter(|i| roll(3, *i, 0, 0, 3, z).has(Affix::Fiery))
                .count()
        };
        assert!(fiery(Some(Archetype::Ember)) > fiery(None) * 2);
        assert!(fiery(Some(Archetype::Frost)) <= fiery(None));
    }

    #[test]
    fn fiery_and_frostbound_never_combine() {
        for i in 0..2000 {
            let a = roll(5, i, i, 0, 3, None);
            assert!(!(a.has(Affix::Fiery) && a.has(Affix::Frostbound)));
        }
    }
}
