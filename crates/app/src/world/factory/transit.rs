//! Items in transit through item pipes. A packet leaves its source, travels
//! the pipe cells one after another at a fixed speed, and is delivered when it
//! reaches the destination. Capacity at the destination is reserved at launch,
//! so a delivery can never overflow and nothing is lost to backpressure.
use super::{DIRS, Network};
use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};

/// Cells (metres) per second. Mechanical-pipe pace: fast enough to keep a
/// bus moving, slow enough that the player can watch a single ingot go by.
pub const SPEED: f32 = 4.0;
/// Hard cap on simultaneous packets; launches stall rather than grow the list.
pub const MAX_PACKETS: usize = 768;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Packet {
    pub item: usize,
    pub amount: u32,
    pub src: IVec3,
    pub dst: IVec3,
    /// Machine cell, every pipe cell in order, machine cell.
    pub path: Vec<IVec3>,
    /// Segments travelled; `path.len() - 1` means arrived.
    pub progress: f32,
}

impl Packet {
    pub fn arrived(&self) -> bool {
        self.progress >= (self.path.len().saturating_sub(1)) as f32
    }

    /// Seconds until delivery at the current speed.
    #[cfg(test)]
    pub fn eta(&self) -> f32 {
        ((self.path.len().saturating_sub(1)) as f32 - self.progress).max(0.0) / SPEED
    }

    /// World position in metres. `cell` is the metre size of one factory cell
    /// per voxel step, `scale` the voxel size.
    pub fn position(&self, scale: f32) -> Vec3 {
        let centre = |c: IVec3| c.as_vec3() * scale + Vec3::splat(0.5);
        let last = self.path.len().saturating_sub(1);
        if last == 0 {
            return self.path.first().map_or(Vec3::ZERO, |c| centre(*c));
        }
        let seg = (self.progress.floor().max(0.0) as usize).min(last - 1);
        let t = (self.progress - seg as f32).clamp(0.0, 1.0);
        centre(self.path[seg]).lerp(centre(self.path[seg + 1]), t)
    }
}

