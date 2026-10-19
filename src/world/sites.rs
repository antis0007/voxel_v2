//! Generator 3 exploration sites. Every feature is a pure function of seed and
//! metre coordinates; edits always override it through World/WorldSnapshot.
use super::{BlockId as B, WorldSampleConfig, WorldType, hash2, sample_column, value_noise2};
use glam::{IVec3, Vec3};
use std::cell::RefCell;
use std::collections::HashMap;

fn region_span(cfg: &WorldSampleConfig) -> f32 { if cfg.generator_version >= 9 { 192.0 } else { 224.0 } }
#[derive(Clone, Copy)]
struct Site {
    x: f32,
    z: f32,
    ground: f32,
    turn: u32,
    kind: u32,
    seed: u32,
    starter: bool,
}
thread_local! {
    static SITES: RefCell<(Option<WorldSampleConfig>, HashMap<(i32,i32),Option<Site>>)> = RefCell::new((None,HashMap::new()));
    static GALLERIES: RefCell<(Option<WorldSampleConfig>, HashMap<(i32,i32),Option<f32>>)> = RefCell::new((None,HashMap::new()));
}

fn cavern_floor(cfg: &WorldSampleConfig, gx: i32, gz: i32) -> Option<f32> {
    let n = hash2(gx, gz, cfg.seed.wrapping_add(1101));
    if n < 0.36 {
        return None;
    }
    let ground = sample_column(
        cfg,
        cfg.blocks_for_height(gx as f32 * 72.0),
        cfg.blocks_for_height(gz as f32 * 72.0),
    )
    .height as f32
        * cfg.voxel_meters();
    Some(((ground - 18.0 - n * 25.0) / cfg.voxel_meters()).floor() * cfg.voxel_meters())
}

fn gallery_floor(cfg: &WorldSampleConfig, p: IVec3, m: Vec3) -> Option<f32> {
    let key = (p.x, p.z);
    if let Some(cached) = GALLERIES.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != Some(*cfg) {
            *c = (Some(*cfg), HashMap::new());
        }
        c.1.get(&key).copied()
    }) {
        return cached;
    }
    let gx = (m.x / 72.0).round() as i32;
    let gz = (m.z / 72.0).round() as i32;
    let mut result = None;
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let (Some(a), Some(b)) = (
            cavern_floor(cfg, gx, gz),
            cavern_floor(cfg, gx + dx, gz + dz),
        ) else {
            continue;
        };
        let along = if dx != 0 {
            (m.x - gx as f32 * 72.0) * dx as f32
        } else {
            (m.z - gz as f32 * 72.0) * dz as f32
        };
        if !(0.0..=72.0).contains(&along) {
            continue;
        }
        let t = along / 72.0;
        let side = if dx != 0 {
            m.z - gz as f32 * 72.0
        } else {
            m.x - gx as f32 * 72.0
        };
        // Symmetric bow: both directions evaluate the same connection.
        let bend = (t * std::f32::consts::PI).sin() * 3.0;
        if (side - bend).abs() < 2.0 {
            result = Some(((a + (b - a) * t) / cfg.voxel_meters()).floor() * cfg.voxel_meters());
            break;
        }
    }
    GALLERIES.with(|c| {
        let mut c = c.borrow_mut();
        if c.1.len() >= 8192 {
            c.1.clear();
        }
        c.1.insert(key, result);
    });
    result
}

