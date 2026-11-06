//! Where each creature kind rises: `assets/spawns.ron` rows matched against a
//! spawn context (biome, realm, time). Spawners own the roll and placement;
//! this module owns the table and one deterministic pick.
use super::World;
use crate::world::WorldType;
use engine_worldgen::landscape::Biome;
use glam::IVec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Realm {
    Surface,
    Underground,
    Nether,
    /// Manyfold and Hall shells, where a biome sample means nothing.
    Shell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Time {
    Any,
    Night,
    Day,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Habitat {
    pub weight: u8,
    /// Biome variant names; empty means every biome.
    #[serde(default)]
    pub biomes: Vec<String>,
    pub realms: Vec<Realm>,
    #[serde(default = "any")]
    pub time: Time,
}

fn any() -> Time {
    Time::Any
}

/// A cell under more than this much ground counts as underground.
pub const UNDERGROUND_M: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Context {
    pub biome: Option<Biome>,
    pub realm: Realm,
    pub night: bool,
}

impl Context {
    /// Classifies the column of `cell`. `roofed` is the caller's cover test so
    /// the spawner's scan is not repeated here.
    pub fn at(world: &World, cell: IVec3, night: bool, roofed: bool) -> Self {
        let realm = match world.world_type {
            WorldType::Nether => Realm::Nether,
            WorldType::Manyfold | WorldType::Hall => Realm::Shell,
            _ => {
                let depth = (world.terrain_height_at(cell.x, cell.z) - cell.y) as f32 * world.voxel_scale.meters();
                if roofed && depth > UNDERGROUND_M { Realm::Underground } else { Realm::Surface }
            }
        };
        let biome = match realm {
            Realm::Surface | Realm::Underground => {
                Some(engine_worldgen::landscape::sample(&world.sample_config(), cell.x, cell.z).biome)
            }
            _ => None,
        };
        Self { biome, realm, night }
    }
}

const ROWS_RON: &str = include_str!("../../../../../assets/spawns.ron");
/// `(enemy kind, habitat)` rows in file order; order is part of the roll.
pub static ROWS: std::sync::LazyLock<Vec<(u32, Habitat)>> = std::sync::LazyLock::new(|| {
    ron::from_str(ROWS_RON).unwrap_or_else(|error| panic!("assets/spawns.ron: {error}"))
});

impl Habitat {
    pub fn admits(&self, ctx: &Context) -> bool {
        self.admits_realm_and_time(ctx)
            && (self.biomes.is_empty() || ctx.biome.is_some_and(|b| self.biomes.iter().any(|name| Biome::from_name(name) == Some(b))))
    }

    fn admits_realm_and_time(&self, ctx: &Context) -> bool {
        self.realms.contains(&ctx.realm)
            && match self.time {
                Time::Any => true,
                Time::Night => ctx.night,
                Time::Day => !ctx.night,
            }
    }
}

/// One kind for `roll`, weighted over the rows `ctx` admits and `keep` accepts.
/// Rows are walked in file order so the same roll maps to the same kind.
pub fn pick(ctx: &Context, roll: u32, keep: impl Fn(u32) -> bool) -> Option<u32> {
    pick_rows(roll, ROWS.iter().filter(|(kind, h)| keep(*kind) && h.admits(ctx)))
}

/// `pick` with the biome ignored: realm and time still hold. Grazer patches
/// use it when their biome lists no grazer, so a grass bank in an odd biome
/// still has animals.
pub fn pick_any_biome(ctx: &Context, roll: u32, keep: impl Fn(u32) -> bool) -> Option<u32> {
    pick_rows(roll, ROWS.iter().filter(|(kind, h)| keep(*kind) && h.admits_realm_and_time(ctx)))
}

fn pick_rows<'a>(roll: u32, rows: impl Iterator<Item = &'a (u32, Habitat)> + Clone) -> Option<u32> {
    let total: u32 = rows.clone().map(|(_, h)| u32::from(h.weight)).sum();
    if total == 0 {
        return None;
    }
    let mut roll = roll % total;
    for (kind, h) in rows {
        let weight = u32::from(h.weight);
        if roll < weight {
            return Some(*kind);
        }
        roll -= weight;
    }
    None
}

/// Kinds with at least one row.
pub fn listed(kind: u32) -> bool {
    ROWS.iter().any(|(k, _)| *k == kind)
}

#[cfg(test)]
#[path = "habitat_tests.rs"]
mod tests;
