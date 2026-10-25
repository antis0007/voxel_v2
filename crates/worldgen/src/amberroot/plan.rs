//! Grove authored layout. Local frame: `u` runs along the approach
//! (entrance at -u, clearing at the origin), `v` to its right.
use super::FLOOR_R;
use super::giant::Giant;
use crate::{WorldSampleConfig, hash2, value_noise2};
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;


pub(super) struct Pool {
    pub c: Vec2,
    pub r: f32,
    pub depth: f32,
    pub level: f32,
}

pub(super) struct Shroom {
    pub x: f32,
    pub z: f32,
    pub base: f32,
    pub height: f32,
    pub stem: f32,
    pub cap: f32,
    pub lean: Vec2,
    pub red: bool,
}

pub(super) struct Plan {
    pub o: Vec2,
    pub f: Vec2,
    pub floor: f32,
    pub seed: u32,
    pub phase: f32,
    pub giants: Vec<Giant>,
    pub shrooms: Vec<Shroom>,
    pub pools: Vec<Pool>,
    /// Door of the hollow tree, local (u, v); the clearing spur ends here.
    pub door: Vec2,
    /// Generator 43 palette: teal canopy layers and moss under the crowns.
    pub palette: bool,
}

/// Local (u, v, trunk radius, height) of the authored giants. Index 0 throws
/// the root arch over the approach; index 3 is the hollow tree. The last two
/// stand behind the clearing as the far, cooler layer.
const GIANTS: [(f32, f32, f32, f32); 8] = [
    (-50.0, 18.0, 3.3, 34.0),
    (-27.0, -25.0, 3.0, 30.0),
    (7.0, -37.0, 3.7, 40.0),
    (32.0, 10.0, 4.6, 36.0),
    (14.0, 36.0, 3.2, 38.0),
    (-17.0, 34.0, 2.8, 28.0),
    (55.0, -31.0, 3.5, 44.0),
    (59.0, 37.0, 3.8, 46.0),
];
pub(super) const HOLLOW: usize = 3;

impl Plan {
    pub fn local(&self, wx: f32, wz: f32) -> (f32, f32) {
        let d = Vec2::new(wx, wz) - self.o;
        (d.dot(self.f), d.dot(self.r()))
    }

    pub fn world(&self, u: f32, v: f32) -> Vec2 {
        self.o + self.f * u + self.r() * v
    }

    fn r(&self) -> Vec2 {
        Vec2::new(-self.f.y, self.f.x)
    }

    /// Shaped floor height before pools: a shallow bowl with a gentle swell.
    pub fn base_floor(&self, u: f32, v: f32) -> f32 {
        let d = u.hypot(v);
        let bowl = 1.8 * (1.0 - (d / FLOOR_R).powi(2)).max(0.0);
        let w = self.world(u, v);
        let swell = (value_noise2(w.x / 17.0, w.y / 17.0, self.seed.wrapping_add(35_011)) - 0.5) * 1.6;
        self.floor - bowl + swell
    }

    pub fn floor_at(&self, u: f32, v: f32) -> f32 {
        let mut h = self.base_floor(u, v);
        for p in &self.pools {
            let d = (Vec2::new(u, v) - p.c).length();
            if d < p.r {
                // The bed is cut from the water line down, so every pool holds water.
                h = h.min(p.level - 0.9 - p.depth * (1.0 - d / p.r).powf(0.6));
            } else if d < p.r + 1.5 {
                // A lip at least half a metre above the water closes the bowl.
                h = h.max(p.level + 0.5);
            }
        }
        h
    }

    /// Still-water surface over a pool bowl.
    pub fn pool_at(&self, u: f32, v: f32) -> Option<f32> {
        self.pools
            .iter()
            .find(|p| (Vec2::new(u, v) - p.c).length() < p.r)
            .map(|p| p.level)
    }

