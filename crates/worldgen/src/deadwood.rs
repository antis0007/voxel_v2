//! Fallen rainforest trunks. A site is surveyed once in metres, then sampled
//! like the surrounding trees, so edits and chunk boundaries share its shape.
use crate::{
    BlockId as B, CachedProfiles, ColumnSample, Profiles, WorldSampleConfig,
    cave_carves, floor_div, generation_cache, hash2, landscape, ruins, sample_column, sites, surface,
    trees::rainforest, value_noise2,
};
use glam::{IVec3, Vec2};

// With a centre in the middle 10 m, an 8.5 m trunk and its bark stay
// inside this cell. The voxel sampler only has to read one profile.
const CELL_M: f32 = 24.0;

#[derive(Clone, Copy)]
pub(super) struct FallenTrunk {
    start: Vec2,
    direction: Vec2,
    length: f32,
    ground: [f32; 2],
    radius: f32,
    seed: u32,
}

fn suitable(cfg: &WorldSampleConfig, c: ColumnSample) -> bool {
    c.height > cfg.water_level() + cfg.blocks_for_span(1.0)
        && c.vegetation_density > 0.30
        && matches!(c.surface, B::Grass | B::Dirt | B::Mud | B::GroveMoss)
}

pub(super) fn profile(cfg: &WorldSampleConfig, cx: i32, cz: i32) -> Option<FallenTrunk> {
    let seed = cfg.seed.wrapping_add(168_019);
    let roll = hash2(cx, cz, seed);
    if roll > 0.78 {
        return None;
    }
    let centre = Vec2::new(
        cx as f32 * CELL_M + 7.0 + hash2(cx, cz, seed ^ 3) * 10.0,
        cz as f32 * CELL_M + 7.0 + hash2(cx, cz, seed ^ 5) * 10.0,
    );
    // Openings left by disturbance have more deadwood than intact stands.
    let opening = rainforest::opening(cfg, centre.x, centre.y);
    if roll > 0.36 + 0.42 * opening {
        return None;
    }
    let coords = |at: Vec2| {
        (cfg.blocks_for_height(at.x), cfg.blocks_for_height(at.y))
    };
    let (x, z) = coords(centre);
    let c = sample_column(cfg, x, z);
    if !suitable(cfg, c) || landscape::sample(cfg, x, z).biome != landscape::Biome::Rainforest {
        return None;
    }
    let angle = hash2(cx, cz, seed ^ 7) * std::f32::consts::TAU;
    let direction = Vec2::from_angle(angle);
    let side = Vec2::new(-direction.y, direction.x);
    let length = 4.5 + hash2(cx, cz, seed ^ 11) * 4.0;
    let radius = 0.95 + hash2(cx, cz, seed ^ 13) * 0.48;
    let start = centre - direction * (length * 0.5);
    let end = start + direction * length;
    let ground_at = |at| {
        let (x, z) = coords(at);
        cfg.world_y(sample_column(cfg, x, z).height)
    };
    let ground = [ground_at(start), ground_at(end)];
    if (ground[1] - ground[0]).abs() > 1.2 {
        return None;
    }
    for i in 0..=8 {
        let t = i as f32 / 8.0;
        let at = start.lerp(end, t);
        let floor = ground[0] + (ground[1] - ground[0]) * t;
        for cross in [-0.65, 0.0, 0.65] {
            let (x, z) = coords(at + side * (radius * cross));
            let c = sample_column(cfg, x, z);
            let h = cfg.world_y(c.height);
            let foot = IVec3::new(x, c.height, z);
            let m = foot.as_vec3() * cfg.voxel_meters();
            if !suitable(cfg, c)
                || (h - floor).abs() > 1.0
                || cave_carves(cfg, foot, &c)
                || CachedProfiles.river_top(cfg, x, z).is_some_and(|water| water > h + cfg.voxel_meters())
                || ruins::clears_vegetation(cfg, m.x, m.z)
                || sites::tree_reserved(cfg, m.x, m.z)
                || surface::trail_clears(cfg, m)
            {
                return None;
            }
        }
    }
    let seed = seed ^ (hash2(cx, cz, seed ^ 17) * 65535.0) as u32;
    Some(FallenTrunk { start, direction, length, ground, radius, seed })
}

pub(super) fn block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> B {
    if cfg.generator_version < engine_voxel::RAINFOREST_DEADWOOD_GENERATOR
        || p.y > column.height + cfg.blocks_for_span(2.8)
        || !suitable(cfg, column)
    {
        return B::Air;
    }
    let span = cfg.blocks_for_span(CELL_M);
    let Some(trunk) = generation_cache::deadwood(cfg, floor_div(p.x, span), floor_div(p.z, span)) else {
        return B::Air;
    };
    let m = p.as_vec3() * cfg.voxel_meters();
    let local = Vec2::new(m.x, m.z) - trunk.start;
    let along = local.dot(trunk.direction);
    let side = local.perp_dot(trunk.direction).abs();
    if !(0.0..=trunk.length).contains(&along) || side > trunk.radius + 0.07 {
        return B::Air;
    }
    let bark = value_noise2(along / 1.6, side / 1.2, trunk.seed);
    // The narrower end splinters at different lengths across its face.
    if along > trunk.length - bark * 0.8 {
        return B::Air;
    }
    let t = along / trunk.length;
    let floor = trunk.ground[0] + (trunk.ground[1] - trunk.ground[0]) * t;
    let axis_y = floor + trunk.radius * (0.54 - 0.08 * t);
    let height = m.y - axis_y;
    let radius = (trunk.radius * (1.0 - 0.27 * t) + (bark - 0.5) * 0.14).max(0.74);
    if Vec2::new(side, height).length_squared() > radius * radius
        || surface::trail_clears(cfg, m)
    {
        return B::Air;
    }
    let under = Vec2::new(side, height - cfg.voxel_meters());
    if (0.15..0.85).contains(&t) && height > radius * 0.40 && bark > 0.56
        && under.length_squared() <= radius * radius
    {
        B::GroveMoss
    } else {
        B::Log
    }
}