/// Shortest pipe route from machine `from` to machine `to` inside `network`.
/// `via` pins the first pipe cell (extractors pull through their own cell).
/// Returns `[from, pipe.., to]`, or `None` when the machines are not joined.
pub(super) fn find_path(
    network: &Network,
    from: IVec3,
    to: IVec3,
    size: i32,
    via: Option<IVec3>,
) -> Option<Vec<IVec3>> {
    let pipes = &network.pipes;
    // `leaving` is the machine face the packet exits. Entry uses the other set,
    // so an In face can accept and an Out face can supply on the same machine.
    let open = |cell: IVec3, machine: IVec3, leaving: bool| {
        let linked = DIRS.iter().any(|d| cell + *d * size == machine)
            && !network.closed.contains(&(cell, machine));
        if !linked {
            return false;
        }
        let blocked = if leaving { &network.no_extract } else { &network.no_insert };
        !blocked.contains(&(cell, machine))
    };
    let starts: Vec<IVec3> = match via {
        Some(v) if pipes.contains(&v) && open(v, from, true) => vec![v],
        Some(_) => return None,
        None => {
            let mut s: Vec<_> = pipes
                .iter()
                .copied()
                .filter(|c| open(*c, from, true))
                .collect();
            s.sort_by_key(|p| (p.x, p.y, p.z));
            s
        }
    };
    let mut parent: HashMap<IVec3, IVec3> = HashMap::new();
    let mut seen: HashSet<IVec3> = starts.iter().copied().collect();
    let mut queue: VecDeque<IVec3> = starts.into_iter().collect();
    while let Some(c) = queue.pop_front() {
        if open(c, to, false) {
            let mut route = vec![c];
            let mut cur = c;
            while let Some(&p) = parent.get(&cur) {
                route.push(p);
                cur = p;
            }
            route.reverse();
            let mut path = Vec::with_capacity(route.len() + 2);
            path.push(from);
            path.extend(route);
            path.push(to);
            return Some(path);
        }
        for d in DIRS {
            let n = c + d * size;
            if pipes.contains(&n) && !network.closed.contains(&(c, n)) && seen.insert(n) {
                parent.insert(n, c);
                queue.push_back(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(len: i32) -> Network {
        Network::new(Vec::new(), (1..=len).map(|x| IVec3::new(x, 0, 0)).collect())
    }

    #[test]
    fn straight_path_runs_through_every_pipe() {
        let net = line(4);
        let path = find_path(&net, IVec3::new(0, 0, 0), IVec3::new(5, 0, 0), 1, None).unwrap();
        assert_eq!(path.len(), 6);
        assert_eq!(path[0], IVec3::new(0, 0, 0));
        assert_eq!(path[5], IVec3::new(5, 0, 0));
    }

    #[test]
    fn takes_the_shorter_branch() {
        // A 3-cell shortcut and a 7-cell detour between the same machines.
        let mut conduits: Vec<IVec3> = (1..=3).map(|x| IVec3::new(x, 0, 0)).collect();
        conduits.extend([
            IVec3::new(1, 1, 0),
            IVec3::new(1, 2, 0),
            IVec3::new(2, 2, 0),
            IVec3::new(3, 2, 0),
            IVec3::new(3, 1, 0),
        ]);
        let net = Network::new(Vec::new(), conduits);
        let path = find_path(&net, IVec3::ZERO, IVec3::new(4, 0, 0), 1, None).unwrap();
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn disconnected_machines_have_no_path() {
        let net = line(2);
        assert!(find_path(&net, IVec3::ZERO, IVec3::new(9, 0, 0), 1, None).is_none());
    }

    #[test]
    fn via_pins_the_first_pipe() {
        let net = line(3);
        // Pulling through the far pipe is rejected: it does not touch the source.
        assert!(
            find_path(
                &net,
                IVec3::ZERO,
                IVec3::new(4, 0, 0),
                1,
                Some(IVec3::new(3, 0, 0))
            )
            .is_none()
        );
        let p = find_path(
            &net,
            IVec3::ZERO,
            IVec3::new(4, 0, 0),
            1,
            Some(IVec3::new(1, 0, 0)),
        )
        .unwrap();
        assert_eq!(p[1], IVec3::new(1, 0, 0));
    }

    #[test]
    fn packet_moves_at_the_stated_speed() {
        let net = line(3);
        let path = find_path(&net, IVec3::ZERO, IVec3::new(4, 0, 0), 1, None).unwrap();
        let mut p = Packet {
            item: 6,
            amount: 64,
            src: path[0],
            dst: path[4],
            path,
            progress: 0.0,
        };
        assert!((p.eta() - 4.0 / SPEED).abs() < 1e-5, "4 segments");
        // Half a second at 4 cells/s is two cells: centre of cell 2, i.e. x = 2.5.
        p.progress += SPEED * 0.5;
        assert!((p.position(1.0).x - 2.5).abs() < 1e-5);
        assert!(!p.arrived());
        p.progress += SPEED * 0.5;
        assert!(p.arrived());
    }

    #[test]
    fn position_honours_voxel_scale() {
        let path = vec![IVec3::new(0, 0, 0), IVec3::new(4, 0, 0)];
        let p = Packet {
            item: 0,
            amount: 1,
            src: path[0],
            dst: path[1],
            path,
            progress: 0.5,
        };
        // Cells sit four voxels apart at 0.25 m per voxel: one metre.
        assert!((p.position(0.25).x - 1.0).abs() < 1e-5);
    }

    use super::super::{CAPACITY, Factory, Kind, Node};

    fn put(f: &mut Factory, x: i32, kind: Kind) -> &mut Node {
        f.nodes
            .entry(IVec3::new(x, 0, 0))
            .or_insert_with(|| Node::new(kind))
    }

    /// Everything of one resource: crates + machines + in-flight packets.
    fn total(f: &Factory, item: usize) -> u32 {
        f.nodes
            .values()
            .map(|n| n.input[item] + n.output[item])
            .sum::<u32>()
            + f.packets
                .iter()
                .filter(|k| k.item == item)
                .map(|k| k.amount)
                .sum::<u32>()
    }

    #[test]
    fn machine_output_travels_the_pipe_before_arriving() {
        let mut f = Factory::default();
        put(&mut f, 0, Kind::Furnace).output[6] = 64;
        for x in 1..=3 {
            put(&mut f, x, Kind::Pipe);
        }
        put(&mut f, 4, Kind::Storage);
        f.tick(0.25, 1);
        assert_eq!(f.packets.len(), 1, "launched");
        assert_eq!(
            f.nodes[&IVec3::new(4, 0, 0)].input[6],
            0,
            "not delivered yet"
        );
        assert_eq!(total(&f, 6), 64, "conserved in flight");
        // 4 segments at 4 cells/s is one second. Step just short of it.
        for _ in 0..7 {
            f.tick(0.1, 1);
            assert_eq!(total(&f, 6), 64);
        }
        assert_eq!(
            f.nodes[&IVec3::new(4, 0, 0)].input[6],
            0,
            "still travelling at 0.7 s"
        );
        for _ in 0..5 {
            f.tick(0.1, 1);
        }
        assert_eq!(f.nodes[&IVec3::new(4, 0, 0)].input[6], 64, "arrived");
        assert!(f.packets.is_empty());
        assert_eq!(total(&f, 6), 64);
    }

    #[test]
    fn extractor_pulls_from_a_crate_and_a_plain_pipe_does_not() {
        for (mode, expect_moved) in [(0u32, false), (1, true)] {
            let mut f = Factory::default();
            put(&mut f, 0, Kind::Storage).input[9] = 64;
            let pipe = put(&mut f, 1, Kind::Pipe);
            pipe.mode = mode;
            put(&mut f, 2, Kind::Pipe);
            put(&mut f, 3, Kind::Furnace);
            for _ in 0..40 {
                f.tick(0.25, 1);
            }
            let furnace = f.nodes[&IVec3::new(3, 0, 0)].input[9];
            assert_eq!(furnace > 0, expect_moved, "mode {mode}");
            assert_eq!(total(&f, 9), 64);
        }
    }

    #[test]
    fn extractor_filter_picks_one_resource() {
        let mut f = Factory::default();
        let crate_ = put(&mut f, 0, Kind::Storage);
        crate_.input[9] = 64;
        crate_.input[10] = 64;
        let pipe = put(&mut f, 1, Kind::Pipe);
        pipe.mode = 1;
        pipe.filter = Some(10);
        put(&mut f, 2, Kind::Furnace);
        for _ in 0..40 {
            f.tick(0.25, 1);
        }
        let furnace = &f.nodes[&IVec3::new(2, 0, 0)];
        assert_eq!((furnace.input[9], furnace.input[10]), (0, 64));
    }

    #[test]
    fn reservation_prevents_overfilling_a_destination() {
        let mut f = Factory::default();
        put(&mut f, 0, Kind::Storage).input[9] = CAPACITY;
        let pipe = put(&mut f, 1, Kind::Pipe);
        pipe.mode = 1;
        put(&mut f, 2, Kind::Furnace).input[9] = CAPACITY - 64;
        for _ in 0..80 {
            f.tick(0.25, 1);
            let furnace = &f.nodes[&IVec3::new(2, 0, 0)];
            let in_flight: u32 = f.packets.iter().map(|k| k.amount).sum();
            assert!(furnace.input[9] + in_flight <= CAPACITY, "overbooked");
        }
        assert_eq!(f.nodes[&IVec3::new(2, 0, 0)].input[9], CAPACITY);
        assert_eq!(total(&f, 9), CAPACITY * 2 - 64);
    }

    #[test]
    fn breaking_a_pipe_refunds_the_items_in_it() {
        let mut f = Factory::default();
        put(&mut f, 0, Kind::Furnace).output[6] = 64;
        for x in 1..=3 {
            put(&mut f, x, Kind::Pipe);
        }
        put(&mut f, 4, Kind::Storage);
        f.tick(0.25, 1);
        assert_eq!(f.packets.len(), 1);
        f.nodes.remove(&IVec3::new(2, 0, 0));
        f.topology = false;
        f.tick(0.25, 1);
        assert!(f.packets.is_empty());
        assert_eq!(f.take_refunds(), vec![(6, 64)]);
        assert_eq!(f.nodes[&IVec3::new(4, 0, 0)].input[6], 0);
    }

    #[test]
    fn output_goes_to_the_nearer_machine() {
        let mut f = Factory::default();
        put(&mut f, 0, Kind::Crusher).output[9] = 64;
        put(&mut f, 1, Kind::Pipe);
        put(&mut f, 2, Kind::Furnace);
        for x in 3..=6 {
            put(&mut f, x, Kind::Pipe);
        }
        put(&mut f, 7, Kind::Furnace);
        for _ in 0..40 {
            f.tick(0.25, 1);
        }
        assert_eq!(f.nodes[&IVec3::new(2, 0, 0)].input[9], 64);
        assert_eq!(f.nodes[&IVec3::new(7, 0, 0)].input[9], 0);
        assert_eq!(total(&f, 9), 64);
    }

    #[test]
    fn distributor_evens_two_containers() {
        let mut f = Factory::default();
        put(&mut f, 0, Kind::Storage).input[6] = 128;
        put(&mut f, 1, Kind::Pipe).mode = 2;
        put(&mut f, 2, Kind::Pipe);
        put(&mut f, 3, Kind::Storage);
        for _ in 0..80 {
            f.tick(0.25, 1);
        }
        let a = f.nodes[&IVec3::new(0, 0, 0)].input[6];
        let b = f.nodes[&IVec3::new(3, 0, 0)].input[6];
        assert_eq!(a + b + f.packets.iter().map(|p| p.amount).sum::<u32>(), 128);
        assert!(
            (a as i32 - b as i32).abs() <= 64,
            "crates diverged: {a} vs {b}"
        );
    }

    #[test]
    fn barrel_accepts_only_the_first_shipment() {
        let mut f = Factory::default();
        let furnace = put(&mut f, 0, Kind::Furnace);
        furnace.output[6] = 64;
        furnace.output[7] = 64;
        put(&mut f, 1, Kind::Pipe);
        put(&mut f, 2, Kind::Storage).mode = 2;
        for _ in 0..40 {
            f.tick(0.25, 1);
        }
        let barrel = &f.nodes[&IVec3::new(2, 0, 0)];
        assert_eq!(barrel.filter, Some(6));
        assert_eq!(barrel.input[6], 64);
        assert_eq!(barrel.input[7], 0);
        assert_eq!(f.nodes[&IVec3::ZERO].output[7], 64);
        assert_eq!(total(&f, 6) + total(&f, 7), 128);
    }

    #[test]
    fn saving_mid_flight_keeps_the_items() {
        let mut f = Factory::default();
        put(&mut f, 0, Kind::Furnace).output[6] = 64;
        for x in 1..=3 {
            put(&mut f, x, Kind::Pipe);
        }
        put(&mut f, 4, Kind::Storage);
        f.tick(0.25, 1);
        assert_eq!(f.packets.len(), 1);
        let mut bytes = Vec::new();
        f.encode(&mut bytes);
        let mut words = bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect::<Vec<_>>()
            .into_iter();
        let mut read = || words.next().ok_or_else(|| anyhow::anyhow!("short"));
        let back = Factory::decode(&mut read, 1, super::super::FACTORY_FORMAT).expect("decodes");
        assert_eq!(back.nodes[&IVec3::new(4, 0, 0)].input[6], 64);
        assert_eq!(back.nodes[&IVec3::ZERO].output[6], 0);
    }
}
