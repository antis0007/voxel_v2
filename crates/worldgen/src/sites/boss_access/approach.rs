//! The shared front of every boss complex: the puzzle courtyard, the sealed
//! entry hall, a bridge over a chasm (with stairs through the chasm as the
//! fallback), and the three treasure wings beside the arena.
//!
//! Shrine-local metres (`u` across, `y` up, `w` away from the ruin):
//! courtyard `w -9..4`, entry hall `w -4..4`, bridge hall `w 4..10`,
//! arena `w 10..29`.
use super::layout::*;
use super::plan::{Builder, Fill, mirror};
use super::{B, Box3};

/// Mechanisms, doors and walk-critical stairs. Pushed first, never dressed.
pub(super) fn mechanisms(b: &mut Builder, f: Family, seed: u32) {
    b.dress = false;
    b.mirror = false;
    b.put(DOOR, f.door_block());
    b.put(WEAK_WALL, B::WeatheredBricks);
    b.put([-1, 1, 1, 3, -6, -4], B::Air);
    match f {
        Family::Foundry => {
            b.put(HOPPER_TORCH, B::Torch);
            for c in HOPPER_COAL {
                b.put(c, B::CoalOre);
            }
            b.put(HOPPER_SOCKET, B::Air);
            b.put(HOPPER, B::Basalt);
        }
        Family::Toll => {
            b.put(LEVER_PIN, B::Log);
            b.put(LEVER_TORCH, B::Torch);
            b.put(LEVER_BRACKET, B::StoneBricks);
        }
        Family::Archive => {
            for c in WALL_CRYSTALS {
                b.put(c, B::Crystal);
            }
            for c in BRAZIER_SOCKETS {
                b.put(c, B::Air);
            }
            for c in BRAZIERS {
                b.put(c, B::CopperBlock);
            }
        }
        Family::Hart => {
            for c in PRISMS {
                b.put(c, B::Crystal);
            }
            // Grown ledges up the hall's left flank: one-metre rises to the roof.
            for i in 1..=5 {
                b.put([-5, -4, 1, 1 + i, i - 4, i - 3], B::MushroomStem);
            }
        }
    }
    // Bridge over the chasm in the bridge hall. Some sites lose a span; the
    // stairs down through the chasm always remain.
    let deck = match f {
        Family::Foundry => B::BlastBrick,
        Family::Toll => B::StoneBricks,
        Family::Archive => B::CopperBlock,
        Family::Hart => B::MushroomStem,
    };
    let gap = (seed >> 3) % 3;
    if gap > 0 {
        b.put([-1, 1, 0, 1, 7, 7 + gap as i32], B::Air);
    }
    b.put([-1, 1, 0, 1, 4, 10], deck);
    b.put([-1, 1, -2, 0, 5, 6], f.wall());
    b.put([-1, 1, -2, 0, 9, 10], f.wall());
    for (w, top) in [(4, 0), (5, -1), (8, -1), (9, 0)] {
        b.put([-3, -1, -2, top, w, w + 1], B::StoneBricks);
    }
}

/// The three treasure rooms, mirrored with the arena.
pub(super) fn treasures(f: Family, mirrored: bool) -> [Treasure; 3] {
    let m = |x: Box3| if mirrored { mirror(x) } else { x };
    let room = |index: u8, r: [Box3; 5], opens| Treasure {
        index,
        tier: index + 1,
        seal: m(r[0]),
        pedestal: m(r[1]),
        cache: m(r[2]),
        opens,
    };
    [
        room(0, HIDDEN, Opens::Cracked),
        room(1, LEVER_ROOM, Opens::Lever(m(f.lever()))),
        room(2, VAULT_ROOM, Opens::Boss),
    ]
}

/// Treasure seals, pedestals, empty caches and the lever pin. Never dressed.
pub(super) fn treasure_fixtures(b: &mut Builder, f: Family, mirrored: bool) {
    b.dress = false;
    b.mirror = false;
    for t in treasures(f, mirrored) {
        let seal = match t.opens {
            Opens::Cracked => B::WeatheredBricks,
            Opens::Lever(pin) => {
                b.put(pin, B::Log);
                f.door_block()
            }
            Opens::Boss => B::VoidStone,
        };
        b.put(t.seal, seal);
        b.put(t.pedestal, B::VoidStone);
        b.put(t.cache, B::Air);
    }
    b.put(CLEAR, B::Air);
}

/// Treasure room interiors and their solid wings (mirrored, dressed).
pub(super) fn wings(b: &mut Builder, f: Family) {
    b.dress = true;
    for (r, floor) in [(HIDDEN, B::Gravel), (LEVER_ROOM, B::StoneBricks), (VAULT_ROOM, B::Basalt)] {
        let room = r[3];
        // A torch in the far corner of each room, standing on its floor.
        b.put([room[1] - 1, room[1], 1, 2, room[5] - 1, room[5]], B::Torch);
        b.put(room, B::Air);
        b.put([r[4][0], r[4][1], 0, 1, r[4][4], r[4][5]], floor);
        b.put(r[4], f.wall());
    }
}

/// Entry hall, bridge hall, courtyard and plaza (unmirrored, dressed).
pub(super) fn shells(b: &mut Builder, f: Family) {
    b.mirror = false;
    b.dress = true;
    // Entry hall with an arch through to the bridge hall.
    b.put([-3, -2, 1, 2, 2, 3], B::Torch);
    b.put([2, 3, 1, 2, 2, 3], B::Torch);
    b.put([-2, 2, 1, 4, 3, 4], B::Air);
    b.put(MAIN_ROOM, B::Air);
    b.put(MAIN, f.wall());
    // Bridge hall: the chasm floor walks at -2, the bridge deck at 1.
    if f == Family::Toll {
        b.put([1, 3, -2, -1, 4, 10], B::Water);
    }
    b.put([-3, 3, -2, 5, 4, 10], B::Air);
    b.put([-4, 4, -4, 6, 4, 10], Fill::from(f.wall()));
    // Courtyard in front of the door, with alleys down both flanks.
    b.put([-8, 8, 1, 9, -9, 4], B::Air);
    b.put([-8, 8, 0, 1, -9, 4], B::StoneBricks);
    b.put([-8, 8, -3, 0, -9, 4], B::Cobblestone);
}
