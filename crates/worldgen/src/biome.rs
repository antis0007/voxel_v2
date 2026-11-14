//! Biome family weights: a column's continuous fields scored against the prototypes in
//! `assets/biomes.ron`. Each axis is a Gaussian tolerance curve, so a family's score is the
//! product of how well every field suits it and borders fall where the fields change.
use crate::landscape::{Biome, TectonicSample, sample_tectonics};
use crate::{WorldSampleConfig, smoothstep};
use engine_content::biomes::{Axis, BiomeTable, FamilyDef, Open, Substrate, biome_table};



/// First generator whose biomes come from these weights.
pub mod report;

pub const BIOME_WEIGHTS_GENERATOR: u32 = engine_voxel::RECOVERED_SYSTEMS_GENERATOR;
fn family(name: &str) -> Biome {
    match name {
        "Meadow" => Biome::Meadow, "Forest" => Biome::Forest,
        "Taiga" => Biome::Taiga, "Tundra" => Biome::Tundra,
        "Alpine" => Biome::Alpine, "Desert" => Biome::Desert,
        "Badlands" => Biome::Badlands, "Savanna" => Biome::Savanna,
        "Rainforest" => Biome::Rainforest, "Heathland" => Biome::Heathland,
        "Fen" => Biome::Fen, "VolcanicField" => Biome::VolcanicField,
        "KarstUpland" => Biome::KarstUpland,
        _ => panic!("assets/biomes.ron: unknown family {name}"),
    }
}

/// What one column offers a family. Rock gates are 0..1 and multiply the score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fields {
    pub temperature: f32,
    pub moisture: f32,
    pub height: f32,
    pub slope: f32,
    pub wetness: f32,
    pub volcanic: f32,
    pub carbonate: f32,
    pub anomaly: f32,
}

/// The two best families and the runner-up's share of their combined score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    pub first: Biome,
    pub second: Biome,
    /// In 0..=0.5.
    pub second_weight: f32,
}

/// Volcanic and carbonate rock gates from the tectonic sample.
pub fn substrate(tect: &TectonicSample) -> (f32, f32) {
    let volcanic = smoothstep(0.40, 0.58, tect.volcanism);
    // Shelf-margin crust, not orogens. This approximates the pre-15 limestone rule; `bedrock_v15`
    // places limestone from its own province noise, so karst and bedrock limestone can disagree
    // until phase 5 couples surface and column.
    let carbonate =
        (1.0 - smoothstep(0.10, 0.30, tect.continentalness)) * (1.0 - tect.orogeny.clamp(0.0, 1.0));
    (volcanic, carbonate)
}

fn distance_sq(axis: Option<&Axis>, value: f32) -> f32 {
    axis.map_or(0.0, |a| {
        let d = (value - a.centre) / a.width;
        let d = match a.open {
            Open::Neither => d,
            Open::Above => d.min(0.0),
            Open::Below => d.max(0.0),
        };
        d * d
    })
}

/// Log of the family's score; `-inf` when its rock is absent.
fn log_score(def: &FamilyDef, f: &Fields) -> f32 {
    let gate = match def.requires {
        None => 1.0,
        Some(Substrate::Volcanic) => f.volcanic,
        Some(Substrate::Carbonate) => f.carbonate,
        Some(Substrate::Anomaly) => f.anomaly,
    };
    if gate <= 0.0 {
        return f32::NEG_INFINITY;
    }
    let d2 = distance_sq(Some(&def.temperature), f.temperature)
        + distance_sq(Some(&def.moisture), f.moisture)
        + distance_sq(def.elevation.as_ref(), f.height)
        + distance_sq(def.slope.as_ref(), f.slope)
        + distance_sq(def.wetness.as_ref(), f.wetness);
    def.bias - d2 + gate.ln()
}

/// Ranks families by log score, so far-off fields never underflow to an empty top.
/// Names identify families independently of row order. Ungated families always
/// have a finite score, so a column without matching rock still gets a family.
pub fn weights(table: &BiomeTable, f: &Fields) -> Weights {
    let mut best = [(f32::NEG_INFINITY, Biome::Meadow); 2];
    for def in &table.families {
        let score = log_score(def, f);
        // Strict comparisons keep the earlier row on ties.
        if score > best[0].0 {
            best[1] = best[0];
            best[0] = (score, family(&def.name));
        } else if score > best[1].0 {
            best[1] = (score, family(&def.name));
        }
    }
    let second_weight = 1.0 / (1.0 + (best[0].0 - best[1].0).exp());
    Weights { first: best[0].1, second: best[1].1, second_weight }
}

/// Weights for one column: `f` carries the climate and terrain fields, the rock gates come
/// from the tectonic sample at `(wx, wz)`. Keep the sample call identical to the one
/// `column_from_landscape` makes, or the rock gates describe different plates than the terrain.
pub(crate) fn weights_at(cfg: &WorldSampleConfig, wx: f32, wz: f32, f: Fields) -> Weights {
    // Same plates as the terrain, including the landward rule from generator 53.
    let tect = sample_tectonics(wx, wz, cfg.seed, cfg.generator_version);
    let (volcanic, carbonate) = substrate(&tect);
    weights(biome_table(), &Fields { volcanic, carbonate, ..f })
}

