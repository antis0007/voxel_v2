//! Walk and seal checks over the complex plans (shrine-local 1 m grid; the
//! plan is scale-independent, so these hold at every voxel scale).
use super::*;
use std::collections::{HashSet, VecDeque};

const FAMILIES: [Family; 4] = [Family::Foundry, Family::Toll, Family::Archive, Family::Hart];

pub(super) fn synthetic(family: Family, seed: u32, modifier: SiteModifier) -> Shrine {
    Shrine { family, center: Vec3::ZERO, turn: 0, voxel: 1.0, seed, modifier }
}

// ---- Walker over the plan's 1 m local grid. -------------------------------

type Cell = (i32, i32, i32);

fn block_at(s: &Shrine, (u, y, w): Cell, open: &[Box3]) -> Option<B> {
    let l = Vec3::new(u as f32 + 0.5, y as f32 + 0.5, w as f32 + 0.5);
    if open.iter().any(|b| plan::inside(*b, l)) {
        return Some(B::Air);
    }
    s.local_block(l)
}

/// Unknown (natural) ground counts as solid; lava is never stood in.
fn solid(b: Option<B>) -> bool {
    b.is_none_or(|b| b.is_solid())
}

fn standable(s: &Shrine, c: Cell, open: &[Box3]) -> bool {
    let (u, y, w) = c;
    let feet = block_at(s, c, open);
    feet != Some(B::Lava)
        && feet.is_some()
        && !solid(feet)
        && !solid(block_at(s, (u, y + 1, w), open))
        && solid(block_at(s, (u, y - 1, w), open))
        && block_at(s, (u, y - 1, w), open) != Some(B::Lava)
}

/// Every standable cell reachable on foot from in front of the door, with
/// one-metre rises (jump height is 1.26 m) and any drop.
fn walk(s: &Shrine, open: &[Box3]) -> HashSet<Cell> {
    let start = (0, 1, -6);
    assert!(standable(s, start, open), "{:?} start", s.family);
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some((u, y, w)) = queue.pop_front() {
        for (du, dw) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nu, nw) = (u + du, w + dw);
            if !(BOUNDS[0]..BOUNDS[1]).contains(&nu) || !(BOUNDS[4]..BOUNDS[5]).contains(&nw) {
                continue;
            }
            let mut targets = Vec::new();
            if !solid(block_at(s, (u, y + 2, w), open)) {
                targets.push((nu, y + 1, nw));
            }
            // Same level, or walk off an edge and fall.
            let mut ny = y;
            while ny > BOUNDS[2] && !solid(block_at(s, (nu, ny, nw), open)) && !solid(block_at(s, (nu, ny + 1, nw), open)) {
                if standable(s, (nu, ny, nw), open) {
                    targets.push((nu, ny, nw));
                    break;
                }
                if block_at(s, (nu, ny - 1, nw), open) == Some(B::Lava) {
                    break;
                }
                ny -= 1;
            }
            for t in targets {
                if standable(s, t, open) && seen.insert(t) {
                    queue.push_back(t);
                }
            }
        }
    }
    seen
}

fn near(seen: &HashSet<Cell>, b: Box3) -> bool {
    seen.iter().any(|&(u, y, w)| {
        (b[0] - 1..=b[1]).contains(&u) && (b[4] - 1..=b[5]).contains(&w) && (b[2] - 2..b[3]).contains(&y)
    })
}

fn variants() -> impl Iterator<Item = Shrine> {
    FAMILIES.into_iter().flat_map(|f| {
        (0..6u32).flat_map(move |seed| {
            std::iter::once(SiteModifier::None)
                .chain(SiteModifier::ALL)
                .filter(move |m| seed < 2 || *m == SiteModifier::None)
                .map(move |m| synthetic(f, hash_u32(seed * 7 + 1), m))
        })
    })
}

#[test]
fn boss_complex_is_walkable_without_softlocks() {
    for s in variants() {
        let f = s.family;
        let tag = format!("{f:?} seed {:x} {:?}", s.seed, s.modifier);
        // Door open; and separately with the bridge deck gone (the chasm stairs).
        for open in [vec![DOOR], vec![DOOR, [-1, 1, 0, 1, 4, 10]]] {
            let seen = walk(&s, &open);
            assert!(seen.contains(&(0, 1, 19)), "{tag} relic unreachable (open {open:?})");
            for t in s.treasures() {
                assert!(near(&seen, t.seal), "{tag} treasure {} seal unreachable", t.index);
                if let Opens::Lever(pin) = t.opens {
                    assert!(near(&seen, pin), "{tag} lever pin unreachable");
                }
            }
            let high = seen.iter().filter(|c| c.2 >= 11).map(|c| c.1).max().unwrap_or(0);
            assert!(high >= 5, "{tag} no height advantage in the arena ({high})");
        }
        // Without opening the door the cracked side wall still lets the player in.
        let seen = walk(&s, &[WEAK_WALL]);
        assert!(seen.contains(&(0, 1, 19)), "{tag} cracked fallback");
    }
}

