use super::gates::{self, Aperture, Outcome};
use super::{sidecar, traverse};
use crate::fluid::VoxelScalePreset;
use crate::player::Player;
use crate::world::{BlockId, World, WorldType};
use glam::{IVec3, Vec3};

const Y: i32 = 200;

fn world() -> World {
    World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7)
}

/// 2 by 3 opening; `axis` is the plane normal, `at` its lowest open cell.
fn ring(w: &mut World, at: IVec3, axis: usize) {
    let u = 2 - axis;
    let mut edits = Vec::new();
    for y in -1..=3 {
        for s in -1..=2 {
            if y == -1 || y == 3 || s == -1 || s == 2 {
                let mut p = at + IVec3::Y * y;
                p[u] += s;
                edits.push((p, gates::FRAME_BLOCK));
            }
        }
    }
    w.set_many(&edits);
}

fn find(w: &World, at: IVec3, axis: usize) -> Option<Aperture> {
    gates::find(w, at - IVec3::Y, if axis == 2 { Vec3::NEG_Z } else { Vec3::NEG_X })
}

fn linked() -> (World, Aperture, Aperture) {
    let mut w = world();
    let a_at = IVec3::new(0, Y + 1, 0);
    let b_at = IVec3::new(50, Y + 1, 10);
    ring(&mut w, a_at, 2);
    ring(&mut w, b_at, 0);
    let a = find(&w, a_at, 2).expect("gate A");
    let b = find(&w, b_at, 0).expect("gate B");
    assert_eq!(w.portals.attune(a, true, 1.0), Outcome::Attuned);
    assert_eq!(w.portals.attune(b, true, 1.0), Outcome::Linked);
    (w, a, b)
}

#[test]
fn a_closed_crystal_ring_is_a_gate_with_its_opening_measured() {
    let mut w = world();
    let at = IVec3::new(4, Y, -3);
    ring(&mut w, at, 2);
    let a = find(&w, at, 2).expect("ring found");
    assert_eq!((a.min, a.max, a.axis), (at, at + IVec3::new(1, 2, 0), 2));
    let f = a.frame(1.0, true);
    assert!((f.center - glam::DVec3::new(5.0, f64::from(Y) + 1.5, -2.5)).length() < 1e-9);
    assert!((f.half_size - glam::DVec2::new(1.0, 1.5)).length() < 1e-9);
    assert!(f.normal().z > 0.999);
}

#[test]
fn a_broken_or_blocked_ring_is_not_a_gate() {
    let mut w = world();
    let at = IVec3::new(0, Y, 0);
    ring(&mut w, at, 2);
    w.set(at + IVec3::new(2, 1, 0), BlockId::Air);
    assert!(find(&w, at, 2).is_none(), "gap in the ring");
    let mut w = world();
    ring(&mut w, at, 2);
    w.set(at + IVec3::new(1, 1, 0), BlockId::Stone);
    assert!(find(&w, at, 2).is_none(), "opening is not a rectangle");
}

#[test]
fn attuning_cycles_through_link_release_and_close() {
    let (mut w, a, b) = linked();
    assert_eq!(w.portals.network().portals.len(), 4, "both faces, both directions");
    assert!(w.portals.network().validate().is_ok());
    assert_eq!(w.portals.attune(b, true, 1.0), Outcome::Closed);
    assert!(w.portals.network().portals.is_empty());
    assert_eq!(w.portals.attune(a, true, 1.0), Outcome::Attuned);
    assert_eq!(w.portals.attune(a, true, 1.0), Outcome::Cancelled);
    assert!(w.portals.pending().is_none());
}

#[test]
fn openings_of_different_size_do_not_link() {
    let mut w = world();
    let a_at = IVec3::new(0, Y, 0);
    ring(&mut w, a_at, 2);
    let a = find(&w, a_at, 2).expect("gate A");
    let mut b = a;
    b.min.x += 10;
    b.max.x += 11;
    assert_eq!(w.portals.attune(a, true, 1.0), Outcome::Attuned);
    assert_eq!(w.portals.attune(b, true, 1.0), Outcome::Mismatched);
    assert!(w.portals.network().portals.is_empty());
}

#[test]
fn breaking_a_ring_closes_its_portals() {
    let (mut w, a, _) = linked();
    w.set(a.min - IVec3::Y, BlockId::Air);
    let mut gates = std::mem::take(&mut w.portals);
    assert!(gates.audit(&w) || gates.audit(&w));
    assert!(gates.network().portals.is_empty());
}

