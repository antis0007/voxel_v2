//! The fluid tech tree: which machine makes or converts which fluid, at which
//! tier, and the chemistry behind it.
//!
//! Tiers are a progression, not a list of recipes: a process unlocks at its
//! tier and may only consume fluids that exist by then. Equations are written
//! with the same representative formulas as `catalog`, so the tests can check
//! that every balanced reaction really conserves atoms.

use super::containers::FluidKind;

/// A machine process. Amounts are relative parts (a ratio taken from the
/// equation where one exists), not litres per second.
pub struct Process {
    pub name: &'static str,
    pub machine: &'static str,
    pub tier: u8,
    pub inputs: &'static [(FluidKind, u32)],
    pub outputs: &'static [(FluidKind, u32)],
    /// Balanced reaction, or `None` for physical steps such as distillation.
    pub equation: Option<&'static str>,
    pub note: &'static str,
}

/// Names of the six progression tiers, index = tier.
pub const TIER_NAMES: [&str; 6] = [
    "Steam Age",
    "Wood & Waste",
    "Petroleum",
    "Chemical Industry",
    "Metallurgy",
    "Cryogenics & Nitrogen",
];

/// Fluids found in the world with no process. Everything else must be made.
pub const RAW: [FluidKind; 6] = [
    FluidKind::Water,
    FluidKind::Seawater,
    FluidKind::Lava,
    FluidKind::CrudeOil,
    FluidKind::Latex,
    FluidKind::Wastewater,
];

