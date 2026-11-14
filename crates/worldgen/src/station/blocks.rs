//! Station cell materials: shell, doorways, lamps and room furnishing.
use super::{Kind, Marker, Plan, Role, Space, roll};
use crate::BlockId as B;
use glam::IVec3;

/// Room-local coordinates: `u` along the spine, `d` away from the corridor, both from 0.
struct Local {
    u: i32,
    d: i32,
    y: i32,
    width: i32,
    depth: i32,
}

fn local(space: &Space, p: IVec3) -> Local {
    let d = if space.side >= 0 { p.z - space.min.z } else { space.max.z - p.z };
    Local { u: p.x - space.min.x, d, y: p.y - space.min.y, width: space.max.x - space.min.x + 1, depth: space.max.z - space.min.z + 1 }
}

/// Doorway columns, which furniture keeps clear all the way to the back wall.
fn in_lane(l: &Local) -> bool {
    let door = l.width / 2 - 1;
    (door..=door + 1).contains(&l.u)
}

/// Station material at `p`, or `None` outside the station.
pub fn block(plan: &Plan, p: IVec3) -> Option<B> {
    if !(p.cmpge(plan.min).all() && p.cmple(plan.max).all()) {
        return None;
    }
    if let Some(space) = plan.space_at(p) {
        return Some(furnish(plan.seed, space, p).unwrap_or(B::Air));
    }
    let mut shells = plan.spaces.iter().filter(|s| s.shell(p)).peekable();
    let first = *shells.peek()?;
    if plan.opening(p) {
        return Some(B::Air);
    }
    let damaged = plan.spaces.iter().any(|s| s.kind == Kind::Damaged && s.shell(p));
    let trim = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::Z, IVec3::NEG_Z].iter().any(|&o| plan.opening(p + o) && !plan.opening(p));
    if let Some(over) = plan.space_at(p + IVec3::Y) {
        return Some(match over.kind {
            Kind::Damaged => B::RustedSteel,
            Kind::Freight if edge(over, p) => B::HazardPaint,
            Kind::Freight => B::Concrete,
            _ => B::SteelPlate,
        });
    }
    if let Some(under) = plan.space_at(p - IVec3::Y) {
        return Some(if lamp(under, p - IVec3::Y) { B::GlowCap } else if damaged { B::RustedSteel } else { B::SteelBlock });
    }
    if trim {
        return Some(if damaged { B::HazardPaint } else { B::CopperBlock });
    }
    if window(first, p) {
        return Some(B::WindowGlass);
    }
    Some(if damaged { B::RustedSteel } else { B::SteelBlock })
}

/// `block` with the damaged section's doorway closed by a steel bulkhead, so
/// the breach cannot vent the rest of the station.
pub fn sealed_block(plan: &Plan, p: IVec3) -> Option<B> {
    if plan.in_bulkhead(p) {
        return Some(B::SteelBlock);
    }
    block(plan, p)
}

fn edge(space: &Space, floor: IVec3) -> bool {
    let p = floor + IVec3::Y;
    p.x == space.min.x || p.x == space.max.x || p.z == space.min.z || p.z == space.max.z
}

/// Ceiling lamp over clear cell `p`: every room and the airlock get light, the damaged section none.
fn lamp(space: &Space, p: IVec3) -> bool {
    let centre_z = (space.min.z + space.max.z).div_euclid(2);
    match space.kind {
        Kind::Damaged => false,
        Kind::Corridor => p.z == 0 && (p.x - space.min.x).rem_euclid(6) == 2,
        _ => {
            let width = space.max.x - space.min.x + 1;
            let lamps = (width / 6).max(1);
            p.z == centre_z && (0..lamps).any(|k| p.x == space.min.x + width * (2 * k + 1) / (2 * lamps))
        }
    }
}

/// Outer-wall viewport of crew rooms, galleys and the trade concourse.
fn window(space: &Space, p: IVec3) -> bool {
    if !matches!(space.kind, Kind::Crew | Kind::Galley | Kind::Trade) || p.y != 1 {
        return false;
    }
    let outer = if space.side > 0 { space.max.z + 1 } else { space.min.z - 1 };
    let mid = (space.min.x + space.max.x) / 2;
    p.z == outer && (p.x - mid).abs() <= 1
}

