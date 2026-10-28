//! Item routing on a large backed-up bus: the cached pass against the reference.
use super::{CAPACITY, DIRS, Factory, Kind, Node, RESOURCE_COUNT};
use glam::IVec3;
use std::time::Duration;

fn put(f: &mut Factory, p: IVec3, kind: Kind) -> &mut Node {
    f.nodes.entry(p).or_insert_with(|| Node::new(kind))
}

/// Two pipe rails joined at every cell (a ladder, so many routes tie), with a
/// machine on each open face. Most buffers are full, so most outputs wait.
fn bus(length: i32) -> Factory {
    let mut f = Factory::default();
    for x in 0..=length {
        for y in 0..2 {
            let pipe = put(&mut f, IVec3::new(x, y, 0), Kind::Pipe);
            if x % 17 == 3 && y == 0 {
                pipe.mode = 1;
                pipe.filter = (x % 2 == 0).then_some((x as usize / 17) % RESOURCE_COUNT);
            }
            if x == 50 && y == 1 {
                pipe.mode = 2;
            }
            if x % 23 == 5 {
                // Closes the +z face toward the machine on that side.
                pipe.target = 1 << 4;
            }
            if x % 31 == 7 && y == 1 {
                // Closes the rung down to the lower rail.
                pipe.target |= 1 << 3;
            }
        }
        for (side, z) in [(0, 1), (1, -1)] {
            for y in 0..2 {
                let h = ((x * 7 + side * 3 + y * 5) % 11) as usize;
                let item = (x as usize * 3 + side as usize + y as usize) % RESOURCE_COUNT;
                let p = IVec3::new(x, y, z);
                let kind = match h {
                    0..=2 => Kind::Furnace,
                    3..=4 => Kind::Crusher,
                    5..=7 => Kind::Storage,
                    8 => Kind::Feeder,
                    _ => Kind::Storage,
                };
                let n = put(&mut f, p, kind);
                match h {
                    0..=4 => {
                        n.output[item] = CAPACITY - (x as u32 % 5) * 64;
                        n.output[(item + 5) % RESOURCE_COUNT] = 64 * (x as u32 % 3);
                        // Inputs full except every seventh machine.
                        let fill = if x % 7 == 0 { 0 } else { CAPACITY };
                        n.input = [fill; RESOURCE_COUNT];
                    }
                    5..=7 => {
                        n.input = [CAPACITY; RESOURCE_COUNT];
                        if x % 9 == 0 {
                            n.input[item] = 0;
                        }
                        if x % 13 == 1 {
                            n.filter = Some(item);
                        }
                    }
                    8 => {
                        n.output[item] = 640;
                        n.filter = (x % 2 == 0).then_some(item);
                    }
                    _ => {
                        n.mode = 2;
                        if x % 4 == 0 {
                            n.input[item] = 128;
                        }
                    }
                }
                if x % 29 == 11 {
                    n.enabled = false;
                }
            }
        }
    }
    f
}

