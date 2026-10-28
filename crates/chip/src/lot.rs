//! A wafer lot: one coupon wafer walked through the route at game time, with
//! a deterministic record per step. Lot id, lab and mask set fix every
//! outcome; reloading a half-finished lot finishes it the same way.
//!
//! Yield: random defects `Y = exp(-D·A)`, D in defects/cm² (raised by each
//! contamination event), A the die area in cm²; an exposure whose alignment
//! error exceeds the process tolerance opens a fraction of that layer's
//! features. Speed bins come from a per-die critical-path spread `sigma`.
use crate::fab::{Grade, Lab, MaskLayer, MaskSet, Step, StepKind};
use crate::registry::Process;
use serde::{Deserialize, Serialize};

pub fn mix(a: u64, b: u64) -> u64 {
    let mut z = a ^ b.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}
/// Roughly normal, unit variance: a centred sum of four uniforms.
fn gauss(h: u64) -> f64 {
    let s: f64 = (0..4).map(|k| unit(mix(h, k))).sum();
    (s - 2.0) * 3f64.sqrt()
}

/// Whole dies on a coupon wafer, with a 20% edge exclusion.
pub fn dies_per_wafer(wafer_mm: f32, die_mm2: f32) -> u16 {
    let r = f64::from(wafer_mm) / 2.0;
    (std::f64::consts::PI * r * r * 0.8 / f64::from(die_mm2.max(0.01))).floor().min(f64::from(u16::MAX)) as u16
}

/// Die centres for a wafer map, in wafer radii: the `n` grid cells nearest
/// the centre, then by row and column. Also returns the die side in radii.
/// `dies_per_wafer` counts by area, so the rim cells can reach past the
/// edge; the whole map then shrinks until every die corner is inside.
pub fn wafer_sites(wafer_mm: f32, die_mm2: f32, n: u16) -> (f32, Vec<(f32, f32)>) {
    let r = (wafer_mm / 2.0).max(0.01);
    let side = die_mm2.max(0.01).sqrt() / r;
    let k = (1.0 / side).ceil() as i32 + 1;
    let mut cells: Vec<(f32, i32, i32)> = Vec::new();
    for j in -k..k {
        for i in -k..k {
            let (x, y) = ((i as f32 + 0.5) * side, (j as f32 + 0.5) * side);
            cells.push((x * x + y * y, j, i));
        }
    }
    cells.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    let sites: Vec<(f32, f32)> = cells.into_iter().take(usize::from(n)).map(|(_, j, i)| ((i as f32 + 0.5) * side, (j as f32 + 0.5) * side)).collect();
    let reach = sites.iter().map(|(x, y)| (x.abs() + side / 2.0).hypot(y.abs() + side / 2.0)).fold(1.0f32, f32::max);
    (side / reach, sites.into_iter().map(|(x, y)| (x / reach, y / reach)).collect())
}

/// One die on a probed wafer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Die {
    Dead,
    /// Works, below the C rating.
    Slow,
    Bin(Grade),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub step: u16,
    pub kind: StepKind,
    pub mask: Option<MaskLayer>,
    /// nm for films, µm for overlay and etch bias, % for dose, dies for probe.
    pub value: f32,
    /// Overlay over tolerance, or contamination picked up.
    pub fault: bool,
}