fn furnish(seed: u32, space: &Space, p: IVec3) -> Option<B> {
    let l = local(space, p);
    let back = l.d == l.depth - 1;
    match space.kind {
        Kind::Corridor => None,
        Kind::Airlock => {
            let bay = p.z == space.min.z;
            (bay && l.y == 0).then(|| match p.x - space.min.x {
                0 => B::GasTank,
                2 | 3 => B::ChestSteel,
                5 => B::GasTank,
                _ => B::Air,
            }).filter(|&b| b != B::Air)
        }
        Kind::Trade => {
            if l.y > 1 || in_lane(&l) && l.d < l.depth - 3 {
                return None;
            }
            if l.d == l.depth - 3 && l.y == 0 && l.u > 0 && l.u < l.width - 1 {
                return Some(B::SteelPlate);
            }
            if back {
                return Some(if l.u % 3 == 0 { B::StorageCrate } else if l.y == 0 { B::Bookcase } else { B::ChestCopper });
            }
            None
        }
        Kind::Crew => {
            if l.y != 0 {
                return None;
            }
            let (cu, cd) = (l.width / 2, l.depth / 2 + 1);
            if (l.u == cu || l.u == cu - 1) && l.d == cd && !in_lane(&l) {
                return Some(B::PlankSlab);
            }
            if back && l.u == 0 {
                return Some(B::Workbench);
            }
            if back && l.u == l.width - 1 {
                return Some(B::Lectern);
            }
            if (l.d == cd) && (l.u == cu - 2 || l.u == cu + 1) && !in_lane(&l) {
                return Some(B::PlankStairs);
            }
            None
        }
        Kind::Quarters => {
            if l.y != 0 || l.d < l.depth - 2 || in_lane(&l) {
                return None;
            }
            match (l.u % 3, back) {
                (0, _) => Some(B::Bed),
                (1, true) => Some(B::ChestSteel),
                _ => None,
            }
        }
        Kind::Freight => {
            if in_lane(&l) || l.d < 2 || l.u == 0 || l.u == l.width - 1 {
                return None;
            }
            let row = l.d == 2 || l.d == 3 || l.d >= l.depth - 2;
            let column = l.u % 4 != 0;
            let tall = 1 + (roll(seed, (p.x * 31 + p.z) as u32) % 3) as i32;
            if !(row && column) || l.y >= tall {
                return None;
            }
            Some(if l.y == 0 && l.u == 1 && back { B::ChestSteel } else if (l.u + l.d) % 3 == 0 { B::SalvageCrate } else { B::StorageCrate })
        }
        Kind::Galley => {
            if back && !in_lane(&l) {
                return match l.y {
                    0 => Some(if l.u == 0 { B::WaterTank } else { B::Dirt }),
                    1 if l.u > 0 => Some([B::WheatCrop, B::CarrotCrop, B::BeetCrop][(l.u % 3) as usize]),
                    _ => None,
                };
            }
            (l.y == 0 && l.d == l.depth / 2 && (l.u == 0 || l.u == l.width - 1)).then_some(B::PlankSlab)
        }
        Kind::Engineering => {
            if !back || l.y > 1 || in_lane(&l) {
                return None;
            }
            Some(match (l.u % 4, l.y) {
                (0, _) => B::GasTank,
                (1, 0) => B::BatteryBank,
                (2, 0) => B::Transformer,
                (3, 0) => B::Workbench,
                _ => return None,
            })
        }
        Kind::Damaged => {
            if in_lane(&l) {
                return None;
            }
            if back && l.y == 0 && l.u == 0 {
                return Some(B::Chest);
            }
            if back && l.y == 0 && l.u == l.width - 1 {
                return Some(B::SalvageCrate);
            }
            // Loot in the back corners keeps a standing cell beside it.
            if l.d >= l.depth - 2 && (l.u <= 1 || l.u >= l.width - 2) {
                return None;
            }
            let h = roll(seed, (p.x * 73 + p.z * 151 + p.y * 7) as u32) % 100;
            match l.y {
                0 if h < 22 => Some(if h.is_multiple_of(2) { B::RustedSteel } else { B::Gravel }),
                1 if h < 6 => Some(B::RustedSteel),
                _ => None,
            }
        }
    }
}

/// Inhabitants of one room, on floor cells the furnishing leaves open.
pub(super) fn markers(space: &Space) -> Vec<Marker> {
    let at = |u: i32, d: i32| {
        let z = if space.side >= 0 { space.min.z + d } else { space.max.z - d };
        IVec3::new(space.min.x + u, 0, z)
    };
    let width = space.max.x - space.min.x + 1;
    let depth = space.max.z - space.min.z + 1;
    let lane = width / 2 - 1;
    match space.kind {
        Kind::Trade => vec![Marker { role: Role::Trader, at: at(lane, depth - 2) }],
        Kind::Crew | Kind::Galley | Kind::Engineering => vec![Marker { role: Role::Resident, at: at(lane, 1) }],
        Kind::Quarters => vec![Marker { role: Role::Resident, at: at(lane + 1, depth - 3) }],
        Kind::Freight => vec![Marker { role: Role::Guard, at: at(lane, 2) }],
        Kind::Damaged => vec![Marker { role: Role::Hostile, at: at(lane, depth - 2) }, Marker { role: Role::Hostile, at: at(lane + 1, depth - 3) }],
        Kind::Airlock | Kind::Corridor => Vec::new(),
    }
}
