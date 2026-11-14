//! Metre-scale colonies: authored Blender plants plus cheap procedural blade
//! clumps. Supporting voxels own placement; edits, chunk reloads and signed
//! boundaries keep the same rules. Generated trees steer root zones and shade.
use super::{ChunkMesh, GeneratedChunk, Vertex3};
use crate::world::BlockId as B;
use glam::{IVec3, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};
mod blades;
mod clumps;
mod ecology;
mod library;
mod stone;
use blades::Blades;
use ecology::Column;
use library::Species;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Plant(Species),
    Clump(Blades),
    Stone,
}

pub(super) struct Detail {
    p: Vec3,
    hash: u32,
    kind: Kind,
    color: [f32; 3],
    pigment: f32,
    growth: f32,
    /// Straw tint of a blade clump's tips.
    dry: f32,
}
impl Detail {
    fn variant(&self) -> usize {
        ((self.hash >> 9) & 1) as usize
    }
    fn size(&self) -> f32 {
        self.growth * (0.88 + ((self.hash >> 2) & 255) as f32 / 255.0 * 0.24)
    }
    fn vertices(&self) -> usize {
        match self.kind {
            Kind::Plant(s) => library::mesh(s, self.variant(), 0).vertices(),
            Kind::Clump(b) => b.vertices(),
            Kind::Stone => stone::VERTICES,
        }
    }
    fn indices(&self) -> usize {
        match self.kind {
            Kind::Plant(s) => library::mesh(s, self.variant(), 0).indices(),
            Kind::Clump(b) => b.indices(),
            Kind::Stone => stone::INDICES,
        }
    }
}
pub(super) struct DetailPlan {
    items: Vec<Detail>,
}

fn hash(p: IVec3) -> u32 {
    let mut h = (p.x as u32).wrapping_mul(0x9e3779b9)
        ^ (p.z as u32).wrapping_mul(0x85ebca6b)
        ^ (p.y as u32).wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb352d);
    h ^ (h >> 15)
}
fn rehash(h: u32, salt: u32) -> u32 {
    hash(IVec3::new(h as i32, salt as i32, 0x2c1b_3c6d))
}
const TUFT_KIND: f32 = crate::material::terrain_family::TUFT as f32;
const STONE_KIND: f32 = crate::material::terrain_family::PEBBLE as f32;
// Bound actual geometry, not plant count: detailed colonies cannot overwhelm
// the existing mesh admission budget. Authored plants and blade clumps have
// separate caps so cheap turf never crowds out flowers and ferns.
const MAX_INDICES: usize = 12_000 * 3;
const MAX_CLUMP_INDICES: usize = 10_000 * 3;

/// Only use the exact one-cell halo shared by neighboring mesh owners. This
/// makes cover and disturbance independent of which chunks are resident.
/// The fourth value counts sides that rise a step: terrace risers and banks.
fn habitat(chunk: &GeneratedChunk, p: IVec3) -> (f32, f32, f32, f32) {
    let mut cover: f32 = 0.0;
    let mut wet: f32 = 0.0;
    let mut disturbance: f32 = 0.0;
    let mut steps: f32 = 0.0;
    for offset in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z] {
        let side = chunk.get_world(p + offset);
        let above = chunk.get_world(p + offset + IVec3::Y);
        if matches!(side, B::Log | B::Leaves | B::PineLeaves | B::Bush)
            || matches!(above, B::Log | B::Leaves | B::PineLeaves | B::Bush)
        {
            cover += 0.25;
        }
        if side == B::Water || above == B::Water {
            wet += 0.35;
        }
        if matches!(
            side,
            B::Gravel | B::Cobblestone | B::Planks | B::StoneBricks
        ) || above.is_opaque()
        {
            disturbance += 0.20;
        }
        if above.is_opaque() || !side.is_opaque() {
            steps += 0.25;
        }
    }
    (cover.min(1.0), wet.min(1.0), disturbance.min(0.8), steps)
}

fn species(substrate: B, cover: f32, wet: f32, colony: f32, hash: u32) -> Species {
    if substrate == B::FungalSoil || (cover > 0.45 && hash & 3 == 0) {
        Species::Mushroom
    } else if cover > 0.20 || wet > 0.3 {
        Species::Fern
    } else if colony > 0.62 && hash & 3 == 1 {
        Species::Flower
    } else if colony < 0.37 || (colony > 0.62 && hash & 3 == 2) {
        Species::Clover
    } else {
        Species::Grass
    }
}

