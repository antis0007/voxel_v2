//! A finite orchard forebay, a low drain and a side ditch beside a silt bank.
//! The ordinary water simulation chooses the route from the edited geometry.
use crate::{BlockId as B, WorldSampleConfig, WorldType};
use glam::Vec3;

pub(super) fn sample(cfg: &WorldSampleConfig, p: Vec3) -> Option<B> {
    if cfg.generator_version < engine_voxel::FERNWAKE_SPILLWAY_GENERATOR
        || cfg.world_type != WorldType::Hills
        || !(449.0..464.0).contains(&p.x)
        || !(51.0..58.0).contains(&p.z)
        || !(-5.0..0.0).contains(&p.y)
    {
        return None;
    }

    let Vec3 { x, y, z } = p;
    let feed = (53.0..54.0).contains(&z);
    let spill = (458.0..459.0).contains(&x);

    // Water prefers the immediate drop into the deep tank. Closing the
    // opening with timber or ice leaves the level side channel available.
    if (spill && feed && (-4.0..0.0).contains(&y))
        || ((458.0..463.0).contains(&x)
            && (51.0..55.0).contains(&z)
            && (-4.0..-2.0).contains(&y))
        || ((457.0..458.0).contains(&x) && (54.0..55.0).contains(&z) && y >= -1.0)
    {
        return Some(B::Air);
    }

    // Dig the exposed dirt beside the feeder for a second way into the ditch.
    // Both routes end one block below the planting bench, next to its roots.
    if y >= -2.0 && (55.0..57.0).contains(&z) && (456.0..461.0).contains(&x) {
        return Some(B::Air);
    }
    if (456.0..457.0).contains(&x) && (54.0..55.0).contains(&z) {
        if y >= -1.0 {
            return Some(B::Dirt);
        }
        if y >= -2.0 {
            return Some(B::Air);
        }
    }

    if y >= -1.0 {
        if (450.0..454.0).contains(&x) && (52.0..54.0).contains(&z) {
            return Some(B::Water);
        }
        if feed && (454.0..458.0).contains(&x) {
            return Some(if x < 455.0 { B::Planks } else { B::Air });
        }
        if (461.0..463.0).contains(&x) && (55.0..57.0).contains(&z) {
            return Some(B::Air);
        }
        if (457.0..463.0).contains(&x) && (57.0..58.0).contains(&z) && y >= -0.5 {
            return Some(B::Air);
        }
    }
    if (-2.0..-1.0).contains(&y) && (461.0..463.0).contains(&x) && (55.0..57.0).contains(&z) {
        return Some(B::Mud);
    }

    Some(B::Limestone)
}
