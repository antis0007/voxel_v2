//! Conduits from voxel assets (cables 16 per metre, pipes 8) (`assets/models/factory`): a thin arm
//! per connection, a junction block where the run bends or branches, framed
//! sockets on faces with no arm, and a flange where an arm enters a machine.
use super::Model;
use super::voxel_asset::{Tint, VoxelModel};
use crate::world::{World, factory::{DIRS, Kind, Node}};
use glam::{IVec3, Quat, Vec3};
use std::sync::LazyLock;

struct Set {
    arm: VoxelModel,
    flange: VoxelModel,
    /// Collar at the cell edge where two conduits meet.
    joint: VoxelModel,
    hub: VoxelModel,
    socket: VoxelModel,
    /// Extractor pipes end in a lit mouth instead of a flange.
    mouth: Option<VoxelModel>,
}

macro_rules! asset {
    ($name:literal) => {
        VoxelModel::with_preview(include_str!(concat!("../../../../../assets/models/factory/", $name, ".vox")), $name)
    };
}

static CABLE: LazyLock<Set> = LazyLock::new(|| Set {
    arm: asset!("cable_arm"),
    flange: asset!("cable_flange"),
    joint: asset!("cable_joint"),
    hub: asset!("cable_hub"),
    socket: asset!("cable_socket"),
    mouth: None,
});
static PIPE: LazyLock<Set> = LazyLock::new(|| Set {
    arm: asset!("pipe_arm"),
    flange: asset!("pipe_flange"),
    joint: asset!("pipe_joint"),
    hub: asset!("pipe_hub"),
    socket: asset!("pipe_socket"),
    mouth: Some(asset!("pipe_mouth")),
});
static FLUID: LazyLock<Set> = LazyLock::new(|| Set {
    arm: asset!("fluid_arm"),
    flange: asset!("fluid_flange"),
    joint: asset!("fluid_joint"),
    hub: asset!("fluid_hub"),
    socket: asset!("fluid_socket"),
    mouth: None,
});

const STEEL: [f32; 3] = [0.27, 0.33, 0.37];

fn tint(n: &Node) -> Tint {
    let stripe = match n.kind {
        Kind::Cable => match n.upgrade {
            0 => [0.10, 0.37, 0.41],
            1 => [0.85, 0.45, 0.12],
            _ => [0.30, 0.85, 0.95],
        },
        Kind::FluidPipe => [0.25, 0.70, 0.85],
        _ => match n.mode {
            2 => [0.95, 0.72, 0.28],
            1 => [0.30, 0.88, 1.0],
            _ => [0.68, 0.38, 0.14],
        },
    };
    Tint { stripe, trim: stripe, glow: 1.0 }
}

