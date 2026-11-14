use super::*;
use crate::{
    fluid::VoxelScalePreset,
    mesh::models,
    player::Player,
    world::{self, DayNightCycle, World, WorldType},
};
use glam::{IVec2, IVec3};

fn bits(v: &Vertex3) -> [u32; 14] {
    bytemuck::cast(*v)
}

/// The deferred scene plus its placements, posed on the CPU, must hold
/// exactly the vertices of the direct scene. Placed vertices skip the
/// caller's surface passes, so this catches a baked pass drifting from
/// `orbital::build`.
#[test]
fn posed_scene_matches_cpu_scene() {
    let world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 74);
    let day = DayNightCycle::new(0.5, 600.0);
    let mut placed_total = 0;
    for cell in [IVec2::ZERO, IVec2::new(1, 0), IVec2::new(0, 1), IVec2::new(-1, -1), IVec2::new(2, -1)] {
        let Some(site) = world::orbital::site(world.seed, cell) else { continue };
        let mut player = Player::new();
        player.pos = Vec3::new(site.x + 400.0, 120.0, site.z - 300.0) / world.voxel_scale.meters();
        player.precise_pos = player.pos.as_dvec3();
        set_enabled(false);
        let cpu = models::build(&world, &player, &day, 0.0005, &crate::render::ChunkCache::new(IVec3::ZERO));
        set_enabled(true);
        let mut deferred = models::build(&world, &player, &day, 0.0005, &crate::render::ChunkCache::new(IVec3::ZERO));
        set_enabled(false);
        let placed: Vec<Vertex3> = with_frame(|placements, pool, _| {
            placements
                .iter()
                .flat_map(|p| {
                    pool[p.first as usize..(p.first + p.count) as usize]
                        .iter()
                        .map(move |v| pose(v, p.rotation, p.translation, p.scale))
                })
                .collect()
        });
        placed_total += placed.len();
        deferred.extend(placed);
        let mut a: Vec<_> = cpu.iter().map(bits).collect();
        let mut b: Vec<_> = deferred.iter().map(bits).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a.len(), b.len(), "cell {cell}: vertex count");
        assert!(a == b, "cell {cell}: deferred scene differs from the direct scene");
    }
    assert!(placed_total > 0, "no anchorage was in view of any probe");
}
