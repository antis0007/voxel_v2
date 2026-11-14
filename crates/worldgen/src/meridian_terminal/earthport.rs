//! Earthport: scale and real footings for the generator 65 anchor monument.
//!
//! Above the datum three flying fins and a crown cage widen the silhouette
//! without crossing the arrival axis. Below it the tether's load path is built
//! as rooms: the root column, strain frame, six tendons and their anchor blocks
//! in a domed vault whose piers are the six buttress caissons, and the service
//! works that reach it (freight ramp, substation, stair, sump, vents).
//!
//! Everything is editable voxels in the monument's own frame. The datum, the
//! shaft, the boarding berth and the station cells are the monument's and are
//! not moved, so the app's fittings and braid socket stay valid.
use super::{X, Z, floor, monument};
use crate::{BlockId as B, WorldSampleConfig};
use glam::{IVec3, Vec3};
use std::f32::consts::{FRAC_PI_4, TAU};

pub const VERSION: u32 = 301;

/// Box holding everything the monument or the earthport can place.
const OUTER: f32 = 130.0;
const APPROACH_END: f32 = 280.0;
/// Horizontal reach of every earthport member, from the tether axis.
const REACH: f32 = 124.0;
/// Lowest lined voxel below the datum (the sump slab). The world floor is at
/// least 79 m below any datum: sea level 12 m, datum 2 m above it, bottom −64 m.
const DEEPEST: f32 = -74.0;

const VAULT_R: f32 = 56.0;
const VAULT_FLOOR: f32 = -64.0;
/// Dome apex; the ceiling falls 12 m to the spring line at the wall.
const VAULT_CROWN: f32 = -22.0;
const GALLERY_Y: f32 = -44.0;
const ROOT_R: f32 = 6.0;
const WELL_R: f32 = 11.5;
/// Underside of the strain frame the root column ends in.
const FRAME_Y: f32 = -40.0;
const FRAME_R: f32 = 10.0;
const SUMP_R: f32 = 7.0;

const RAMP_IN: f32 = 66.0;
const RAMP_OUT: f32 = 72.0;
const RAMP_CLEAR: f32 = 5.0;
/// Bearing of the terrace portal; the road turns once from here.
const RAMP_START: f32 = FRAC_PI_4;
/// Degrees of descent; the last 15 are the level landing at the vault door.
const RAMP_FALL_DEG: f32 = 345.0;

const FIN_TOP: f32 = 226.0;
const CROWN_BASE: f32 = 236.0;

const S3: f32 = 0.866_025_4;
/// Unit bearings every 30 degrees. Six-fold and three-fold members pick their
/// spoke by dot product, so no voxel pays for `atan2` to find its sector.
const DIR12: [(f32, f32); 12] = [
    (1.0, 0.0), (S3, 0.5), (0.5, S3), (0.0, 1.0), (-0.5, S3), (-S3, 0.5),
    (-1.0, 0.0), (-S3, -0.5), (-0.5, -S3), (0.0, -1.0), (0.5, -S3), (S3, -0.5),
];
/// Vault door off the ramp landing, bearing 40 degrees.
const HALL_DOOR: (f32, f32) = (0.766_044_44, 0.642_787_6);
/// Gallery door off the ramp, bearing 285 degrees, where the road passes −44.5 m.
const GALLERY_DOOR: (f32, f32) = (0.258_819_05, -0.965_925_8);
/// Service bearing, 160 degrees: control-wing stair, ramp crossing, substation.
const SERVICE: (f32, f32) = (-0.939_692_6, 0.342_020_14);
/// Road level where the service bearing crosses the ramp (115 of 345 degrees).
const SERVICE_ROAD: f32 = VAULT_FLOOR * 115.0 / RAMP_FALL_DEG;
const STAIR_HEAD: f32 = 40.0;
const HALL_FLOOR: f32 = -22.0;
/// Vent stacks rise from the dome haunch between buttresses, clear of both wings.
const VENTS: [(f32, f32); 2] = [(0.0, -34.0), (29.44, -17.0)];

#[derive(Clone, Copy)]
struct Plan {
    x: f32,
    z: f32,
    r: f32,
}

impl Plan {
    /// (radial, lateral) in the frame of unit bearing `d`.
    fn frame(self, (c, s): (f32, f32)) -> (f32, f32) {
        (self.x * c + self.z * s, self.z * c - self.x * s)
    }

    /// Frame of the nearest spoke among `first, first + step, ..` (twelfths of a turn).
    fn spoke(self, first: usize, step: usize) -> (f32, f32) {
        let mut best = (f32::MIN, 0.0);
        let mut i = first;
        while i < 12 {
            let (radial, lateral) = self.frame(DIR12[i]);
            if radial > best.0 {
                best = (radial, lateral);
            }
            i += step;
        }
        best
    }

