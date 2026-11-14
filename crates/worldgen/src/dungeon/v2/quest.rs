//! Generator 294: a mission graph over the room tree.
//!
//! The critical path runs from the entry to the Core through the spanning tree.
//! Up to two critical-path corridors take a Locked Door; the corridor into the
//! Core takes a Great Door. Every lock's key waits in a cache reachable before
//! that lock, preferably off the critical path, so the player sees a door early
//! and opens it after a detour. The Great Key waits beyond the last small lock.
//! Keys are dungeon-bound: the app counts them per complex.
use engine_content::BlockId as B;

pub const VERSION: u32 = 294;
/// Small locks on one critical path.
const MAX_LOCKS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Small,
    Great,
}

#[derive(Clone, Debug, Default)]
pub struct Quest {
    /// Room index and the key its cache holds.
    pub keys: Vec<(usize, KeyKind)>,
    /// Tree edges that carry a door, with the door block.
    pub doors: Vec<((usize, usize), B)>,
    /// Room that holds the reading lectern.
    pub lectern: Option<usize>,
}

impl Quest {
    pub fn key_in(&self, room: usize) -> Option<KeyKind> {
        self.keys.iter().find(|(r, _)| *r == room).map(|(_, k)| *k)
    }
    pub fn door_on(&self, a: usize, b: usize) -> Option<B> {
        self.doors.iter().find(|((x, y), _)| (*x, *y) == (a.min(b), a.max(b))).map(|(_, d)| *d)
    }
    pub fn small_locks(&self) -> usize {
        self.doors.iter().filter(|(_, d)| *d == B::LockedDoor).count()
    }
    pub fn great_lock(&self) -> bool {
        self.doors.iter().any(|(_, d)| *d == B::BossDoor)
    }
}

fn neighbours(i: usize, tree: &[(usize, usize)]) -> impl Iterator<Item = usize> + '_ {
    tree.iter().filter_map(move |&(a, b)| if a == i { Some(b) } else if b == i { Some(a) } else { None })
}

/// Component label of every room once `cut` edges are removed from the tree.
pub(super) fn regions(n: usize, tree: &[(usize, usize)], cut: &[(usize, usize)], entry: usize) -> Vec<usize> {
    let mut label = vec![usize::MAX; n];
    let mut next = 0;
    for start in std::iter::once(entry).chain(0..n) {
        if label[start] != usize::MAX {
            continue;
        }
        let mut stack = vec![start];
        label[start] = next;
        while let Some(i) = stack.pop() {
            for j in neighbours(i, tree) {
                let edge = (i.min(j), i.max(j));
                if label[j] == usize::MAX && !cut.contains(&edge) {
                    label[j] = next;
                    stack.push(j);
                }
            }
        }
        next += 1;
    }
    label
}

/// Plans locks and keys. `eligible[i]` is true for rooms whose cache spot can
/// hold a key (ordinary rooms, not the entry, reveal, Core, puzzles or voids).
pub(super) fn assign(
    tree: &[(usize, usize)],
    hops: &[u32],
    entry: usize,
    objective: Option<usize>,
    eligible: &[bool],
    salt: f32,
) -> Quest {
    let n = hops.len();
    let mut quest = Quest { lectern: Some(entry), ..Quest::default() };
    let Some(objective) = objective else { return quest };
    // Walk back from the objective to the entry along decreasing hops.
    let mut path = vec![objective];
    while let Some(&last) = path.last() {
        if last == entry {
            break;
        }
        let Some(prev) = neighbours(last, tree).find(|&p| hops[p].wrapping_add(1) == hops[last]) else { return quest };
        path.push(prev);
    }
    path.reverse();
    let on_path = |i: usize| path.contains(&i);
    let great_edge = (path.len() >= 2).then(|| {
        let (a, b) = (path[path.len() - 2], objective);
        (a.min(b), a.max(b))
    });
    // Small-lock candidates: past the reveal hall, short of the Great Door.
    let candidates: Vec<(usize, usize)> = path
        .windows(2)
        .skip(1)
        .map(|w| (w[0].min(w[1]), w[0].max(w[1])))
        .filter(|e| Some(*e) != great_edge)
        .collect();
    let want = candidates.len().min(MAX_LOCKS);
    let mut locks: Vec<(usize, usize)> = (0..want)
        .map(|k| candidates[((k as f32 + 0.5 + (salt - 0.5) * 0.4) / want as f32 * candidates.len() as f32) as usize % candidates.len()])
        .collect();
    locks.dedup();

    // Each lock's key lies in the region just before it; drop locks without one.
    let mut placed: Vec<(usize, (usize, usize))> = Vec::new();
    for (i, &lock) in locks.iter().enumerate() {
        let mut cut: Vec<(usize, usize)> = locks[i..].to_vec();
        cut.extend(great_edge);
        let label = regions(n, tree, &cut, entry);
        let reachable = |r: usize| label[r] == label[entry];
        let taken = |r: usize| quest.keys.iter().any(|(k, _)| *k == r);
        let pick = (0..n)
            .filter(|&r| reachable(r) && eligible[r] && !taken(r))
            .max_by_key(|&r| (!on_path(r), hops[r], r));
        if let Some(room) = pick {
            quest.keys.push((room, KeyKind::Small));
            placed.push((room, lock));
        }
    }
    let locks: Vec<(usize, usize)> = placed.iter().map(|(_, l)| *l).collect();
    for &lock in &locks {
        quest.doors.push((lock, B::LockedDoor));
    }
    // The Great Key: beyond every small lock if possible, otherwise anywhere open.
    if let Some(edge) = great_edge {
        let cut = [edge];
        let label = regions(n, tree, &cut, entry);
        let taken = |r: usize| quest.keys.iter().any(|(k, _)| *k == r);
        let behind_last = |r: usize| {
            let mut before = locks.clone();
            before.push(edge);
            regions(n, tree, &before, entry)[r] != label[entry] || locks.is_empty()
        };
        let pick = (0..n)
            .filter(|&r| label[r] == label[entry] && eligible[r] && !taken(r) && r != objective)
            .max_by_key(|&r| (behind_last(r), !on_path(r), hops[r], r));
        if let Some(room) = pick {
            quest.keys.push((room, KeyKind::Great));
            quest.doors.push((edge, B::BossDoor));
        }
    }
    quest
}