fn site(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<Site> {
    let cached = SITES.with(|s| {
        let mut c = s.borrow_mut();
        if c.0 != Some(*cfg) {
            *c = (Some(*cfg), HashMap::new());
        }
        c.1.get(&(rx, rz)).copied()
    });
    if let Some(s) = cached {
        return s;
    }
    let starter = rx == 0 && rz == 0;
    let seed = cfg
        .seed
        .wrapping_add((rx as u32).wrapping_mul(73856093))
        .wrapping_add((rz as u32).wrapping_mul(19349663));
    let x = if starter {
        0.0
    } else {
        rx as f32 * region_span(cfg) + (hash2(rx, rz, seed) - 0.5) * if cfg.generator_version >= 9 { 30.0 } else { 70.0 }
    };
    let z = if starter {
        -42.0
    } else {
        rz as f32 * region_span(cfg) + (hash2(rx, rz, seed.wrapping_add(1)) - 0.5) * if cfg.generator_version >= 9 { 30.0 } else { 70.0 }
    };
    let ground = sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height
        as f32
        * cfg.voxel_meters();
    let result = if starter
        || (ground > 17.0 && ground < 105.0 && hash2(rx, rz, seed.wrapping_add(3)) > 0.18)
    {
        Some(Site {
            x,
            z,
            ground,
            turn: if starter {
                0
            } else {
                (hash2(rx, rz, seed.wrapping_add(4)) * 4.0) as u32 % 4
            },
            kind: if starter {
                0
            } else {
                let kinds = if cfg.generator_version >= 4 { 6 } else { 3 };
                (hash2(rx, rz, seed.wrapping_add(5)) * kinds as f32) as u32 % kinds
            },
            seed,
            starter,
        })
    } else {
        None
    };
    SITES.with(|s| {
        let mut c = s.borrow_mut();
        if c.1.len() >= 256 {
            c.1.clear();
        }
        c.1.insert((rx, rz), result);
    });
    result
}

fn local(s: Site, p: Vec3) -> Vec3 {
    let x = p.x - s.x;
    let z = p.z - s.z;
    let (x, z) = match s.turn {
        1 => (-z, x),
        2 => (-x, -z),
        3 => (z, -x),
        _ => (x, z),
    };
    Vec3::new(x, p.y - s.ground, z)
}

fn site_point(s: Site, p: Vec3) -> Vec3 {
    let (x,z) = match s.turn { 1=>(p.z,-p.x),2=>(-p.x,-p.z),3=>(-p.z,p.x),_=>(p.x,p.z) };
    Vec3::new(s.x+x,s.ground+p.y,s.z+z)
}

/// These fixtures share the ruin's orientation and metre scale across voxel sizes.
pub fn foundry_stations(cfg: &WorldSampleConfig, id: IVec3) -> Vec<Vec3> {
    let Some(s) = site(cfg,id.x,id.y) else { return Vec::new(); };
    if id.z != 0 || s.kind != 3 { return Vec::new(); }
    [(-5.0,-6.0),(0.0,-6.0),(5.0,-6.0),(0.0,8.0),(-12.0,-7.0)]
        .into_iter().map(|(x,z)|site_point(s,Vec3::new(x,cfg.voxel_meters(),z))).collect()
}

pub fn foundry_rotation(cfg: &WorldSampleConfig, id: IVec3) -> f32 {
    site(cfg,id.x,id.y).map_or(0.0,|s|s.turn as f32*std::f32::consts::FRAC_PI_2)
}

/// Returns `(world_pos_meters, kind, turn, size_variation)` for surface sites within `max_dist` metres.
pub fn horizon_landmarks(
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    max_dist: f32,
) -> Vec<(Vec3, u32, u32, f32)> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return Vec::new();
    }
    let span = region_span(cfg);
    let radius = (max_dist / span).ceil() as i32 + 1;
    let crx = (center.0 as f32 / span).round() as i32;
    let crz = (center.1 as f32 / span).round() as i32;
    let mut out = Vec::new();
    for rz in crz - radius..=crz + radius {
        for rx in crx - radius..=crx + radius {
            let Some(s) = site(cfg, rx, rz) else {
                continue;
            };
            let dist = (s.x - center.0 as f32).hypot(s.z - center.1 as f32);
            if dist > max_dist {
                continue;
            }
            let variation = if s.starter {
                0.0
            } else {
                hash2(0, 0, s.seed.wrapping_add(67)) * 3.0
            };
            out.push((Vec3::new(s.x, s.ground, s.z), s.kind, s.turn, variation));
        }
    }
    out
}

/// A bounded, on-interaction survey; never run this during voxel sampling or every frame.
pub fn next_relic(cfg: &WorldSampleConfig, origin: Vec3, collected: u32) -> Option<(u32,Vec3)> {
    if cfg.generator_version < 4 || cfg.world_type == WorldType::Superflat { return None; }
    let target = if collected & 1==0 {3} else if collected & 2==0 {5} else {8};
    let mut best: Option<(u32,Vec3)> = None;
    let mut consider = |kind: u32, p: Vec3| {
        if best.is_none_or(|(_,q)| (p-origin).length_squared() < (q-origin).length_squared()) { best=Some((kind,p)); }
    };
    let rx=(origin.x/region_span(cfg)).round() as i32;
    let rz=(origin.z/region_span(cfg)).round() as i32;
    for x in rx-5..=rx+5 { for z in rz-5..=rz+5 {
        if let Some(s)=site(cfg,x,z) {
            if s.kind==target {
                consider(s.kind,site_point(s,Vec3::Y));
            }
        }
    } }
    if target==8 && collected & 4==0 {
        let gx=(origin.x/72.0).round() as i32; let gz=(origin.z/72.0).round() as i32;
        for x in gx-8..=gx+8 { for z in gz-8..=gz+8 {
            if (x as f32*72.0).hypot(z as f32*72.0)<100.0 || reward_kind(cfg,IVec3::new(x,z,1))!=8 {continue;}
            if let Some(floor)=cavern_floor(cfg,x,z) {consider(8,Vec3::new(x as f32*72.0,floor+1.0,z as f32*72.0));}
        } }
    }
    best
}

