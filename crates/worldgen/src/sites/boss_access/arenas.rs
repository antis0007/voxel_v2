//! Per-family boss arenas. Each complex is assembled from seeded variants
//! (mirroring, roof height, pillar damage, bridge spans, channel layout), so
//! no two sites read the same. Arena interior: `u -11..11`, `w 11..27`.
use super::approach;
use super::layout::*;
use super::plan::{Builder, Fill, Plan};
use super::B;
use crate::sites::modifiers::SiteModifier;
use engine_core::hash::hash_u32;

/// The whole complex for one site.
pub(super) fn build(f: Family, seed: u32, modifier: SiteModifier, ossuary: bool) -> Plan {
    let mirrored = seed & 1 == 1;
    let mut b = Builder::new(modifier);
    approach::mechanisms(&mut b, f, seed);
    approach::treasure_fixtures(&mut b, f, mirrored);
    b.mirror = mirrored;
    match f {
        Family::Foundry => if ossuary { burial_works(&mut b, seed) } else { foundry(&mut b, seed) },
        Family::Toll => toll(&mut b, seed),
        Family::Archive => archive(&mut b, seed),
        Family::Hart => hart(&mut b, seed),
    }
    b.mirror = mirrored;
    approach::wings(&mut b, f);
    approach::shells(&mut b, f);
    b.finish()
}

fn bits(seed: u32, salt: u32) -> u32 {
    hash_u32(seed ^ salt.wrapping_mul(0x9e37_79b9))
}

/// Rectangular hall: front arch, floor, walls and roof at `top`.
fn hall(b: &mut Builder, wall: impl Into<Fill> + Copy, floor: B, top: i32) {
    b.dress = true;
    b.put([-3, 3, 1, 5, 10, 11], B::Air);
    b.put([-11, 11, 1, top, 11, 27], B::Air);
    b.put([-11, 11, 0, 1, 11, 27], floor);
    b.put([-13, 13, -5, top + 1, 10, 28], wall);
}

/// Gallery ring at walk height 6 (slab `y 5..6`): stairs up the left front,
/// left and back runs, a right run that may have lost a span, posts beneath.
fn galleries(b: &mut Builder, seed: u32, slab: B) {
    b.dress = false;
    for i in 1..=5 {
        b.put([-11, -9, 1, 1 + i, 10 + i, 11 + i], B::StoneBricks);
    }
    b.put([-10, -9, 6, 7, 26, 27], B::Torch);
    b.put([9, 10, 6, 7, 26, 27], B::Torch);
    b.put([-11, -9, 5, 6, 16, 27], slab);
    b.put([-11, 11, 5, 6, 25, 27], slab);
    b.dress = true;
    if bits(seed, 7).is_multiple_of(3) {
        let s = 15 + (bits(seed, 8) % 6) as i32;
        b.put([9, 11, 5, 6, s, s + 2], B::Air);
    }
    b.put([9, 11, 5, 6, 11, 25], slab);
    for w in [18, 22] {
        b.put([-11, -10, 1, 5, w, w + 1], slab);
        b.put([10, 11, 1, 5, w, w + 1], slab);
    }
    for u in [-5, 4] {
        b.put([u, u + 1, 1, 5, 26, 27], slab);
    }
}

/// Seeded cover pillars at `(u, w)` corners (2 x 2 m): whole, broken to a
/// stump with rubble beside it, a rubble heap, or gone.
fn pillars(b: &mut Builder, seed: u32, spots: &[(i32, i32)], base: i32, top: i32, stone: impl Into<Fill> + Copy) {
    b.dress = true;
    for (n, &(u, w)) in spots.iter().enumerate() {
        let h = bits(seed, 100 + n as u32);
        match h % 5 {
            0 | 1 => b.put([u, u + 2, base, top, w, w + 2], stone),
            2 => {
                let stump = (base + 2 + (h >> 4) as i32 % 3).min(top);
                b.put([u, u + 2, base, stump, w, w + 2], stone);
                b.put([u + 2, u + 3, base, base + 1, w, w + 1], Fill::Rubble);
            }
            3 => b.put([u, u + 2, base, base + 1, w, w + 2], Fill::Rubble),
            _ => {}
        }
    }
}

/// Unfinished Engine: blast-brick foundry hall, slag (lava) channels, a gallery
/// ring with a gantry across the front of the arm's reach, ore stock alcoves.
fn foundry(b: &mut Builder, seed: u32) {
    let top = 11 + (bits(seed, 1) % 3) as i32;
    galleries(b, seed, B::Basalt);
    b.dress = false;
    // Gantry bridge between the side galleries, railed on its arena side.
    let gw = if bits(seed, 2) & 1 == 0 { 12 } else { 23 };
    b.put([-8, 8, 6, 7, gw, gw + 1], B::CopperBlock);
    b.put([-9, 9, 5, 6, gw, gw + 2], B::Basalt);
    b.dress = true;
    for (n, (u, ore)) in [(-8, B::CoalOre), (-1, B::IronOre), (6, B::CopperOre)].into_iter().enumerate() {
        let ore = if bits(seed, 30 + n as u32).is_multiple_of(4) { B::TinOre } else { ore };
        b.put([u, u + 2, 1, 3, 27, 28], ore);
    }
    b.put([-9, -8, 0, 1, 12, 26], B::Lava);
    b.put([8, 9, 0, 1, 12, 26], B::Lava);
    pillars(b, seed, &[(-7, 15), (5, 15), (-7, 19), (5, 19), (-7, 23), (5, 23)], 1, top, B::BlastBrick);
    hall(b, B::BlastBrick, B::StoneBricks, top);
}

