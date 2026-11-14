//! Planning a complex: footprint, room graph, levels, the sealed core and its
//! puzzle, room states, corridors. Runs once per complex.
use super::footprint::{SLOT, fit};
use super::{Outline, Ceiling, Complex, Corr, Damage, Index, Job, Kind, LEVEL, Room, WALL};
use crate::{BlockId as B, hash2};
use glam::Vec3;

pub fn generate(seed: u32, cx: f32, cz: f32, ground: f32, version: u32) -> Complex {
    let r = |k: u32| hash2(k as i32, 77, seed);
    let job = Job::roll_versioned(r(1), r(8), version);
    let top_floor = (ground - job.depth(r(2))).round();
    let (cells, entry) = if version >= 90 {
        super::footprint::districts(r(3), r(4), r(5), cx, cz)
    } else if version >= super::sprawl::VERSION {
        super::footprint::sprawling(r(3), r(4), r(5), cx, cz)
    } else { fit(r(3), r(4), r(5), cx, cz) };
    let n = cells.len();
    let adjacent = |a: usize, b: usize| {
        let (p, q) = (cells[a], cells[b]);
        (p.0 - q.0).abs() + (p.1 - q.1).abs() == 1
    };
    // The reveal hall is the entry's best-connected neighbour.
    let reveal = (0..n)
        .filter(|&b| adjacent(entry, b))
        .max_by_key(|&b| ((0..n).filter(|&c| adjacent(b, c)).count(), b))
        .unwrap_or(entry);

    // Spanning tree plus loops, weighted by hash.
    let mut edges: Vec<(f32, usize, usize)> = Vec::new();
    for a in 0..n {
        for b in a + 1..n {
            if adjacent(a, b) {
                edges.push((hash2(a as i32, b as i32, seed.wrapping_add(31)), a, b));
            }
        }
    }
    edges.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut parent: Vec<usize> = (0..n).collect();
    let (mut tree, mut extra) = (Vec::new(), Vec::new());
    // The entry–reveal edge is always in the tree: the balcony is the overlook.
    if reveal != entry {
        tree.push((entry.min(reveal), entry.max(reveal)));
        let (ra, rb) = (find(&mut parent, entry), find(&mut parent, reveal));
        parent[ra] = rb;
    }
    for &(w, a, b) in &edges {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
            tree.push((a, b));
        } else if !tree.contains(&(a, b)) && w < 0.4 {
            extra.push((a, b));
        }
    }
    if extra.is_empty()
        && let Some(&(_, a, b)) = edges.iter().find(|(_, a, b)| !tree.contains(&(*a, *b)))
    {
        extra.push((a, b));
    }

    // Objective: the tree leaf farthest from the entry whose parent is ordinary.
    let hops = bfs(n, &tree, entry);
    let neighbours = |i: usize, set: &[(usize, usize)]| -> Vec<usize> {
        set.iter().filter_map(|&(a, b)| if a == i { Some(b) } else if b == i { Some(a) } else { None }).collect()
    };
    let objective = (0..n)
        .filter(|&i| i != entry && i != reveal && neighbours(i, &tree).len() == 1)
        .filter(|&i| {
            let p = neighbours(i, &tree)[0];
            p != entry && p != reveal
        })
        .max_by_key(|&i| (hops[i], i));
    let ante = objective.map(|o| neighbours(o, &tree)[0]);
    let sealed_ante = top_floor <= ground - 30.0;
    extra.retain(|&(a, b)| {
        let touches = |i: Option<usize>| i.is_some_and(|i| a == i || b == i);
        !(touches(objective) || sealed_ante && touches(ante))
    });

    // Rooms: kind, size, height, position.
    let archive = (0..n)
        .filter(|&i| i != entry && i != reveal && Some(i) != objective && Some(i) != ante)
        .max_by_key(|&i| (neighbours(i, &tree).len() == 1, hops[i], i));
    let kinds = job.kinds();
    let mut rooms: Vec<Room> = Vec::with_capacity(n);
    for (i, &(ci, cj)) in cells.iter().enumerate() {
        let h = |k: u32| hash2(ci * 7 + k as i32, cj * 13 + 5, seed);
        let kind = if i == entry {
            Kind::Entry
        } else if i == reveal {
            Kind::Reveal
        } else if Some(i) == objective {
            Kind::Core
        } else if Some(i) == ante {
            job.guard_puzzle(h(1))
        } else if version >= super::sprawl::VERSION {
            if Some(i) == archive { Kind::Scriptorium }
            else if version >= 90 {
                let programme = super::inhabited::programme(job);
                programme[(hops[i] as usize + i) % programme.len()]
            } else { super::sprawl::purpose(job, neighbours(i, &tree).len() == 1, hops[i] as usize, i) }
        } else if version >= 46 {
            // Rotate a job-specific programme rather than independently
            // rolling the same room repeatedly. Special graph nodes above
            // retain the entrance, reveal and sealed-objective contracts.
            let programme = super::inhabited::programme(job);
            let used = rooms.iter().filter(|r| programme.contains(&r.kind)).count();
            programme[(used + (r(19) * programme.len() as f32) as usize) % programme.len()]
        } else if version >= 41 {
            // Public/work rooms near arrival, services on side branches,
            // ceremonial and dangerous spaces deeper in the complex.
            // Special rooms above retain their objective/puzzle contract.
            let leaf = neighbours(i, &tree).len() == 1;
            match (job, leaf, hops[i] <= 2) {
                (_, true, _) => match job {
                    Job::Catacombs | Job::Sanctum => Kind::Crypt,
                    Job::Orrery => Kind::Library,
                    Job::HollowFloor => Kind::Chasm,
                    _ => Kind::Store,
                },
                (Job::Catacombs, _, true) => Kind::Ossuary,
                (Job::Undercroft, _, true) => Kind::Sluice,
                (Job::ForgeHalls, _, true) => Kind::Forge,
                (Job::Quarantine, _, true) => Kind::Barracks,
                (Job::Orrery, _, true) => Kind::Library,
                (Job::Sanctum, _, true) => Kind::Shrine,
                _ => kinds[(h(2) * kinds.len() as f32) as usize % kinds.len()],
            }
        } else {
            kinds[(h(2) * kinds.len() as f32) as usize % kinds.len()]
        };
        let (mut w, mut d) = match kind {
            Kind::Entry => (12.0 + h(3) * 4.0, 12.0 + h(4) * 4.0),
            Kind::Reveal => (21.0 + h(3) * 3.0, 21.0 + h(4) * 3.0),
            Kind::Chasm => (17.0 + h(3) * 4.0, 12.0 + h(4) * 3.0),
            Kind::Labyrinth | Kind::Pitfall | Kind::Orrery => (16.0 + h(3) * 4.0, 16.0 + h(4) * 4.0),
            Kind::Cells => (12.0 + h(3) * 8.0, 9.0 + h(4) * 4.0),
            _ => (11.0 + h(3) * 9.0, 11.0 + h(4) * 9.0),
        };
        if version >= 54 {
            // Room purpose changes its proportions, not just its furnishings.
            match kind {
                Kind::Library | Kind::Scriptorium => { w = 21.0; d = 14.0; }
                Kind::Refectory | Kind::Armory => { w = 23.0; d = 12.0; }
                Kind::Shrine => { w = 14.0; d = 23.0; }
                Kind::Crypt | Kind::Store => { w = 12.0; d = 11.0; }
                _ => {}
            }
        }
        if version >= 90 {
            match kind {
                Kind::Cistern | Kind::Garden | Kind::Infirmary => { w = 24.0; d = 18.0; }
                Kind::Foundry | Kind::Barracks => { w = 24.0; d = 16.0; }
                _ => {}
            }
        }
        if version >= 55 && kind == Kind::Forge { w = 22.0; d = 16.0; }
        if version >= super::maze::TECH_VERSION && kind == Kind::Labyrinth {
            // A longer maze: more dead ends to hold caches, more false walls.
            w = 21.0 + h(3) * 6.0;
            d = 21.0 + h(4) * 6.0;
        }
        if h(6) > 0.5 && !matches!(kind, Kind::Entry | Kind::Reveal) {
            std::mem::swap(&mut w, &mut d);
        }
        let (w, d) = (w.round(), d.round());
        let mut height = match kind {
            Kind::Entry => 5.5,
            Kind::Reveal => LEVEL * 2.0 + 2.0,
            Kind::Labyrinth => 4.6 + h(7) * 0.6,
            Kind::Pitfall => 5.5 + h(7),
            Kind::Cells => 4.4,
            Kind::Orrery => 9.0 + h(7) * 2.0,
            Kind::Forge | Kind::Foundry | Kind::Cistern | Kind::Library | Kind::Scriptorium => 6.0 + h(7) * 2.5,
            Kind::Core => 6.5,
            _ => 4.6 + h(7) * 2.0,
        };
        if version >= 54 && matches!(kind, Kind::Library | Kind::Scriptorium | Kind::Shrine | Kind::Refectory) {
            height = 8.0;
        }
        let slack = |s: f32| ((SLOT - s) * 0.5 - 6.0).max(0.0);
        let (jx, jz) = if i == entry { (0.0, 0.0) } else { ((h(8) - 0.5) * 2.0 * slack(w), (h(9) - 0.5) * 2.0 * slack(d)) };
        let (ox, oz) = (cx + (ci - cells[entry].0) as f32 * SLOT + jx, cz + (cj - cells[entry].1) as f32 * SLOT + jz);
        let ceiling = match kind {
            Kind::Reveal => Ceiling::Coffered,
            Kind::Labyrinth | Kind::Cells | Kind::Entry => Ceiling::Beams,
            Kind::Cistern | Kind::Sluice => Ceiling::Ribbed,
            Kind::Orrery | Kind::Core => Ceiling::Corbel,
            _ if w.min(d) >= 13.0 && height >= 5.5 => {
                if h(10) < 0.5 { Ceiling::Ribbed } else { Ceiling::Corbel }
            }
            _ => Ceiling::Beams,
        };
        rooms.push(Room {
            x0: (ox - w * 0.5).round(),
            x1: (ox - w * 0.5).round() + w,
            z0: (oz - d * 0.5).round(),
            z1: (oz - d * 0.5).round() + d,
            floor: top_floor,
            h: height.round(),
            level: 0,
            kind,
            damage: Damage::Intact,
            ceiling,
            outline: Outline::roll(kind, h(11)),
            rift: 0.0,
            cache: false,
            stair: 0,
            sealed: kind == Kind::Core || (Some(i) == ante && sealed_ante),
            salt: seed.wrapping_add((i as u32).wrapping_mul(0x9e37_79b9)),
        });
    }

    // Levels down the tree; a ramp is never steeper than 3:4.
    let order = bfs_order(n, &tree, entry);
    let mut level = vec![0u8; n];
    level[reveal] = 1;
    for &(child, from) in &order {
        if child == reveal {
            continue;
        }
        let base = if from == reveal { 1 } else { level[from] as i32 };
        let roll = hash2(child as i32, 3, seed.wrapping_add(41));
        let want = (base + if roll < 0.45 { 1 } else if roll < 0.6 { -1 } else { 0 }).clamp(0, 2) as u8;
        let gap = gap(&rooms[from], &rooms[child]);
        level[child] = if (port_level(rooms[from].kind, level[from], want) - want as f32).abs() * LEVEL <= (gap - 4.0) * 0.75 {
            want
        } else if from == reveal {
            1
        } else {
            level[from]
        };
    }
    for (i, room) in rooms.iter_mut().enumerate() {
        room.level = level[i];
        room.floor = top_floor - level[i] as f32 * LEVEL;
    }
    extra.retain(|&(a, b)| {
        let (pa, pb) = (port(&rooms[a], rooms[b].level), port(&rooms[b], rooms[a].level));
        (pa - pb).abs() <= (gap(&rooms[a], &rooms[b]) - 4.0) * 0.75
    });
    if version >= 41 && extra.is_empty() {
        // Level assignment can invalidate every proposed loop. Recover a
        // buildable return route when the footprint permits one, without
        // bypassing the core or its sealed puzzle antechamber.
        let candidate = edges.iter()
            .filter(|&&(_, a, b)| !tree.contains(&(a, b)))
            .filter(|&&(_, a, b)| {
                let touches = |i: Option<usize>| i.is_some_and(|i| a == i || b == i);
                !(touches(objective) || sealed_ante && touches(ante))
            })
            .filter(|&&(_, a, b)| {
                let (pa, pb) = (port(&rooms[a], rooms[b].level), port(&rooms[b], rooms[a].level));
                (pa - pb).abs() <= (gap(&rooms[a], &rooms[b]) - 4.0) * 0.75
            })
            .max_by_key(|&&(_, a, b)| (hops[a].abs_diff(hops[b]), hops[a] + hops[b]));
        if let Some(&(_, a, b)) = candidate { extra.push((a, b)); }
    }

    // States: the hazard is the job gone wrong.
    if version >= super::sprawl::VERSION {
        let mut loops: Vec<_> = edges.iter().copied().filter(|&(_, a, b)| {
            let touches = |i: Option<usize>| i.is_some_and(|i| a == i || b == i);
            !tree.contains(&(a, b)) && !touches(objective) && !(sealed_ante && touches(ante))
                && (port(&rooms[a], rooms[b].level) - port(&rooms[b], rooms[a].level)).abs()
                    <= (gap(&rooms[a], &rooms[b]) - 4.0) * 0.75
        }).collect();
        // Prefer shortcuts that eliminate the longest retracing of the tree.
        loops.sort_by_key(|&(_, a, b)| (std::cmp::Reverse(bfs(n, &tree, a)[b]), a, b));
        extra = loops.into_iter().take(if version >= 90 { 4 } else { 3 }).map(|(_, a, b)| (a, b)).collect();
    }
    let odds = job.odds();
    let deepest = *level.iter().max().unwrap_or(&0);
    for (i, room) in rooms.iter_mut().enumerate() {
        if version >= super::sprawl::VERSION && room.kind == Kind::Scriptorium { continue; }
        if matches!(room.kind, Kind::Entry | Kind::Reveal | Kind::Core | Kind::Orrery | Kind::Chasm) || room.kind.is_puzzle() || Some(i) == ante {
            continue;
        }
        let x = hash2(i as i32, 9, seed.wrapping_add(53));
        room.damage = if job == Job::Undercroft && room.level == deepest && deepest > 0 {
            Damage::Flooded
        } else if x < odds.collapsed {
            Damage::Collapsed
        } else if x < odds.collapsed + odds.flooded {
            Damage::Flooded
        } else if x < odds.collapsed + odds.flooded + odds.overgrown {
            Damage::Overgrown
        } else if x < odds.collapsed + odds.flooded + odds.overgrown + odds.breach {
            Damage::Breach
        } else {
            Damage::Intact
        };
    }

    // Generator 294: locks on the critical path, keys in caches before them.
    let quest = if version >= super::quest::VERSION {
        let eligible: Vec<bool> = rooms.iter().enumerate().map(|(i, room)| {
            let special = matches!(room.kind, Kind::Entry | Kind::Reveal | Kind::Core | Kind::Chasm | Kind::Orrery
                | Kind::Cells | Kind::Cistern | Kind::Sluice | Kind::Scriptorium) || room.kind.is_puzzle();
            !special && Some(i) != ante && matches!(room.damage, Damage::Intact | Damage::Overgrown)
        }).collect();
        super::quest::assign(&tree, &hops, entry, objective, &eligible, hash2(n as i32, 41, seed))
    } else {
        super::quest::Quest::default()
    };
    // A loop between two lock regions would walk around a door: seal it.
    let lock_region = (!quest.doors.is_empty()).then(|| {
        let cut: Vec<(usize, usize)> = quest.doors.iter().map(|(e, _)| *e).collect();
        super::quest::regions(n, &tree, &cut, entry)
    });
    let mut corrs = Vec::new();
    for &(a, b) in &tree {
        link(&rooms, a, b, seed, quest.door_on(a, b), &mut corrs);
    }
    for (k, &(a, b)) in extra.iter().enumerate() {
        let x = hash2(a as i32, b as i32, seed.wrapping_add(61));
        let plug = if version >= 90 && k < 2 || version >= super::sprawl::VERSION && k == 0 { None } else { match job {
            Job::Quarantine => Some(B::Planks),
            _ if x < 0.3 => Some(B::Gravel),
            _ => None,
        } };
        let plug = if lock_region.as_ref().is_some_and(|l| l[a] != l[b]) { Some(B::TombStone) } else { plug };
        link(&rooms, a, b, seed.wrapping_add(k as u32 + 1), plug, &mut corrs);
    }
    // Dead ends hold a cache. Some intact side rooms off loop routes also
    // reward taking the long way around in newer worlds.
    for (i, room) in rooms.iter_mut().enumerate() {
        let ways = tree.iter().chain(&extra).filter(|&&(a, b)| a == i || b == i).count();
        let special = matches!(room.kind, Kind::Entry | Kind::Reveal | Kind::Core | Kind::Chasm | Kind::Orrery | Kind::Cells | Kind::Cistern | Kind::Sluice) || room.kind.is_puzzle();
        let side_cache = version >= super::TREASURE_VERSION && ways == 2
            && room.damage == Damage::Intact && hash2(i as i32, 27, seed) < 0.45;
        room.cache = (ways == 1 || side_cache) && !special && Some(i) != ante
            && !(version >= super::sprawl::VERSION && room.kind == Kind::Scriptorium);
    }
    for &(room, _) in &quest.keys {
        rooms[room].cache = true;
    }
    for room in rooms.iter_mut().filter(|r| r.kind == Kind::Chasm) {
        room.rift = super::chasm::depth(&corrs, room);
    }
    for room in &mut rooms {
        room.stair = super::gallery::choose(version, room, &corrs);
        if version >= super::fine::VERSION && room.kind == Kind::Orrery && super::starry::plan(room) == super::starry::Plan::Islands {
            room.rift = super::starry::void_depth(&corrs, room);
            room.stair = super::escape::choose(&corrs, room, room.rift);
            if room.stair == super::escape::NONE {
                // Walkways everywhere: a void one step deep needs no stair.
                room.rift = 1.0;
            }
        }
    }

    let (ex, ez) = rooms[entry].centre();
    let rv = &rooms[reveal];
    let guard = ante.map(|a| neighbours(a, &tree).into_iter().find(|&p| Some(p) != objective).unwrap_or(reveal)).unwrap_or(reveal);
    let gr = &rooms[guard];
    let guard_point = if guard == reveal {
        Vec3::new(rv.centre().0 - rv.w() * 0.38, rv.floor + 1.0, rv.centre().1)
    } else {
        Vec3::new(gr.centre().0, gr.floor + 1.0, gr.centre().1)
    };
    let anchors = [guard_point, Vec3::new(rv.centre().0 + rv.w() * 0.38, rv.floor + 1.0, rv.centre().1)];

    let mut min = Vec3::new(ex - 10.0, f32::MAX, ez - 10.0);
    let mut max = Vec3::new(ex + 10.0, ground + 14.0, ez + 10.0);
    for room in &rooms {
        min = min.min(Vec3::new(room.x0 - WALL - 1.0, room.floor - room.sub() - WALL - 2.0, room.z0 - WALL - 1.0));
        max = max.max(Vec3::new(room.x1 + WALL + 1.0, room.floor + room.top() + WALL + 1.0, room.z1 + WALL + 1.0));
    }
    for c in &corrs {
        min = min.min(Vec3::new(c.x0 - 2.0, c.fa.min(c.fb) - 3.0, c.z0 - 2.0));
        max = max.max(Vec3::new(c.x1 + 2.0, c.fa.max(c.fb) + c.h + 3.0, c.z1 + 2.0));
    }
    let index = Index::build(min, max, &rooms, &corrs);
    Complex {
        version,
        job,
        pal: job.palette(),
        seed,
        ground,
        rooms,
        corrs,
        shaft: (ex, ez, ground),
        gatehouse: super::super::gatehouse(seed),
        entry,
        reveal,
        objective,
        anchors,
        quest,
        min,
        max,
        index,
    }
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn bfs(n: usize, edges: &[(usize, usize)], start: usize) -> Vec<u32> {
    let mut hops = vec![u32::MAX; n];
    hops[start] = 0;
    for &(child, from) in &bfs_order(n, edges, start) {
        hops[child] = hops[from] + 1;
    }
    hops
}

