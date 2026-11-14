//! Streets, building lots and the transition back to natural terrain.
use super::{Kind, rotate, shapes::roll};
use crate::{BlockId as B, WorldSampleConfig, sample_column, value_noise2};
use glam::{Vec2, Vec3};

pub(super) const AVENUE: f32 = 5.0;
pub(super) const CITY_HALF: Vec2 = Vec2::new(220.0, 160.0);
pub(super) const CITY_X: [f32; 7] = [-173.0, -111.0, -49.0, 0.0, 52.0, 114.0, 174.0];
pub(super) const CITY_Z: [f32; 5] = [-110.0, -56.0, -4.0, 48.0, 103.0];
pub(super) const CROSS_AVENUE: f32 = CITY_Z[3];
pub(super) const CITY_PIERS: [f32; 8] = [-197.0, -142.0, -80.0, -25.0, 26.0, 83.0, 144.0, 197.0];
const RAMP_GRADE: f32 = 0.45;

#[derive(Clone, Copy)]
pub(super) struct Approach {
    edge: Vec2,
    direction: Vec2,
    pub run: f32,
    pub grade: f32,
    pub base: f32,
}

impl Approach {
    pub fn survey(
        cfg: &WorldSampleConfig,
        centre: Vec2,
        turn: u32,
        datum: f32,
        edge: Vec2,
        direction: Vec2,
        base: f32,
    ) -> Self {
        let ground = |out: f32| {
            let q = edge + direction * out;
            let p = centre + rotate(turn, q.x, q.y);
            cfg.world_y(
                sample_column(cfg, cfg.blocks_for_height(p.x), cfg.blocks_for_height(p.y)).height
                    + 1,
            )
        };
        let run = (1..=60)
            .find(|&i| (ground(i as f32) - datum - base).abs() <= RAMP_GRADE * i as f32)
            .unwrap_or(60) as f32;
        let grade = ((ground(run) - datum - base) / run).clamp(-0.65, 0.65);
        Self {
            edge,
            direction,
            run,
            grade,
            base,
        }
    }

    pub fn offset(self, q: Vec2, margin: f32) -> Option<(f32, f32)> {
        let delta = q - self.edge;
        let along = delta.dot(self.direction);
        let across = delta.perp_dot(self.direction).abs();
        (across <= AVENUE + margin && (-margin..=self.run + margin).contains(&along))
            .then_some((along, across))
    }

    /// Edge point, outward direction and run of the ramp.
    pub fn span(self) -> (Vec2, Vec2, f32) {
        (self.edge, self.direction, self.run)
    }

