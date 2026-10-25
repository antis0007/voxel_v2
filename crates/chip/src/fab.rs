//! Planar fabrication: a lab grade, the materials a route consumes, the
//! ordered process route for a layout's layer count, and the immutable mask
//! set derived from a released revision. Step durations and material counts
//! are game presets, not factory minutes or recipes.
use crate::design::Design;
use crate::registry::Registry;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provenance {
    /// A prepared blank die personalised by its top metal.
    Prepared,
    /// A die from a wafer lot through a reusable mask set.
    Fabricated { mask_set: u64, lot: u32, die: u16 },
}

/// Speed bin from wafer test. Every bin runs the same firmware; the rating
/// is the clock the die meets with margin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Grade {
    #[default]
    A,
    B,
    C,
}

impl Grade {
    /// Fraction of the revision's characterised clock the bin is rated for.
    pub fn clock_factor(self) -> f32 {
        match self {
            Grade::A => 1.0,
            Grade::B => 0.85,
            Grade::C => 0.7,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lab {
    pub id: String,
    pub name: String,
    pub defect_per_cm2: f32,
    pub sigma: f32,
    pub overlay_um: f32,
    pub contamination: f32,
    pub filters: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaskLayer {
    Diffusion,
    Gate,
    Contact,
    Metal(u8),
    Via(u8),
}

impl MaskLayer {
    fn code(self) -> u64 {
        match self {
            MaskLayer::Diffusion => 1,
            MaskLayer::Gate => 2,
            MaskLayer::Contact => 3,
            MaskLayer::Metal(l) => 16 + u64::from(l),
            MaskLayer::Via(l) => 32 + u64::from(l),
        }
    }

    pub fn label(self) -> String {
        match self {
            MaskLayer::Diffusion => "Diffusion".into(),
            MaskLayer::Gate => "Gate".into(),
            MaskLayer::Contact => "Contact".into(),
            MaskLayer::Metal(l) => format!("M{}", l + 1),
            MaskLayer::Via(l) => format!("V{}", l),
        }
    }
}

/// Consumables a route draws. The owner maps them to its own resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Material {
    Wafer,
    MaskBlank,
    Resist,
    Solvent,
    Dopant,
    Gas,
    Metal,
    Filter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepKind {
    Clean,
    Oxidize,
    Coat,
    Expose,
    Develop,
    Etch,
    /// Resist removal by a sealed organic clean; it does not remove oxide.
    Strip,
    Implant,
    Anneal,
    DepositFilm,
    DepositMetal,
    Passivate,
    Probe,
    Package,
}

impl StepKind {
    pub fn label(self) -> &'static str {
        match self {
            StepKind::Clean => "Clean",
            StepKind::Oxidize => "Oxidize",
            StepKind::Coat => "Coat Resist",
            StepKind::Expose => "Expose",
            StepKind::Develop => "Develop",
            StepKind::Etch => "Etch",
            StepKind::Strip => "Strip Resist",
            StepKind::Implant => "Implant",
            StepKind::Anneal => "Anneal",
            StepKind::DepositFilm => "Deposit Oxide",
            StepKind::DepositMetal => "Deposit Metal",
            StepKind::Passivate => "Passivate",
            StepKind::Probe => "Probe",
            StepKind::Package => "Dice & Package",
        }
    }
    /// Game-time preset, seconds.
    pub fn seconds(self) -> u32 {
        match self {
            StepKind::Oxidize | StepKind::Anneal => 8,
            StepKind::Implant | StepKind::DepositMetal | StepKind::DepositFilm => 5,
            StepKind::Probe | StepKind::Package => 4,
            _ => 2,
        }
    }
    pub fn uses(self) -> &'static [(Material, u32)] {
        match self {
            StepKind::Clean | StepKind::Strip => &[(Material::Solvent, 1)],
            StepKind::Coat => &[(Material::Resist, 1)],
            StepKind::Implant => &[(Material::Dopant, 1)],
            StepKind::Oxidize | StepKind::Anneal | StepKind::DepositFilm | StepKind::Passivate => &[(Material::Gas, 1)],
            StepKind::DepositMetal => &[(Material::Metal, 2)],
            StepKind::Package => &[(Material::Metal, 1)],
            _ => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    pub mask: Option<MaskLayer>,
}

/// Pattern one layer: coat, expose through its mask, develop, etch, strip.
fn pattern(route: &mut Vec<Step>, layer: MaskLayer) {
    for kind in [StepKind::Coat, StepKind::Expose, StepKind::Develop, StepKind::Etch, StepKind::Strip] {
        route.push(Step { kind, mask: (kind == StepKind::Expose).then_some(layer) });
    }
}

/// The planar route for a layout with `layers` metal layers. Ordering is the
/// point: resist before exposure, implant after the diffusion window is
/// open, anneal after implant, a dielectric and via mask between metals.
pub fn route(layers: u8) -> Vec<Step> {
    let s = |kind| Step { kind, mask: None };
    let mut r = vec![s(StepKind::Clean), s(StepKind::Oxidize)];
    pattern(&mut r, MaskLayer::Diffusion);
    r.extend([s(StepKind::Implant), s(StepKind::Anneal), s(StepKind::DepositFilm)]);
    pattern(&mut r, MaskLayer::Gate);
    r.push(s(StepKind::DepositFilm));
    pattern(&mut r, MaskLayer::Contact);
    for l in 0..layers.max(1) {
        if l > 0 {
            r.push(s(StepKind::DepositFilm));
            pattern(&mut r, MaskLayer::Via(l));
        }
        r.push(s(StepKind::DepositMetal));
        pattern(&mut r, MaskLayer::Metal(l));
    }
    r.extend([s(StepKind::Passivate), s(StepKind::Probe), s(StepKind::Package)]);
    r
}

/// Masks are content-addressed: one per layer, patterned from the layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaskSet {
    pub id: u64,
    pub revision: u32,
    /// Each layer and the sites it patterns.
    pub layers: Vec<(MaskLayer, u32)>,
}

pub fn mask_set(design: &Design, registry: &Registry, revision: u32, fingerprint: u64) -> MaskSet {
    let area: u32 = design.parts.iter().filter_map(|p| registry.module(&p.module)).map(|s| s.area()).sum();
    let contacts = design.nets.iter().map(|n| n.pins.len() as u32).sum();
    let mut layers = vec![(MaskLayer::Diffusion, area), (MaskLayer::Gate, area / 2), (MaskLayer::Contact, contacts)];
    for l in 0..design.layers {
        if l > 0 {
            let vias = design.nets.iter().flat_map(|n| n.paths.iter()).flat_map(|p| p.windows(2))
                .filter(|w| w[0].layer.max(w[1].layer) == l && w[0].layer != w[1].layer)
                .count() as u32;
            layers.push((MaskLayer::Via(l), vias));
        }
        let wire = design.nets.iter().flat_map(|n| n.paths.iter().flatten()).filter(|s| s.layer == l).count() as u32;
        layers.push((MaskLayer::Metal(l), wire));
    }
    let mut id = fingerprint ^ 0x6d61_736b;
    for (layer, n) in &layers {
        id = crate::lot::mix(id, (layer.code() << 32) | u64::from(*n));
    }
    MaskSet { id, revision, layers }
}