    /// Path centreline offset: a lazy S-bend that straightens into the clearing.
    pub(super) fn path_v(&self, u: f32) -> f32 {
        4.5 * (u / 19.0 + self.phase).sin() * crate::smoothstep(-8.0, -30.0, u)
    }

    pub fn path_distance(&self, u: f32, v: f32) -> f32 {
        let approach = if u < -6.0 { (v - self.path_v(u)).abs() } else { f32::MAX };
        let spur = segment_distance(Vec2::new(u, v), Vec2::ZERO, self.door);
        approach.min(spur)
    }

    pub fn on_path(&self, u: f32, v: f32) -> bool {
        self.path_distance(u, v) < 1.6 || u.hypot(v) < 4.0
    }

    /// Navigable negative space: the approach, the clearing and the door spur.
    pub fn keep_clear(&self, u: f32, v: f32) -> bool {
        self.path_distance(u, v) < 2.6 || u.hypot(v) < 13.0
    }

    /// 1 along the open approach mouth, 0 elsewhere.
    pub fn approach_weight(&self, u: f32, v: f32) -> f32 {
        crate::smoothstep(18.0, 9.0, (v - self.path_v(u)).abs()) * crate::smoothstep(-20.0, -40.0, u)
    }

    /// Generator 43: moss where a giant's crown shades the floor.
    pub fn shaded(&self, wx: f32, wz: f32) -> bool {
        self.palette && self.giants.iter().any(|g| g.shades(wx, wz))
    }

    /// Blue-violet mycelium under the giants and around the mushroom grove.
    pub fn mossy(&self, wx: f32, wz: f32) -> bool {
        let patch = value_noise2(wx / 7.0, wz / 7.0, self.seed.wrapping_add(35_027));
        let near_giant = self.giants.iter().any(|g| (g.x - wx).hypot(g.z - wz) < g.radius * 3.4);
        let near_shroom = self.shrooms.iter().any(|s| (s.x - wx).hypot(s.z - wz) < s.cap + 2.0);
        (near_giant && patch > 0.42) || (near_shroom && patch > 0.3)
    }
}

fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

pub(super) fn layout(cfg: &WorldSampleConfig, o: Vec2, floor: f32, seed: u32) -> Plan {
    // Enter from the spawn side, walking away from it into the hollow.
    let f = o.normalize_or(Vec2::X);
    let mut plan = Plan {
        o,
        f,
        floor,
        seed,
        phase: hash2(5, 1, seed) * TAU,
        giants: Vec::new(),
        shrooms: Vec::new(),
        pools: Vec::new(),
        door: Vec2::ZERO,
        palette: cfg.generator_version >= super::PALETTE_VERSION,
    };
    for (c, r, depth) in [(Vec2::new(2.0, -20.0), 5.0, 1.5), (Vec2::new(-22.0, 13.0), 3.6, 1.1)] {
        // Water stands 0.6 m under the lowest rim point, so every rim voxel
        // rises at least level with it and nothing spills.
        let rim = (0..16)
            .map(|i| {
                let a = i as f32 * TAU / 16.0;
                let q = c + Vec2::new(a.cos(), a.sin()) * (r + 0.8);
                plan.base_floor(q.x, q.y)
            })
            .fold(f32::MAX, f32::min);
        plan.pools.push(Pool { c, r, depth, level: rim - 0.5 });
    }
    let vm = cfg.voxel_meters();
    for (i, &(u, v, radius, height)) in GIANTS.iter().enumerate() {
        let j = |salt: u32| hash2(i as i32, salt as i32, seed) - 0.5;
        let (u, v) = (u + j(1) * 5.0, v + j(2) * 4.0);
        let w = plan.world(u, v);
        let base = plan.floor_at(u, v);
        let radius = (radius * (1.0 + j(3) * 0.18)).max(vm * 2.0);
        let height = height * (1.0 + j(4) * 0.2);
        let toward_clearing = (plan.world(0.0, 0.0) - w).normalize_or(Vec2::X);
        // Generator 43: the far pair is all teal, a cool back layer; near
        // crowns mix teal and broadleaf masses.
        let teal = match (plan.palette, i >= 6) {
            (false, _) => 0.0,
            (true, true) => 1.0,
            (true, false) => 0.55,
        };
        let mut g = Giant::new(Vec3::new(w.x, base, w.y), radius, height, seed.wrapping_add(i as u32 * 97), teal);
        if i == HOLLOW {
            g.hollow = Some(toward_clearing);
            plan.door = Vec2::new(u, v) - Vec2::new(u, v).normalize_or(Vec2::X) * (g.ground_radius() + 1.0);
        }
        plan.giants.push(g);
    }
    // The arch springs from the first giant, crosses the path and lands beyond it.
    let u_cross = plan.local(plan.giants[0].x, plan.giants[0].z).0 - 5.0;
    let over = plan.world(u_cross, plan.path_v(u_cross));
    let land = plan.world(u_cross - 4.0, plan.path_v(u_cross) - 13.0);
    let g0 = &plan.giants[0];
    let to = (over - Vec2::new(g0.x, g0.z)).normalize_or(Vec2::X);
    let spring = Vec2::new(g0.x, g0.z) + to * g0.ground_radius() * 0.8;
    let arch = [
        Vec3::new(spring.x, g0.base + 3.5, spring.y),
        Vec3::new(over.x, plan.base_floor(u_cross, plan.path_v(u_cross)) + 12.5, over.y),
        Vec3::new(land.x, plan.base_floor(u_cross - 4.0, plan.path_v(u_cross) - 13.0) - 0.8, land.y),
    ];
    plan.giants[0].set_arch(arch);
    place_shrooms(&mut plan);
    plan
}