/// Burial works: weak partitions demonstrate excavation; basalt buttresses
/// survive it. The side aisles remain open for the service approach.
fn burial_works(b: &mut Builder, seed: u32) {
    galleries(b, seed, B::StoneBricks);
    b.dress = false;
    // Interrupted burial galleries: narrow walls can be baited into new passages.
    for w in [15, 22] {
        for (a,z) in [(-7,-2),(2,7)] {
            b.put([a,z,1,3,w,w+1], B::WeatheredBricks);
        }
    }
    // Known breaking-strength contrast, present in every version-59 layout.
    for (u,w) in [(-6,18),(5,18)] {
        b.put([u,u+2,1,4,w,w+2], B::Basalt);
        b.put([u,u+2,4,5,w,w+2], B::CarvedStone);
    }
    // Existing cut profile and displaced sarcophagi at the approach.
    for (u,w) in [(-8,13),(7,24)] {
        b.put([u,u+1,1,2,w,w+2], B::TombStone);
    }
    b.put([-2,2,0,1,11,27], B::Limestone);
    b.put([-11,-9,1,2,22,24], B::Torch);
    b.put([9,10,1,2,13,14], B::Torch);
    hall(b, B::StoneBricks, B::StoneBricks, 9);
}

/// Last Tollkeeper: a stone causeway down the middle with parapet cover lines,
/// flooded maintenance trenches either side, plank cross-bridges, stairs out of
/// the trenches at the back, a timber gatehouse frame on the arch.
fn toll(b: &mut Builder, seed: u32) {
    let top = 11 + (bits(seed, 1) % 3) as i32;
    galleries(b, seed, B::Planks);
    b.dress = false;
    for side in [-1i32, 1] {
        let (u0, u1) = if side < 0 { (-8, -4) } else { (4, 8) };
        for i in 1..=4 {
            b.put([u0, u1, -3, -3 + i, 22 + i, 23 + i], B::StoneBricks);
        }
        // Cross bridges; one of the four may be down (the trench still connects).
        for (k, w) in [14, 20].into_iter().enumerate() {
            let pier = if side < 0 { -6 } else { 5 };
            b.put([pier, pier + 1, -3, 0, w, w + 2], B::Log);
            if bits(seed, 40) % 4 != (k as u32 * 2 + (side > 0) as u32) {
                b.put([u0, u1, 0, 1, w, w + 2], B::Planks);
            }
        }
        b.put([u0 + 1, u0 + 3, -3, top, 17, 19], B::StoneBricks);
    }
    // Cover lines across the causeway, leaving the centre lane open.
    for w in [12, 24] {
        b.put([-3, -1, 1, 3, w, w + 1], B::StoneBricks);
        b.put([1, 3, 1, 3, w, w + 1], B::StoneBricks);
    }
    // Timber gatehouse frame around the arch.
    b.put([-4, -3, 1, 6, 10, 11], B::Log);
    b.put([3, 4, 1, 6, 10, 11], B::Log);
    b.put([-4, 4, 5, 6, 10, 11], B::Log);
    b.dress = true;
    for (u0, u1) in [(-8, -4), (4, 8)] {
        b.put([u0, u1, -3, -2, 12, 23], B::Water);
        b.put([u0, u1, -3, 1, 12, 27], B::Air);
    }
    pillars(b, seed, &[(-11, 21), (9, 21)], 1, 5, B::StoneBricks);
    hall(b, B::StoneBricks, B::StoneBricks, top);
}