pub static PROCESSES: &[Process] = &[
    Process {
        name: "Boil water",
        machine: "Boiler",
        tier: 0,
        inputs: &[(FluidKind::Water, 1)],
        outputs: &[(FluidKind::Steam, 1)],
        equation: Some("H2O -> H2O"),
        note: "Heat water past 100 °C and the volume grows about 1600 times. That push runs engines.",
    },
    Process {
        name: "Desalinate",
        machine: "Multi-effect distiller",
        tier: 1,
        inputs: &[(FluidKind::Seawater, 2)],
        outputs: &[(FluidKind::Water, 1), (FluidKind::Brine, 1)],
        equation: None,
        note: "Evaporate, condense, and keep the salt behind as brine.",
    },
    Process {
        name: "Treat wastewater",
        machine: "Treatment works",
        tier: 1,
        inputs: &[(FluidKind::Wastewater, 1)],
        outputs: &[(FluidKind::Water, 1)],
        equation: None,
        note: "Settle, filter and digest the waste. Clean water returns to the river.",
    },
    Process {
        name: "Distil wood",
        machine: "Retort",
        tier: 1,
        inputs: &[],
        outputs: &[(FluidKind::Creosote, 1)],
        equation: None,
        note: "Heat wood without air. The gas, tar and charcoal are the earliest chemical plant.",
    },
    Process {
        name: "Ferment sugar",
        machine: "Fermenter",
        tier: 1,
        inputs: &[],
        outputs: &[(FluidKind::BioEthanol, 2)],
        equation: Some("C6H12O6 -> 2 C2H6O + 2 CO2"),
        note: "Yeast turns one glucose into two ethanol and two carbon dioxide.",
    },
    Process {
        name: "Press seeds",
        machine: "Oil press",
        tier: 1,
        inputs: &[],
        outputs: &[(FluidKind::PlantOil, 1)],
        equation: None,
        note: "Squeeze oilseed under pressure. Cold-pressed oil keeps its flavour.",
    },
    Process {
        name: "Distil crude",
        machine: "Atmospheric column",
        tier: 2,
        inputs: &[(FluidKind::CrudeOil, 1)],
        outputs: &[
            (FluidKind::Naphtha, 1),
            (FluidKind::Kerosene, 1),
            (FluidKind::Diesel, 1),
        ],
        equation: None,
        note: "Boil crude in a tall column. Lighter fractions rise higher before they condense.",
    },
    Process {
        name: "Crack kerosene",
        machine: "Catalytic cracker",
        tier: 3,
        inputs: &[(FluidKind::Kerosene, 1)],
        outputs: &[(FluidKind::Gasoline, 1), (FluidKind::Ethylene, 2)],
        equation: Some("C12H26 -> C8H18 + 2 C2H4"),
        note: "Break long chains with heat and a catalyst. Shorter chains make petrol and ethylene.",
    },
    Process {
        name: "Contact process",
        machine: "Contact plant",
        tier: 3,
        inputs: &[],
        outputs: &[(FluidKind::SulfuricAcid, 1)],
        equation: None,
        note: "Burn sulfur, oxidise it on a catalyst, and absorb it in water. The most-made acid.",
    },
    Process {
        name: "Roast cinnabar",
        machine: "Roasting furnace",
        tier: 3,
        inputs: &[],
        outputs: &[(FluidKind::Mercury, 1)],
        equation: Some("HgS -> Hg + S"),
        note: "Heat red mercury sulfide until the mercury distils off. Handle with great care.",
    },
    Process {
        name: "Electrolyse water",
        machine: "Electrolyser",
        tier: 4,
        inputs: &[(FluidKind::Water, 2)],
        outputs: &[(FluidKind::Hydrogen, 2), (FluidKind::Oxygen, 1)],
        equation: Some("2 H2O -> 2 H2 + O2"),
        note: "Electricity splits water. Hydrogen collects at the cathode, oxygen at the anode.",
    },
    Process {
        name: "Burn gasoline",
        machine: "Combustion engine",
        tier: 4,
        inputs: &[(FluidKind::Gasoline, 2), (FluidKind::Oxygen, 25)],
        outputs: &[(FluidKind::Steam, 18)],
        equation: Some("2 C8H18 + 25 O2 -> 16 CO2 + 18 H2O"),
        note: "Complete combustion. Every drop of petrol leaves as heat, CO2 and water vapour.",
    },
    Process {
        name: "Melt iron",
        machine: "Induction furnace",
        tier: 4,
        inputs: &[],
        outputs: &[(FluidKind::MoltenIron, 1)],
        equation: Some("Fe -> Fe"),
        note: "Iron melts at 1538 °C. Pour it into moulds before it freezes.",
    },
    Process {
        name: "Melt copper",
        machine: "Induction furnace",
        tier: 4,
        inputs: &[],
        outputs: &[(FluidKind::MoltenCopper, 1)],
        equation: Some("Cu -> Cu"),
        note: "Copper melts at 1085 °C, far cooler than iron. Good for wire casting.",
    },
    Process {
        name: "Cast steel",
        machine: "Basic oxygen furnace",
        tier: 5,
        inputs: &[],
        outputs: &[(FluidKind::MoltenSteel, 1)],
        equation: None,
        note: "Blow oxygen through molten iron to burn out carbon and make steel.",
    },
    Process {
        name: "Liquefy nitrogen",
        machine: "Cold box",
        tier: 5,
        inputs: &[],
        outputs: &[(FluidKind::LiquidNitrogen, 1)],
        equation: Some("N2 -> N2"),
        note: "Compress air, cool it, and let it expand. Nitrogen liquefies first, at -196 °C.",
    },
    Process {
        name: "Liquefy oxygen",
        machine: "Cold box",
        tier: 5,
        inputs: &[],
        outputs: &[(FluidKind::LiquidOxygen, 1)],
        equation: Some("O2 -> O2"),
        note: "Oxygen liquefies at -183 °C. Dense and very eager to burn.",
    },
    Process {
        name: "Haber synthesis",
        machine: "Haber reactor",
        tier: 5,
        inputs: &[(FluidKind::LiquidNitrogen, 1), (FluidKind::Hydrogen, 3)],
        outputs: &[(FluidKind::Ammonia, 2)],
        equation: Some("N2 + 3 H2 -> 2 NH3"),
        note: "Iron catalyst, high pressure, and heat. Turns air into fertiliser.",
    },
];

/// Lowest tier at which `kind` can be obtained. Raw fluids are tier 0.
pub fn unlock_tier(kind: FluidKind) -> Option<u8> {
    if RAW.contains(&kind) {
        return Some(0);
    }
    PROCESSES
        .iter()
        .filter(|p| p.outputs.iter().any(|(k, _)| *k == kind))
        .map(|p| p.tier)
        .min()
}