/// (child, parent) pairs in breadth-first order from `start`.
fn bfs_order(n: usize, edges: &[(usize, usize)], start: usize) -> Vec<(usize, usize)> {
    let mut seen = vec![false; n];
    seen[start] = true;
    let mut queue = std::collections::VecDeque::from([start]);
    let mut out = Vec::new();
    while let Some(i) = queue.pop_front() {
        for &(a, b) in edges {
            let j = if a == i { b } else if b == i { a } else { continue };
            if !seen[j] {
                seen[j] = true;
                out.push((j, i));
                queue.push_back(j);
            }
        }
    }
    out
}

/// Level of the doorway a room offers to a neighbour on `other` level: the
/// reveal hall meets level 0 at its balcony and deeper levels at its floor.
fn port_level(kind: Kind, own: u8, other: u8) -> f32 {
    if kind == Kind::Reveal && other == 0 { 0.0 } else { own as f32 }
}

/// Height of the doorway `room` offers to a neighbour on `other` level.
pub(super) fn port(room: &Room, other: u8) -> f32 {
    if room.kind == Kind::Reveal && other == 0 { room.floor + LEVEL } else { room.floor }
}

/// Distance between facing walls of two neighbours on the lattice.
fn gap(a: &Room, b: &Room) -> f32 {
    let dx = (a.x0.max(b.x0) - a.x1.min(b.x1)).max(0.0);
    let dz = (a.z0.max(b.z0) - a.z1.min(b.z1)).max(0.0);
    dx.max(dz)
}

