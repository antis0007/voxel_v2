//! Cached vents selected from the same plate field as continental terrain.
use super::flank_erosion;
use super::hotspots::{Sample, Tracks};
use super::stratovolcanoes;
use super::tectonics::Plates;
use super::volcanic_landforms::Landforms;
use super::SEA;
use crate::{BlockId, WorldSampleConfig};
use crate::volcano::{self, Vent};
use engine_core::cube_sphere::atlas;
use glam::DVec3;
use std::collections::HashMap;

pub(super) const CELL_M: f64 = 2600.0;

pub(super) type Bins = HashMap<[i32; 3], Vec<usize>>;

pub(super) fn cell(dir: DVec3, pitch: f64) -> [i32; 3] {
    (dir * atlas::RADIUS_M / pitch).floor().as_ivec3().to_array()
}

pub(super) fn bins(sites: impl Iterator<Item = (DVec3, f64)>, pitch: f64) -> Bins {
    let mut bins = Bins::new();
    for (i, (dir, reach)) in sites.enumerate() {
        let centre = dir * atlas::RADIUS_M;
        let reach = DVec3::splat(reach);
        let lo = ((centre - reach) / pitch).floor().as_ivec3();
        let hi = ((centre + reach) / pitch).floor().as_ivec3();
        for x in lo.x..=hi.x { for y in lo.y..=hi.y { for z in lo.z..=hi.z {
            bins.entry([x, y, z]).or_default().push(i);
        } } }
    }
    bins
}

pub struct Field {
    pub vents: Vec<Vent>,
    bins: Bins,
    crater_bins: Bins,
    asymmetry: Option<u32>,
    tracks: Option<Tracks>,
    landforms: Option<Landforms>,
    flanks: Option<flank_erosion::Field>,
    stratovolcanoes: Option<stratovolcanoes::Field>,
    reefs: Option<super::reefs::Field>,
}

impl Field {
    pub fn new(cfg: &WorldSampleConfig, plates: &Plates) -> Self {
        let seed = cfg.seed;
        let evolved = cfg.generator_version >= engine_voxel::HOTSPOT_PLANET_GENERATOR;
        let composite = cfg.generator_version >= engine_voxel::STRATOVOLCANO_PLANET_GENERATOR;
        let tracks = evolved.then(|| Tracks::new(seed, plates));
        let mut vents = Vec::new();
        let bound = (atlas::RADIUS_M / CELL_M).ceil() as i64;
        for x in -bound..bound {
            for y in -bound..bound {
                for z in -bound..bound {
                    if let Some(v) = volcano::vent_in_cell_with(seed, [x, y, z], !evolved, |d| {
                        plates.vent_tectonics(d)
                    }) { vents.push(v); }
                }
            }
        }
        let dissected = cfg.generator_version >= engine_voxel::VOLCANIC_FLANK_EROSION_GENERATOR;
        let stratovolcanoes = composite.then(|| {
            for vent in vents.iter_mut().filter(|v| v.kind == volcano::Kind::Strato) {
                vent.radius_m *= 1.42;
                let room = (465.0 - plates.at(vent.dir).height).max(150.0);
                vent.height_m = (vent.height_m * 1.28 + 36.0).min(room / 1.12);
            }
            stratovolcanoes::Field::new(
                seed,
                vents.iter().filter(|v| v.kind == volcano::Kind::Strato).copied(),
                !dissected,
            )
        });
        let mut cone_bins = bins(vents.iter().map(|v| (v.dir, v.radius_m * 2.2)), CELL_M);
        if composite {
            for indices in cone_bins.values_mut() {
                indices.retain(|&i| vents[i].kind != volcano::Kind::Strato);
            }
        }
        let active = tracks.as_ref().map_or(&[][..], Tracks::active_vents);
        let crater_bins = bins(active.iter().map(|v| (v.dir, v.radius_m)), CELL_M);
        vents.extend_from_slice(active);
        Self { vents, bins: cone_bins, crater_bins, asymmetry: evolved.then_some(seed), tracks,
            landforms: None, flanks: None, stratovolcanoes, reefs: None }
    }

    pub(super) fn bake_reefs(&mut self, mesh: std::sync::Arc<super::topology::Mesh>, height: &mut [f64], rain: &[f64], temperature: &[f64], fine: bool) {
        if let Some(tracks) = &self.tracks {
            self.reefs = Some(super::reefs::Field::bake(mesh, height, rain, temperature, tracks, fine));
        }
    }

    pub(super) fn reef_sediment(&self, dir: DVec3) -> f64 {
        self.reefs.as_ref().map_or(0.0, |field| field.sediment(dir))
    }

    pub(super) fn reef_material(&self, dir: DVec3, height: f64) -> Option<BlockId> {
        self.reefs.as_ref().and_then(|field| field.material(dir, height))
    }

