//! Real-world properties of every fluid the factory knows about.
//!
//! Values are typical room-temperature or process-condition figures, rounded
//! for teaching. Formulas are representative: mixtures such as crude oil or
//! diesel use a single hydrocarbon as a teaching stand-in, and that is stated
//! where it matters (see `formula`).

use super::containers::FluidKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Liquid,
    Gas,
    /// Liquid metal or slag above its melting point.
    Molten,
    /// Liquid below room temperature, kept in insulated tanks.
    Cryogenic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hazard {
    Flammable,
    Toxic,
    Corrosive,
    Hot,
    Cryogenic,
    /// Reacts violently with water or air. Store dry and sealed.
    Reactive,
}

pub struct FluidSpec {
    pub kind: FluidKind,
    pub name: &'static str,
    pub phase: Phase,
    /// Representative molecular formula. `None` for mixtures and alloys.
    pub formula: Option<&'static str>,
    /// kg per cubic metre at storage conditions.
    pub density: f32,
    /// Typical storage temperature in °C.
    pub temperature_c: f32,
    /// Body colour in sRGB, `[0, 1]`. Derived from the pixel palette roles
    /// where a fluid has a natural palette family, otherwise chosen to read
    /// clearly as a distinct fluid inside a tank.
    pub color: [f32; 3],
    pub hazards: &'static [Hazard],
    /// One line for the in-game tooltip: what the fluid is used for, or why it
    /// matters.
    pub why: &'static str,
}

const fn hex(v: u32) -> [f32; 3] {
    [
        ((v >> 16) & 0xff) as f32 / 255.0,
        ((v >> 8) & 0xff) as f32 / 255.0,
        (v & 0xff) as f32 / 255.0,
    ]
}

