use crate::{BlockId as B, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec3};
use std::cell::Cell;

pub const BRIDGE: std::ops::Range<i32> = 16..24;
pub const HOLDINGS: [(i32, i32); 2] = [(36, -8), (51, 8)];
pub const LANES: [f32; 2] = [-1.25, 1.25];
pub const DETOUR_Z: f32 = 8.25;
const MARKET_Z: i32 = 8;
const WEST_HOME: (i32, i32) = (-3, -8);
pub const WEST_SETTLEMENT: &str = "Westbank Market";
pub const EAST_SETTLEMENT: &str = "Eastbank Farms";

fn service_crossing(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= engine_voxel::PLANET_FREIGHT_DETOUR_GENERATOR
}

fn buildings(cfg: &WorldSampleConfig) -> impl Iterator<Item = (i32, i32)> {
    [(0, MARKET_Z), HOLDINGS[0], HOLDINGS[1]].into_iter()
        .chain(service_crossing(cfg).then_some(WEST_HOME))
}

#[derive(Clone, Copy)]
pub struct Site {
    pub origin: Vec3,
}

impl Site {
    pub fn market(self) -> Vec3 {
        self.origin + Vec3::new(0.5, 0.5, 5.5)
    }

    pub fn field(self, holding: usize) -> Vec3 {
        let (x, z) = HOLDINGS[holding];
        self.origin + Vec3::new(x as f32 + 5.5, 0.0, z as f32 + 0.5)
    }
}

thread_local! {
    static SITE: Cell<Option<(WorldSampleConfig, Option<Site>)>> = const { Cell::new(None) };
}