    pub fn foot(self) -> Vec3 {
        let p = self.edge + self.direction * (self.run + 2.0);
        Vec3::new(p.x, 0.0, p.y)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Urban {
    pub half: Vec2,
    pub city: bool,
    pub cross_street: f32,
    pub side_streets: [f32; 2],
    pub back_streets: [f32; 2],
    corners: [f32; 4],
    wards: Option<Wards>,
}

#[derive(Clone, Copy)]
struct Wards {
    elevation: [f32; 2],
    bend: [f32; 7],
}

impl Urban {
    pub fn new(half: Vec2, seed: u32, city: bool) -> Self {
        Self {
            half,
            city,
            cross_street: 16.0 + (roll(seed, 82, 19) * 8.0).floor(),
            side_streets: [
                -62.0 + (roll(seed, 22, 19) * 7.0).floor(),
                57.0 + (roll(seed, 32, 19) * 7.0).floor(),
            ],
            back_streets: [
                -62.0 + (roll(seed, 42, 19) * 8.0).floor(),
                56.0 + (roll(seed, 52, 19) * 7.0).floor(),
            ],
            corners: std::array::from_fn(|i| 19.0 + roll(seed, i as i32, 83) * 22.0),
            wards: None,
        }
    }

    pub fn survey_wards(
        mut self,
        cfg: &WorldSampleConfig,
        centre: Vec2,
        turn: u32,
        datum: f32,
        seed: u32,
    ) -> Self {
        let elevation = [-134.0, 133.0].map(|z| {
            let mut samples = [-156.0, -78.0, 0.0, 78.0, 156.0].map(|x| {
                let p = centre + rotate(turn, x, z);
                cfg.world_y(
                    sample_column(cfg, cfg.blocks_for_height(p.x), cfg.blocks_for_height(p.y))
                        .height
                        + 1,
                )
            });
            samples.sort_by(f32::total_cmp);
            (samples[2] - datum).round().clamp(-7.0, 7.0)
        });
        let bend = std::array::from_fn(|i| {
            if CITY_X[i] == 0.0 {
                return 0.0;
            }
            let side = if roll(seed, i as i32, 921) < 0.5 {
                -1.0
            } else {
                1.0
            };
            side * (5.0 + roll(seed, i as i32, 922) * 5.0)
        });
        self.wards = Some(Wards { elevation, bend });
        self
    }

    pub fn terraced(self) -> bool {
        self.wards.is_some()
    }

    pub fn street_x(self, i: usize, z: f32) -> f32 {
        let Some(wards) = self.wards else {
            return CITY_X[i];
        };
        let s = ((z - CITY_Z[2]) / 66.0).clamp(-1.0, 1.0);
        CITY_X[i] + wards.bend[i] * s * s.abs() * (3.0 - 2.0 * s.abs())
    }

    pub fn elevation(self, z: f32) -> f32 {
        let Some(wards) = self.wards else {
            return 0.0;
        };
        let n = ((CITY_Z[0] + 8.0 - z) / 16.0).clamp(0.0, 1.0);
        let s = ((z - CITY_Z[4] + 8.0) / 16.0).clamp(0.0, 1.0);
        wards.elevation[0] * n + wards.elevation[1] * s
    }

    pub fn street_margin(self, z: f32) -> f32 {
        if self.terraced() && (z == CITY_Z[0] || z == CITY_Z[4]) {
            8.3
        } else if z == CROSS_AVENUE {
            AVENUE + 2.0
        } else {
            5.0
        }
    }

    pub fn edge(self, q: Vec2) -> f32 {
        let corner = usize::from(q.x >= 0.0) + 2 * usize::from(q.y >= 0.0);
        let from_edge = self.half - q.abs();
        from_edge
            .min_element()
            .min((from_edge.element_sum() - self.corners[corner]) * std::f32::consts::FRAC_1_SQRT_2)
    }

    pub fn parcel(self, lo: Vec2, hi: Vec2) -> bool {
        if self.city {
            return CITY_X.iter().enumerate().all(|(i, &x)| {
                let a = self.street_x(i, lo.y);
                let b = self.street_x(i, hi.y);
                let margin = if x == 0.0 { AVENUE + 2.0 } else { 5.0 };
                lo.x > a.max(b) + margin || hi.x < a.min(b) - margin
            }) && CITY_Z.into_iter().all(|z| {
                let margin = self.street_margin(z);
                lo.y > z + margin || hi.y < z - margin
            }) && [lo, hi, Vec2::new(lo.x, hi.y), Vec2::new(hi.x, lo.y)]
                .into_iter()
                .all(|p| self.edge(p) > 5.0);
        }
        (lo.x > AVENUE + 2.0 || hi.x < -AVENUE - 2.0)
            && (lo.y > self.cross_street + 6.0 || hi.y < self.cross_street - 6.0)
            && self
                .side_streets
                .into_iter()
                .all(|x| lo.x > x + 4.0 || hi.x < x - 4.0)
            && self
                .back_streets
                .into_iter()
                .all(|z| lo.y > z + 4.0 || hi.y < z - 4.0)
            && [lo, hi, Vec2::new(lo.x, hi.y), Vec2::new(hi.x, lo.y)]
                .into_iter()
                .all(|p| self.edge(p) > 5.0)
    }

    pub fn paving(self, kind: Kind, q: Vec3, v: f32, seed: u32) -> B {
        if self.city {
            return city_street(self, q, v, seed).unwrap_or_else(|| {
                let wear = value_noise2(q.x / 8.0, q.z / 8.0, seed ^ 0xe81);
                if wear > 0.70 {
                    B::Grass
                } else if wear > 0.63 {
                    B::GroveMoss
                } else {
                    B::WeatheredBricks
                }
            });
        }
        let x = q.x.abs();
        let z = (q.z - self.cross_street).abs();
        let side = (q.x - self.side_streets[0])
            .abs()
            .min((q.x - self.side_streets[1]).abs());
        let back = (q.z - self.back_streets[0])
            .abs()
            .min((q.z - self.back_streets[1]).abs());
        let street = if x < AVENUE + 2.0 {
            Some((x, q.z, AVENUE))
        } else if z < 6.0 {
            Some((z, q.x, 4.0))
        } else if side < 4.0 {
            Some((side, q.z, 2.5))
        } else if back < 4.0 {
            Some((back, q.x, 2.5))
        } else {
            None
        };
        if let Some((across, along, width)) = street {
            if across >= width {
                return if across < width + v.max(0.5) {
                    B::DressedStone
                } else {
                    B::StoneBricks
                };
            }
            let crossing = z > 3.0 && z < 5.0 && x < AVENUE - 0.8 && q.x.rem_euclid(2.2) < 1.2;
            let centre_line = across < (v * 0.5).max(0.22)
                && along.rem_euclid(9.0) < 4.0
                && (x > AVENUE + 2.0 || z > 6.0)
                && side > 4.0
                && back > 4.0;
            if crossing || centre_line {
                return B::HazardPaint;
            }
            return if value_noise2(q.x * 0.27, q.z * 0.27, seed) > 0.76 {
                B::Gravel
            } else {
                B::Basalt
            };
        }
        let wear = value_noise2(q.x / 6.0, q.z / 6.0, seed ^ 0xe81);
        if wear > 0.72 {
            B::Grass
        } else if wear > 0.63 {
            B::GroveMoss
        } else if wear > 0.58 {
            B::Gravel
        } else if kind == Kind::Offices && q.z > self.cross_street {
            B::WeatheredBricks
        } else {
            B::Concrete
        }
    }
}

fn city_street(urban: Urban, q: Vec3, v: f32, seed: u32) -> Option<B> {
    let near = |streets: &[f32], x: f32, main: f32| {
        streets.iter().find_map(|&at| {
            let delta = (x - at).abs();
            let width = if at == main { AVENUE } else { 3.0 };
            (delta < width + 2.0).then_some((delta, width))
        })
    };
    let x = CITY_X.iter().enumerate().find_map(|(i, &x)| {
        let d = (q.x - urban.street_x(i, q.z)).abs();
        let width = if x == 0.0 { AVENUE } else { 3.0 };
        (d < width + 2.0).then_some((d, width))
    });
    let z = near(&CITY_Z, q.z, CROSS_AVENUE);
    let paint = match (x, z) {
        (Some((dx, wx)), Some((dz, wz))) => {
            if dx >= wx && dz >= wz {
                return Some(B::DressedStone);
            }
            (dx < wx - 0.6 && (dz - wz).abs() < 0.85 && q.x.rem_euclid(2.0) < 1.0)
                || (dz < wz - 0.6 && (dx - wx).abs() < 0.85 && q.z.rem_euclid(2.0) < 1.0)
        }
        (Some((d, w)), None) | (None, Some((d, w))) => {
            if d >= w {
                return Some(if d < w + v.max(0.65) {
                    B::DressedStone
                } else {
                    B::StoneBricks
                });
            }
            let along = if x.is_some() { q.z } else { q.x };
            d < v.max(0.42) * 0.5 && along.rem_euclid(10.0) < 3.5
        }
        (None, None) => return None,
    };
    let wash = urban
        .terraced()
        .then(|| value_noise2(q.x / 8.0, q.z / 8.0, seed ^ 0x1bd));
    Some(if paint && wash.is_none_or(|wear| wear < 0.61) {
        B::HazardPaint
    } else if wash.is_some_and(|wear| wear > 0.71) {
        B::GroveMoss
    } else if wash.is_some_and(|wear| wear > 0.63) {
        B::Gravel
    } else if value_noise2(q.x * 0.27, q.z * 0.27, seed) > 0.79 {
        B::Gravel
    } else {
        B::Basalt
    })
}

#[cfg(test)]
mod route_support_bound_tests {
    use super::*;
    #[test]
    fn all_authored_approach_support_is_inside_conservative_radius() {
        for half in [Vec2::new(120.0,92.0),Vec2::new(112.0,96.0),CITY_HALF] {
            for side in [-1.0,1.0] {
                for (edge,direction) in [(Vec2::new(0.0,side*half.y),Vec2::Y*side),
                    (Vec2::new(side*half.x,CROSS_AVENUE),Vec2::X*side)] {
                    for run in 1..=60 {
                        let a=Approach {edge,direction,run:run as f32,grade:0.0,base:0.0};
                        for along in [0.0,run as f32*0.5,run as f32] {
                            for across in [-AVENUE,0.0,AVENUE] {
                                let q=edge+direction*along+direction.perp()*across;
                                assert!(a.offset(q,0.0).is_some());
                                for turn in 0..4 {
                                    let world=rotate(turn,q.x,q.y);
                                    assert!(world.length()<285.0);
                                    assert!(world.length()<512.0);
                                }
                            }
                        }
                        for turn in 0..4 {
                            for q in [Vec2::new(512.0,0.0),Vec2::new(-512.0,0.0),Vec2::new(0.0,512.0),Vec2::new(0.0,-512.0)] {
                                assert!(a.offset(rotate(turn,q.x,q.y),0.0).is_none());
                            }
                        }
                    }
                }
            }
        }
    }
}