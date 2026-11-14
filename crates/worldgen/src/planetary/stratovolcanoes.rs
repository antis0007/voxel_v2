//! Composite arc volcanoes, sampled in the same body coordinates as drainage.
use super::SEA;
use super::hotspots::Sample;
use super::tectonics::smooth;
use super::volcanism::{Bins, CELL_M, bins, cell};
use crate::BlockId;
use crate::planet::noise_at;
use crate::volcano::{Shaped, Vent};
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::{DVec2, DVec3};

const REACH: f64 = 1.75;

struct Cone {
    vent: Vent,
    down: DVec3,
    cross: DVec3,
    seed: u32,
    flank: f64,
    collapse: f64,
    exposure: f64,
    gullies: Option<[Gully; 5]>,
}

struct Gully {
    axis: DVec2,
    head: f64,
    end: f64,
    bend: f64,
    width: f64,
}

impl Gully {
    fn incision(&self, p: DVec2, rough: f64) -> f64 {
        let run = p.dot(self.axis);
        let across = self.axis.perp_dot(p) - self.bend * run * run;
        let width = (0.028 + 0.095 * run) * self.width;
        if !(self.head..self.end).contains(&run) {
            return 0.0;
        }
        smooth(self.head, self.head + 0.16, run)
            * (1.0 - smooth(self.end - 0.22, self.end, run))
            * (1.0 - smooth(width * 0.22, width, (across + rough * 0.05).abs()))
    }
}

struct Profile {
    lift: f64,
    radius: f64,
    scar: f64,
    debris: f64,
    ravine: f64,
    rough: f64,
}

impl Cone {
    fn new(vent: Vent, seed: u32, legacy_gullies: bool) -> Self {
        let east = DVec3::Y.cross(vent.dir).try_normalize().unwrap_or(DVec3::X);
        let north = vent.dir.cross(east);
        let roll = |salt: u32| f64::from(crate::hash_u32(seed ^ salt)) / f64::from(u32::MAX);
        let angle = roll(12407) * std::f64::consts::TAU;
        let down = east * angle.cos() + north * angle.sin();
        let history = roll(12413);
        let gullies = legacy_gullies.then(|| std::array::from_fn(|i| {
            let salt = 12_441 + 29 * i as u32;
            let angle = i as f64 * 1.26 + (roll(salt) - 0.5) * 1.16;
            Gully {
                axis: DVec2::new(angle.cos(), angle.sin()),
                head: 0.15 + roll(salt + 1) * 0.28,
                end: 0.69 + roll(salt + 2) * 0.45,
                bend: (roll(salt + 3) - 0.5) * 0.83,
                width: 0.56 + roll(salt + 4) * 0.73,
            }
        }));
        Self {
            vent,
            down,
            cross: vent.dir.cross(down),
            seed,
            flank: roll(12421) * 0.36 - 0.18,
            collapse: smooth(0.56, 0.91, history) * if vent.active { 0.48 } else { 1.0 },
            gullies,
            exposure: if vent.active {
                0.04 + history * 0.21
            } else {
                0.24 + history * 0.65
            },
        }
    }

    fn local(&self, dir: DVec3) -> Option<DVec2> {
        let offset = (dir - self.vent.dir) * (RADIUS_M / self.vent.radius_m);
        if offset.length_squared() >= REACH * REACH {
            return None;
        }
        Some(DVec2::new(offset.dot(self.down), offset.dot(self.cross)))
    }