/// Every fluid, in tech-tree-ish order: raw resources first, then refined
/// products, then exotic and molten materials.
pub static SPECS: [FluidSpec; 26] = [
    FluidSpec {
        kind: FluidKind::Water,
        name: "Water",
        phase: Phase::Liquid,
        formula: Some("H2O"),
        density: 997.0,
        temperature_c: 20.0,
        color: hex(0x65b7af),
        hazards: &[],
        why: "The first resource. Boilers, cooling and every chemical reaction in this list.",
    },
    FluidSpec {
        kind: FluidKind::Steam,
        name: "Steam",
        phase: Phase::Gas,
        formula: Some("H2O"),
        density: 0.6,
        temperature_c: 150.0,
        color: hex(0xdde9ea),
        hazards: &[Hazard::Hot],
        why: "Water above 100 °C. Drives turbines: the oldest power source in the book.",
    },
    FluidSpec {
        kind: FluidKind::Seawater,
        name: "Seawater",
        phase: Phase::Liquid,
        formula: None,
        density: 1025.0,
        temperature_c: 15.0,
        color: hex(0x3c7f86),
        hazards: &[],
        why: "About 3.5% dissolved salt. Drinking it dehydrates you. Distilling it gives water.",
    },
    FluidSpec {
        kind: FluidKind::Brine,
        name: "Brine",
        phase: Phase::Liquid,
        formula: None,
        density: 1200.0,
        temperature_c: 20.0,
        color: hex(0x6f8f8a),
        hazards: &[],
        why: "Concentrated salt water left behind by desalination. Feeds chlor-alkali plants.",
    },
    FluidSpec {
        kind: FluidKind::Lava,
        name: "Lava",
        phase: Phase::Molten,
        formula: None,
        density: 2600.0,
        temperature_c: 1150.0,
        color: hex(0xe8551f),
        hazards: &[Hazard::Hot],
        why: "Molten basalt from deep underground. Hot enough to melt stone; it cools to rock.",
    },
    FluidSpec {
        kind: FluidKind::CrudeOil,
        name: "Crude oil",
        phase: Phase::Liquid,
        formula: None,
        density: 850.0,
        temperature_c: 20.0,
        color: hex(0x2a2218),
        hazards: &[Hazard::Flammable],
        why: "Ancient plankton, cooked slowly. A mixture of thousands of hydrocarbons.",
    },
    FluidSpec {
        kind: FluidKind::Naphtha,
        name: "Naphtha",
        phase: Phase::Liquid,
        formula: Some("C6H14"),
        density: 700.0,
        temperature_c: 20.0,
        color: hex(0xf2eec4),
        hazards: &[Hazard::Flammable],
        why: "Light fraction from distillation. Feedstock for plastics and petrochemicals.",
    },
    FluidSpec {
        kind: FluidKind::Kerosene,
        name: "Kerosene",
        phase: Phase::Liquid,
        formula: Some("C12H26"),
        density: 800.0,
        temperature_c: 20.0,
        color: hex(0xbfdde6),
        hazards: &[Hazard::Flammable],
        why: "Middle fraction. Jet fuel and lamp oil, and the feed for cracking into petrol.",
    },
    FluidSpec {
        kind: FluidKind::Diesel,
        name: "Diesel",
        phase: Phase::Liquid,
        formula: Some("C16H34"),
        density: 840.0,
        temperature_c: 20.0,
        color: hex(0x9c6b2c),
        hazards: &[Hazard::Flammable],
        why: "Heavy fraction. Compression-ignition engines and generators.",
    },
    FluidSpec {
        kind: FluidKind::Gasoline,
        name: "Gasoline",
        phase: Phase::Liquid,
        formula: Some("C8H18"),
        density: 740.0,
        temperature_c: 20.0,
        color: hex(0xcfd35a),
        hazards: &[Hazard::Flammable, Hazard::Toxic],
        why: "Spark-ignition fuel. Cracked from kerosene because short chains burn cleanly.",
    },
    FluidSpec {
        kind: FluidKind::Creosote,
        name: "Creosote",
        phase: Phase::Liquid,
        formula: None,
        density: 1050.0,
        temperature_c: 20.0,
        color: hex(0x6a381b),
        hazards: &[Hazard::Toxic],
        why: "Dark tar from heating wood without air. The first oil people ever refined.",
    },
    FluidSpec {
        kind: FluidKind::BioEthanol,
        name: "Bio-ethanol",
        phase: Phase::Liquid,
        formula: Some("C2H6O"),
        density: 789.0,
        temperature_c: 20.0,
        color: hex(0xdaad60),
        hazards: &[Hazard::Flammable],
        why: "Sugar fermented by yeast. A renewable fuel, and it burns to CO2 and water.",
    },
    FluidSpec {
        kind: FluidKind::PlantOil,
        name: "Plant oil",
        phase: Phase::Liquid,
        formula: None,
        density: 920.0,
        temperature_c: 20.0,
        color: hex(0xd9b93c),
        hazards: &[],
        why: "Pressed from seeds. Cooking oil, and a biodiesel feedstock.",
    },
    FluidSpec {
        kind: FluidKind::Latex,
        name: "Latex",
        phase: Phase::Liquid,
        formula: None,
        density: 920.0,
        temperature_c: 20.0,
        color: hex(0xf4f1e4),
        hazards: &[],
        why: "Milky sap tapped from rubber trees. Polymer chains that vulcanise into rubber.",
    },
    FluidSpec {
        kind: FluidKind::Wastewater,
        name: "Wastewater",
        phase: Phase::Liquid,
        formula: None,
        density: 1000.0,
        temperature_c: 20.0,
        color: hex(0x5d5a3c),
        hazards: &[Hazard::Toxic],
        why: "Used water from homes and farms. Treat it before it returns to a river.",
    },
    FluidSpec {
        kind: FluidKind::Ethylene,
        name: "Ethylene",
        phase: Phase::Gas,
        formula: Some("C2H4"),
        density: 1.18,
        temperature_c: 20.0,
        color: hex(0xd7e3f2),
        hazards: &[Hazard::Flammable],
        why: "The most-made organic chemical on Earth. Polyethylene, antifreeze, plastics.",
    },
    FluidSpec {
        kind: FluidKind::SulfuricAcid,
        name: "Sulfuric acid",
        phase: Phase::Liquid,
        formula: Some("H2SO4"),
        density: 1830.0,
        temperature_c: 20.0,
        color: hex(0xe5dc6b),
        hazards: &[Hazard::Corrosive],
        why: "Made in the most-produced chemical industry. Fertiliser, batteries, refining.",
    },
    FluidSpec {
        kind: FluidKind::Ammonia,
        name: "Ammonia",
        phase: Phase::Liquid,
        formula: Some("NH3"),
        density: 682.0,
        temperature_c: -33.0,
        color: hex(0xb8d0f0),
        hazards: &[Hazard::Toxic, Hazard::Corrosive, Hazard::Cryogenic],
        why: "Fixed nitrogen from the Haber process. Feeds crops and feeds half the planet.",
    },
    FluidSpec {
        kind: FluidKind::Mercury,
        name: "Mercury",
        phase: Phase::Liquid,
        formula: Some("Hg"),
        density: 13534.0,
        temperature_c: 20.0,
        color: hex(0xc9ccd1),
        hazards: &[Hazard::Toxic],
        why: "Liquid metal at room temperature. Dangerous to handle; a lesson in why we retired it.",
    },
    FluidSpec {
        kind: FluidKind::Hydrogen,
        name: "Hydrogen",
        phase: Phase::Gas,
        formula: Some("H2"),
        density: 0.09,
        temperature_c: 20.0,
        color: hex(0xf5f9ff),
        hazards: &[Hazard::Flammable],
        why: "Lightest element. Fuel, ammonia feedstock, and the first step toward cryogenics.",
    },
    FluidSpec {
        kind: FluidKind::Oxygen,
        name: "Oxygen",
        phase: Phase::Gas,
        formula: Some("O2"),
        density: 1.43,
        temperature_c: 20.0,
        color: hex(0x8fc9ff),
        hazards: &[Hazard::Reactive],
        why: "About a fifth of the air. Makes fire burn hotter and keeps lungs working.",
    },
    FluidSpec {
        kind: FluidKind::LiquidNitrogen,
        name: "Liquid nitrogen",
        phase: Phase::Cryogenic,
        formula: Some("N2"),
        density: 807.0,
        temperature_c: -196.0,
        color: hex(0xb9e3f5),
        hazards: &[Hazard::Cryogenic],
        why: "Air cooled until it liquefies. Flash-freezes food and shrinks metal for assembly.",
    },
    FluidSpec {
        kind: FluidKind::LiquidOxygen,
        name: "Liquid oxygen",
        phase: Phase::Cryogenic,
        formula: Some("O2"),
        density: 1141.0,
        temperature_c: -183.0,
        color: hex(0x4f9be0),
        hazards: &[Hazard::Cryogenic, Hazard::Reactive],
        why: "Dense, pale-blue oxidiser. Rocket engines burn fuel in it.",
    },
    FluidSpec {
        kind: FluidKind::MoltenIron,
        name: "Molten iron",
        phase: Phase::Molten,
        formula: Some("Fe"),
        density: 7000.0,
        temperature_c: 1538.0,
        color: hex(0xff9a4d),
        hazards: &[Hazard::Hot],
        why: "Iron above its melting point. Cast into tools, pipes and the first machines.",
    },
    FluidSpec {
        kind: FluidKind::MoltenCopper,
        name: "Molten copper",
        phase: Phase::Molten,
        formula: Some("Cu"),
        density: 8000.0,
        temperature_c: 1085.0,
        color: hex(0xff6f5a),
        hazards: &[Hazard::Hot],
        why: "Copper melts easily and conducts well. Wire, coils and motor windings.",
    },
    FluidSpec {
        kind: FluidKind::MoltenSteel,
        name: "Molten steel",
        phase: Phase::Molten,
        formula: None,
        density: 7800.0,
        temperature_c: 1500.0,
        color: hex(0xffd08a),
        hazards: &[Hazard::Hot],
        why: "Iron with a controlled dose of carbon. Stronger than iron, and the start of heavy industry.",
    },
];

