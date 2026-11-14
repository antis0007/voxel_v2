//! Survey a dry spur and its approach against the generated terrain in metres.
use super::{SEA, geo::Ground, tectonics::smooth};
use glam::DVec2;

pub(super) struct Outlook {
    tip: DVec2,
    approach: DVec2,
    aim: DVec2,
    toward_water: DVec2,
    tip_m: f64,
    approach_m: f64,
}

#[derive(Clone, Copy)]
pub(super) struct Grade {
    pub level: f64,
    pub weight: f64,
    pub clearing: f64,
}

impl Outlook {
    pub fn survey(outlet: DVec2, terrain: impl Fn(DVec2) -> Ground) -> Option<Self> {
        let mut best: Option<(f64, Self)> = None;
        let dry = |p| {
            let g = terrain(p);
            let height = f64::from(g.top_m);
            (height >= SEA + 5.0 && g.water_m.is_none()).then_some(height)
        };
        let seaward = outlet.normalize();
        let views = [outlet, outlet + seaward * 86.0, outlet + seaward * 172.0]
            .map(|p| (p, f64::from(terrain(p).top_m) < SEA));
        for x in [-120.0, -86.0, -52.0, -18.0, 16.0, 50.0] {
            for z in [-240.0, -192.0, -144.0, -96.0, 96.0, 144.0, 192.0, 240.0] {
                let tip = DVec2::new(x, z);
                let Some(tip_m) = dry(tip).filter(|&h| (SEA + 8.0..=SEA + 95.0).contains(&h))
                else {
                    continue;
                };
                let Some(aim) = views.into_iter().find_map(|(aim, wet)| {
                    (wet && (1..16).all(|step| {
                        let t = f64::from(step) / 16.0;
                        let ray = (tip_m + 2.0) * (1.0 - t) + (SEA + 1.5) * t;
                        f64::from(terrain(tip.lerp(aim, t)).top_m) <= ray
                    }))
                    .then_some(aim)
                }) else {
                    continue;
                };
                let toward_water = (aim - tip).normalize();
                let drop = tip_m - f64::from(terrain(tip + toward_water * 24.0).top_m);
                if drop < 3.0 {
                    continue;
                }
                for bend in [-0.90, -0.45, 0.0, 0.45, 0.90] {
                    let back = (toward_water + toward_water.perp() * bend).normalize();
                    let approach = tip - back * 56.0;
                    if approach.x < -192.0 || approach.y.abs() > 284.0 {
                        continue;
                    }
                    let Some(approach_m) = dry(approach) else {
                        continue;
                    };
                    let rise = tip_m - approach_m;
                    if !(-5.0..=12.0).contains(&rise) {
                        continue;
                    }
                    let mut cut = 0.0f64;
                    let suitable = (0..=16).all(|step| {
                        let t = f64::from(step) / 16.0;
                        let center = approach.lerp(tip, t);
                        let level = approach_m + rise * t;
                        [-3.0, 0.0, 3.0].into_iter().all(|side| {
                            dry(center + back.perp() * side).is_some_and(|h| {
                                let difference = (h - level).abs();
                                cut = cut.max(difference);
                                difference <= 3.0
                            })
                        })
                    });
                    if !suitable {
                        continue;
                    }
                    let score = drop.min(22.0) + (tip_m - SEA) * 0.12 - cut * 2.0;
                    if best.as_ref().is_none_or(|(old, _)| score > *old) {
                        best = Some((
                            score,
                            Self {
                                tip,
                                approach,
                                aim,
                                toward_water,
                                tip_m,
                                approach_m,
                            },
                        ));
                    }
                }
            }
        }
        best.map(|(_, outlook)| outlook)
    }

    pub fn sample(&self, point: DVec2) -> Option<Grade> {
        let offset = point - self.tip;
        if offset.length_squared() > 76.0f64.powi(2) {
            return None;
        }
        let edge = self.tip - self.approach;
        let t = ((point - self.approach).dot(edge) / edge.length_squared()).clamp(0.0, 1.0);
        let distance = point.distance(self.approach + edge * t);
        let entrance = smooth(0.0, 0.13, t);
        let trail = (1.0 - smooth(2.2, 8.0, distance)) * entrance;
        let front = offset.dot(self.toward_water);
        let side = offset.dot(self.toward_water.perp());
        let radius = (front / if front > 0.0 { 10.0 } else { 17.0 }).hypot(side / 11.0);
        let shelf = 1.0 - smooth(0.48, 1.0, radius);
        let weight = trail.max(shelf);
        if weight <= 0.0 {
            return None;
        }
        let path = self.approach_m + (self.tip_m - self.approach_m) * t;
        Some(Grade {
            level: path + (self.tip_m - path) * shelf,
            weight,
            clearing: ((1.0 - smooth(3.0, 7.0, distance)) * entrance)
                .max(1.0 - smooth(0.70, 1.17, radius)),
        })
    }

    pub fn points(&self) -> [DVec2; 3] {
        [self.approach, self.tip, self.aim]
    }

    pub fn checksum(&self, mut hash: u64) -> u64 {
        for value in [
            self.tip.x,
            self.tip.y,
            self.approach.x,
            self.approach.y,
            self.aim.x,
            self.aim.y,
            self.toward_water.x,
            self.toward_water.y,
            self.tip_m,
            self.approach_m,
        ] {
            hash = (hash ^ value.to_bits()).wrapping_mul(0x100000001b3);
        }
        hash
    }
}