fn habitat_species(
    climate: engine_worldgen::landscape::Landscape,
    shade: f32,
    wet: f32,
    colony: f32,
    hash: u32,
) -> Species {
    let moisture = (climate.moisture + 0.5 * wet + 0.25 * climate.riparian).min(1.0);
    let sun = 1.0 - shade;
    let temperate = (1.0 - 3.2 * (climate.temperature - 0.52).abs()).max(0.0);
    let damp = ((moisture - 0.43) / 0.35).clamp(0.0, 1.0);
    let weights = [
        (Species::Grass, 0.15 + sun * (1.0 - 0.65 * damp)),
        (Species::Clover, sun * temperate * (1.0 - colony) * (1.0 - wet)),
        (Species::Flower, 1.8 * sun * temperate * colony.powi(2) * (1.0 - wet)),
        (Species::Fern, 2.4 * damp * shade.max(wet)),
        (Species::Mushroom, 1.3 * damp * shade.powi(2) * colony),
    ];
    let total: f32 = weights.iter().map(|&(_, weight)| weight).sum();
    let mut roll = (hash & 65535) as f32 / 65536.0 * total;
    for (species, weight) in weights {
        if roll < weight {
            return species;
        }
        roll -= weight;
    }
    Species::Grass
}

fn plant_tint(biome: engine_worldgen::landscape::Biome) -> [f32; 3] {
    use engine_worldgen::landscape::Biome::*;
    match biome {
        Marsh | Fen | CycadMarsh => [0.87, 1.04, 0.92],
        Rainforest | CloudForest | RedwoodGrove => [0.82, 1.04, 0.97],
        Taiga | Tundra | Alpine => [0.90, 0.96, 1.12],
        Savanna | Heathland | Badlands => [1.12, 0.94, 0.81],
        _ => [1.0; 3],
    }
}

fn fern_colony(p: IVec3, scale: f32, column: &Column, wet: f32, disturbance: f32, lush: bool) -> bool {
    let shade = if lush { column.rainforest.max(column.shade * 0.80) } else { column.rainforest };
    if shade < if lush { 0.34 } else { 0.25 } || column.root > 0.55 || disturbance >= 0.40 {
        return false;
    }
    let habitat = 0.55 * column.meadow + 0.30 * column.local + 0.15 * shade;
    if habitat + 0.18 * wet < 0.44 {
        return false;
    }
    // One broad fern per two-metre square. Choose the anchor in world space
    // so fine voxels and neighboring chunk owners share the spacing.
    plant_anchor(p, scale, 2.0, 166_067)
}

fn plant_anchor(p: IVec3, scale: f32, spacing: f32, salt: i32) -> bool {
    let span = (spacing / scale).round().max(1.0) as i32;
    let (x, z) = (p.x.div_euclid(span), p.z.div_euclid(span));
    let anchor = hash(IVec3::new(x, salt, z));
    // Keep centres in the middle half of each square so colonies do not
    // place two broad fronds against opposite sides of the same boundary.
    let offset_x = (128 + (anchor & 255) as i32) * span / 512;
    let offset_z = (128 + ((anchor >> 8) & 255) as i32) * span / 512;
    p.x.rem_euclid(span) == offset_x && p.z.rem_euclid(span) == offset_z
}

