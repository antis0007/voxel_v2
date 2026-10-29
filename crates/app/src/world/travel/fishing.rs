//! Rod casts: a hashed wait, a bite window, and a weighted catch from `assets/fishing.ron`.
use super::{ROD, World, water_along};
use crate::world::inventory::{DroppedStack, Item, ItemStack};
use glam::{IVec3, Vec3};

const SECOND: u64 = 120;
const CAST_METRES: f32 = 16.0;
const LINE_METRES: f32 = 24.0;
pub const BITE_WINDOW: u64 = 3 * SECOND / 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cast {
    pub cell: IVec3,
    pub slot: usize,
    /// Step of the next bite.
    pub bite: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Catch {
    pub item: Item,
    pub count: u32,
    pub weight: u32,
}

const FISHING_RON: &str = include_str!("../../../../../assets/fishing.ron");

static CATCHES: std::sync::LazyLock<Vec<Catch>> = std::sync::LazyLock::new(|| {
    ron::from_str(FISHING_RON).unwrap_or_else(|error| {
        eprintln!("[fishing] assets/fishing.ron: {error}");
        Vec::new()
    })
});

fn roll(seed: u32, cell: IVec3, step: u64, salt: u32) -> u32 {
    engine_core::hash::hash_u32(seed ^ (cell.x as u32).wrapping_mul(0x9e37_79b9) ^ (cell.y as u32).wrapping_mul(0x85eb_ca6b)
        ^ (cell.z as u32).wrapping_mul(0xc2b2_ae35) ^ (step as u32).wrapping_mul(0x27d4_eb2f) ^ ((step >> 32) as u32) ^ salt)
}

/// Steps until the next bite: 5 to 20 seconds.
pub fn wait(seed: u32, cell: IVec3, now: u64) -> u64 {
    5 * SECOND + u64::from(roll(seed, cell, now, 0x5157)) % (15 * SECOND)
}

/// The catch for a bite at `bite`; the same inputs always give the same row.
pub fn catch(seed: u32, cell: IVec3, bite: u64) -> Option<Catch> {
    let total: u32 = CATCHES.iter().map(|c| c.weight).sum();
    if total == 0 {
        return None;
    }
    let mut pick = roll(seed, cell, bite, 0xf15) % total;
    CATCHES.iter().copied().find(|c| {
        if pick < c.weight {
            return true;
        }
        pick -= c.weight;
        false
    })
}

pub(super) fn use_rod(world: &mut World, eye_m: Vec3, forward: Vec3, slot: usize) {
    let clock = world.farming.clock;
    if let Some(cast) = world.travel.cast.take() {
        let landed = (cast.bite..cast.bite + BITE_WINDOW).contains(&clock);
        match landed.then(|| catch(world.seed, cast.cell, cast.bite)).flatten() {
            Some(c) => {
                let (_, left) = world.inventory.try_insert(ItemStack::new(c.item, c.count));
                if left > 0 {
                    world.inventory.dropped.push(DroppedStack { pos: eye_m, stack: ItemStack::new(c.item, left) });
                }
                world.sync_from_inventory();
                world.expedition.say(format!("{} × {}", c.item.name(), c.count));
            }
            None => world.expedition.say("Line In"),
        }
        return;
    }
    match water_along(world, eye_m, forward, CAST_METRES) {
        Some(cell) => {
            let bite = clock + wait(world.seed, cell, clock);
            world.travel.cast = Some(Cast { cell, slot, bite });
            world.expedition.say("Line Cast");
        }
        None => world.expedition.say("No Water in Range"),
    }
}

/// Bite cue, missed bites and dropped lines.
pub(super) fn tick(world: &mut World, feet: Vec3) {
    let Some(mut cast) = world.travel.cast else { return };
    let s = world.voxel_scale.meters();
    let held = world.inventory.hotbar.get(cast.slot).copied().flatten().map(|s| s.item);
    if held != Some(ROD) || ((cast.cell.as_vec3() + Vec3::splat(0.5)) * s).distance(feet) > LINE_METRES
        || world.get(cast.cell) != crate::world::BlockId::Water
    {
        world.travel.cast = None;
        return;
    }
    let clock = world.farming.clock;
    if clock == cast.bite {
        world.expedition.say("Bite");
    } else if clock >= cast.bite + BITE_WINDOW {
        cast.bite = clock + wait(world.seed, cast.cell, clock);
        world.travel.cast = Some(cast);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fishing_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*CATCHES, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = FISHING_RON.replace('\r', "");
        let body = file.find("\n[\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end(), "rerun dump_fishing_ron");
        assert!(CATCHES.iter().all(|c| c.weight > 0 && c.count > 0));
    }

    #[test]
    #[ignore = "writes assets/fishing.ron"]
    fn dump_fishing_ron() {
        let header: String = FISHING_RON.lines().take_while(|l| l.starts_with("//")).map(|l| format!("{l}\n")).collect();
        let text = ron::ser::to_string_pretty(&*CATCHES, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fishing.ron");
        std::fs::write(path, format!("{header}{text}\n")).unwrap();
    }

    #[test]
    fn fishing_catch_and_wait_are_deterministic() {
        let cell = IVec3::new(4, 9, -7);
        assert_eq!(catch(77, cell, 1200), catch(77, cell, 1200));
        assert_eq!(wait(77, cell, 50), wait(77, cell, 50));
        let waits: Vec<u64> = (0..64).map(|t| wait(77, cell, t * 997)).collect();
        assert!(waits.iter().all(|w| (5 * SECOND..20 * SECOND).contains(w)));
        let fish = (0..400u64).filter(|&t| catch(77, cell, t * 131).is_some_and(|c| c.item == Item::Food(crate::world::inventory::FoodKind::RawFish))).count();
        assert!((160..320).contains(&fish), "fish share follows the weights: {fish}");
    }
}