fn sample_site(s: Site, cfg: &WorldSampleConfig, p: Vec3) -> Option<B> {
    let q = local(s, p);
    let v = cfg.voxel_meters();
    // A dry, walkable descent connects each mine to its first fungal chamber.
    // Positive local depth is forward into the hillside (negative local z).
    let depth = -q.z;
    if s.kind == 0 && q.x.abs() < 2.6 && (-1.0..=44.0).contains(&depth) {
        let floor = -(depth.max(0.0) * 0.45).min(18.0);
        let floor = (floor / v).floor() * v;
        if q.y >= floor && q.y < floor + v {
            return Some(B::Cobblestone);
        }
        if q.y >= floor + v && q.y < floor + 4.4 {
            if depth > 4.0 && depth.rem_euclid(7.0) < v && (q.x.abs() > 1.7 || q.y > floor + 3.5) {
                return Some(B::Log);
            }
            return Some(B::Air);
        }
        if q.y >= floor - v && q.y < floor {
            return Some(B::Limestone);
        }
    }
    // Broad chamber, side galleries and physically sized mushroom clusters.
    let cx = q.x;
    let cz = q.z + 48.0;
    let chamber = (cx / 17.0).powi(2) + (cz / 19.0).powi(2);
    if s.kind == 0 && chamber < 1.12 && q.y >= -20.0 && q.y < 0.0 {
        let floor = -19.0;
        let roof = -6.0 - chamber * 3.0 + (cx * 0.25).sin() * 0.8;
        if chamber < 1.0 && q.y >= floor && q.y < roof {
            if q.y < floor + v {
                return Some(B::FungalSoil);
            }
            if let Some(b) = fungus(cfg, s.seed, Vec3::new(cx, q.y - floor, cz), None) {
                return Some(b);
            }
            // Ore seams are exposed in the chamber floor, not random treasure.
            if q.y < floor + 2.0 * v && cx.abs() > 11.0 {
                return Some(if cz > 3.0 { B::TinOre } else { B::CopperOre });
            }
            if (cx - 10.0).abs() < 1.0 && cz.abs() < 1.0 && q.y < floor + 3.0 {
                return Some(B::Crystal);
            }
            return Some(B::Air);
        }
        if chamber < 1.12 && q.y < roof + 1.0 {
            return Some(B::Limestone);
        }
    }
    // Ruin grammars: workshop court, watchtower, and colonnaded reservoir.
    let variation = if s.starter {
        0.0
    } else {
        hash2(0, 0, s.seed.wrapping_add(67)) * 3.0
    };
    let width = match s.kind {
        1 => 5.0,
        3 => 14.0 + variation,
        4 => 12.0,
        _ => 9.0 + variation,
    };
    let length = match s.kind {
        2 | 5 => 13.0,
        3 => 11.0,
        4 => 12.0,
        _ => 8.0 + variation,
    };
    let max_height = if s.kind == 1 { 22.0 } else { 14.0 };
    if q.x.abs() > width + 1.0 || q.z.abs() > length + 1.0 || q.y < -12.0 || q.y > max_height {
        return None;
    }
    let inside = q.x.abs() < width && q.z.abs() < length;
    if !inside {
        return None;
    }
    if q.y < 0.0 {
        return Some(B::WeatheredBricks);
    }
    if s.kind == 1 && q.y >= 10.0 {
        let r = q.x.hypot(q.z);
        if q.y < 15.5 {
            if r > 4.8 {
                return None;
            }
            if r >= 3.8 {
                let arch_window = q.y > 11.5
                    && q.y < 14.0
                    && (q.x.abs() < 0.85 || q.z.abs() < 0.85);
                return Some(if arch_window { B::Air } else { B::StoneBricks });
            }
            if q.y < 10.0 + v && (q.x > -1.8 || q.z > -1.8) {
                return Some(B::Planks);
            }
            if q.x.abs() < 0.75 && q.z.abs() < 0.75 && (12.5..14.2).contains(&q.y) {
                return Some(B::Crystal);
            }
            return Some(B::Air);
        } else {
            let spire_t = ((q.y - 15.5) / 6.0).clamp(0.0, 1.0);
            let cone_r = 5.2 * (1.0 - spire_t * 0.92);
            if r <= cone_r {
                return Some(if spire_t > 0.82 { B::Crystal } else { B::Planks });
            }
            return None;
        }
    }
    let wall = q.x.abs() > width - 1.0 || q.z.abs() > length - 1.0;
    let doorway = q.x.abs() < 2.0 && q.z.abs() > length - 1.0;
    let corner_turret = s.kind == 3 && q.x.abs() > width - 2.8 && q.z.abs() > length - 2.8;
    let wall_height = if s.kind == 1 {
        10.0
    } else if corner_turret {
        let merlon = ((q.x.floor() as i32 + q.z.floor() as i32) & 1) == 0;
        if merlon { 8.6 } else { 7.6 }
    } else {
        3.0 + value_noise2(q.x * 0.19, q.z * 0.19, s.seed) * 3.0
    };
    if wall && !doorway && q.y < wall_height {
        let window = q.y > 3.0 && q.y < 5.0 && (q.x.abs() < 1.5 || q.z.abs() < 1.5);
        if !window {
            return Some(B::WeatheredBricks);
        }
    }
    if q.y < v {
        // Mine mouth is already carved above; other rooms have solid floor.
        if cfg.generator_version >= 12 && s.kind == 3 {
            // An inlaid process line leads from the operating plate to the three valves.
            return Some(if q.x.abs()<0.6 || (q.z+6.0).abs()<0.4 && q.x.abs()<6.0 { B::Basalt } else { B::StoneBricks });
        }
        return Some(B::StoneBricks);
    }
    if cfg.generator_version >= 12 && s.kind == 3 {
        // An authored court: broad central dodge lanes, two waist-high cover islands,
        // and a screened maintenance alcove accessible around its southern end.
        if (q.x.abs()-6.0).abs()<0.65 && q.z.abs()<2.0 && q.y<1.5 {return Some(B::Basalt);}
        if (q.x+10.7).abs()<0.5 && q.z < -4.0 && q.z > -9.5 && q.y<3.2 {return Some(B::WeatheredBricks);}
        if q.x < -10.7 && q.z < -4.0 && q.z > -9.5 && q.y>3.0 && q.y<3.5 {return Some(B::Planks);}
        // The entrance's surviving arch frames the heart rather than blocking it.
        if q.z>8.5 && q.z<9.3 && ((q.x.abs()-3.0).abs()<0.55 && q.y<5.0 || q.x.abs()<3.5 && q.y>4.4 && q.y<5.0) {return Some(B::Basalt);}
    }
    if cfg.generator_version >= 4 {
        // Upper tower cache rewards climbing; all caches have exactly 1 m³ of loot.
        if s.kind == 1
            && (-1.0..0.0).contains(&q.x)
            && (3.0..4.0).contains(&q.z)
            && (6.0..7.0).contains(&q.y)
        {
            return Some(B::SurveyCache);
        }
        // Foundry: twin furnace stacks, overhead trusses, and ore bunkers.
        if s.kind == 3 {
            if (q.x.abs() - 9.0).abs() < 1.5 && (q.z + 5.0).abs() < 1.5 && q.y < 10.0 {
                let hollow = (q.x.abs() - 9.0).abs() < 0.65 && (q.z + 5.0).abs() < 0.65;
                return Some(if hollow { B::Air } else { B::Basalt });
            }
            if q.y > 6.0 && q.y < 7.0 && q.z.abs() < 1.0 && q.x < width - 2.0 {
                return Some(B::Log);
            }
            if q.x > 7.0 && q.x < 10.0 && q.z > 4.0 && q.z < 7.0 && q.y < 2.0 {
                return Some(B::IronOre);
            }
            if q.x < -7.0 && q.x > -10.0 && q.z > 4.0 && q.z < 7.0 && q.y < 2.0 {
                return Some(B::CoalOre);
            }
        }
        // Conservatory: broken colonnade and four walkable mushroom beds.
        if s.kind == 4 {
            if (q.x.abs() - 7.0).abs() < 0.6 && (q.z.abs() - 7.0).abs() < 0.6 && q.y < 7.0 {
                return Some(B::Limestone);
            }
            if q.x.abs() > 2.5 && q.x.abs() < 6.0 && q.z.abs() > 3.5 && q.z.abs() < 6.0 {
                if q.y < 1.0 {
                    return Some(B::FungalSoil);
                }
                if q.y < 2.0 {
                    return Some(if q.x * q.z > 0.0 {
                        B::RedCap
                    } else {
                        B::GlowCap
                    });
                }
            }
            if q.y > 7.0 && q.y < 8.0 && q.x > 0.0 && q.z.abs() > 6.5 && q.z.abs() < 7.5 {
                return Some(B::Planks);
            }
        }
        // Archive: branching aisles, eroded dividers and a crystal-lit rear alcove.
        if s.kind == 5 {
            if q.z.abs() < 9.0 && q.z.abs() > 1.5 && (q.x.abs() - 5.0).abs() < 0.6 && q.y < 3.5 {
                return Some(B::Planks);
            }
            if q.z < -10.0 && q.x.abs() > 5.5 && q.x.abs() < 6.5 && q.y < 3.0 {
                return Some(B::Crystal);
            }
        }
    }
    // Props retain a one-metre body at every voxel resolution. Cache contents
    // are volume-weighted by the expedition system.
    let point = |x: f32, z: f32| {
        q.x >= x && q.x < x + 1.0 && q.z >= z && q.z < z + 1.0 && q.y >= 1.0 && q.y < 2.0
    };
    if point(-4.0, 2.0) {
        return Some(B::Workbench);
    }
    if point(4.0, 2.0) {
        return Some(B::Smelter);
    }
    if point(-4.0, -3.0) {
        return Some(if cfg.generator_version < 4 {
            B::SalvageCrate
        } else {
            match s.kind {
                1 | 3 | 5 => B::SurveyCache,
                2 | 4 => B::CultivatorCache,
                _ => B::SalvageCrate,
            }
        });
    }
    // Raised pedestals make small-scale stations discoverable and reachable.
    if q.y < 1.0 && ((q.x + 4.0).abs() < 0.8 || (q.x - 4.0).abs() < 0.8) && (q.z - 2.0).abs() < 0.8
    {
        return Some(B::Planks);
    }
    if q.y < 1.0 && (q.x + 4.0).abs() < 0.8 && (q.z + 3.0).abs() < 0.8 {
        return Some(B::Planks);
    }
    if s.kind == 2 && q.y < 2.0 && q.x.abs() < 4.0 && q.z.abs() < 5.0 {
        return Some(B::Water);
    }
    if s.kind == 1 {
        let stair = ((q.z + 4.0) * 0.65).floor();
        if q.x > 2.0 && q.x < 4.0 && q.z > -4.0 && q.z < 4.0 && q.y > 0.0 && q.y < stair + v {
            return Some(B::StoneBricks);
        }
        if q.y > 5.0 && q.y < 5.0 + v && q.x < 2.0 && q.z > 2.0 {
            return Some(B::Planks);
        }
    }
    if s.starter && q.z > 4.0 && q.x.abs() < 1.0 && q.y < 3.0 {
        return Some(B::Air);
    }
    Some(B::Air)
}