    fn bearing(self) -> f32 {
        self.z.atan2(self.x)
    }

    /// True for the one voxel column whose footprint holds the nearest post of a
    /// ring of posts at `ring` metres, spaced `step` radians from `phase`.
    fn post(self, v: f32, ring: f32, step: f32, phase: f32) -> bool {
        if (self.r - ring).abs() >= v {
            return false;
        }
        let a = ((self.bearing() - phase) / step).round() * step + phase;
        let (s, c) = a.sin_cos();
        self.holds(v, ring * c, ring * s)
    }

    fn holds(self, v: f32, x: f32, z: f32) -> bool {
        let half = 0.5 * v;
        (-half..half).contains(&(x - self.x)) && (-half..half).contains(&(z - self.z))
    }
}

/// Underside of the vault dome at plan radius `r`.
fn ceiling(r: f32) -> f32 {
    let u = r * (1.0 / VAULT_R);
    VAULT_CROWN - 12.0 * u * u
}

/// Degrees travelled from the portal and the road level there.
fn ramp(at: Plan) -> (f32, f32) {
    let turn = (at.bearing() - RAMP_START).rem_euclid(TAU) * (360.0 / TAU);
    (turn, VAULT_FLOOR * (turn * (1.0 / RAMP_FALL_DEG)).min(1.0))
}

/// Tread level of the service stair at `radial` metres along its bearing.
fn stair(radial: f32) -> f32 {
    SERVICE_ROAD * (radial - STAIR_HEAD) * (1.0 / (RAMP_IN - STAIR_HEAD))
}

/// A lamp standing on `base`: two metres of post and a single torch voxel.
fn lamp(y: f32, base: f32, v: f32) -> Option<B> {
    if y < base {
        None
    } else if y < base + 2.0 {
        Some(B::Basalt)
    } else if y < base + 2.0 + v {
        Some(B::Torch)
    } else {
        None
    }
}

/// The tether root: dark bundle with a steel clamp every eight metres.
fn root(y: f32) -> B {
    if y.rem_euclid(8.0) < 1.0 { B::SteelPlate } else { B::Basalt }
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3, height: i32) -> Option<B> {
    let v = cfg.voxel_meters();
    let (x, z) = ((p.x as f32 + 0.5) * v - X, (p.z as f32 + 0.5) * v - Z);
    // Every voxel in the world asks this. Outside the monument's own footprint
    // (128 m disc plus the approach road to z = 278) answer before the datum lookup.
    if x.abs() > OUTER || !(-OUTER..APPROACH_END).contains(&z) {
        return None;
    }
    let datum = floor(cfg);
    let q = Vec3::new(x, (p.y as f32 + 0.5) * v - datum, z);
    if q.x.abs() < REACH && q.z.abs() < REACH && (DEEPEST..monument::SHAFT_HEIGHT).contains(&q.y) {
        // Plain square root: `hypot` guards overflow these coordinates cannot reach, at several times the cost.
        let at = Plan { x: q.x, z: q.z, r: (q.x * q.x + q.z * q.z).sqrt() };
        if at.r < REACH {
            let own = if q.y >= 0.0 {
                above(q, at)
            } else {
                cavity(q, at, v).or_else(|| lining(q, at, (height + 1) as f32 * v - datum))
            };
            if own.is_some() {
                return own;
            }
        }
    }
    monument::block(cfg, p, height)
}