    pub fn edifice(&self, dir: DVec3) -> Sample {
        let mut sample = self.tracks.as_ref().map_or_else(Sample::default, |tracks| tracks.sample(dir));
        if let Some(stratovolcanoes) = &self.stratovolcanoes {
            let arc = stratovolcanoes.sample(dir);
            let total = sample.lift + arc.lift;
            if arc.lift > 0.0 {
                sample.exposure = (sample.exposure * sample.lift + arc.exposure * arc.lift) / total;
                sample.lift = total;
            }
        }
        sample
    }

    pub fn bake_landforms(
        &mut self,
        cfg: &WorldSampleConfig,
        workers: usize,
        substrate_at: impl Fn(DVec3) -> f64 + Sync,
        rain_at: impl Fn(DVec3) -> f64 + Sync,
    ) {
        let Some(tracks) = &self.tracks else { return };
        let ground = |dir| substrate_at(dir) + self.edifice(dir).lift;
        let grouped = super::evolved_volcanoes(cfg.generator_version);
        let assembled = |dir| {
            let height = ground(dir);
            if grouped { self.cone_shape(dir, height, &ground).metres } else { height }
        };
        let flanks = (cfg.generator_version >= engine_voxel::VOLCANIC_FLANK_EROSION_GENERATOR)
            .then(|| flank_erosion::Field::bake(
                self.vents.iter()
                    .filter(|v| matches!(v.kind, volcano::Kind::Shield | volcano::Kind::Strato))
                    .copied()
                    .chain(tracks.weathered_shields().filter(|vent| ground(vent.dir) > SEA + 25.0)),
                cfg.seed,
                workers,
                cfg.generator_version == super::WATERSHED_EROSION_GENERATOR,
                grouped,
                |dir| {
                    let edifice = self.edifice(dir);
                    let height = substrate_at(dir) + edifice.lift;
                    let height = if grouped { self.cone_shape(dir, height, &ground).metres } else { height };
                    (height, rain_at(dir), edifice.exposure)
                },
            ));
        self.landforms = Some(Landforms::bake(
            tracks.active_rifts(),
            |dir| assembled(dir) + flanks.as_ref().map_or(0.0, |field| field.sample(dir).0),
            flanks.is_some(),
            cfg.generator_version >= engine_voxel::CALDERA_FAULTS_GENERATOR,
            grouped,
        ));
        self.flanks = flanks;
    }

    pub fn reserves_summit(&self, dir: DVec3, margin: f64) -> bool {
        self.landforms.as_ref().is_some_and(|field| field.reserves_summit(dir, margin))
            || self.stratovolcanoes.as_ref().is_some_and(|field| field.reserves_summit(dir, margin))
    }

    fn cone_shape(&self, dir: DVec3, height: f64, base_at: &impl Fn(DVec3) -> f64) -> volcano::Shaped {
        let cell = cell(dir, CELL_M);
        let nearby = self.bins.get(&cell).into_iter().flatten().map(|&i| &self.vents[i]);
        volcano::shape_from(dir, height, nearby, base_at, self.asymmetry)
    }

    pub fn shape(&self, dir: DVec3, height: f64, base_at: impl Fn(DVec3) -> f64) -> volcano::Shaped {
        let cell = cell(dir, CELL_M);
        let mut shaped = self.cone_shape(dir, height, &base_at);
        if let Some(flanks) = &self.flanks {
            let (change, exposed) = flanks.sample(dir);
            shaped.metres += change;
            if exposed { shaped.crust = Some(crate::BlockId::Basalt); }
        }
        if let Some(stratovolcanoes) = &self.stratovolcanoes {
            stratovolcanoes.shape(dir, &base_at, &mut shaped);
        }
        if let Some(landforms) = &self.landforms {
            landforms.shape(dir, &mut shaped);
        } else if let Some(tracks) = &self.tracks {
            for &i in self.crater_bins.get(&cell).into_iter().flatten() {
                let vent = &tracks.active_vents()[i];
                let distance = (dir - vent.dir).length() * atlas::RADIUS_M;
                let heat = 1.0 - super::smooth(0.0, vent.radius_m, distance);
                shaped.heat = shaped.heat.max(heat);
                let crater = distance / vent.crater_radius_m();
                if crater >= 1.0 { continue; }
                let head = base_at(vent.dir) - 11.0;
                shaped.metres -= 24.0 * (1.0 - crater * crater).powi(2);
                let wall = 96.0 * crater * (1.0 - crater * crater) / vent.crater_radius_m();
                shaped.slope = shaped.slope.max(wall);
                if head > SEA && shaped.metres < head {
                    shaped.lake = Some((head, BlockId::Lava));
                }
            }
        }
        if let Some(material) = self.reef_material(dir, shaped.metres - 0.05) {
            shaped.crust = Some(material);
        }
        // Fit compressed planetary relief below the physical world ceiling
        // without cutting high cones into flat horizontal caps.
        let roof = |h: f64| if h <= 450.0 { h } else { 450.0 + 45.0 * (1.0 - (-(h - 450.0) / 45.0).exp()) };
        shaped.metres = roof(shaped.metres);
        shaped.lake = shaped.lake.map(|(h, fluid)| (roof(h), fluid));
        shaped
    }
}
