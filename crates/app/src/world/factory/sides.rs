//! Per-side conduit connections and machine item faces.
//! A conduit stores one closed bit per face in `Node::target` (bit i is `DIRS[i]`).
//! A machine stores a two-bit item role per face in `Node::faces`.
use super::{DIRS, Factory, Kind, Node, anchor, step};
use crate::world::World;
use glam::IVec3;

/// Item role of one machine face. `Both` is the saved default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
    Both = 0,
    In = 1,
    Out = 2,
    Off = 3,
}

/// Axis names. +X is sunrise, +Z is the sun's constant offset, so these match
/// the usual tech-mod side labels.
pub const FACE_NAME: [&str; 6] = ["East", "West", "Up", "Down", "South", "North"];
pub const ROLE_NAME: [&str; 4] = ["Both", "In", "Out", "Off"];

pub fn role(n: &Node, face: usize) -> Role {
    match (n.faces >> (face * 2)) & 3 {
        1 => Role::In,
        2 => Role::Out,
        3 => Role::Off,
        _ => Role::Both,
    }
}

pub fn apply_role(n: &mut Node, face: u8, role: u8) -> bool {
    if face >= 6 || role > 3 || n.kind.conduit() || n.kind.structural() {
        return false;
    }
    let shift = u32::from(face) * 2;
    n.faces = (n.faces & !(3u32 << shift)) | (u32::from(role) << shift);
    true
}

/// Sets one item face and marks the networks dirty.
pub fn set_face(world: &mut World, p: IVec3, face: u8, role: u8) -> bool {
    let ok = world
        .factory
        .nodes
        .get_mut(&p)
        .is_some_and(|n| apply_role(n, face, role));
    if !ok {
        return false;
    }
    world.factory.topology = false;
    world.expedition.say(format!(
        "{}: {}",
        FACE_NAME[face as usize], ROLE_NAME[role as usize]
    ));
    true
}

pub const SIDE_BITS: u32 = (1 << 6) - 1;