pub fn spec(kind: FluidKind) -> &'static FluidSpec {
    SPECS
        .iter()
        .find(|s| s.kind == kind)
        .expect("every FluidKind has a catalog entry")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_exactly_one_entry() {
        assert_eq!(SPECS.len(), FluidKind::ALL.len());
        for kind in FluidKind::ALL {
            let count = SPECS.iter().filter(|s| s.kind == kind).count();
            assert_eq!(count, 1, "{kind:?}");
        }
    }

    #[test]
    fn names_are_unique_and_non_empty() {
        for (i, a) in SPECS.iter().enumerate() {
            assert!(!a.name.is_empty());
            assert!(!a.why.is_empty(), "{} needs a tooltip line", a.name);
            for b in &SPECS[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn colours_are_distinct_inside_a_tank() {
        for (i, a) in SPECS.iter().enumerate() {
            for b in &SPECS[i + 1..] {
                let d: f32 = (0..3).map(|c| (a.color[c] - b.color[c]).abs()).sum();
                assert!(d > 0.03, "{} and {} look too alike", a.name, b.name);
            }
        }
    }

    #[test]
    fn physical_values_are_plausible() {
        for s in &SPECS {
            assert!(s.density > 0.0 && s.density < 20000.0, "{}", s.name);
            assert!(s.temperature_c > -273.15, "{}", s.name);
            match s.phase {
                Phase::Cryogenic => assert!(s.temperature_c < 0.0, "{}", s.name),
                Phase::Molten => assert!(s.temperature_c > 700.0, "{}", s.name),
                Phase::Gas => assert!(s.density < 10.0, "{}", s.name),
                Phase::Liquid => {}
            }
        }
    }

    #[test]
    fn hazardous_cryogens_are_marked() {
        assert!(
            spec(FluidKind::LiquidNitrogen)
                .hazards
                .contains(&Hazard::Cryogenic)
        );
        assert!(spec(FluidKind::Oxygen).hazards.contains(&Hazard::Reactive));
    }
}
