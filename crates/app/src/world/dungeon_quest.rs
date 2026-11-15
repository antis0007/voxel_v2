//! Dungeon quest runtime (generator 294): dungeon-bound keys, locked doors and
//! the entry lectern. The plan lives in `engine_worldgen::dungeon::quest_at`;
//! keys held per complex live in the `block_data` sidecar (`KeyRing`), so the
//! save body is unchanged. Opened doors are ordinary voxel edits.
use super::block_data::KeyRing;
use super::{BlockId, World};
use crate::audio::{Cue, Event};
use engine_worldgen::dungeon::{QuestKey, quest_at};
use glam::{IVec3, Vec3};
use std::collections::HashSet;

/// Door cells one unlock may clear; a quest door is a corridor-wide slab.
const DOOR_CELLS: usize = 1024;

pub fn is_door(b: BlockId) -> bool {
    matches!(b, BlockId::LockedDoor | BlockId::BossDoor)
}

fn centre(world: &World, p: IVec3) -> Vec3 {
    (p.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters()
}

fn chime(world: &mut World, cue: Cue, at: Vec3, pitch: f32) {
    world.adventure.sounds.push(Event { cue, pos: Some(at), gain: 0.9, pitch });
}

/// First opening of a key room's cache hands over its key, once per room.
pub fn on_cache_open(world: &mut World, p: IVec3) {
    let at = centre(world, p);
    let Some(q) = quest_at(&world.sample_config(), at) else { return };
    let Some((room, kind)) = q.key_here else { return };
    let ring = world.block_data.dungeon_keys.entry(q.id).or_default();
    let bit = 1u64 << (room as u32).min(63);
    if ring.looted & bit != 0 {
        return;
    }
    ring.looted |= bit;
    let message = match kind {
        QuestKey::Small => {
            ring.small = ring.small.saturating_add(1);
            format!("Small Key: {}", ring.small)
        }
        QuestKey::Great => {
            ring.great = true;
            "Great Key".to_string()
        }
    };
    world.expedition.say(message);
    chime(world, Cue::Treasure, at, if kind == QuestKey::Great { 0.8 } else { 1.15 });
}

/// Use on a quest door: spend a key and clear the connected door slab.
pub fn try_open(world: &mut World, p: IVec3, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> bool {
    let door = world.get(p);
    if !is_door(door) {
        return false;
    }
    let at = centre(world, p);
    let Some(q) = quest_at(&world.sample_config(), at) else {
        world.expedition.say(format!("{}: Sealed", door.display_name()));
        return true;
    };
    let ring = world.block_data.dungeon_keys.get(&q.id).copied().unwrap_or_default();
    let ok = match door {
        BlockId::LockedDoor => ring.small > 0,
        _ => ring.great,
    };
    if !ok {
        world.expedition.say(match door {
            BlockId::LockedDoor => "Locked Door: Requires Small Key",
            _ => "Great Door: Requires Great Key",
        });
        chime(world, Cue::Clang, at, 0.6);
        return true;
    }
    let cells = slab(world, p, door);
    let edits: Vec<(IVec3, BlockId)> = cells.iter().map(|&c| (c, BlockId::Air)).collect();
    world.set_many_mechanism(&edits);
    let opened: Vec<IVec3> = cells.into_iter().filter(|&c| world.get(c) == BlockId::Air).collect();
    if opened.is_empty() {
        return true;
    }
    for &c in &opened {
        blocks.insert(c);
        chunks.insert(super::chunk_coord_from_world_block(c));
    }
    let entry = world.block_data.dungeon_keys.entry(q.id).or_insert(KeyRing::default());
    if door == BlockId::LockedDoor {
        entry.small = entry.small.saturating_sub(1);
    }
    let left = entry.small;
    chime(world, Cue::ForgeSlam, at, if door == BlockId::BossDoor { 0.45 } else { 0.7 });
    world.expedition.say(match door {
        BlockId::LockedDoor => format!("Locked Door: Open · Small Key: {left}"),
        _ => "Great Door: Open".to_string(),
    });
    true
}

/// Face-connected cells of the same door block around `p`, bounded.
fn slab(world: &World, p: IVec3, door: BlockId) -> Vec<IVec3> {
    let mut seen = HashSet::from([p]);
    let mut stack = vec![p];
    let mut out = Vec::new();
    while let Some(c) = stack.pop() {
        out.push(c);
        if out.len() >= DOOR_CELLS {
            break;
        }
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
            let n = c + d;
            if !seen.contains(&n) && world.get(n) == door {
                seen.insert(n);
                stack.push(n);
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
enum When {
    Always,
    SmallLocks,
    GreatLock,
    Mechanisms,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Line {
    when: When,
    text: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Hints {
    lines: Vec<Line>,
}

const HINTS_RON: &str = include_str!("../../../../assets/dungeon_hints.ron");
static HINTS: std::sync::LazyLock<Hints> = std::sync::LazyLock::new(|| {
    ron::from_str(HINTS_RON).unwrap_or_else(|error| panic!("assets/dungeon_hints.ron: {error}"))
});

/// Lectern text for an unwritten lectern inside a quest dungeon.
pub fn lectern_lines(world: &World, p: IVec3) -> Option<Vec<String>> {
    let q = quest_at(&world.sample_config(), centre(world, p))?;
    let ring = world.block_data.dungeon_keys.get(&q.id).copied().unwrap_or_default();
    let lines = HINTS
        .lines
        .iter()
        .filter(|l| match l.when {
            When::Always => true,
            When::SmallLocks => q.small_locks > 0,
            When::GreatLock => q.great_lock,
            When::Mechanisms => engine_worldgen::features::legacy_preview(world.generator_version, engine_voxel::DUNGEON_MECHANISMS_GENERATOR),
        })
        .map(|l| {
            l.text
                .replace("{small}", &q.small_locks.to_string())
                .replace("{held}", &ring.small.to_string())
                .replace("{great}", &u8::from(q.great_lock).to_string())
        })
        .collect();
    Some(lines)
}