fn place_shrooms(plan: &mut Plan) {
    // A warm cluster between the clearing and the hollow tree, plus two at the
    // arch's foot that announce the grove from the approach.
    let cluster = Vec2::new(17.0, -6.0);
    let mut spots: Vec<Vec2> = (0..9)
        .map(|i| {
            let a = hash2(i, 31, plan.seed) * TAU;
            let d = 3.0 + hash2(i, 37, plan.seed) * 7.0;
            cluster + Vec2::new(a.cos(), a.sin()) * d
        })
        .collect();
    spots.push(Vec2::new(-44.0, plan.path_v(-44.0) - 7.0));
    spots.push(Vec2::new(-40.0, plan.path_v(-40.0) + 6.5));
    for (i, s) in spots.into_iter().enumerate() {
        let h = |salt: i32| hash2(i as i32, salt, plan.seed.wrapping_add(7));
        let cap = 2.0 + h(1) * 2.6;
        let clash = plan.keep_clear(s.x, s.y)
            || plan.path_distance(s.x, s.y) < cap + 1.0
            || plan.pools.iter().any(|p| (s - p.c).length() < p.r + cap)
            || plan.giants.iter().any(|g| {
                let (gu, gv) = plan.local(g.x, g.z);
                (s - Vec2::new(gu, gv)).length() < g.ground_radius() + cap + 1.5
            })
            || plan.shrooms.iter().any(|o| {
                let (ou, ov) = plan.local(o.x, o.z);
                (s - Vec2::new(ou, ov)).length() < o.cap + cap * 0.7
            });
        if clash {
            continue;
        }
        let w = plan.world(s.x, s.y);
        plan.shrooms.push(Shroom {
            x: w.x,
            z: w.y,
            base: plan.floor_at(s.x, s.y),
            height: 3.2 + h(2) * 5.0 + cap * 0.5,
            stem: 0.5 + h(3) * 0.5,
            cap,
            lean: Vec2::new(h(4) - 0.5, h(5) - 0.5) * 0.25,
            red: h(6) > 0.8,
        });
    }
}