fn fungus(cfg: &WorldSampleConfig, seed: u32, q: Vec3, ecosystem: Option<u32>) -> Option<B> {
    let gx = (q.x / 7.0).round() as i32;
    let gz = (q.z / 7.0).round() as i32;
    let n = if gx == 0 && gz == 1 {
        0.9
    } else {
        hash2(gx, gz, seed.wrapping_add(73))
    };
    let ax = gx as f32 * 7.0 + (n - 0.5) * 2.5;
    let az = gz as f32 * 7.0 + (hash2(gx, gz, seed.wrapping_add(79)) - 0.5) * 2.5;
    let radius = (q.x - ax).hypot(q.z - az);
    let variant = if cfg.generator_version >= 4 {
        ecosystem.unwrap_or((hash2(0, 0, seed.wrapping_add(167)) * 4.0) as u32 % 4)
    } else {
        0
    };
    let height = 2.0 + n * if variant == 2 { 5.0 } else { 3.5 };
    let cap = if variant == 1 && !(gx == 0 && gz == 1) {
        B::RedCap
    } else if n > 0.4 {
        B::GlowCap
    } else {
        B::RedCap
    };
    if radius < 0.5_f32.max(cfg.voxel_meters() * 0.7) && q.y < height && q.y >= cfg.voxel_meters() {
        return Some(B::MushroomStem);
    }
    if q.y >= height - 0.7
        && q.y < height + 1.1
        && radius < (2.4 + n) * (1.0 - ((q.y - height) / 1.6).powi(2)).max(0.0)
    {
        return Some(cap);
    }
    // Small luminous fruiting bodies make a continuous readable floor trail.
    if radius > 2.8 && radius < 3.5 && q.y < cfg.voxel_meters() * 2.0 && n > 0.45 {
        return Some(B::GlowCap);
    }
    None
}