fn walk_through(w: &mut World, p: &mut Player, from_z: f32, to_z: f32) -> bool {
    p.pos = Vec3::new(1.0, (Y + 1) as f32, from_z);
    p.precise_pos = p.pos.as_dvec3();
    p.vel = Vec3::new(0.0, 0.0, -4.0);
    p.yaw = 0.0;
    let before = traverse::before(w, p);
    p.pos.z = to_z;
    p.precise_pos = p.pos.as_dvec3();
    traverse::step(w, p, before)
}

#[test]
fn walking_into_a_gate_leaves_the_linked_gate_turned_with_it() {
    let (mut w, _, _) = linked();
    let mut p = Player::new();
    assert!(walk_through(&mut w, &mut p, 1.2, 0.2));
    // Gate B faces +X at x = 50.5: the walker comes out on that side, moving +X.
    assert!(p.pos.x > 50.5 && p.pos.x < 51.5, "exit side: {:?}", p.pos);
    assert!((p.pos.z - 11.0).abs() < 0.05, "same offset across the opening: {:?}", p.pos);
    assert!((p.vel - Vec3::new(4.0, 0.0, 0.0)).length() < 1e-3, "speed kept, turned: {:?}", p.vel);
    assert!((p.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "facing +X: {}", p.yaw);
}

#[test]
fn an_obstructed_exit_stops_the_walker_at_the_plane() {
    let (mut w, _, b) = linked();
    let mut wall = Vec::new();
    for y in 0..3 {
        for z in 0..2 {
            wall.push((IVec3::new(51, b.min.y + y, b.min.z + z), BlockId::Stone));
        }
    }
    w.set_many(&wall);
    let mut p = Player::new();
    assert!(!walk_through(&mut w, &mut p, 1.2, 0.2));
    assert!((p.pos.z - 1.2).abs() < 1e-6, "held on the entrance side: {:?}", p.pos);
}

#[test]
fn gate_links_round_trip_and_damage_is_rejected() {
    let (w, _, _) = linked();
    let bytes = sidecar::encode(&w.portals);
    let links = sidecar::decode(&bytes).expect("decodes");
    assert_eq!(links.len(), 1);
    assert!(sidecar::decode(b"garbage").is_err());
    let mut other = world();
    other.portals.links = links;
    other.portals.rebuild(1.0);
    assert_eq!(other.portals.network().portals.len(), 4);
}

#[test]
fn a_window_never_contains_the_far_face_of_its_own_gate() {
    use super::views::{Camera, Plan};
    let (w, _, _) = linked();
    let network = w.portals.network();
    let camera = Camera {
        realm: gates::realm(),
        eye: glam::DVec3::new(1.0, f64::from(Y) + 2.6, 6.0),
        rotation: glam::DQuat::IDENTITY,
        projection: glam::dcamera::rh::proj::directx::perspective_infinite_reverse(1.2, 16.0 / 9.0, 0.05),
    };
    let plan = Plan::build(network, camera, [1600, 900], |_, _, _| true);
    assert!(plan.views.len() >= 2, "gate A is in view");
    for view in &plan.views[1..] {
        let parent = &plan.views[view.parent.expect("child")];
        if let Some(through) = parent.portal_id.and_then(|id| network.portals.iter().find(|p| p.id == id)) {
            let portal = network.portals.iter().find(|p| Some(p.id) == view.portal_id).expect("portal");
            assert!(!super::views::coincides(portal, through), "nested window on its own exit plane");
        }
    }
}

#[test]
fn a_distant_gate_gets_the_membrane_instead_of_a_window() {
    use super::views::{Camera, Plan};
    let (w, _, _) = linked();
    let network = w.portals.network();
    let camera = |z: f64| Camera {
        realm: gates::realm(),
        eye: glam::DVec3::new(1.0, f64::from(Y) + 2.6, z),
        rotation: glam::DQuat::IDENTITY,
        projection: glam::dcamera::rh::proj::directx::perspective_infinite_reverse(1.2, 16.0 / 9.0, 0.05),
    };
    assert!(Plan::build(network, camera(6.0), [1600, 900], |_, _, _| true).views.len() >= 2);
    // At 300 m the opening covers a few dozen pixels: no terrain pass for it.
    assert_eq!(Plan::build(network, camera(300.0), [1600, 900], |_, _, _| true).views.len(), 1);
}
