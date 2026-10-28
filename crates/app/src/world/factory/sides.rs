//! Per-side conduit connections. A conduit stores one "closed" bit per face
//! in `Node::target` (bit i is `DIRS[i]`); machines never close a face.
use super::{DIRS, Factory, Kind, anchor, step};
use crate::world::World;
use glam::IVec3;

pub const SIDE_BITS: u32 = (1 << 6) - 1;

pub fn is_conduit(kind: Kind) -> bool {
    matches!(kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe)
}

impl Factory {
    /// True when the node at `p` has not closed face `side`.
    pub fn side_open(&self, p: IVec3, side: usize) -> bool {
        self.nodes
            .get(&p)
            .is_none_or(|n| !is_conduit(n.kind) || n.target >> side & 1 == 0)
    }

    /// True when the conduit at `p` and its neighbour along `side` both leave the shared face open.
    pub fn link_open(&self, p: IVec3, side: usize, size: i32) -> bool {
        self.side_open(p, side) && self.side_open(p + DIRS[side] * size, side ^ 1)
    }
}

/// The conduit and face a player is aiming at, from the hit cell and the empty cell in front of it.
pub fn side_toward(world: &World, cell: IVec3, front: IVec3) -> Option<(IVec3, u8)> {
    let size = step(world);
    let p = anchor(cell, size);
    if !is_conduit(world.factory.nodes.get(&p)?.kind) {
        return None;
    }
    let out = front - cell;
    let face = if anchor(front, size) != p && out.abs().element_sum() == 1 {
        out
    } else {
        let centre = (cell * 2 + IVec3::ONE) - (p * 2 + IVec3::splat(size));
        let axis = centre.abs().max_element();
        let pick = |v: i32| if v.abs() == axis { v.signum() } else { 0 };
        let mut face = IVec3::new(pick(centre.x), 0, 0);
        if face == IVec3::ZERO {
            face = IVec3::new(0, pick(centre.y), 0);
        }
        if face == IVec3::ZERO {
            face = IVec3::new(0, 0, pick(centre.z));
        }
        face
    };
    let side = DIRS.iter().position(|d| *d == face)?;
    Some((p, side as u8))
}

/// Flips a face of the conduit at `p`; networks rebuild on the next tick.
pub fn toggle_side(world: &mut World, p: IVec3, side: u8) -> bool {
    let Some(n) = world.factory.nodes.get_mut(&p) else {
        return false;
    };
    if !is_conduit(n.kind) || side >= 6 {
        return false;
    }
    n.target ^= 1 << side;
    let closed = n.target >> side & 1 == 1;
    world.factory.topology = false;
    world
        .expedition
        .say(if closed { "Side closed." } else { "Side open." });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::factory::Node;

    fn line(kind: Kind, len: i32) -> Factory {
        let mut f = Factory::default();
        for x in 0..len {
            f.nodes.insert(IVec3::new(x, 0, 0), Node::new(kind));
        }
        f
    }

    #[test]
    fn a_closed_face_splits_the_network() {
        let mut f = line(Kind::Cable, 4);
        assert_eq!(f.networks(Kind::Cable, 1).len(), 1);
        f.nodes.get_mut(&IVec3::new(1, 0, 0)).unwrap().target = 1;
        let nets = f.networks(Kind::Cable, 1);
        assert_eq!(nets.len(), 2);
        let mut sizes: Vec<_> = nets.iter().map(|n| n.conduits.len()).collect();
        sizes.sort();
        assert_eq!(sizes, [2, 2]);
    }

    #[test]
    fn either_end_closing_the_shared_face_is_enough() {
        let mut f = line(Kind::Pipe, 2);
        f.nodes.get_mut(&IVec3::new(1, 0, 0)).unwrap().target = 1 << 1;
        assert!(!f.link_open(IVec3::ZERO, 0, 1));
        assert_eq!(f.networks(Kind::Pipe, 1).len(), 2);
    }

    #[test]
    fn a_closed_face_drops_the_adjacent_machine_port() {
        let mut f = line(Kind::Pipe, 1);
        f.nodes.insert(IVec3::new(1, 0, 0), Node::new(Kind::Storage));
        assert_eq!(f.networks(Kind::Pipe, 1)[0].ports.len(), 1);
        f.nodes.get_mut(&IVec3::ZERO).unwrap().target = 1;
        assert!(f.networks(Kind::Pipe, 1)[0].ports.is_empty());
    }

    #[test]
    fn closed_faces_block_the_packet_route() {
        use crate::world::factory::transit::find_path;
        let mut f = line(Kind::Pipe, 3);
        f.nodes.insert(IVec3::new(-1, 0, 0), Node::new(Kind::Storage));
        f.nodes.insert(IVec3::new(3, 0, 0), Node::new(Kind::Furnace));
        let net = f.networks(Kind::Pipe, 1).remove(0);
        assert!(find_path(&net, IVec3::new(-1, 0, 0), IVec3::new(3, 0, 0), 1, None).is_some());
        f.nodes.get_mut(&IVec3::new(1, 0, 0)).unwrap().target = 1;
        let nets = f.networks(Kind::Pipe, 1);
        assert!(nets.iter().all(|n| find_path(n, IVec3::new(-1, 0, 0), IVec3::new(3, 0, 0), 1, None).is_none()));
    }
}
