//! Factory bases: machine groups that keep an id while machines come and go.
use crate::fast_hash::FastMap;
use glam::IVec3;

const ALIAS_LIMIT: usize = 256;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Base {
    pub id: u32,
    /// Stays on the same machine until it is removed, so pins keep their place.
    pub anchor: IVec3,
    pub members: Vec<IVec3>,
}

#[derive(Clone, Debug, Default)]
pub struct Bases {
    pub list: Vec<Base>,
    next_id: u32,
    /// `(absorbed id, surviving id)` after merges, newest last.
    aliases: Vec<(u32, u32)>,
    layout: u64,
    link_sq: u32,
}

fn mix(p: IVec3) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for v in [p.x, p.y, p.z] {
        h = (h ^ u64::from(v as u32)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^ (h >> 29)
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Groups positions within `link` voxels using a spatial hash, not pairwise checks.
/// Each group lists indices into `positions`, ordered by first index.
pub fn group(positions: &[IVec3], link: f32) -> Vec<Vec<usize>> {
    let cell = link.ceil().max(1.0) as i32;
    let link_sq = link * link;
    let mut grid: FastMap<IVec3, Vec<usize>> = FastMap::default();
    for (i, p) in positions.iter().enumerate() {
        grid.entry(p.div_euclid(IVec3::splat(cell))).or_default().push(i);
    }
    let mut parent: Vec<usize> = (0..positions.len()).collect();
    for (i, p) in positions.iter().enumerate() {
        let c = p.div_euclid(IVec3::splat(cell));
        for dz in -1..=1 { for dy in -1..=1 { for dx in -1..=1 {
            let Some(bucket) = grid.get(&(c + IVec3::new(dx, dy, dz))) else { continue };
            for &j in bucket {
                if j > i && p.as_vec3().distance_squared(positions[j].as_vec3()) <= link_sq {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    if a != b { parent[a.max(b)] = a.min(b); }
                }
            }
        } } }
    }
    let mut slot = vec![usize::MAX; positions.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in 0..positions.len() {
        let root = find(&mut parent, i);
        if slot[root] == usize::MAX {
            slot[root] = groups.len();
            groups.push(Vec::new());
        }
        groups[slot[root]].push(i);
    }
    groups
}

impl Bases {
    /// The id a stored id now belongs to, following merges.
    pub fn resolve(&self, mut id: u32) -> u32 {
        for _ in 0..self.aliases.len() {
            match self.aliases.iter().rev().find(|(old, _)| *old == id) {
                Some(&(_, to)) => id = to,
                None => break,
            }
        }
        id
    }

    /// Regroups `positions` and returns each group's base index with member indices.
    /// Unchanged layouts reuse the previous grouping and cost one pass.
    pub fn refresh(&mut self, positions: &[IVec3], link: f32) -> Vec<(usize, Vec<usize>)> {
        let link_sq = (link * link) as u32;
        let layout = positions.iter().fold(positions.len() as u64, |h, &p| h.wrapping_add(mix(p)));
        let groups = group(positions, link);
        if layout == self.layout && link_sq == self.link_sq && self.list.len() == groups.len() {
            return groups.into_iter().enumerate().collect();
        }
        self.layout = layout;
        self.link_sq = link_sq;
        let mut owner: FastMap<IVec3, usize> = FastMap::default();
        for (i, base) in self.list.iter().enumerate() {
            for &p in &base.members { owner.insert(p, i); }
        }
        // Old base -> the new group holding most of its members.
        let mut home = vec![(usize::MAX, 0usize); self.list.len()];
        let mut overlap: Vec<FastMap<usize, usize>> = vec![FastMap::default(); self.list.len()];
        for (g, members) in groups.iter().enumerate() {
            for &m in members {
                if let Some(&b) = owner.get(&positions[m]) { *overlap[b].entry(g).or_default() += 1; }
            }
        }
        for (b, counts) in overlap.iter().enumerate() {
            for (&g, &n) in counts {
                if n > home[b].1 || (n == home[b].1 && g < home[b].0) { home[b] = (g, n); }
            }
        }
        let mut next: Vec<Base> = Vec::with_capacity(groups.len());
        for (g, members) in groups.iter().enumerate() {
            let mut claimants: Vec<usize> = (0..self.list.len()).filter(|&b| home[b].0 == g).collect();
            claimants.sort_by_key(|&b| self.list[b].id);
            let mut cells: Vec<IVec3> = members.iter().map(|&m| positions[m]).collect();
            cells.sort_unstable_by_key(|p| (p.x, p.y, p.z));
            let (id, old_anchor) = if let Some(&win) = claimants.first() {
                for &b in &claimants[1..] {
                    self.aliases.push((self.list[b].id, self.list[win].id));
                }
                (self.list[win].id, Some(self.list[win].anchor))
            } else {
                self.next_id += 1;
                (self.next_id, None)
            };
            let anchor = old_anchor
                .filter(|a| cells.binary_search_by_key(&(a.x, a.y, a.z), |p| (p.x, p.y, p.z)).is_ok())
                .or_else(|| old_anchor.and_then(|a| cells.iter().copied()
                    .min_by_key(|p| (p.as_vec3().distance_squared(a.as_vec3()) as i64, p.x, p.y, p.z))))
                .unwrap_or(cells[0]);
            next.push(Base { id, anchor, members: cells });
        }
        let excess = self.aliases.len().saturating_sub(ALIAS_LIMIT);
        self.aliases.drain(..excess);
        self.next_id = self.next_id.max(next.iter().map(|b| b.id).max().unwrap_or(0));
        self.list = next;
        groups.into_iter().enumerate().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32) -> IVec3 { IVec3::new(x, 0, 0) }
    fn run(bases: &mut Bases, xs: &[i32]) -> Vec<(u32, usize)> {
        let mut p: Vec<_> = xs.iter().map(|&x| at(x)).collect();
        p.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        bases.refresh(&p, 10.0);
        bases.list.iter().map(|b| (b.id, b.members.len())).collect()
    }

    #[test]
    fn hash_grouping_matches_the_distance_rule_at_negative_coordinates() {
        let p = [at(-100), at(-91), at(-80), at(0), at(5), at(1000)];
        let groups = group(&p, 10.0);
        assert_eq!(groups, vec![vec![0, 1], vec![2], vec![3, 4], vec![5]]);
    }

    #[test]
    fn ids_survive_removal_growth_split_and_merge() {
        let mut b = Bases::default();
        let first = run(&mut b, &[0, 5, 100, 105, 110]);
        assert_eq!(first, vec![(1, 2), (2, 3)]);
        let anchor = b.list[0].anchor;
        // Removing the anchor machine keeps the id; the anchor moves to a survivor.
        assert_eq!(run(&mut b, &[5, 100, 105, 110]), vec![(1, 1), (2, 3)]);
        assert_ne!(b.list[0].anchor, anchor);
        // Growth keeps ids.
        assert_eq!(run(&mut b, &[5, 8, 100, 105, 110]), vec![(1, 2), (2, 3)]);
        // Split: the larger part keeps the id, the smaller gets a new one.
        assert_eq!(run(&mut b, &[5, 8, 100, 105, 200]), vec![(1, 2), (2, 2), (3, 1)]);
        // Merge: the older id wins and the other resolves to it.
        assert_eq!(run(&mut b, &[5, 8, 100, 105, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200]).len(), 2);
        let mut m = Bases::default();
        run(&mut m, &[0, 5, 100, 105, 110]);
        assert_eq!(run(&mut m, &[0, 5, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 105, 110]), vec![(1, 14)]);
        assert_eq!(m.resolve(2), 1);
    }
}