/// Holy-mountain mouth 18 m east of spawn, then four stacked landings.
fn noita_descent(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    if cfg.generator_version < 13 {
        return None;
    }
    let dx = m.x - 18.0;
    let dz = m.z;
    if dx.abs() > 14.0 || dz.abs() > 10.0 {
        return None;
    }
    let v = cfg.voxel_meters();
    let ground = sample_column(cfg, cfg.blocks_for_height(18.0), 0).height as f32 * v;
    let below = ground - m.y;
    if dz.abs() < 1.15 && (8.0..16.5).contains(&m.x) {
        let path = sample_column(cfg, cfg.blocks_for_height(m.x), cfg.blocks_for_height(m.z)).height as f32 * v;
        if (m.y - path).abs() < v {
            return Some(B::Cobblestone);
        }
    }
    if !(-5.0..=96.0).contains(&below) {
        return None;
    }
    let shaft = dx.abs() < 1.6 && dz.abs() < 1.6;
    if shaft && (0.4..90.0).contains(&below) {
        let turn = below * 0.85;
        let sx = turn.cos() * 1.05;
        let sz = turn.sin() * 1.05;
        if (dx - sx).hypot(dz - sz) < 0.55 && below.fract() < v.max(0.4) {
            return Some(B::StoneBricks);
        }
        if below.rem_euclid(8.0) < v && (dx - sx).hypot(dz - sz) < 0.7 {
            return Some(B::GlowCap);
        }
    }
    let band = (below / 24.0).floor();
    let room = (1.0..=3.0).contains(&band)
        && (1.6..9.0).contains(&dx)
        && dz.abs() < 4.0
        && below.rem_euclid(24.0) < 6.0;
    let landing = below > 8.0 && below.rem_euclid(24.0) < 1.2 && dx.abs() < 7.0 && dz.abs() < 5.0;
    if shaft || room {
        if landing && m.y < ground - 1.0 {
            return Some(match band as u32 {
                0 => B::Gravel,
                1 => B::FungalSoil,
                2 => B::Ice,
                _ => B::Basalt,
            });
        }
        if m.y < ground + v {
            return Some(B::Air);
        }
    }
    let mouth = (-4.5..1.0).contains(&below) && dx.abs() < 2.4 && dz.abs() < 2.4;
    let opening = dx.abs() < 1.5 && dz.abs() < 1.5 && (-3.6..0.2).contains(&below);
    if mouth && !opening {
        return Some(B::StoneBricks);
    }
    None
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3) -> Option<B> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    if let Some(carved) = noita_descent(cfg, m) {
        return Some(carved);
    }
    let rx = (m.x / region_span(cfg)).round() as i32;
    let rz = (m.z / region_span(cfg)).round() as i32;
    if cfg.generator_version >= 9 && rz != 0 && m.x.abs() < 32.0 && (-130.0..0.0).contains(&m.z) {
        if let Some(s) = site(cfg, 0, 0) {
            if let Some(b) = sample_site(s, cfg, m) { return Some(b); }
        }
    }
    // Site bounds stay within their owning region even after rotation/jitter.
    if let Some(s) = site(cfg, rx, rz) {
        if let Some(b) = sample_site(s, cfg, m) {
            return Some(b);
        }
    }
    None
}