fn above(q: Vec3, at: Plan) -> Option<B> {
    let y = q.y;
    if y < FIN_TOP {
        // Three flying fins on the bearings between buttresses, none on the
        // arrival axis. Each springs from a foot at 92..118 m and leans in to
        // the top collar, leaving a tall open arc over the wings below it.
        let (radial, lateral) = at.spoke(1, 4);
        let t = y * (1.0 / FIN_TOP);
        if lateral.abs() < 7.0 && radial > 11.0 {
            if lateral.abs() < 4.0 - 2.0 * t {
                let outer = 24.0 + 94.0 * (1.0 - t).powf(1.7);
                let inner = outer - (26.0 - 19.0 * t);
                if radial < outer && radial >= inner {
                    return Some(if radial >= outer - 2.5 {
                        B::Basalt
                    } else if y.rem_euclid(32.0) < 1.5 {
                        B::StoneBricks
                    } else {
                        B::DressedStone
                    });
                }
                // The fin head ties into the shaft's top collar.
                if y >= FIN_TOP - 4.0 && radial < inner {
                    return Some(B::Basalt);
                }
            }
            // Stepped plinth under the foot.
            if (90.0..120.0).contains(&radial) && (y < 4.0 || (y < 8.0 && lateral.abs() < 5.5)) {
                return Some(B::Concrete);
            }
        }
    }
    if y >= CROWN_BASE {
        // Crown cage: a prong off each shaft pier, flaring to a tie ring.
        let (radial, lateral) = at.spoke(0, 2);
        let lean = (y - CROWN_BASE) * 0.35;
        if lateral.abs() < 1.6 && (9.0 + lean..11.5 + lean).contains(&radial) {
            return Some(B::StoneBricks);
        }
        if y >= monument::SHAFT_HEIGHT - 3.0 && (15.5..18.5).contains(&at.r) {
            return Some(B::Basalt);
        }
        return None;
    }
    if y >= 7.0 {
        return None;
    }
    for (x, z) in VENTS {
        let (dx, dz) = (q.x - x, q.z - z);
        let d2 = dx * dx + dz * dz;
        if d2 < 4.5 * 4.5 {
            if y >= 6.0 {
                return Some(B::SteelPlate);
            }
            let slot = (3.0..5.0).contains(&y) && (dx.abs() < 1.0 || dz.abs() < 1.0);
            return (d2 >= 2.5 * 2.5 && d2 < 4.0 * 4.0 && !slot).then_some(B::Concrete);
        }
    }
    if y >= 1.0 {
        return None;
    }
    // Kerbs: the two light slots beside the berth, and the open cut of the ramp.
    if (7.0..WELL_R).contains(&at.r) && q.x.abs() > 4.0 {
        return (!well_slot(q, at)).then_some(B::HazardPaint);
    }
    if (RAMP_IN - 1.0..RAMP_OUT + 1.0).contains(&at.r) && !(RAMP_IN..RAMP_OUT).contains(&at.r) {
        let (turn, road) = ramp(at);
        if turn > 1.5 && road + RAMP_CLEAR > 0.0 {
            return Some(B::HazardPaint);
        }
    }
    None
}

/// Crescent openings in the anchor-chamber floor, either side of the berth walk.
fn well_slot(q: Vec3, at: Plan) -> bool {
    (8.0..10.5).contains(&at.r) && q.x.abs() > 5.0
}

/// Open space below the datum and whatever stands in it.
fn cavity(q: Vec3, at: Plan, v: f32) -> Option<B> {
    let y = q.y;
    if at.r < VAULT_R + 10.0 && y < GALLERY_Y + 4.0 {
        // Both doors are cut first so wall ribs never stand in an opening.
        let (radial, lateral) = at.frame(HALL_DOOR);
        if (54.0..RAMP_IN).contains(&radial) && lateral.abs() < 3.0 && (VAULT_FLOOR..VAULT_FLOOR + 5.0).contains(&y) {
            return Some(B::Air);
        }
        let (radial, lateral) = at.frame(GALLERY_DOOR);
        if (54.0..RAMP_IN).contains(&radial) && lateral.abs() < 2.0 && y >= GALLERY_Y {
            return Some(B::Air);
        }
    }
    if at.r < VAULT_R {
        if y < VAULT_FLOOR {
            // Sump under the strain frame: the low point every drain falls to.
            return (at.r < SUMP_R && y >= VAULT_FLOOR - 6.0)
                .then_some(if y < VAULT_FLOOR - 4.0 { B::Water } else { B::Air });
        }
        if y < ceiling(at.r) {
            return Some(vault(q, at, v));
        }
        if at.r < WELL_R {
            // Light well round the root, from the dome to the chamber floor slab.
            if at.r < ROOT_R {
                return (y < -2.0).then(|| root(y));
            }
            if y < -1.0 || well_slot(q, at) {
                return Some(B::Air);
            }
            return None;
        }
        for (x, z) in VENTS {
            let (dx, dz) = (q.x - x, q.z - z);
            if dx * dx + dz * dz < 2.5 * 2.5 {
                return Some(B::Air);
            }
        }
    }
    if (RAMP_IN..RAMP_OUT).contains(&at.r) {
        let (_, road) = ramp(at);
        if y >= road && y < road + RAMP_CLEAR {
            // A lamp on the inner kerb every 20 degrees, clear of all three doors.
            // None in the open cut, where the post would end at the terrace.
            if road < -3.5 && y < road + 3.0 && at.post(v, RAMP_IN + 0.6, TAU / 18.0, RAMP_START + TAU / 36.0) {
                return lamp(y, road, v).or(Some(B::Air));
            }
            return Some(B::Air);
        }
    }
    let (radial, lateral) = at.frame(SERVICE);
    if radial >= STAIR_HEAD && lateral.abs() < 18.0 {
        if radial < RAMP_IN {
            let tread = stair(radial);
            if lateral.abs() < 1.5 && y >= tread && y < tread + 3.5 {
                return Some(B::Air);
            }
        } else if (77.0..93.0).contains(&radial) {
            if (HALL_FLOOR..HALL_FLOOR + 9.0).contains(&y) {
                return Some(substation(q, at, radial, lateral, v));
            }
        } else if (RAMP_OUT..77.0).contains(&radial) && lateral.abs() < 2.5 && (HALL_FLOOR..HALL_FLOOR + 5.0).contains(&y) {
            return Some(B::Air);
        }
    }
    None
}

