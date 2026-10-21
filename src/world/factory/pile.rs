//! Arcane pile: a Generator mode that burns arcane cores for four times coal's
//! power, and runs hot. Heat lives in `Node::progress`, coolant is the same
//! water the washers use, and an overheated pile melts down. Demand is the
//! control rod: a full power buffer stops fission, so an idle pile cools.
use super::{Kind, Node};

/// Resource index of arcane cores.
pub const FUEL: usize = 17;
pub const MAX_HEAT: u32 = 1000;
pub const WARN_HEAT: u32 = 750;
/// Fuel taken per fission portion, and energy that portion yields.
pub const PORTION: u32 = 8;
pub const CAPACITY: u32 = 1800;
/// Energy released per 0.25 s tick while running.
pub const RATE: u32 = 64;
const BUFFER: u32 = 2000;
/// Coolant drawn per tick (1/512 m³ units) and the heat it removes.
const COOLANT: u32 = 8;
const COOLANT_REMOVES: u32 = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NeedsFuel,
    Generating,
    Overheating,
    BufferFull,
    Meltdown,
}

impl Outcome {
    pub fn status(self) -> &'static str {
        match self {
            Self::NeedsFuel => "Needs fuel",
            Self::Generating => "Generating",
            Self::Overheating => "Overheating",
            Self::BufferFull => "Buffer full",
            Self::Meltdown => "Meltdown",
        }
    }
}

pub fn is_pile(n: &Node) -> bool {
    n.kind == Kind::Generator && n.mode == 1
}

/// Passive decay for any generator that is not actively fissioning.
pub fn cool_idle(n: &mut Node) {
    if n.kind == Kind::Generator {
        n.progress = n.progress.saturating_sub(if n.enabled { 1 } else { 2 });
    }
}

/// Advance one 0.25 s tick for an enabled pile.
pub fn step(n: &mut Node) -> Outcome {
    debug_assert!(is_pile(n));
    // Coolant works whether or not the pile is burning.
    if n.water >= COOLANT && n.progress > 0 {
        n.water -= COOLANT;
        n.progress = n.progress.saturating_sub(COOLANT_REMOVES);
    }
    if n.energy > BUFFER - RATE {
        // Demand satisfied: control rods in, no fission.
        n.progress = n.progress.saturating_sub(1);
        return Outcome::BufferFull;
    }
    if n.burn == 0 && n.input[FUEL] >= PORTION {
        n.input[FUEL] -= PORTION;
        n.burn = CAPACITY;
    }
    let energy = n.burn.min(RATE);
    n.burn -= energy;
    n.energy += energy;
    if energy == 0 {
        n.progress = n.progress.saturating_sub(1);
        return Outcome::NeedsFuel;
    }
    n.progress = (n.progress + energy / 8).min(MAX_HEAT);
    if n.progress >= MAX_HEAT {
        n.enabled = false;
        n.burn = 0;
        n.input[FUEL] = 0;
        return Outcome::Meltdown;
    }
    if n.progress >= WARN_HEAT {
        Outcome::Overheating
    } else {
        Outcome::Generating
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pile() -> Node {
        let mut n = Node::new(Kind::Generator);
        n.mode = 1;
        n.input[FUEL] = 64;
        n
    }

    /// Drain the buffer the way a consumer would so fission keeps running.
    fn load(n: &mut Node) {
        n.energy = 0;
    }

    #[test]
    fn uncooled_pile_melts_down() {
        let mut n = pile();
        let mut ticks = 0;
        loop {
            load(&mut n);
            ticks += 1;
            if step(&mut n) == Outcome::Meltdown {
                break;
            }
            assert!(ticks < 400, "never melted down");
        }
        assert!(!n.enabled && n.burn == 0 && n.input[FUEL] == 0);
        assert!(ticks > 80, "melted too fast to react: {ticks}");
    }

    #[test]
    fn coolant_holds_the_pile_stable() {
        let mut n = pile();
        for _ in 0..600 {
            load(&mut n);
            n.input[FUEL] = 64;
            n.water = 4096;
            assert_ne!(step(&mut n), Outcome::Meltdown);
        }
        assert!(n.progress < WARN_HEAT, "heat {}", n.progress);
        assert!(n.water < 4096, "coolant was never drawn");
    }

    #[test]
    fn idle_pile_cools_without_demand() {
        let mut n = pile();
        n.progress = 800;
        n.energy = BUFFER;
        for _ in 0..100 {
            assert_eq!(step(&mut n), Outcome::BufferFull);
        }
        assert_eq!(n.progress, 700);
        assert_eq!(n.input[FUEL], 64, "no fuel burned while buffer is full");
    }

    #[test]
    fn pile_out_produces_coal() {
        let mut n = pile();
        load(&mut n);
        step(&mut n);
        // A coal generator yields 16 energy per tick.
        assert!(n.energy >= 4 * 16);
    }

    #[test]
    fn disabled_generator_cools_faster() {
        let mut n = pile();
        n.progress = 500;
        n.enabled = false;
        cool_idle(&mut n);
        assert_eq!(n.progress, 498);
    }

    #[test]
    fn hot_pile_survives_a_save_round_trip() {
        use super::super::Factory;
        let mut factory = Factory::default();
        let mut n = pile();
        n.progress = 640;
        n.burn = 900;
        n.energy = 1200;
        n.water = 4096;
        factory.nodes.insert(glam::IVec3::new(4, 8, 12), n);
        let mut bytes = Vec::new();
        factory.encode(&mut bytes);
        let mut words = bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect::<Vec<_>>()
            .into_iter();
        let mut read = || words.next().ok_or_else(|| anyhow::anyhow!("short"));
        let back = Factory::decode(&mut read, 1, 8).expect("pile must decode");
        let m = &back.nodes[&glam::IVec3::new(4, 8, 12)];
        assert_eq!(
            (m.mode, m.progress, m.burn, m.energy, m.water),
            (1, 640, 900, 1200, 4096)
        );
    }
}