/// Additional dry underground ecosystems between sites, independent of sea level.
pub fn cavern(cfg: &WorldSampleConfig, p: IVec3, height: i32) -> Option<B> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    if m.x.hypot(m.z) < 100.0 {
        return None;
    }
    let depth = height as f32 * cfg.voxel_meters() - m.y;
    if depth < 9.0 || depth > 70.0 {
        return None;
    }
    if let Some(floor) = gallery_floor(cfg, p, m) {
        let dy = m.y - floor;
        if dy >= 0.0 && dy < 4.2 {
            return Some(if dy < cfg.voxel_meters() {
                B::FungalSoil
            } else {
                B::Air
            });
        }
    }
    let gx = (m.x / 72.0).round() as i32;
    let gz = (m.z / 72.0).round() as i32;
    let n = hash2(gx, gz, cfg.seed.wrapping_add(1101));
    if n < 0.36 {
        return None;
    }
    let ax = gx as f32 * 72.0;
    let az = gz as f32 * 72.0;
    let ground = sample_column(cfg, cfg.blocks_for_height(ax), cfg.blocks_for_height(az)).height
        as f32
        * cfg.voxel_meters();
    let floor = ((ground - 18.0 - n * 25.0) / cfg.voxel_meters()).floor() * cfg.voxel_meters();
    let dx = m.x - ax;
    let dz = m.z - az;
    let dy = m.y - floor;
    let variant = if cfg.generator_version >= 4 {
        (hash2(gx, gz, cfg.seed.wrapping_add(1187)) * 4.0) as u32 % 4
    } else {
        0
    };
    let ellipse =
        (dx / (18.0 + n * 9.0)).powi(2) + (dz / if variant == 2 { 29.0 } else { 23.0 }).powi(2);
    if ellipse > 1.1 || dy < -1.5 || dy > 14.0 {
        return None;
    }
    let roof = (if variant == 2 { 14.0 } else { 11.0 }) - ellipse * 5.0 + (dx * 0.21).sin();
    // The broken road is an optional construction puzzle, with a walkable floor detour.
    // Version-gated so existing worlds retain their terrain and discoveries.
    if cfg.generator_version >= 12 && variant == 2 && ellipse < 0.85
        && (-15.0..17.0).contains(&dx) && (6.0..12.0).contains(&dz)
        && (0.0..7.0).contains(&dy) {
        if dy < cfg.voxel_meters() { return Some(B::Limestone); }
        // Broad steps lead onto the western bridge; a three-metre gap invites building.
        let stair = (((dx + 15.0) * 0.5 / cfg.voxel_meters()).floor() * cfg.voxel_meters()).clamp(0.0, 3.0);
        if dx < -7.0 && (8.0..11.0).contains(&dz) && dy < stair + 1.0 { return Some(B::StoneBricks); }
        if (8.0..11.0).contains(&dz) && dx >= -7.0 && dx < 14.0
            && (dx < -1.5 || dx > 1.5) && (3.0..4.0).contains(&dy) { return Some(B::StoneBricks); }
        // Survey cache on a luminous landing, reachable by bridge, climbing, or excavation.
        if (13.0..16.0).contains(&dx) && (7.0..12.0).contains(&dz) && dy < 4.0 { return Some(B::Limestone); }
        if (14.0..15.0).contains(&dx) && (9.0..10.0).contains(&dz) && (4.0..5.0).contains(&dy) { return Some(B::SurveyCache); }
        if (dx + 7.0).abs() < 0.5 && (6.0..7.0).contains(&dz) && dy < 2.0 { return Some(B::StoneBricks); }
        if (dx.abs() - 5.0).abs() < 0.5 && (11.0..12.0).contains(&dz) && (0.5..1.5).contains(&dy) { return Some(B::Crystal); }
        return Some(B::Air);
    }
    if ellipse < 1.0 && dy > 0.0 && dy < roof {
        if dy < cfg.voxel_meters() {
            return Some(B::FungalSoil);
        }
        if cfg.generator_version >= 4 {
            // A perimeter niche leaves the main gallery clear and rewards detours.
            if (8.0..9.0).contains(&dx) && (5.0..6.0).contains(&dz) && (1.0..2.0).contains(&dy) {
                return Some(if variant == 2 {
                    B::SurveyCache
                } else {
                    B::CultivatorCache
                });
            }
            if (8.0..9.0).contains(&dx) && (5.0..6.0).contains(&dz) && dy < 1.0 {
                return Some(B::Limestone);
            }
            // Rich seams and geodes become visible along the chamber wall.
            if ellipse > 0.77 && ellipse < 0.95 && dy < 2.5 && dz.abs() < 3.0 {
                return Some(match variant {
                    1 => B::CoalOre,
                    2 => B::Crystal,
                    3 => B::TinOre,
                    _ => B::IronOre,
                });
            }
            if variant == 2 && (dx.abs() - 10.0).abs() < 1.0 && (dz.abs() - 11.0).abs() < 1.0 {
                return Some(if dy > roof - 2.0 || dy < 2.0 {
                    B::Crystal
                } else {
                    B::Limestone
                });
            }
            // Shallow pools split by a dry causeway, never a mandatory swim.
            if variant == 3 && dx.abs() > 3.0 && dx.abs() < 8.0 && dz.abs() < 9.0 && dy < 1.6 {
                return Some(B::Water);
            }
            if variant == 1 && ellipse > 0.90 && dy < 4.0 {
                return Some(B::Basalt);
            }
        }
        return Some(
            fungus(
                cfg,
                if cfg.generator_version >= 4 {
                    cfg.seed
                        .wrapping_add((gx as u32).wrapping_mul(73856093))
                        .wrapping_add((gz as u32).wrapping_mul(19349663))
                } else {
                    cfg.seed.wrapping_add((gx ^ gz) as u32)
                },
                Vec3::new(dx, dy, dz),
                Some(variant),
            )
            .unwrap_or(B::Air),
        );
    }
    if dy < roof + 1.0 {
        return Some(B::Limestone);
    }
    None
}

