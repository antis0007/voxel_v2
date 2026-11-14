//! Authored Observatory encounter, using ordinary guardian and relic ownership.
use super::*;
use crate::world::chunk_coord_from_world_block;
use engine_worldgen::undercrown::v2::observatory::{ENCOUNTER_ID, ENCOUNTER_VERSION, Gallery};

pub(super) fn is_site(site: IVec3) -> bool {
    site == ENCOUNTER_ID
}

fn gallery(world: &World) -> Option<Gallery> {
    if world.generator_version < ENCOUNTER_VERSION {
        return None;
    }
    undercrown::plan(&world.sample_config())
        .v2
        .as_ref()?
        .flooded_observatory()
}

fn resident(world: &World, point: Vec3, radius: f32, height: f32) -> bool {
    let scale = world.voxel_scale.meters();
    let lo = ((point - Vec3::new(radius, 0.2, radius)) / scale)
        .floor()
        .as_ivec3();
    let hi = ((point + Vec3::new(radius, height, radius)) / scale)
        .floor()
        .as_ivec3();
    let lo = chunk_coord_from_world_block(lo);
    let hi = chunk_coord_from_world_block(hi);
    (lo.x..=hi.x).all(|x| {
        (lo.y..=hi.y)
            .all(|y| (lo.z..=hi.z).all(|z| world.chunks.contains_key(&IVec3::new(x, y, z))))
    })
}

pub(super) fn append(adventure: &mut Adventure, world: &World, feet: Vec3) {
    let Some(gallery) = gallery(world) else {
        return;
    };
    let pos = gallery.reliquary();
    if (feet - pos).length_squared() > 70.0 * 70.0
        || !resident(world, pos, 0.3, 1.0)
        || !factory::solid_point(world, pos - Vec3::Y * 0.15)
        || !free(world, pos, 0)
        || (adventure.records.len() >= MAX_RECORDS
            && !adventure.records.contains_key(&ENCOUNTER_ID))
    {
        return;
    }
    let state = *adventure
        .records
        .entry(ENCOUNTER_ID)
        .or_insert(reliquaries::PHYSICAL);
    adventure.reliquaries.push(reliquaries::Reliquary {
        site: ENCOUNTER_ID,
        pos,
        kind: 3,
        state,
    });
}

/// No random relocation: excavation or a placed machine may block the cradle.
/// Saved Engine restoration remains in the shared spawn path before this call.
pub(super) fn spawn(adventure: &mut Adventure, world: &World, eye: Vec3) -> bool {
    let Some(gallery) = gallery(world) else {
        return false;
    };
    let pos = gallery.guardian();
    if (pos - eye).length_squared() <= 2.5 * 2.5
        || !resident(world, pos, 1.0, 2.0)
        || !factory::solid_point(world, pos - Vec3::Y * 0.15)
        || !free(world, pos, engine::KIND)
    {
        return false;
    }
    adventure
        .enemies
        .push(Enemy::new(ENCOUNTER_ID, 3, engine::KIND, pos, 0));
    true
}