/// Glassweaver: a book-walled rotunda (centre `w 19`, radius 8.5) with two
/// gallery rings, stairs between them and crystal anchor plinths at three
/// heights.
fn archive(b: &mut Builder, seed: u32) {
    let (cu, cw) = (0.0, 19.0);
    b.dress = false;
    // Floor to ring A (walk 5) on the left; ring A to ring B (walk 9) at the back.
    for i in 1..=4 {
        b.put([-5, -4, 1, 1 + i, 18 + i, 19 + i], B::StoneBricks);
        b.put([i - 3, i - 2, 5, 5 + i, 24, 26], B::StoneBricks);
    }
    // Anchor plinths: copper base, crystal top, at floor, ring A and ring B.
    let turn = (bits(seed, 3) % 2) as i32;
    let plinths: [(i32, i32, i32); 4] = [(5, 15, 1), (5 - 11 * turn, 23, 1), (-7, 14, 5), (6, 23, 9)];
    for (u, w, y) in plinths {
        b.put([u, u + 1, y + 1, y + 2, w, w + 1], B::Crystal);
        b.put([u, u + 1, y, y + 1, w, w + 1], B::CopperBlock);
    }
    // Keep the rotunda's book wall out of the bridge hall.
    b.put([-3, 3, -2, 5, 4, 10], B::Air);
    b.put([-3, 3, 1, 5, 10, 12], B::Air);
    b.put([8, 11, 1, 3, 18, 20], B::Air);
    b.put([-11, -8, 1, 3, 18, 20], B::Air);
    b.put([6, 8, 1, 3, 11, 15], B::Air);
    b.ring(cu, cw, 5.8, 8.5, (4, 5), B::StoneBricks);
    b.ring(cu, cw, 6.8, 8.5, (8, 9), B::StoneBricks);
    b.dress = true;
    // Ring B sometimes lost a span on the right (the left run to the pin stays).
    if bits(seed, 4).is_multiple_of(2) {
        let w = 14 + (bits(seed, 5) % 6) as i32;
        b.put([6, 9, 8, 9, w, w + 2], B::Air);
    }
    // Toppled book stacks as cover, and fallen masonry.
    pillars(b, seed, &[(-7, 16), (5, 17), (5, 21), (-3, 24), (1, 12)], 1, 4, Fill::Books);
    // Hanging star-glass lantern over the reliquary.
    b.put([-1, 1, 12, 13, -1 + 19, 1 + 19], B::StarGlass);
    b.ring(cu, cw, 0.0, 8.5, (1, 14), B::Air);
    b.ring(cu, cw, 0.0, 8.5, (0, 1), B::StoneBricks);
    b.ring(cu, cw, 8.5, 10.0, (-5, 14), Fill::Books);
    b.ring(cu, cw, 0.0, 10.0, (14, 15), B::StarGlass);
    b.put([-5, 5, -4, 7, 10, 12], B::StoneBricks);
    b.put([4, 10, 0, 4, 10, 16], B::StoneBricks);
}

/// Prism Hart: a flooded seed archive. Shallow water channels, two grown
/// ledges (walk 4 and 7) climbed by root steps, log supports, ice and glowcaps.
fn hart(b: &mut Builder, seed: u32) {
    let top = 10 + (bits(seed, 1) % 2) as i32;
    b.dress = false;
    // Tunnels through the ledges to the side rooms.
    b.put([8, 11, 1, 3, 18, 20], B::Air);
    b.put([-11, -8, 1, 3, 18, 20], B::Air);
    for i in 1..=3 {
        b.put([-11, -8, 1, 1 + i, 10 + i, 11 + i], B::MushroomStem);
    }
    for i in 1..=6 {
        b.put([8, 11, 1, 1 + i, 10 + i, 11 + i], B::MushroomStem);
    }
    b.put([-10, -9, 4, 5, 22, 23], B::GlowCap);
    b.put([9, 10, 7, 8, 21, 22], B::GlowCap);
    b.put([-11, -8, 1, 4, 13, 27], B::MushroomStem);
    b.put([8, 11, 1, 7, 16, 27], B::MushroomStem);
    b.dress = true;
    for (n, (u, w)) in [(-6, 12), (5, 12), (-6, 25), (5, 25), (-2, 13), (1, 25)].into_iter().enumerate() {
        if !bits(seed, 50 + n as u32).is_multiple_of(3) {
            b.put([u, u + 1, 1, top, w, w + 1], B::Log);
        } else {
            b.put([u, u + 1, top - 2, top, w, w + 1], B::Ice);
        }
    }
    let cross = if bits(seed, 2) & 1 == 0 { 12 } else { 24 };
    b.put([-7, -6, 0, 1, 11, 27], B::Water);
    b.put([6, 7, 0, 1, 11, 27], B::Water);
    b.put([-8, 8, 0, 1, cross, cross + 1], B::Water);
    b.put([-8, 8, top - 1, top, 18, 19], B::Leaves);
    b.put([-11, 11, 1, top, 11, 27], B::Air);
    b.put([-3, 3, 1, 5, 10, 11], B::Air);
    b.put([-11, 11, 0, 1, 11, 27], B::FungalSoil);
    b.put([-13, 13, -3, top + 1, 10, 28], B::Limestone);
}

/// Walk-critical stair runs (canonical boxes), for tests.
#[cfg(test)]
pub(super) fn stair_runs(f: Family) -> Vec<Vec<super::Box3>> {
    let run = |n: i32, mk: &dyn Fn(i32) -> super::Box3| (1..=n).map(mk).collect::<Vec<_>>();
    match f {
        Family::Foundry | Family::Toll => vec![run(5, &|i| [-11, -9, 1, 1 + i, 10 + i, 11 + i])],
        Family::Archive => vec![
            run(4, &|i| [-5, -4, 1, 1 + i, 18 + i, 19 + i]),
            run(4, &|i| [i - 3, i - 2, 5, 5 + i, 24, 26]),
        ],
        Family::Hart => vec![
            run(3, &|i| [-11, -8, 1, 1 + i, 10 + i, 11 + i]),
            run(6, &|i| [8, 11, 1, 1 + i, 10 + i, 11 + i]),
        ],
    }
}