/// Corridor between neighbouring rooms. Doors sit at hashed points along each
/// wall, never only at the midpoint; straight when the walls overlap enough,
/// a dog-leg otherwise. Floors ramp along the path.
fn link(rooms: &[Room], a: usize, b: usize, seed: u32, plug: Option<B>, out: &mut Vec<Corr>) {
    let h = |k: u32| hash2(a as i32 * 31 + b as i32, k as i32, seed.wrapping_add(71));
    let half = [1.0, 1.5, 1.5, 2.0][(h(1) * 4.0) as usize % 4];
    let height = 3.2 + (h(2) * 1.2).round();
    let pointed = h(3) < 0.4;
    let (ra, rb) = (&rooms[a], &rooms[b]);
    let (fa, fb) = (port(ra, rb.level), port(rb, ra.level));
    let horizontal = (ra.x0.max(rb.x0) - ra.x1.min(rb.x1)) > (ra.z0.max(rb.z0) - ra.z1.min(rb.z1));
    // Work in (along, across) space; flip back when emitting.
    let a_first = if horizontal { ra.x0 < rb.x0 } else { ra.z0 < rb.z0 };
    let (lo, hi, flo, fhi) = if a_first {
        (ra, rb, fa, fb)
    } else {
        (rb, ra, fb, fa)
    };
    let span = |r: &Room| if horizontal { (r.z0, r.z1) } else { (r.x0, r.x1) };
    let (s, e) = if horizontal { (lo.x1, hi.x0) } else { (lo.z1, hi.z0) };
    let margin = half + 2.0;
    let pick = |(p0, p1): (f32, f32), t: f32| (p0 + margin + t * ((p1 - p0) - 2.0 * margin).max(0.0)).round();
    let (sl, sh) = (span(lo), span(hi));
    let (ov0, ov1) = (sl.0.max(sh.0), sl.1.min(sh.1));
    let (ta, tb) = if ov1 - ov0 >= 2.0 * margin + 1.0 {
        let t = pick((ov0, ov1), h(4));
        (t, t)
    } else {
        (pick(sl, h(4)), pick(sh, h(5)))
    };
    let mid = (s + (e - s) * (0.35 + h(6) * 0.3)).round();
    // Ramps run between flat landings at each bend, so overlapping legs agree.
    let (l1, l2, l3) = ((mid - s - half).max(0.0), ((tb - ta).abs() - 2.0 * half).max(0.0), (e - mid - half).max(0.0));
    let tot = (l1 + l2 + l3).max(1e-3);
    let (j1, j2) = (flo + (fhi - flo) * l1 / tot, flo + (fhi - flo) * (l1 + l2) / tot);
    let mut push = |c0: f32, c1: f32, across: f32, a0: f32, a1: f32, f0: f32, f1: f32, run_along: bool| {
        let along_x = run_along == horizontal;
        let (x0, x1, z0, z1) = if along_x { (c0, c1, across - half, across + half) } else { (across - half, across + half, c0, c1) };
        out.push(Corr { x0, x1, z0, z1, along_x, a0, a1, fa: f0, fb: f1, h: height, pointed, plug: None });
    };
    push(s - 0.6, mid + half, ta, s, mid - half, flo, j1, true);
    let bent = (tb - ta).abs() > 0.01;
    if bent {
        let (lo_t, hi_t) = (ta.min(tb), ta.max(tb));
        let (z0, z1) = if l2 > 0.0 { (lo_t + half, hi_t - half) } else { ((lo_t + hi_t) * 0.5, (lo_t + hi_t) * 0.5) };
        let (f0, f1) = if ta <= tb { (j1, j2) } else { (j2, j1) };
        push(lo_t - half, hi_t + half, mid, z0, z1, f0, f1, false);
    }
    push(mid - half, e + 0.6, tb, mid + half, e, j2, fhi, true);
    if let Some(p) = plug {
        let k = out.len() - 1 - usize::from(bent);
        out[k].plug = Some(p);
    }
}
