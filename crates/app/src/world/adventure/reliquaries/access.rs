//! Generator 32 boss complexes. The mechanism, door and vault state are read
//! from world blocks, which the save already persists, so no record bit or
//! save field is added. Digging through any wall stays a valid route.
use super::*;
use crate::world::BlockId;
use engine_core::hash::hash_u32;
use engine_worldgen::sites::boss_access::{self as access, Family, Opens, Rule, Shrine, Treasure};

fn cell_solves(rule: Rule, b: BlockId) -> bool {
    match rule {
        Rule::Fill(ok) => ok.contains(&b) || (b.is_torch() && ok.contains(&BlockId::Torch)),
        Rule::Break(generated) => b != generated,
    }
}

/// Every key box has at least one cell in its solved state.
pub(super) fn keys_solved(world: &World, shrine: &Shrine) -> bool {
    let (keys, rule) = shrine.family.keys();
    keys.iter().all(|k| {
        shrine
            .cells(*k)
            .iter()
            .any(|p| cell_solves(rule, world.get(*p)))
    })
}

/// Writes `to` into every cell (optionally only those holding `from`) and
/// returns how many cells now hold `to`. Non-resident or protected cells are
/// skipped by `set_many` and retried on the next pass.
fn replace(world: &mut World, cells: &[IVec3], from: Option<BlockId>, to: BlockId) -> usize {
    let edits: Vec<_> = cells
        .iter()
        .filter(|p| from.is_none_or(|f| world.get(**p) == f))
        .map(|p| (*p, to))
        .collect();
    if edits.is_empty() {
        return 0;
    }
    world.set_many(&edits);
    edits.iter().filter(|(p, _)| world.get(*p) == to).count()
}

/// Once per encounter pass: open a solved shrine door, and grant each treasure
/// room whose seal is solved (cracked wall broken, lever pin pulled, boss down).
pub(super) fn tend(adventure: &mut Adventure, world: &mut World) {
    if world.generator_version < access::VERSION {
        return;
    }
    let cfg = world.sample_config();
    for i in 0..adventure.reliquaries.len() {
        let relic = &adventure.reliquaries[i];
        if bosses::kind(relic.kind) == 0 {
            continue;
        }
        let Some(shrine) = access::shrine(&cfg, relic.site, relic.pos) else {
            continue;
        };
        let state = adventure.relic_state(relic);
        let pos = relic.pos;
        let family = shrine.family;
        let door = shrine.cells(access::DOOR);
        let sealed = door.iter().any(|p| world.get(*p) == family.door_block());
        // The foundry's three valves (the older courtyard puzzle) also vent the door.
        let valves = family == Family::Foundry && state & quest::VALVES == quest::VALVES;
        if sealed {
            let solved = keys_solved(world, &shrine);
            if (solved || valves)
                && replace(world, &door, Some(family.door_block()), BlockId::Air) > 0
            {
                world.expedition.say(if solved {
                    family.opened_message()
                } else {
                    "The valves vent the furnace. Its door grinds open."
                });
                sound(&mut adventure.sounds, Cue::Clang, Some(pos), 0.9);
            }
        }
        for t in shrine.treasures() {
            open_treasure(adventure, world, &shrine, t, state & BOSS_DEFEATED != 0, pos);
        }
    }
}

fn treasure_open(world: &World, shrine: &Shrine, t: &Treasure, boss_down: bool) -> bool {
    match t.opens {
        Opens::Boss => boss_down,
        Opens::Cracked => shrine.cells(t.seal).iter().any(|p| world.get(*p) != BlockId::WeatheredBricks),
        Opens::Lever(pin) => shrine.cells(pin).iter().any(|p| world.get(*p) != BlockId::Log),
    }
}

/// Grants one treasure room once: clears its seal (lever gate or boss ward),
/// fills the cache, and drops a found wand of the room's tier. The pedestal
/// turns to tombstone, which cannot be edited, so the grant never repeats.
fn open_treasure(adventure: &mut Adventure, world: &mut World, shrine: &Shrine, t: Treasure, boss_down: bool, pos: Vec3) {
    let pedestal = shrine.cells(t.pedestal);
    if pedestal.iter().all(|p| world.get(*p) == BlockId::TombStone) || !treasure_open(world, shrine, &t, boss_down) {
        return;
    }
    let family = shrine.family;
    match t.opens {
        Opens::Boss => {
            replace(world, &shrine.cells(t.seal), None, BlockId::Air);
        }
        Opens::Lever(_) => {
            replace(world, &shrine.cells(t.seal), Some(family.door_block()), BlockId::Air);
        }
        Opens::Cracked => {}
    }
    let cache = shrine.cells(t.cache);
    replace(world, &cache, None, family.cache_block());
    if replace(world, &pedestal, None, BlockId::TombStone) != pedestal.len() {
        return;
    }
    world.expedition.say(match t.opens {
        Opens::Boss => "Reward Vault Open",
        Opens::Lever(_) => "Treasure Gate Open",
        Opens::Cracked => "Cache Open",
    });
    sound(&mut adventure.sounds, Cue::Treasure, Some(pos), 0.8);
    let Some(&c) = cache.first() else {
        return;
    };
    let seed = hash_u32(
        (c.x as u32).wrapping_mul(73_856_093)
            ^ (c.y as u32).wrapping_mul(19_349_663)
            ^ (c.z as u32).wrapping_mul(83_492_791)
            ^ u32::from(t.index).wrapping_mul(0x9e37_79b9),
    )
    .max(1);
    if let Some(item) = adventure.mint_wand(super::super::wand_items::generate(seed, u32::from(t.tier))) {
        let v = world.sample_config().voxel_meters();
        world.inventory.dropped.push(crate::world::inventory::DroppedStack {
            pos: (c.as_vec3() + Vec3::splat(0.5)) * v + Vec3::Y,
            stack: crate::world::inventory::ItemStack::new(item, 1),
        });
    }
}

#[cfg(test)]
#[path = "access_tests.rs"]
mod tests;