pub fn location(cfg: &WorldSampleConfig, p: Vec3) -> Option<&'static str> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let m = p * cfg.voxel_meters();
    if cfg.generator_version >= 4 && m.x.hypot(m.z) > 100.0 {
        let gx = (m.x / 72.0).round() as i32;
        let gz = (m.z / 72.0).round() as i32;
        if let Some(floor) = cavern_floor(cfg, gx, gz) {
            if m.y > floor
                && m.y < floor + 14.0
                && (m.x - gx as f32 * 72.0).abs() < 28.0
                && (m.z - gz as f32 * 72.0).abs() < 29.0
            {
                return Some(
                    match (hash2(gx, gz, cfg.seed.wrapping_add(1187)) * 4.0) as u32 % 4 {
                        0 => "Lanternroot grove",
                        1 => "Embercap hollows",
                        2 if cfg.generator_version >= 12 => "Prismatic cathedral - the broken road",
                        2 => "Prismatic cathedral",
                        _ => "Drowned mycelium gardens",
                    },
                );
            }
        }
    }
    let s = site(
        cfg,
        (m.x / region_span(cfg)).round() as i32,
        (m.z / region_span(cfg)).round() as i32,
    )?;
    let q = local(s, m);
    if s.kind == 0 && q.x.abs() < 20.0 && (q.z + 48.0).abs() < 23.0 && q.y < -5.0 {
        Some("Lanternroot caverns")
    } else if q.x.abs() < 18.0 && q.z.abs() < 16.0 {
        Some(match s.kind {
            0 => "Ruined artificer workshop",
            1 => "Overgrown watchtower",
            2 => "Sunken waterworks",
            3 => "Abandoned foundry",
            4 => "Mycologist conservatory",
            _ => "Resonant archive",
        })
    } else {
        None
    }
}