#[test]
fn boss_arena_is_large_clear_and_floored() {
    for s in variants() {
        let tag = format!("{:?} seed {:x} {:?}", s.family, s.seed, s.modifier);
        for u in CLEAR[0]..CLEAR[1] {
            for w in CLEAR[4]..CLEAR[5] {
                assert!(solid(block_at(&s, (u, 0, w), &[])), "{tag} floor {u},{w}");
                for y in CLEAR[2]..CLEAR[3] {
                    assert_eq!(block_at(&s, (u, y, w), &[]), Some(B::Air), "{tag} clear {u},{y},{w}");
                }
            }
        }
        // The arena: at least 180 m2 of open floor-level air (3 m clear) to fight over.
        let mut open = 0;
        for u in -12..12 {
            for w in 10..29 {
                if (1..4).all(|y| !solid(block_at(&s, (u, y, w), &[]))) {
                    open += 1;
                }
            }
        }
        assert!(open >= 180, "{tag} open arena columns {open}");
    }
}

#[test]
fn boss_treasure_rooms_stay_sealed_until_opened() {
    for s in variants() {
        let tag = format!("{:?} seed {:x} {:?}", s.family, s.seed, s.modifier);
        for t in s.treasures() {
            let c = (t.cache[0], t.cache[2], t.cache[4]);
            assert_eq!(block_at(&s, c, &[]), Some(B::Air), "{tag} cache {} empty", t.index);
            assert_eq!(block_at(&s, (c.0, c.1 - 1, c.2), &[]), Some(B::VoidStone), "{tag} pedestal");
            let seal = match t.opens {
                Opens::Cracked => B::WeatheredBricks,
                Opens::Lever(pin) => {
                    assert_eq!(block_at(&s, (pin[0], pin[2], pin[4]), &[]), Some(B::Log), "{tag} pin");
                    s.family.door_block()
                }
                Opens::Boss => B::VoidStone,
            };
            for u in t.seal[0]..t.seal[1] {
                for y in t.seal[2]..t.seal[3] {
                    for w in t.seal[4]..t.seal[5] {
                        assert_eq!(block_at(&s, (u, y, w), &[]), Some(seal), "{tag} seal {}", t.index);
                    }
                }
            }
            // Flood the room's air from the cache: it never leaks out.
            let mut seen = HashSet::from([c]);
            let mut queue = VecDeque::from([c]);
            while let Some((u, y, w)) = queue.pop_front() {
                assert!(seen.len() < 80, "{tag} treasure {} leaks", t.index);
                for d in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                    let n = (u + d.0, y + d.1, w + d.2);
                    let b = block_at(&s, n, &[]);
                    if b.is_some_and(|b| !b.is_solid()) && seen.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
        }
    }
}

#[test]
fn boss_bridges_span_their_gaps_on_supports() {
    for s in variants().filter(|s| s.modifier == SiteModifier::None) {
        let tag = format!("{:?} seed {:x}", s.family, s.seed);
        // The chasm is open below the deck, the deck is held by two piers.
        for w in 4..10 {
            assert!(!solid(block_at(&s, (2, -1, w), &[])), "{tag} chasm {w}");
        }
        for w in [5, 9] {
            for y in -2..1 {
                assert!(solid(block_at(&s, (0, y, w), &[])), "{tag} pier {w} {y}");
            }
        }
        let deck = (4..10).filter(|w| solid(block_at(&s, (0, 0, *w), &[]))).count();
        assert!(deck >= 4, "{tag} deck {deck}");
        for run in arenas::stair_runs(s.family) {
            let mut last = None;
            for b in run {
                let b = if s.mirrored() { plan::mirror(b) } else { b };
                let top = b[3];
                assert!(solid(block_at(&s, (b[0], top - 1, b[4]), &[])), "{tag} stair {b:?}");
                assert!(!solid(block_at(&s, (b[0], top, b[4]), &[])), "{tag} stair head {b:?}");
                if let Some(prev) = last {
                    assert_eq!(top - prev, 1, "{tag} rise");
                }
                last = Some(top);
            }
        }
    }
}