impl Record {
    pub fn text(&self) -> String {
        let what = match self.mask {
            Some(m) => format!("{} {}", self.kind.label(), m.label()),
            None => self.kind.label().to_string(),
        };
        let v = match self.kind {
            StepKind::Oxidize | StepKind::DepositFilm | StepKind::GateStack | StepKind::DepositMetal | StepKind::Passivate => format!("{:.0} nm", self.value),
            StepKind::Expose => format!("Overlay {:.2} µm", self.value),
            StepKind::Etch => format!("Bias {:.2} µm", self.value),
            StepKind::Implant => format!("Dose {:.1}%", self.value),
            StepKind::Probe => format!("{:.0} Good", self.value),
            StepKind::Package => format!("{:.0} Packaged", self.value),
            StepKind::Clean | StepKind::Strip => if self.fault { "Residue".into() } else { "Clean".into() },
            _ => "OK".into(),
        };
        let fault = if self.fault && self.kind == StepKind::Expose { " · Over Tolerance" } else { "" };
        format!("{what} · {v}{fault}")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub dies: u16,
    pub good: u16,
    /// Good dies per bin A, B, C.
    pub bins: [u16; 3],
    /// Functional but below the C rating: rejected.
    pub slow: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaferLot {
    #[serde(default)]
    pub route_version: u8,
    pub id: u32,
    pub revision: u32,
    pub mask_set: u64,
    pub lab: String,
    pub seed: u64,
    /// Next step to complete; `route.len()` once finished.
    pub step: u16,
    pub busy_until: u64,
    pub records: Vec<Record>,
    pub contamination: u8,
    /// Fraction of dies surviving alignment faults so far.
    pub survival: f64,
    pub outcome: Option<Outcome>,
}

impl WaferLot {
    pub fn new(id: u32, revision: u32, mask_set: u64, lab: &str, now: u64, first: Step) -> Self {
        Self {
            route_version: 0,
            id,
            revision,
            mask_set,
            lab: lab.to_string(),
            seed: mix(u64::from(id), mask_set),
            step: 0,
            busy_until: now + u64::from(first.kind.seconds()) * 120,
            records: Vec::new(),
            contamination: 0,
            survival: 1.0,
            outcome: None,
        }
    }

    pub fn done(&self, route: &[Step]) -> bool {
        usize::from(self.step) >= route.len()
    }

    /// Complete every step due by `now`. Returns the graded good dies when
    /// the lot is packaged.
    pub fn advance(&mut self, now: u64, route: &[Step], lab: &Lab, process: &Process, masks: &MaskSet, die_mm2: f32) -> Option<Vec<(u16, Grade)>> {
        let mut packaged = None;
        while !self.done(route) && now >= self.busy_until {
            let step = route[usize::from(self.step)];
            let h = mix(self.seed, u64::from(self.step) + 1);
            let (value, fault) = match step.kind {
                StepKind::Oxidize => (100.0 * (1.0 + 0.04 * gauss(h)), false),
                StepKind::DepositFilm | StepKind::Passivate => (400.0 * (1.0 + 0.04 * gauss(h)), false),
                StepKind::DepositMetal => (800.0 * (1.0 + 0.04 * gauss(h)), false),
                StepKind::GateStack => (200.0 * (1.0 + 0.04 * gauss(h)), false),
                StepKind::Implant => (100.0 + 3.0 * gauss(h), false),
                StepKind::Etch => (0.2 * gauss(h).abs(), false),
                StepKind::Expose => {
                    let e = gauss(h).abs() * f64::from(lab.overlay_um);
                    let over = e > f64::from(process.overlay_um);
                    if over {
                        let features = masks.layers.iter().find(|(m, _)| Some(*m) == step.mask).map_or(0, |(_, n)| *n);
                        let p = 0.002 * e / f64::from(process.overlay_um);
                        self.survival *= (1.0 - p).powi(features.min(4000) as i32);
                    }
                    (e, over)
                }
                StepKind::Clean | StepKind::Strip => {
                    let dirty = unit(h) < f64::from(lab.contamination);
                    self.contamination = self.contamination.saturating_add(u8::from(dirty));
                    (0.0, dirty)
                }
                StepKind::Probe => {
                    let o = self.probe(lab, process, die_mm2);
                    self.outcome = Some(o);
                    (f64::from(o.good), false)
                }
                StepKind::Package => {
                    let dies = self.graded(lab, process, die_mm2);
                    let n = dies.len();
                    packaged = Some(dies);
                    (n as f64, false)
                }
                _ => (0.0, false),
            };
            self.records.push(Record { step: self.step, kind: step.kind, mask: step.mask, value: value as f32, fault });
            self.step += 1;
            if let Some(next) = route.get(usize::from(self.step)) {
                self.busy_until += u64::from(next.kind.seconds()) * 120;
            }
        }
        packaged
    }

    /// Every die's probe result in die order, once the lot has been probed.
    pub fn dies(&self, lab: &Lab, process: &Process, die_mm2: f32) -> Option<Vec<Die>> {
        let o = self.outcome?;
        let y = self.yield_(lab, die_mm2);
        let dies = (0..dies_per_wafer(process.wafer_mm, die_mm2))
            .map(|i| match self.die(lab, i, y) {
                None => Die::Dead,
                Some(None) => Die::Slow,
                Some(Some(g)) => Die::Bin(g),
            })
            .collect::<Vec<_>>();
        (dies.len() == usize::from(o.dies)).then_some(dies)
    }

    fn die(&self, lab: &Lab, i: u16, yield_: f64) -> Option<Option<Grade>> {
        if unit(mix(self.seed, (u64::from(i) << 8) | 1)) >= yield_ {
            return None;
        }
        let speed = 1.0 + f64::from(lab.sigma) * gauss(mix(self.seed, (u64::from(i) << 8) | 2));
        Some(match speed {
            s if s >= 0.97 => Some(Grade::A),
            s if s >= 0.85 => Some(Grade::B),
            s if s >= 0.70 => Some(Grade::C),
            _ => None,
        })
    }

    fn yield_(&self, lab: &Lab, die_mm2: f32) -> f64 {
        let d = f64::from(lab.defect_per_cm2) * (1.0 + f64::from(self.contamination));
        (-d * f64::from(die_mm2) / 100.0).exp() * self.survival
    }

    fn probe(&self, lab: &Lab, process: &Process, die_mm2: f32) -> Outcome {
        let dies = dies_per_wafer(process.wafer_mm, die_mm2);
        let y = self.yield_(lab, die_mm2);
        let mut o = Outcome { dies, ..Outcome::default() };
        for i in 0..dies {
            match self.die(lab, i, y) {
                Some(Some(g)) => {
                    o.good += 1;
                    o.bins[g as usize] += 1;
                }
                Some(None) => o.slow += 1,
                None => {}
            }
        }
        o
    }

    fn graded(&self, lab: &Lab, process: &Process, die_mm2: f32) -> Vec<(u16, Grade)> {
        let y = self.yield_(lab, die_mm2);
        (0..dies_per_wafer(process.wafer_mm, die_mm2)).filter_map(|i| Some((i, self.die(lab, i, y)??))).collect()
    }
}