/// Fluids reachable from the raw set by repeatedly running any process whose
/// inputs are already available. Processes with no fluid inputs always run.
pub fn reachable() -> Vec<FluidKind> {
    let mut have: Vec<FluidKind> = RAW.to_vec();
    loop {
        let before = have.len();
        for p in PROCESSES {
            if p.inputs.iter().all(|(k, _)| have.contains(k)) {
                for (k, _) in p.outputs {
                    if !have.contains(k) {
                        have.push(*k);
                    }
                }
            }
        }
        if have.len() == before {
            return have;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::catalog;

    /// Standard atomic masses (g/mol) for the elements used in the equations.
    fn atomic_mass(symbol: &str) -> Option<f64> {
        Some(match symbol {
            "H" => 1.008,
            "C" => 12.011,
            "N" => 14.007,
            "O" => 15.999,
            "S" => 32.06,
            "Fe" => 55.845,
            "Cu" => 63.546,
            "Hg" => 200.59,
            _ => return None,
        })
    }

    /// Element counts for a formula such as `C12H26`, or `CO2`.
    fn counts(formula: &str) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = Vec::new();
        let chars: Vec<char> = formula.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            assert!(chars[i].is_ascii_uppercase(), "bad formula {formula}");
            let mut symbol = chars[i].to_string();
            i += 1;
            while i < chars.len() && chars[i].is_ascii_lowercase() {
                symbol.push(chars[i]);
                i += 1;
            }
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let n: u32 = if start == i {
                1
            } else {
                chars[start..i].iter().collect::<String>().parse().unwrap()
            };
            match out.iter_mut().find(|(s, _)| *s == symbol) {
                Some(entry) => entry.1 += n,
                None => out.push((symbol, n)),
            }
        }
        out
    }

    /// Element totals on one side of an equation, with coefficients applied.
    fn side_totals(side: &str) -> Vec<(String, u32)> {
        let mut totals: Vec<(String, u32)> = Vec::new();
        for term in side.split(" + ") {
            let term = term.trim();
            let (coeff, formula) = match term.split_once(' ') {
                Some((c, f)) if c.chars().all(|ch| ch.is_ascii_digit()) => {
                    (c.parse::<u32>().unwrap(), f)
                }
                _ => (1, term),
            };
            for (symbol, n) in counts(formula) {
                match totals.iter_mut().find(|(s, _)| *s == symbol) {
                    Some(entry) => entry.1 += n * coeff,
                    None => totals.push((symbol, n * coeff)),
                }
            }
        }
        totals.sort();
        totals
    }

    fn mass_of(totals: &[(String, u32)]) -> f64 {
        totals
            .iter()
            .map(|(s, n)| atomic_mass(s).expect("known element") * *n as f64)
            .sum()
    }

    #[test]
    fn every_balanced_equation_conserves_atoms_and_mass() {
        for p in PROCESSES {
            let Some(eq) = p.equation else { continue };
            let (left, right) = eq.split_once(" -> ").expect(p.name);
            let l = side_totals(left);
            let r = side_totals(right);
            assert_eq!(l, r, "{}: {eq} is not balanced", p.name);
            assert!((mass_of(&l) - mass_of(&r)).abs() < 1e-9);
        }
    }

    #[test]
    fn every_fluid_is_reachable_from_the_raw_set() {
        let reachable = reachable();
        for kind in FluidKind::ALL {
            assert!(
                reachable.contains(&kind),
                "{kind:?} can never be made from the raw resources"
            );
        }
    }

    #[test]
    fn no_process_needs_a_fluid_from_a_later_tier() {
        for p in PROCESSES {
            for (input, _) in p.inputs {
                let needed = unlock_tier(*input).expect("input is obtainable");
                assert!(
                    needed <= p.tier,
                    "{} (tier {}) needs {input:?}, which only unlocks at tier {needed}",
                    p.name,
                    p.tier
                );
            }
        }
    }

    #[test]
    fn tiers_are_named_and_in_range() {
        for p in PROCESSES {
            assert!((p.tier as usize) < TIER_NAMES.len(), "{}", p.name);
        }
        for kind in FluidKind::ALL {
            assert!(unlock_tier(kind).is_some(), "{kind:?} has no tier");
        }
    }

    #[test]
    fn process_names_and_notes_are_present() {
        for (i, a) in PROCESSES.iter().enumerate() {
            assert!(!a.note.is_empty(), "{} needs a teaching note", a.name);
            for b in &PROCESSES[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
    }
}