impl DetailPlan {
    pub fn empty_plan() -> Self {
        Self { items: Vec::new() }
    }
    pub fn new(chunk: &GeneratedChunk, cancel: &AtomicBool) -> Option<Self> {
        let mut items = Vec::new();
        let mut clumps = Vec::new();
        let scale = chunk.voxel_meters;
        let trees = ecology::local_trees(chunk);
        let rainforest = trees.iter().any(|tree| tree.rainforest);
        let lush = chunk.generator_version >= engine_voxel::CANOPY_UNDERSTORY_GENERATOR;
        let watershed = chunk.flora_config.as_ref().is_some_and(|cfg| {
            cfg.is_planet() && cfg.generator_version == engine_voxel::WATERSHED_EROSION_GENERATOR
        });
        for z in 0..32 {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            for x in 0..32 {
                let roof = chunk
                    .sky_ceiling
                    .get(((z + 1) * 34 + x + 1) as usize)
                    .copied();
                let max_y = roof.map_or(32, |h| (h - chunk.chunk.y * 32 + 1).clamp(0, 32));
                for y in (0..max_y).rev() {
                    let p = chunk.chunk * 32 + IVec3::new(x, y, z);
                    let b = chunk.get_world(p);
                    let cold_turf = watershed && b == B::SnowGrass;
                    let growing = matches!(b, B::Grass | B::FungalSoil);
                    let forest_moss = (rainforest || lush) && b == B::GroveMoss;
                    let wet_soil = lush && matches!(b, B::Mud | B::Sand);
                    if !growing && !forest_moss && !wet_soil && !cold_turf
                        && !matches!(b, B::Gravel | B::Stone | B::Sand | B::Dirt | B::Limestone)
                    {
                        continue;
                    }
                    // One surface per column. A cover removes this plant even
                    // when the owning surface lies on the upper chunk boundary.
                    if chunk.get_world(p + IVec3::Y) != B::Air {
                        break;
                    }
                    let h = hash(p);
                    let world = p.as_vec3() * scale;
                    let (cover, wet, disturbance, steps) = habitat(chunk, p);
                    let column = Column::at(world.x, world.z, &trees, world.y + scale);
                    let canopy = cover.max(column.shade * 0.6);
                    let fern = (b == B::Grass || forest_moss)
                        && fern_colony(p, scale, &column, wet, disturbance, lush);
                    let sedge = wet_soil && wet >= 0.35 && disturbance < 0.4
                        && column.meadow > 0.35 && plant_anchor(p, scale, 1.5, 194_071);
                    let cold_tuft = cold_turf && column.sward > 0.58 && column.bare < 0.5
                        && column.root < 0.55 && disturbance < 0.4
                        && plant_anchor(p, scale, 1.5, 86_243);
                    if b == B::Grass {
                        clumps::plan(chunk, &mut clumps, p, h, &column, wet, disturbance, steps);
                    }
                    let density = if growing {
                        // A second, finer field breaks large blobs into related
                        // colonies. Quiet gaps survive between their centers.
                        (0.10 + 1.6 * column.meadow.powi(2))
                            * (0.4 + column.local)
                            * (1.0 - disturbance)
                            * (1.0 - column.bare)
                            * (1.0 - 0.7 * column.root)
                    } else if b == B::Sand {
                        0.035
                    } else {
                        0.11
                    };
                    if !fern && !sedge && !cold_tuft
                        && (forest_moss || cold_turf || b == B::Mud || (h & 65535) as f32 / 65535.0 > density * scale * scale) {
                        break;
                    }
                    let top = p.as_vec3()
                        + Vec3::new(
                            0.22 + ((h >> 16) & 255) as f32 / 455.0,
                            1.0,
                            0.22 + ((h >> 24) & 255) as f32 / 455.0,
                        );
                    if !fern && b != B::FungalSoil && chunk.sky_visibility(top, Vec3::Y) < 0.18 {
                        break;
                    }
                    let mut tint = [1.0; 3];
                    let kind = if cold_tuft {
                        Kind::Clump(blades::Blades::Bent)
                    } else if sedge {
                        Kind::Clump(blades::Blades::SeedHead)
                    } else if fern {
                        Kind::Plant(Species::Fern)
                    } else if growing && !(h >> 12).is_multiple_of(19) {
                        let plant = if b == B::FungalSoil {
                            Species::Mushroom
                        } else if let Some(cfg) = &chunk.flora_config {
                            let climate = engine_worldgen::landscape::sample(cfg, p.x, p.z);
                            if watershed {
                                tint = plant_tint(climate.biome);
                            }
                            habitat_species(climate, canopy, wet, column.colony, rehash(h, cfg.seed))
                        } else {
                            species(b, canopy, wet, column.colony, h)
                        };
                        Kind::Plant(plant)
                    } else {
                        Kind::Stone
                    };
                    let color = if matches!(kind, Kind::Plant(_)) {
                        tint
                    } else if growing {
                        [0.29, 0.27, 0.22]
                    } else {
                        b.color(p)
                    };
                    items.push(Detail {
                        p: top,
                        hash: h,
                        kind,
                        color,
                        pigment: 0.96 + ((h >> 8) & 255) as f32 / 255.0 * 0.08,
                        growth: if cold_tuft { 0.7 + column.local * 0.45 }
                            else if sedge { 1.5 + column.local * 0.6 }
                            else if fern && lush { 2.35 + column.local * 1.20 }
                            else if fern { 1.9 + column.local * 0.9 }
                            else { 1.0 },
                        dry: if cold_tuft { 0.65 + column.dry * 0.35 } else { 0.0 },
                    });
                    break;
                }
            }
        }
        cap(&mut items, MAX_INDICES);
        cap(&mut clumps, MAX_CLUMP_INDICES);
        items.append(&mut clumps);
        Some(Self { items })
    }
    pub fn empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn vertices(&self) -> usize {
        self.items.iter().map(Detail::vertices).sum()
    }
    pub fn indices(&self) -> usize {
        self.items.iter().map(Detail::indices).sum()
    }
    pub fn append(&self, chunk: &GeneratedChunk, mesh: &mut ChunkMesh, lod: u8) {
        if lod > 1 {
            return;
        }
        for d in &self.items {
            match d.kind {
                Kind::Plant(_) => library::append(mesh, d, chunk.voxel_meters, lod),
                // Thin blades shimmer past the near band: keep a third of the
                // tall forms there so the switch thins cover instead of clearing it.
                Kind::Clump(b) if lod == 0 || (b.keeps_at_lod1() && (d.hash >> 28) % 3 == 0) => {
                    blades::append(mesh, d, b, d.dry, chunk.voxel_meters)
                }
                Kind::Clump(_) => {}
                Kind::Stone => {
                    let size = (0.065 + (d.hash & 255) as f32 / 255.0 * 0.10) / chunk.voxel_meters;
                    stone::append(mesh, d, chunk.voxel_meters, size);
                }
            }
        }
    }
}

/// Keep a hash-ordered prefix within `limit` indices, so which items survive
/// does not depend on scan order.
fn cap(items: &mut Vec<Detail>, limit: usize) {
    items.sort_unstable_by_key(|d| d.hash);
    let mut indices = 0;
    items.retain(|d| {
        let n = d.indices();
        if indices + n > limit {
            false
        } else {
            indices += n;
            true
        }
    });
}

#[cfg(test)]
mod tests;