pub fn is_conduit(kind: Kind) -> bool {
    super::ports::conduit(kind).is_some()
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

    /// Physical arm shared by picking, rendering and the side inspector.
    pub fn conduit_join(&self, p: IVec3, side: usize, size: i32) -> bool {
        let (Some(node), Some(other)) = (self.nodes.get(&p), self.nodes.get(&(p + DIRS[side] * size))) else {
            return false;
        };
        if !is_conduit(node.kind) || node.target & (1 << side) != 0
            || (is_conduit(other.kind) && other.target & (1 << (side ^ 1)) != 0)
        {
            return false;
        }
        if !super::ports::accepts(node.kind, other.kind) { return false; }
        if node.kind == Kind::ExhaustDuct { return true; }
        if other.kind == node.kind {
            return node.kind != Kind::Cable || node.upgrade == other.upgrade;
        }
        !other.kind.conduit() && !other.kind.structural()
            && (node.kind != Kind::Pipe || role(other, side ^ 1) != Role::Off)
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

    const IRON: usize = 6;

    fn belt() -> Factory {
        let mut f = Factory::default();
        let mut furnace = Node::new(Kind::Furnace);
        furnace.output[IRON] = 64;
        f.nodes.insert(IVec3::new(0, 0, 0), furnace);
        f.nodes.insert(IVec3::new(1, 0, 0), Node::new(Kind::Pipe));
        f.nodes.insert(IVec3::new(2, 0, 0), Node::new(Kind::Storage));
        f
    }

    fn shipped(f: &Factory) -> Option<&super::super::transit::Packet> {
        f.packets.first()
    }

    #[test]
    fn an_unconfigured_face_still_ships_the_short_way() {
        assert!(Kind::Storage.accepts(IRON));
        let mut f = belt();
        f.tick(0.25, 1);
        assert_eq!(f.nodes[&IVec3::ZERO].output[IRON], 0);
        let path = &shipped(&f).unwrap().path;
        assert_eq!(path, &vec![IVec3::new(0, 0, 0), IVec3::new(1, 0, 0), IVec3::new(2, 0, 0)]);
    }

    #[test]
    fn an_in_face_does_not_let_output_leave() {
        let mut f = belt();
        assert!(apply_role(f.nodes.get_mut(&IVec3::ZERO).unwrap(), 0, Role::In as u8));
        f.tick(0.25, 1);
        assert_eq!(f.nodes[&IVec3::ZERO].output[IRON], 64);
        assert!(f.packets.is_empty());
    }

    #[test]
    fn an_out_face_does_not_accept_a_delivery() {
        let mut f = belt();
        assert!(apply_role(f.nodes.get_mut(&IVec3::new(2, 0, 0)).unwrap(), 1, Role::Out as u8));
        f.tick(0.25, 1);
        assert_eq!(f.nodes[&IVec3::ZERO].output[IRON], 64);
        assert!(f.packets.is_empty());
    }

    #[test]
    fn in_and_out_faces_still_form_a_line() {
        let mut f = belt();
        apply_role(f.nodes.get_mut(&IVec3::ZERO).unwrap(), 0, Role::Out as u8);
        apply_role(f.nodes.get_mut(&IVec3::new(2, 0, 0)).unwrap(), 1, Role::In as u8);
        f.tick(0.25, 1);
        assert_eq!(f.nodes[&IVec3::ZERO].output[IRON], 0);
        assert_eq!(shipped(&f).unwrap().dst, IVec3::new(2, 0, 0));
    }

    #[test]
    fn an_extractor_on_an_in_face_does_not_pull() {
        let mut f = belt();
        f.nodes.get_mut(&IVec3::new(1, 0, 0)).unwrap().mode = 1;
        apply_role(f.nodes.get_mut(&IVec3::ZERO).unwrap(), 0, Role::In as u8);
        f.tick(0.25, 1);
        assert_eq!(f.nodes[&IVec3::ZERO].output[IRON], 64);
        assert!(f.packets.is_empty());
    }

    #[test]
    fn routing_walks_past_a_nearer_illegal_face() {
        let mut f = Factory::default();
        let mut furnace = Node::new(Kind::Furnace);
        furnace.output[IRON] = 64;
        apply_role(&mut furnace, 0, Role::In as u8);
        apply_role(&mut furnace, 1, Role::Out as u8);
        apply_role(&mut furnace, 2, Role::In as u8);
        f.nodes.insert(IVec3::ZERO, furnace);
        let mut storage = Node::new(Kind::Storage);
        apply_role(&mut storage, 1, Role::Out as u8);
        apply_role(&mut storage, 2, Role::In as u8);
        f.nodes.insert(IVec3::new(2, 0, 0), storage);
        for p in [
            IVec3::new(1, 0, 0),
            IVec3::new(-1, 0, 0),
            IVec3::new(-1, 1, 0),
            IVec3::new(0, 1, 0),
            IVec3::new(1, 1, 0),
            IVec3::new(2, 1, 0),
        ] {
            f.nodes.insert(p, Node::new(Kind::Pipe));
        }
        f.tick(0.25, 1);
        let path = &shipped(&f).unwrap().path;
        assert_eq!(path[1], IVec3::new(-1, 0, 0));
        assert_eq!(*path.last().unwrap(), IVec3::new(2, 0, 0));
        assert!(!path.contains(&IVec3::new(1, 0, 0)));
    }

    #[test]
    fn an_off_item_face_keeps_the_power_port() {
        let mut items = Factory::default();
        let mut furnace = Node::new(Kind::Furnace);
        furnace.faces = 0xFFF;
        items.nodes.insert(IVec3::ZERO, furnace.clone());
        items.nodes.insert(IVec3::X, Node::new(Kind::Pipe));
        let nets = items.networks(Kind::Pipe, 1);
        assert!(nets.iter().all(|n| !n.ports.contains(&IVec3::ZERO)));

        let mut power = Factory::default();
        power.nodes.insert(IVec3::ZERO, furnace);
        power.nodes.insert(IVec3::X, Node::new(Kind::Cable));
        let nets = power.networks(Kind::Cable, 1);
        assert_eq!(nets[0].ports, vec![IVec3::ZERO]);
    }

    #[test]
    fn face_roles_round_trip_and_old_layouts_stay_both() {
        let mut f = Factory::default();
        let mut furnace = Node::new(Kind::Furnace);
        apply_role(&mut furnace, 0, Role::In as u8);
        apply_role(&mut furnace, 4, Role::Off as u8);
        f.nodes.insert(IVec3::ZERO, furnace);
        let mut current = Vec::new();
        f.encode_format(&mut current, 13);
        let mut words = current.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap()));
        let back = Factory::decode(&mut || Ok(words.next().unwrap()), 1, 13).unwrap();
        assert!(words.next().is_none());
        assert_eq!(role(&back.nodes[&IVec3::ZERO], 0), Role::In);
        assert_eq!(role(&back.nodes[&IVec3::ZERO], 4), Role::Off);
        assert_eq!(role(&back.nodes[&IVec3::ZERO], 1), Role::Both);

        let mut old = Vec::new();
        f.encode_format(&mut old, 12);
        let mut words = old.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap()));
        let back = Factory::decode(&mut || Ok(words.next().unwrap()), 1, 12).unwrap();
        assert!(words.next().is_none());
        assert_eq!(back.nodes[&IVec3::ZERO].faces, 0);
    }

    #[test]
    fn conduits_and_wild_roles_are_rejected() {
        let mut pipe = Node::new(Kind::Pipe);
        assert!(!apply_role(&mut pipe, 0, Role::In as u8));
        assert_eq!(pipe.faces, 0);
        let mut furnace = Node::new(Kind::Furnace);
        assert!(!apply_role(&mut furnace, 6, Role::In as u8));
        assert!(!apply_role(&mut furnace, 0, 4));
    }
}