fn fnv(hash: &mut u64, words: impl IntoIterator<Item = u32>) {
    for w in words {
        for b in w.to_le_bytes() {
            *hash ^= u64::from(b);
            *hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
}

fn checksum(f: &Factory) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325;
    let mut cells: Vec<_> = f.nodes.keys().copied().collect();
    cells.sort_by_key(|p| (p.x, p.y, p.z));
    for p in cells {
        let n = &f.nodes[&p];
        fnv(&mut h, [p.x as u32, p.y as u32, p.z as u32, n.kind as u32]);
        fnv(&mut h, n.input);
        fnv(&mut h, n.output);
        fnv(&mut h, [n.filter.map_or(u32::MAX, |i| i as u32), n.activity.to_bits()]);
        fnv(&mut h, n.status.bytes().map(u32::from));
    }
    for k in &f.packets {
        fnv(&mut h, [k.item as u32, k.amount, k.progress.to_bits(), k.path.len() as u32]);
        fnv(&mut h, k.path.iter().flat_map(|c| [c.x as u32, c.y as u32, c.z as u32]));
    }
    let mut reserved: Vec<_> = f.reserved.iter().map(|(&(q, i), &n)| (q.x, q.y, q.z, i, n)).collect();
    reserved.sort_unstable();
    for (x, y, z, i, n) in reserved {
        fnv(&mut h, [x as u32, y as u32, z as u32, i as u32, n]);
    }
    for &(i, n) in &f.refunds {
        fnv(&mut h, [i as u32, n]);
    }
    h
}

/// Checksums after each of `passes` routing passes, the packets launched,
/// and the total time inside `tick` and the worst pass after the first (which
/// also builds the networks).
fn run(f: &mut Factory, passes: usize) -> (Vec<u64>, usize, Duration, Duration) {
    let mut sums = Vec::with_capacity(passes);
    let (mut launched, mut total, mut worst) = (0, Duration::ZERO, Duration::ZERO);
    for pass in 0..passes {
        let start = std::time::Instant::now();
        f.tick(0.25, 1);
        let spent = start.elapsed();
        total += spent;
        if pass > 0 {
            worst = worst.max(spent);
        }
        // Packets advance before routing, so only this pass's launches sit at 0.
        launched += f.packets.iter().filter(|k| k.progress == 0.0).count();
        sums.push(checksum(f));
    }
    (sums, launched, total, worst)
}

/// The cached routing pass leaves the factory byte-for-byte where the
/// uncached reference pass does, pass after pass. The long bus saturates the
/// packet cap; the short one delivers quickly and keeps traffic moving.
#[test]
fn large_backed_up_bus_routes_identically() {
    const PASSES: usize = 240;
    for length in [160, 30] {
        let mut cached = bus(length);
        let mut reference = bus(length);
        reference.reference_routing = true;
        let machines = cached.nodes.values().filter(|n| !n.kind.conduit()).count();
        let (want, launched, slow, slow_worst) = run(&mut reference, PASSES);
        let (got, _, fast, fast_worst) = run(&mut cached, PASSES);
        println!(
            "bus {length}: {machines} machines, {PASSES} passes, {launched} launches: reference {slow:?} (worst pass {slow_worst:?}), cached {fast:?} (worst pass {fast_worst:?})"
        );
        if let Some(pass) = want.iter().zip(&got).position(|(a, b)| a != b) {
            panic!("bus {length}: routing diverged at pass {pass}");
        }
        assert!(launched > 300, "bus {length}: traffic moved: {launched}");
        let waiting = cached
            .nodes
            .values()
            .filter(|n| !n.kind.conduit() && n.output.iter().any(|&o| o > 0))
            .count();
        assert!(waiting * 4 > machines, "bus {length}: outputs stay backed up: {waiting}");
        if length == 160 {
            assert!(machines >= 300, "{machines} machines");
        }
    }
}

/// Every cached route equals the route `find_path` searches for, including
/// closed faces, the closed rung and extractor pulls through one pipe.
#[test]
fn cached_routes_match_find_path() {
    let mut f = bus(160);
    f.tick(0.25, 1);
    let mut routes = super::routes::Routes::default();
    let mut checked = 0;
    for (ni, net) in f.items.iter().enumerate() {
        for &from in net.ports.iter().step_by(11) {
            for &to in &net.ports {
                if from == to {
                    continue;
                }
                let want = super::transit::find_path(net, from, to, 1, None);
                assert_eq!(routes.path(ni, net, 1, from, to, None), want, "{from} -> {to}");
                checked += 1;
            }
        }
        for &via in net.conduits.iter().step_by(7) {
            for d in DIRS {
                let from = via + d;
                for &to in net.ports.iter().step_by(3) {
                    let want = super::transit::find_path(net, from, to, 1, Some(via));
                    assert_eq!(routes.path(ni, net, 1, from, to, Some(via)), want, "{from} via {via} -> {to}");
                }
            }
        }
    }
    assert!(checked > 20_000, "{checked} pairs");
}

#[test]
fn directions_match_closed_face_bits() {
    // The fixture closes faces by index; keep the indices honest.
    assert_eq!(DIRS[4], IVec3::Z);
    assert_eq!(DIRS[3], IVec3::NEG_Y);
}