/// The anchor vault's interior: the tension works, the caissons and the gallery.
fn vault(q: Vec3, at: Plan, v: f32) -> B {
    let y = q.y;
    if y >= FRAME_Y {
        if at.r < ROOT_R {
            return root(y);
        }
        if y < FRAME_Y + 4.0 && at.r < FRAME_R {
            return if at.r >= FRAME_R - 1.0 { B::HazardPaint } else { B::SteelPlate };
        }
    }
    // Buttress caissons carry on down through the hall as its six piers. The
    // gallery passes through each by a portal on the wall side.
    let (radial, lateral) = at.spoke(0, 2);
    if (42.0..54.0).contains(&radial) && lateral.abs() < 6.0 {
        let portal = radial >= 50.0 && (GALLERY_Y - 1.0..GALLERY_Y + 4.0).contains(&y);
        if !portal {
            return if lateral.abs() > 4.5 || radial < 43.5 { B::Basalt } else { B::DressedStone };
        }
    }
    // Between the piers: a tendon from the frame rim down to each anchor block.
    let (radial, lateral) = at.spoke(1, 2);
    if lateral.abs() < 6.0 && radial >= FRAME_R - 1.0 {
        if (41.0..52.0).contains(&radial) {
            let top = VAULT_FLOOR + 7.0 - (46.0 - radial).max(0.0) * 1.2;
            if y < top {
                return if y >= top - 1.0 && lateral.abs() < 2.5 { B::SteelPlate } else { B::Concrete };
            }
        }
        if lateral.abs() < 0.9 && radial < 46.5 {
            let along = radial - (FRAME_R - 1.0);
            if (y - (FRAME_Y + 2.0 - along * (21.0 / 37.0))).abs() < 1.0 {
                return if along.rem_euclid(9.0) < 1.0 { B::CopperBlock } else { B::SteelPlate };
            }
        }
    }
    if at.r >= 49.0 {
        if (GALLERY_Y - 1.0..GALLERY_Y).contains(&y) {
            return B::RustedSteel;
        }
        if at.r < 50.0 && (GALLERY_Y..GALLERY_Y + 1.0).contains(&y) {
            return B::SteelPlate;
        }
        if at.r >= 54.5 && (at.bearing() * (36.0 / TAU)).rem_euclid(1.0) < 0.2 {
            return B::DressedStone;
        }
        if (GALLERY_Y..GALLERY_Y + 3.0).contains(&y) && at.post(v, 52.5, TAU / 6.0, TAU / 12.0) {
            return lamp(y, GALLERY_Y, v).unwrap_or(B::Air);
        }
    }
    if y < VAULT_FLOOR + 3.0 {
        if (SUMP_R..SUMP_R + 1.0).contains(&at.r) && y < VAULT_FLOOR + 1.0 {
            return B::HazardPaint;
        }
        if at.post(v, 46.0, TAU / 12.0, TAU / 24.0)
            || at.post(v, 30.0, TAU / 12.0, TAU / 24.0)
            || at.post(v, 18.0, TAU / 6.0, 0.0)
        {
            return lamp(y, VAULT_FLOOR, v).unwrap_or(B::Air);
        }
    }
    B::Air
}