pub fn site(cfg: &WorldSampleConfig) -> Option<Site> {
    if !cfg.civilization_enabled() || !crate::features::enabled(cfg, crate::features::Feature::Freight)
        || cfg.generator_version < engine_voxel::PLANET_FREIGHT_GENERATOR
    {
        return None;
    }
    if let Some((key, result)) = SITE.get()
        && key == *cfg
    {
        return result;
    }
    let spawn = if crate::planetary::active(cfg) {
        crate::planetary::spawn_point(cfg)
    } else {
        glam::DVec3::ZERO
    };
    let mut best = None;
    // Search outwards when the landing is surrounded by steep or wet terrain.
    for radius in [0.0_f32, 96.0, 192.0, 288.0] {
        let ring = [-radius, -radius * 0.5, 0.0, radius * 0.5, radius];
        let (xs, zs): (&[f32], &[f32]) = if radius == 0.0 {
            (
                &[0.0, -65.0, 25.0, -90.0],
                &[14.0, -14.0, 27.0, -27.0, 45.0, -45.0],
            )
        } else {
            (&ring, &ring)
        };
        for &dx in xs {
            for &dz in zs {
                if radius > 0.0 && dx.abs() != radius && dz.abs() != radius {
                    continue;
                }
                let x = spawn.x.floor() as f32 + dx;
                let z = spawn.z.floor() as f32 + dz;
                let (_, local) =
                    engine_core::cube_sphere::atlas::split(Vec3::new(x, 0.0, z).as_dvec3());
                let half = engine_core::cube_sphere::atlas::HALF_M;
                if (crate::planetary::active(cfg)
                    && (local.x < -half + 8.0 || local.x > half - 58.0 || local.z.abs() > half - 15.0))
                    || crate::planetary::reserves_volcanic_summit(cfg, x + 25.0, z, 38.0)
                    || crate::planetary::trail_reserves(cfg, x + 25.0, z, 38.0)
                {
                    continue;
                }
                let mut low = f32::INFINITY;
                let mut high = f32::NEG_INFINITY;
                for sx in [-5.0, 0.0, 11.0, 16.0, 24.0, 30.0, 36.0, 43.0, 51.0, 56.0] {
                    for sz in [-12.0, -4.0, 0.0, 4.0, 12.0] {
                        let g = crate::geo::ground(cfg, x + sx, z + sz, cfg.voxel_meters());
                        low = low.min(g.top_m);
                        high = high.max(if g.water_m.is_some() {
                            f32::INFINITY
                        } else {
                            g.top_m
                        });
                    }
                }
                let relief = high - low;
                if low < crate::WATER_LEVEL_METERS + 3.0 || relief > 12.0 {
                    continue;
                }
                let score = relief * 3.0 + dx.hypot(dz) * 0.12;
                if best.is_none_or(|(cost, _)| score < cost) {
                    let v = cfg.voxel_meters();
                    let datum = ((low + high) * 0.5 / v).round() * v;
                    best = Some((
                        score,
                        Site {
                            origin: Vec3::new(x, datum, z),
                        },
                    ));
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    let result = best.map(|(_, site)| site);
    SITE.set(Some((*cfg, result)));
    result
}

fn inside(q: Vec3) -> bool {
    (-7.0..58.0).contains(&q.x) && q.z.abs() < 13.0
}

fn shelter_roof(z: f32, landscaped: bool) -> f32 {
    4.0 + if landscaped { (2.0 - z.abs()).max(0.0).floor() } else { 0.0 }
}

pub fn visit_skylines(
    cfg: &WorldSampleConfig,
    center: glam::Vec2,
    radius: f32,
    mut emit: impl FnMut([Vec3; 9], f32, B, B),
) {
    let Some(site) = site(cfg) else { return };
    let landscaped = cfg.generator_version >= engine_voxel::PLANET_FREIGHT_SITE_GENERATOR;
    for (x, z) in buildings(cfg) {
        let base = site.origin + Vec3::new(x as f32, 0.0, z as f32);
        if center.distance_squared(glam::Vec2::new(base.x, base.z)) > radius * radius {
            continue;
        }
        let ring = [(-4., -3.), (0., -3.), (4., -3.), (4., 0.), (4., 3.),
            (0., 3.), (-4., 3.), (-4., 0.), (0., 0.)];
        emit(ring.map(|(x, z)| base + Vec3::new(x, shelter_roof(z, landscaped) + 1.0, z)),
            base.y, B::Log, B::Planks);
    }
}

pub(crate) fn clears(cfg: &WorldSampleConfig, m: Vec3) -> bool {
    site(cfg).is_some_and(|site| inside(m - site.origin))
}

pub(crate) fn block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> Option<B> {
    if (p.y - column.height).abs() > cfg.blocks_for_span(20.0) {
        return None;
    }
    let v = cfg.voxel_meters();
    let site = site(cfg)?;
    let q = (p.as_vec3() + Vec3::splat(0.5)) * v - site.origin;
    if !inside(q) {
        return None;
    }
    let top = cfg.world_y(column.height + 1) - site.origin.y;
    let landscaped = cfg.generator_version >= engine_voxel::PLANET_FREIGHT_SITE_GENERATOR;
    let clearance = if landscaped { 8.0 } else { 6.0 };
    if q.y < top.min(-5.0) || q.y >= top.max(clearance) {
        return None;
    }
    if (16.0..24.0).contains(&q.x) {
        if service_crossing(cfg)
            && (q.z - DETOUR_Z).abs() < 1.25
        {
            return Some(if q.y < -v { B::Cobblestone } else if q.y < 0.0 { B::StoneBricks } else { B::Air });
        }
        let bank = if landscaped {
            top.max(-4.0) * crate::smoothstep(8.0, 13.0, q.z.abs())
                - 4.0 * (1.0 - crate::smoothstep(8.0, 13.0, q.z.abs()))
        } else {
            -4.0
        };
        let abutment = landscaped && q.z.abs() < 3.5 && q.y < -v && (q.x < 17.0 || q.x >= 23.0);
        return Some(if q.y < bank || abutment {
            B::Cobblestone
        } else if q.z.abs() < 3.5 && (-v..0.0).contains(&q.y) {
            B::Planks
        } else if (2.5..3.5).contains(&q.z.abs()) && (0.0..1.0).contains(&q.y) {
            B::Log
        } else {
            B::Air
        });
    }
    for (x, z) in buildings(cfg) {
        let dx = q.x - x as f32;
        let dz = q.z - z as f32;
        if (-4.0..4.0).contains(&dx) && (-3.0..3.0).contains(&dz) {
            if q.y < 0.0 {
                return Some(B::StoneBricks);
            }
            if q.y >= 4.0 {
                let roof = shelter_roof(dz, landscaped);
                return Some(if q.y >= roof + 1.0 {
                    B::Air
                } else if q.y >= roof {
                    B::Planks
                } else if dx.abs() >= 3.0 {
                    B::Log
                } else {
                    B::Air
                });
            }
            if x == 0 && (0.0..1.0).contains(&dx) && (-3.0..-2.0).contains(&dz) && q.y < 1.0 {
                return Some(B::Planks);
            }
            let door = (-1.0..1.0).contains(&dx) && dz * (z as f32).signum() < 0.0 && q.y < 3.0;
            let window = landscaped
                && (1.0..2.0).contains(&q.y)
                && ((dx.abs() >= 3.0 && dz.abs() < 0.5)
                    || (dz.abs() >= 2.0 && (1.5..2.5).contains(&dx.abs())));
            return Some(
                if (dx < -3.0 || dx >= 3.0 || dz < -2.0 || dz >= 2.0) && !door && !window {
                    B::Log
                } else {
                    B::Air
                },
            );
        }
    }
    for (x, z) in HOLDINGS {
        if (4.0..7.0).contains(&(q.x - x as f32)) && (q.z - z as f32).abs() < 2.0 {
            if (-v..0.0).contains(&q.y) {
                return Some(B::FungalSoil);
            }
            if (0.0..v).contains(&q.y) {
                return Some(B::WheatCrop);
            }
        }
    }
    let service_road = service_crossing(cfg)
        && ((14.0..26.0).contains(&q.x) && (q.z - DETOUR_Z).abs() < 1.25
            || ((q.x - 14.0).abs() < 1.0 || (q.x - 26.0).abs() < 1.0)
                && (-1.0..9.5).contains(&q.z));
    let paved = q.z.abs() < 3.5 || service_road
        || buildings(cfg)
            .any(|(x, z)| {
                (q.x - x as f32).abs() < 1.5 && q.z.abs() <= z.abs() as f32 && q.z * z as f32 >= 0.0
            });
    let apron = if landscaped && !paved {
        let edge = (q.z.abs() - 9.5).max(-4.0 - q.x).max(q.x - 56.0);
        (top * crate::smoothstep(0.0, 3.0, edge) / v).round() * v
    } else {
        0.0
    };
    if landscaped && !paved && (apron - top).abs() < v * 0.5 {
        return None;
    }
    Some(if q.y >= apron {
        B::Air
    } else if q.y < apron - v {
        B::Dirt
    } else if paved {
        B::Gravel
    } else if landscaped && column.surface.is_solid() {
        column.surface
    } else {
        B::Grass
    })
}
