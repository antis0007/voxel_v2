//! Grid-aligned homes with complete shells and a shared stair/landing opening.
use super::*;

pub(super) const GENERATOR: u32 = 77;
#[derive(Clone)]
pub(in crate::valley) struct Prepared {
    parcels: Vec<Parcel>,
}
#[derive(Clone)]
struct Parcel {
    origin: Vec3,
    ax: Vec3,
    az: Vec3,
    hw: f32,
    hd: f32,
    role: u8,
    start: Vec3,
    path: Vec3,
    inverse_length: f32,
}

impl Prepared {
    pub(in crate::valley) fn new(plan: &Plan, cfg: &WorldSampleConfig) -> Self {
        let v = cfg.voxel_meters();
        let right = Vec3::new(-plan.hamlet_fwd.z, 0.0, plan.hamlet_fwd.x);
        let cardinal = if right.x.abs() > right.z.abs() {
            Vec3::X * right.x.signum()
        } else {
            Vec3::Z * right.z.signum()
        };
        let parcels = PLOTS
            .into_iter()
            .map(|(x, z, w, d, role)| {
                let facing = if z < 0.0 { -1.0 } else { 1.0 };
                let az = cardinal * facing;
                let ax = Vec3::new(az.z, 0.0, -az.x);
                let mut origin = plan.hamlet + plan.hamlet_fwd * x + right * z;
                origin.x = (origin.x / v).round() * v;
                origin.z = (origin.z / v).round() * v;
                origin.y =
                    cfg.world_y(cfg.blocks_for_height(plan.hamlet.y + x * plan.hamlet_grade));
                let (origin, ax, az) = if role == 2 {
                    workshop_frame(plan, cfg)
                } else {
                    (origin, ax, az)
                };
                let (hw, hd) = if role == 2 {
                    (4.0, 4.0)
                } else {
                    (w.max(8.0) * 0.5, d.max(8.0) * 0.5)
                };
                let start = origin - az * hd;
                let finish = plan.hamlet + plan.hamlet_fwd * x + right * (2.0 * facing);
                let path = Vec3::new(finish.x - start.x, 0.0, finish.z - start.z);
                Parcel {
                    origin,
                    ax,
                    az,
                    hw,
                    hd,
                    role: if role == 0 && x < 0.0 { 4 } else { role },
                    start,
                    path,
                    inverse_length: 1.0 / path.length_squared().max(0.01),
                }
            })
            .collect();
        Self { parcels }
    }

    pub(in crate::valley) fn sample(&self, m: Vec3, street_floor: f32, v: f32) -> Option<B> {
        for p in &self.parcels {
            let q = m - p.origin;
            if !(-6.0..16.0).contains(&q.y) {
                continue;
            }
            let bound = p.hw.max(p.hd) + 1.5;
            if q.x.abs() <= bound && q.z.abs() <= bound {
                let (x, z) = (q.dot(p.ax), q.dot(p.az));
                let block = if p.role == 2 {
                    facade::sample(x, q.y, z, v, p.ax.x.abs() > 0.5)
                } else {
                    house(p, x, q.y, z, v)
                };
                if block.is_some() {
                    return block;
                }
            }
            let delta = Vec3::new(m.x - p.start.x, 0.0, m.z - p.start.z);
            // Cheap rectangle rejection before the connector projection.
            if delta.x < p.path.x.min(0.0) - 1.5
                || delta.x > p.path.x.max(0.0) + 1.5
                || delta.z < p.path.z.min(0.0) - 1.5
                || delta.z > p.path.z.max(0.0) + 1.5
            {
                continue;
            }
            let along = (delta.dot(p.path) * p.inverse_length).clamp(0.0, 1.0);
            if (delta - p.path * along).length_squared() < 2.25 {
                let y = m.y - (p.origin.y + (street_floor - p.origin.y) * along);
                if (-2.0..v).contains(&y) {
                    return Some(B::DressedStone);
                }
                if (v..v + 2.8).contains(&y) {
                    return Some(B::Air);
                }
            }
        }
        None
    }
}

fn house(p: &Parcel, x: f32, y: f32, z: f32, v: f32) -> Option<B> {
    let (hw, hd, t) = (p.hw, p.hd, v.max(0.25));
    let eave = v.max(0.65);
    if x.abs() >= hw + eave || z.abs() >= hd + eave {
        return None;
    }
    let inside = x.abs() < hw && z.abs() < hd;
    let two = p.role == 1 || p.role == 4;
    let plate = if two { 8.0 } else { 4.0 };
    let roof = plate + (hw - x.abs()) * 0.65;
    let under = roof - 0.65 * v;
    if y >= under && y < roof + v.max(0.3) {
        return Some(B::RoofTiles);
    }
    if !inside {
        // Leave forecourts to the graded connector and shared street sampler.
        return if y >= plate - eave * 0.65 - v {
            Some(B::Air)
        } else {
            None
        };
    }
    if y < 0.0 {
        return Some(B::DressedStone);
    }
    if y < v {
        return Some(B::Planks);
    }
    if y >= under {
        return Some(B::Air);
    }
    let side = x.abs() >= hw - t;
    let end = z.abs() >= hd - t;
    let floor = if two && y >= 4.0 + v { 4.0 + v } else { v };
    if side || end {
        let front = z < -hd + t;
        if front && x.abs() < 1.25 && y < v + 2.5 {
            return Some(B::Air);
        }
        let along = if side { z } else { x };
        let span = if side { hd } else { hw };
        let corner = side && end;
        let window = !corner
            && (along.abs() - span * 0.55).abs() < v.max(0.65)
            && y >= floor + 1.0
            && y < floor + 2.0
            && y < plate - t;
        if window {
            return Some(B::Air);
        }
        return Some(
            if corner || y >= plate - t || two && (4.0..4.0 + v).contains(&y) {
                B::Log
            } else {
                B::Limestone
            },
        );
    }
    let stair_z = hd - t - 2.0;
    let left = -hw + t;
    let right = hw - t;
    if two {
        // The same footprint owns both the flight and its ceiling opening.
        if z >= stair_z {
            let rise = (((x - left) / (right - left - 1.0)).clamp(0.0, 1.0) * 4.0 / v).floor() * v;
            return Some(if y < v + rise { B::Planks } else { B::Air });
        }
        if (4.0..4.0 + v).contains(&y) {
            return Some(B::Planks);
        }
        if y >= 4.0 + v && y < 5.0 + v && z >= stair_z - t && x < right - 1.0 {
            return Some(B::Log);
        }
        if y >= 4.0 + v && y < plate && x >= 0.0 && x < t {
            return Some(if z.abs() < 1.25 && y < 4.0 + v + 2.5 {
                B::Air
            } else {
                B::Limestone
            });
        }
    } else if z >= 0.0 && z < t && y < plate {
        return Some(if x.abs() < 1.25 && y < v + 2.5 {
            B::Air
        } else {
            B::Limestone
        });
    }
    // Furnish wall bays; leave the central entrance, doors and stairs clear.
    if y >= floor && y < floor + v.max(0.6) {
        if x < left + 1.0 && z > -hd + t && z < -hd + t + 1.0 {
            return Some(if floor > v { B::Bed } else { B::Workbench });
        }
        if !two && x < left + 1.0 && z > hd - t - 1.0 {
            return Some(if p.role == 3 { B::SalvageCrate } else { B::Bed });
        }
        if floor > v && x > right - 1.0 && z < -hd + t + 1.0 {
            return Some(B::Bed);
        }
    }
    Some(B::Air)
}