/// Substation hall: four transformer bays under one overhead bus.
fn substation(q: Vec3, at: Plan, radial: f32, lateral: f32, v: f32) -> B {
    let y = q.y;
    let bay = (lateral + 16.0).rem_euclid(8.0) - 4.0;
    if lateral.abs() < 14.0 && (84.0..88.0).contains(&radial) && bay.abs() < 1.5 {
        if y < HALL_FLOOR + 4.0 {
            return B::SteelPlate;
        }
        if y < HALL_FLOOR + 7.0 && bay.abs() < 0.5 && (85.5..86.5).contains(&radial) {
            return B::CopperBlock;
        }
    }
    if (85.5..86.5).contains(&radial) && (HALL_FLOOR + 7.0..HALL_FLOOR + 8.0).contains(&y) && lateral.abs() < 17.0 {
        return B::CopperBlock;
    }
    if (79.0..81.0).contains(&radial) && (lateral.abs() - 9.0).abs() < 1.0 && y < HALL_FLOOR + 3.0 {
        let side = 9.0_f32.copysign(lateral);
        let (c, s) = SERVICE;
        if at.holds(v, 80.0 * c - side * s, 80.0 * s + side * c) {
            return lamp(y, HALL_FLOOR, v).unwrap_or(B::Air);
        }
    }
    B::Air
}

/// Structural shell round every cavity, so natural caves and aquifers never
/// open into the works, plus the buried footings of the fins.
fn lining(q: Vec3, at: Plan, terrain: f32) -> Option<B> {
    let y = q.y;
    if at.r < VAULT_R + 3.0 {
        if y >= VAULT_FLOOR - 3.0 {
            if y < ceiling(at.r.min(VAULT_R)) + 3.0 {
                return Some(if (VAULT_FLOOR - 1.0..VAULT_FLOOR).contains(&y) && at.r < VAULT_R {
                    B::DressedStone
                } else {
                    B::Concrete
                });
            }
            if at.r < WELL_R + 2.0 && y < -2.0 {
                return Some(B::Concrete);
            }
            for (x, z) in VENTS {
                let (dx, dz) = (q.x - x, q.z - z);
                if dx * dx + dz * dz < 4.0 * 4.0 {
                    return Some(B::Concrete);
                }
            }
        } else if at.r < SUMP_R + 2.0 && y >= VAULT_FLOOR - 8.0 {
            return Some(B::Concrete);
        }
    }
    if (RAMP_IN - 2.0..RAMP_OUT + 2.0).contains(&at.r) {
        let (turn, road) = ramp(at);
        // Rock plug where the landing meets the bearing of the portal above it.
        if turn < 1.0 && (VAULT_FLOOR - 2.0..VAULT_FLOOR + RAMP_CLEAR + 2.0).contains(&y) {
            return Some(B::Concrete);
        }
        if y >= road - 2.0 && y < road + RAMP_CLEAR + 2.0 {
            let deck = y >= road - 1.0 && y < road && (RAMP_IN..RAMP_OUT).contains(&at.r);
            return Some(if !deck {
                B::Concrete
            } else if at.r < RAMP_IN + 0.6 || at.r >= RAMP_OUT - 0.6 {
                B::HazardPaint
            } else {
                B::StoneBricks
            });
        }
    }
    if at.r < RAMP_IN && at.r >= VAULT_R {
        let (radial, lateral) = at.frame(HALL_DOOR);
        if radial >= VAULT_R && lateral.abs() < 5.0 && (VAULT_FLOOR - 2.0..VAULT_FLOOR + 7.0).contains(&y) {
            return Some(B::Concrete);
        }
        let (radial, lateral) = at.frame(GALLERY_DOOR);
        if radial >= VAULT_R && lateral.abs() < 4.0 && (GALLERY_Y - 2.0..GALLERY_Y + 6.0).contains(&y) {
            return Some(B::Concrete);
        }
    }
    let (radial, lateral) = at.frame(SERVICE);
    if radial >= STAIR_HEAD - 1.0 && lateral.abs() < 20.0 {
        if radial < RAMP_IN {
            let tread = stair(radial.max(STAIR_HEAD));
            if lateral.abs() < 2.5 && y >= tread - 1.0 && y < tread + 4.5 {
                return Some(B::StoneBricks);
            }
        } else if (75.0..95.0).contains(&radial) {
            if (HALL_FLOOR - 2.0..HALL_FLOOR + 11.0).contains(&y) {
                return Some(if (HALL_FLOOR - 1.0..HALL_FLOOR).contains(&y) { B::DressedStone } else { B::Concrete });
            }
        } else if radial < 75.0 && lateral.abs() < 4.5 && (HALL_FLOOR - 2.0..HALL_FLOOR + 7.0).contains(&y) {
            return Some(B::Concrete);
        }
    }
    // Fin footings reach natural ground wherever the terrace stands proud of it.
    if at.r >= 90.0 {
        let (radial, lateral) = at.spoke(1, 4);
        if (90.0..120.0).contains(&radial) && lateral.abs() < 7.0 && y >= (terrain - 4.0).min(-6.0) {
            return Some(B::Concrete);
        }
    }
    None
}