    fn profile(&self, p: DVec2, body: DVec3) -> Profile {
        let rough = noise_at(body / 270.0, self.seed) - 0.5;
        let side = p.y - p.x * self.flank;
        let radius = DVec2::new(p.x / if p.x > 0.0 { 1.13 } else { 0.83 }, side / 0.84).length();
        let warped = radius * (1.0 + 0.22 * rough);
        let peak = 0.80 * (1.0 - warped / 1.15).max(0.0).powi(2)
            + 0.20 * (1.0 - warped / 0.53).max(0.0).powf(1.48);
        // The older shoulder has its own centre and runs out into a broad skirt.
        let shoulder_r = DVec2::new((p.x + 0.32) / 0.80, (side - 0.28) / 0.58).length();
        let shoulder = (0.20 + self.exposure * 0.16) * (1.0 - shoulder_r).max(0.0).powi(2);
        let apron_r = DVec2::new((p.x - 0.20) / 1.33, side / 1.0).length();
        let apron = 0.12 * (1.0 - apron_r).max(0.0).powi(2);

        // Incisions start and end at different elevations, curve independently
        // and widen downslope. The strips between them become buttresses.
        let ravine = self
            .gullies
            .iter()
            .flatten()
            .map(|g| g.incision(p, rough))
            .fold(0.0, f64::max);

        let chute = (side - 0.24 * p.x * p.x).abs();
        let width = (0.16 + 0.36 * p.x).max(0.06);
        let scar = self.collapse
            * smooth(-0.15, 0.16, p.x)
            * (1.0 - smooth(0.55, 1.03, p.x))
            * (1.0 - smooth(width * 0.60, width, chute + rough * 0.08));
        let fan_r = DVec2::new((p.x - 0.85) / 0.66, (side - 0.19) / 0.49).length();
        let debris = self.collapse * (1.0 - smooth(0.17, 1.0, fan_r + rough * 0.22));
        let lift = self.vent.height_m
            * ((peak + shoulder) * (1.0 - 0.79 * scar) * (1.0 - 0.48 * ravine)
                + apron
                + 0.13 * debris);
        Profile {
            lift,
            radius,
            scar,
            debris,
            ravine,
            rough,
        }
    }

    fn surface(
        &self,
        p: DVec2,
        profile: Profile,
        base_at: &impl Fn(DVec3) -> f64,
        out: &mut Shaped,
    ) {
        let edge = profile.rough * 0.20;
        if profile.scar > 0.28 + edge || profile.debris > 0.26 + edge {
            out.crust = Some(BlockId::Ash);
        } else if profile.radius < 0.24 + edge {
            out.crust = Some(BlockId::Basalt);
        } else if profile.ravine > 0.56 + edge && profile.lift > 44.0 {
            out.crust = Some(BlockId::Gravel);
        }
        let pit = (p / DVec2::new(0.12, 0.087)).length() + p.x * 0.9 - p.y * 0.55;
        if pit >= 1.05 {
            return;
        }
        let depth = self.vent.height_m * 0.23;
        let cut = 1.0 - smooth(0.59, 1.05, pit);
        out.metres -= depth * cut;
        if self.vent.active {
            out.active_caldera = true;
            out.heat = out.heat.max(cut);
            let head = base_at(self.vent.dir) - depth * 0.68;
            if pit < 0.80 && head > SEA && out.metres < head {
                out.lake = Some((head, BlockId::Lava));
            }
        }
    }
}

pub(super) struct Field {
    cones: Vec<Cone>,
    bins: Bins,
}

impl Field {
    pub fn new(seed: u32, vents: impl Iterator<Item = Vent>, legacy_gullies: bool) -> Self {
        let cones: Vec<_> = vents
            .map(|v| {
                let p = (v.dir * RADIUS_M).round().as_ivec3();
                let key = seed
                    ^ (p.x as u32).wrapping_mul(73_856_093)
                    ^ (p.y as u32).wrapping_mul(19_349_663)
                    ^ (p.z as u32).wrapping_mul(83_492_791);
                Cone::new(v, key, legacy_gullies)
            })
            .collect();
        let bins = bins(
            cones.iter().map(|c| (c.vent.dir, c.vent.radius_m * REACH)),
            CELL_M,
        );
        Self { cones, bins }
    }

    fn profiles(&self, dir: DVec3) -> impl Iterator<Item = (&Cone, DVec2, Profile)> {
        self.bins
            .get(&cell(dir, CELL_M))
            .into_iter()
            .flatten()
            .filter_map(move |&i| {
                let cone = &self.cones[i];
                let p = cone.local(dir)?;
                Some((cone, p, cone.profile(p, dir * RADIUS_M)))
            })
    }

    pub fn sample(&self, dir: DVec3) -> Sample {
        let mut out = Sample::default();
        for (cone, _, profile) in self.profiles(dir) {
            if profile.lift > out.lift {
                out = Sample {
                    lift: profile.lift,
                    exposure: cone.exposure,
                };
            }
        }
        out
    }

    pub fn shape(&self, dir: DVec3, base_at: &impl Fn(DVec3) -> f64, out: &mut Shaped) {
        for (cone, p, profile) in self.profiles(dir) {
            cone.surface(p, profile, base_at, out);
        }
    }

    pub fn reserves_summit(&self, dir: DVec3, margin: f64) -> bool {
        self.cones.iter().any(|cone| {
            let reach = cone.vent.radius_m * 0.20 + margin;
            (cone.vent.dir - dir).length_squared() * RADIUS_M * RADIUS_M < reach * reach
        })
    }
}
