//! Cart and counter crates drawn from the freight simulation's positions.
use super::Model;
use crate::{
    mesh::prop_palette::{BRASS, IRON, OAK},
    world::{World, farming::freight},
};
use glam::{Quat, Vec3};

pub(super) fn draw(mesh: &mut Model, world: &World, viewer: Vec3) {
    let planet = world.world_type == crate::world::WorldType::Planet;
    let view = if planet {
        crate::world::farming::planet_freight::view(world, viewer)
    } else {
        freight::view(world, viewer)
    };
    if let Some(cart) = view.cart {
        let turn = Quat::from_rotation_y(view.cart_yaw);
        mesh.cuboid(
            cart + Vec3::Y * 0.15,
            Vec3::new(0.95, 0.10, 0.48),
            turn,
            OAK,
            0.0,
        );
        for offset in [-0.32, 0.32] {
            mesh.cuboid(
                cart + turn * Vec3::new(offset, 0.085, 0.0),
                Vec3::new(0.17, 0.17, 0.53),
                turn,
                IRON,
                0.0,
            );
        }
        mesh.cuboid(
            cart + Vec3::Y * 0.37,
            Vec3::new(0.62, 0.34, 0.42),
            turn,
            OAK,
            0.0,
        );
        mesh.cuboid(
            cart + Vec3::Y * 0.39,
            Vec3::new(0.12, 0.16, 0.43),
            turn,
            BRASS,
            0.0,
        );
    }
    if let Some((bench, count)) = view.crates {
        for index in 0..count {
            let offset = if planet {
                Vec3::new(0.46 * (index % 2) as f32 - 0.23, 0.16 + 0.33 * (index / 2) as f32, 0.0)
            } else {
                Vec3::new(0.46 * (index as f32 - 1.0), 0.16, 0.0)
            };
            let position = bench + offset;
            mesh.cuboid(
                position,
                Vec3::new(0.40, 0.32, 0.39),
                Quat::IDENTITY,
                OAK,
                0.0,
            );
            mesh.cuboid(
                position + Vec3::Y * 0.14,
                Vec3::new(0.42, 0.08, 0.41),
                Quat::IDENTITY,
                BRASS,
                0.0,
            );
        }
    }
}