/// `near` selects the voxel model; far runs are one box per arm.
pub(super) fn draw(mesh: &mut Model, world: &World, p: IVec3, n: &Node, o: Vec3, step: i32, near: bool) {
    let set: &Set = match n.kind {
        Kind::Cable => &CABLE,
        Kind::FluidPipe => &FLUID,
        _ => &PIPE,
    };
    let tint = tint(n);
    let target = |side: usize| world.factory.nodes.get(&(p + DIRS[side] * step));
    let joins = |side: usize| world.factory.conduit_join(p, side, step);
    let arms: Vec<usize> = (0..6).filter(|s| joins(*s)).collect();
    let straight = arms.len() == 2 && arms[0] ^ 1 == arms[1];
    let c = o + Vec3::splat(0.5);

    if !near {
        // Matches the near tube: pipes 0.5 m, fluid 0.25 m, cables a 1/8 m core.
        let half = match n.kind {
            Kind::Pipe => 0.25,
            Kind::Cable => 0.0625,
            _ => 0.125,
        };
        for &s in &arms {
            let dir = DIRS[s].as_vec3();
            let size = dir.abs() * 0.5 + (Vec3::ONE - dir.abs()) * (half * 2.0);
            mesh.cuboid(c + dir * 0.25, size, Quat::IDENTITY, STEEL, 0.0);
        }
        return;
    }

    let turn = |s: usize| Quat::from_rotation_arc(Vec3::X, DIRS[s].as_vec3());
    if !straight {
        set.hub.draw(mesh, o, Quat::IDENTITY, &tint);
        for s in (0..6).filter(|s| !arms.contains(s)) {
            set.socket.draw(mesh, o, turn(s), &tint);
        }
    }
    let extractor = n.kind == Kind::Pipe && (n.mode == 1 || n.mode == 2);
    for &s in &arms {
        let q = turn(s);
        if n.kind == Kind::Pipe && n.activity > 0.0 {
            // Flow: a brightness wave whose phase advances with cell position
            // along the run, so busy pipes read as moving goods.
            let along = (p.x + p.y + p.z) as f32 * 1.3;
            let wave = 0.5 + 0.5 * (world.adventure.time * 5.0 - along).sin();
            let lit = 1.0 + n.activity.min(1.0) * 1.4 * wave;
            set.arm.draw(mesh, o, q, &Tint { stripe: tint.stripe, trim: tint.trim, glow: lit });
        } else {
            set.arm.draw(mesh, o, q, &tint);
        }
        let to_machine = target(s).is_some_and(|other| other.kind != n.kind);
        if extractor && let Some(mouth) = &set.mouth {
            let pulse = 0.65 + 0.35 * (world.adventure.time * 4.0).sin().abs();
            mouth.draw(mesh, o, q, &Tint { stripe: tint.stripe.map(|v| v * pulse), trim: tint.trim, glow: 1.0 });
        } else if to_machine {
            set.flange.draw(mesh, o, q, &tint);
        } else {
            set.joint.draw(mesh, o, q, &tint);
        }
        if n.activity > 0.0 && n.kind != Kind::Pipe {
            let travel = (world.adventure.time * 1.8).fract() * 0.4 + 0.05;
            let dir = DIRS[s].as_vec3();
            let bead = if n.kind == Kind::Cable { [0.2, 0.9, 1.0] } else { [0.55, 0.85, 1.0] };
            // The cable pulse sits just proud of its 1/8 m core.
            let size = if n.kind == Kind::Cable { 0.14 } else { 0.125 };
            mesh.cuboid(c + dir * travel, Vec3::splat(size), Quat::IDENTITY, bead, n.activity * 1.8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{World, WorldType, factory};

    fn bounds(vertices: &[crate::mesh::Vertex3]) -> (Vec3, Vec3) {
        vertices.iter().fold((Vec3::splat(1e9), Vec3::splat(-1e9)), |(lo, hi), v| {
            let p = Vec3::from_array(v.pos);
            (lo.min(p), hi.max(p))
        })
    }

    #[test]
    fn a_straight_item_pipe_is_a_half_metre_tube_with_edge_collars() {
        let mut world = World::new(WorldType::Superflat, crate::fluid::VoxelScalePreset::OneMeter);
        for x in 0..3 {
            factory::fixture_place(&mut world, IVec3::new(x, 100, 0), Kind::Pipe);
        }
        let p = IVec3::new(1, 100, 0);
        let n = world.factory.nodes.get(&p).expect("pipe node").clone();
        let mut mesh = Model::with_vertices(1.0, Vec::new());
        draw(&mut mesh, &world, p, &n, p.as_vec3(), 1, true);
        let (lo, hi) = bounds(&mesh.vertices);
        // Tube is 0.5 m; the cell-edge collars reach 0.75 m.
        assert!(hi.y - lo.y <= 0.76 && hi.z - lo.z <= 0.76, "tube is {:?}", hi - lo);
        assert!((hi.x - lo.x - 1.0).abs() < 0.01, "length {}", hi.x - lo.x);
    }
}