/// Encounter anchors share the terrain's deterministic site layout. Coordinates
/// are metres; the third key component separates ruins from fungal chambers.
pub fn encounters(cfg:&WorldSampleConfig, player:Vec3) -> Vec<(IVec3,Vec3,u32)> {
    let mut out=Vec::new();
    if cfg.generator_version<3 || cfg.world_type==WorldType::Superflat { return out; }
    let rx=(player.x/region_span(cfg)).round() as i32; let rz=(player.z/region_span(cfg)).round() as i32;
    for x in rx-1..=rx+1 { for z in rz-1..=rz+1 {
        if let Some(s)=site(cfg,x,z) {
            let point=if s.starter { Vec3::new(11.0,s.ground-18.0,-90.0) } else { Vec3::new(s.x,s.ground+1.0,s.z+3.0) };
            if (point-player).length()<52.0 { out.push((IVec3::new(x,z,0),point,if s.starter {0} else {1+s.kind%2})); }
        }
    } }
    let gx=(player.x/72.0).round() as i32; let gz=(player.z/72.0).round() as i32;
    for x in gx-1..=gx+1 { for z in gz-1..=gz+1 {
        if (x as f32*72.0).hypot(z as f32*72.0)<100.0 { continue; }
        if let Some(floor)=cavern_floor(cfg,x,z) {
            let point=Vec3::new(x as f32*72.0,floor+1.0,z as f32*72.0);
            if (point-player).length()<46.0 { out.push((IVec3::new(x,z,1),point,(hash2(x,z,cfg.seed.wrapping_add(1187))*3.0) as u32%3)); }
        }
    } }
    out
}

/// Reward identity follows the actual generated structure, not the enemy mix.
pub fn reward_kind(cfg:&WorldSampleConfig,id:IVec3)->u32 {
    if id.z==0 {site(cfg,id.x,id.y).map_or(0,|s|s.kind)}
    else {6+if cfg.generator_version>=4 {(hash2(id.x,id.y,cfg.seed.wrapping_add(1187))*4.0) as u32%4}else{0}}
}

/// A readable environmental clue; the route accepts building, climbing, and digging.
pub fn inscription(cfg: &WorldSampleConfig, p: IVec3) -> Option<&'static str> {
    if cfg.generator_version < 12 { return None; }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    let gx = (m.x / 72.0).round() as i32;
    let gz = (m.z / 72.0).round() as i32;
    if (hash2(gx,gz,cfg.seed.wrapping_add(1187))*4.0) as u32 % 4 != 2 { return None; }
    let floor = cavern_floor(cfg,gx,gz)?;
    let q = m - Vec3::new(gx as f32*72.0,floor,gz as f32*72.0);
    if (-15.0..17.0).contains(&q.x) && (6.0..12.0).contains(&q.z) && (0.0..7.0).contains(&q.y) {
        Some("The carved road ends in midair. Across the gap, a survey chest rests on pale stone. The inscription reads: The doorway has no lock; the missing road is the key.")
    } else { None }
}

/// Opt-in native review spawn: uses the real generator, without modifying terrain.
pub fn review_causeway_spawn(cfg: &WorldSampleConfig) -> Option<Vec3> {
    if cfg.generator_version < 12 || cfg.world_type == WorldType::Superflat { return None; }
    for ring in 2i32..12 {
        for gx in -ring..=ring {
            for gz in -ring..=ring {
                if gx.abs().max(gz.abs()) != ring { continue; }
                if (hash2(gx,gz,cfg.seed.wrapping_add(1187))*4.0) as u32 % 4 != 2 { continue; }
                if let Some(floor) = cavern_floor(cfg,gx,gz) {
                    return Some(Vec3::new(gx as f32*72.0-5.0,floor+4.1,gz as f32*72.0+9.5) / cfg.voxel_meters());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod holy_mountain {
    use super::*;
    #[test]
    fn spawn_has_a_brick_mouth_and_a_carved_shaft() {
        use crate::fluid::VoxelScalePreset;
        let world = crate::world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 17);
        let cfg = world.sample_config();
        let ground = sample_column(&cfg, cfg.blocks_for_height(18.0), 0).height;
        let mouth = world.get(IVec3::new(19, ground + 2, 0));
        let shaft = world.get(IVec3::new(18, ground - 12, 0));
        assert_eq!(mouth, B::StoneBricks);
        assert_eq!(shaft, B::Air);
        assert_eq!(world.get(IVec3::new(12, ground, 0)), B::Cobblestone);
    }
}
#[cfg(test)]
mod causeway_review {
    use super::*;
    #[test]
    fn causeway_deck_gap_and_reward_exist_at_each_voxel_scale() {
        use crate::fluid::VoxelScalePreset;
        for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::HalfMeter,VoxelScalePreset::QuarterMeter] {
            let world = crate::world::World::with_seed(WorldType::Hills,scale,3040332434);
            let cfg = world.sample_config();
            let spawn = review_causeway_spawn(&cfg).unwrap() * scale.meters();
            let sample = |offset:Vec3| world.get(((spawn+offset)/scale.meters()).floor().as_ivec3());
            assert_eq!(sample(Vec3::new(0.0,-0.6,0.0)),B::StoneBricks,"deck at {:?}",scale);
            assert_eq!(sample(Vec3::new(5.0,-0.6,0.0)),B::Air,"gap at {:?}",scale);
            assert_eq!(sample(Vec3::new(19.5,0.4,0.0)),B::SurveyCache,"cache at {:?}",scale);
        }
    }
}
