//! Partial blocks drawn and collided as boxes: stairs, slabs, fences, signs, chest, lectern.
use crate::BlockId as B;
use glam::{IVec3, Vec3};

/// Facing per orientation index; authored geometry faces +Z.
pub const FACINGS: [IVec3; 4] = [IVec3::Z, IVec3::NEG_Z, IVec3::X, IVec3::NEG_X];

const FAMILIES: [[B; 4]; 5] = [
    [B::SignPost, B::SignPostSouth, B::SignPostWest, B::SignPostEast],
    [B::WallSign, B::WallSignSouth, B::WallSignWest, B::WallSignEast],
    [B::PlankStairs, B::PlankStairsSouth, B::PlankStairsWest, B::PlankStairsEast],
    [B::BrickStairs, B::BrickStairsSouth, B::BrickStairsWest, B::BrickStairsEast],
    [B::CobbleStairs, B::CobbleStairsSouth, B::CobbleStairsWest, B::CobbleStairsEast],
];
const SLABS: [B; 3] = [B::PlankSlab, B::BrickSlab, B::CobbleSlab];
const FENCES: [B; 3] = [B::PlankFence, B::BrickFence, B::CobbleFence];

fn family(b: B) -> Option<(usize, usize)> {
    FAMILIES
        .iter()
        .enumerate()
        .find_map(|(f, members)| members.iter().position(|&m| m == b).map(|i| (f, i)))
}

pub fn is_shaped(b: B) -> bool {
    family(b).is_some() || SLABS.contains(&b) || FENCES.contains(&b) || matches!(b, B::Chest | B::Lectern)
}

pub fn is_sign(b: B) -> bool {
    family(b).is_some_and(|(f, _)| f < 2)
}

pub fn is_stairs(b: B) -> bool {
    family(b).is_some_and(|(f, _)| f >= 2)
}

pub fn is_fence(b: B) -> bool {
    FENCES.contains(&b)
}

/// The item any orientation of `b` drops; wall signs drop the sign item.
pub fn canonical(b: B) -> Option<B> {
    match family(b)? {
        (0 | 1, _) => Some(B::SignPost),
        (f, _) => Some(FAMILIES[f][0]),
    }
}

/// Direction the authored +Z side of `b` points to.
pub fn facing(b: B) -> Option<IVec3> {
    family(b).map(|(_, i)| FACINGS[i])
}

/// The block whose looks borrow `b`'s texture family.
pub fn base(b: B) -> Option<B> {
    if !is_shaped(b) {
        return None;
    }
    let material = match family(b) {
        Some((f, _)) => f.saturating_sub(2),
        None => SLABS.iter().chain(FENCES.iter()).position(|&m| m == b).map_or(0, |i| i % 3),
    };
    Some([B::Planks, B::StoneBricks, B::Cobblestone][material])
}

/// Text capacity of a block holding written lines: (lines, characters per line).
pub fn text_limits(b: B) -> Option<(usize, usize)> {
    if is_sign(b) {
        Some((4, 16))
    } else if b == B::Lectern {
        Some((12, 24))
    } else {
        None
    }
}

/// The block to place for `selected` on a face with outward `normal`, with the
/// placer looking along the horizontal axis `forward`.
pub fn placed(selected: B, normal: IVec3, forward: IVec3) -> Option<B> {
    let Some((f, _)) = family(selected) else { return Some(selected) };
    let pick = |f: usize, dir: IVec3| FACINGS.iter().position(|&d| d == dir).map(|i| FAMILIES[f][i]);
    if f < 2 && normal.y == 0 {
        // A wall sign rests on the face it was placed against.
        pick(1, -normal)
    } else if f < 2 {
        pick(0, forward)
    } else {
        pick(f, forward)
    }
}

/// Dominant horizontal axis of a look direction.
pub fn horizontal_axis(dir: Vec3) -> IVec3 {
    if dir.x.abs() > dir.z.abs() {
        IVec3::new(dir.x.signum() as i32, 0, 0)
    } else {
        IVec3::new(0, 0, if dir.z < 0.0 { -1 } else { 1 })
    }
}

/// Bit i set when a fence at `p` joins its neighbour along `FACINGS[i]`.
pub fn links(b: B, p: IVec3, get: impl Fn(IVec3) -> B) -> u8 {
    if !is_fence(b) {
        return 0;
    }
    FACINGS.iter().enumerate().fold(0, |m, (i, &d)| {
        let n = get(p + d);
        if is_fence(n) || n.is_opaque() { m | 1 << i } else { m }
    })
}

/// World-voxel boxes for `b` at `p`; `links` only shapes fences.
pub fn boxes(b: B, p: IVec3, links: u8) -> Vec<(Vec3, Vec3)> {
    let v = Vec3::new;
    let mut authored = match family(b) {
        Some((0, _)) => vec![
            (v(0.4375, 0.0, 0.4375), v(0.5625, 0.5, 0.5625)),
            (v(0.0625, 0.5, 0.4375), v(0.9375, 1.0, 0.5625)),
        ],
        Some((1, _)) => vec![(v(0.0625, 0.25, 0.875), v(0.9375, 0.75, 1.0))],
        Some(_) => vec![(Vec3::ZERO, v(1.0, 0.5, 1.0)), (v(0.0, 0.5, 0.5), Vec3::ONE)],
        None if SLABS.contains(&b) => vec![(Vec3::ZERO, v(1.0, 0.5, 1.0))],
        None if b == B::Chest => vec![(v(0.0625, 0.0, 0.0625), v(0.9375, 0.875, 0.9375))],
        None if b == B::Lectern => vec![
            (v(0.25, 0.0, 0.25), v(0.75, 0.125, 0.75)),
            (v(0.375, 0.125, 0.375), v(0.625, 0.75, 0.625)),
            (v(0.125, 0.75, 0.125), v(0.875, 0.875, 0.875)),
        ],
        None if is_fence(b) => vec![(v(0.375, 0.0, 0.375), v(0.625, 1.0, 0.625))],
        None => return Vec::new(),
    };
    let facing = facing(b).unwrap_or(IVec3::Z);
    let mut out: Vec<(Vec3, Vec3)> = authored
        .drain(..)
        .map(|(lo, hi)| {
            let (a, c) = (crate::thin::turn(lo, facing), crate::thin::turn(hi, facing));
            (a.min(c), a.max(c))
        })
        .collect();
    if is_fence(b) {
        for (i, &d) in FACINGS.iter().enumerate() {
            if links & (1 << i) == 0 {
                continue;
            }
            for y in [0.375, 0.75] {
                let (a, c) = (
                    crate::thin::turn(v(0.4375, y, 0.625), d),
                    crate::thin::turn(v(0.5625, y + 0.1875, 1.0), d),
                );
                out.push((a.min(c), a.max(c)));
            }
        }
    }
    let base = p.as_vec3();
    out.into_iter().map(|(lo, hi)| (base + lo, base + hi)).collect()
}

/// Written face of a sign: centre, outward normal and rightward axis, in world voxels.
pub fn text_face(b: B, p: IVec3) -> Option<(Vec3, Vec3, Vec3)> {
    let (f, _) = family(b).filter(|&(f, _)| f < 2)?;
    let facing = facing(b)?;
    let authored = if f == 0 { Vec3::new(0.5, 0.75, 0.43) } else { Vec3::new(0.5, 0.5, 0.87) };
    let centre = crate::thin::turn(authored, facing);
    let right = crate::thin::turn(authored - Vec3::X, facing) - centre;
    Some((p.as_vec3() + centre, -facing.as_vec3(), right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stairs_climb_away_from_the_placer() {
        for forward in FACINGS {
            let b = placed(B::PlankStairs, IVec3::Y, forward).expect("stairs");
            assert_eq!(facing(b), Some(forward));
            assert_eq!(canonical(b), Some(B::PlankStairs));
            let boxes = boxes(b, IVec3::ZERO, 0);
            // The upper step sits on the far half along `forward`.
            let (lo, hi) = boxes[1];
            let mid = (lo + hi) * 0.5 - Vec3::splat(0.5);
            assert!(mid.dot(forward.as_vec3()) > 0.2, "{b:?} {mid:?}");
            for (lo, hi) in boxes {
                assert!(lo.cmpge(Vec3::ZERO).all() && hi.cmple(Vec3::ONE).all());
            }
        }
    }

    #[test]
    fn signs_hang_on_walls_and_stand_on_floors() {
        assert_eq!(placed(B::SignPost, IVec3::X, IVec3::Z), Some(B::WallSignEast));
        assert_eq!(placed(B::SignPost, IVec3::Y, IVec3::NEG_Z), Some(B::SignPostSouth));
        assert_eq!(canonical(B::WallSignWest), Some(B::SignPost));
        for b in FAMILIES[0].iter().chain(FAMILIES[1].iter()).copied() {
            let (centre, normal, right) = text_face(b, IVec3::ZERO).expect("face");
            assert!(centre.cmpge(Vec3::ZERO).all() && centre.cmple(Vec3::ONE).all());
            assert!((normal.dot(right)).abs() < 1e-5 && (right.length() - 1.0).abs() < 1e-5);
            assert_eq!(normal, -facing(b).expect("facing").as_vec3());
        }
        assert_eq!(text_limits(B::WallSign), Some((4, 16)));
        assert_eq!(text_limits(B::Lectern), Some((12, 24)));
        assert_eq!(text_limits(B::Chest), None);
    }

    #[test]
    fn fences_join_fences_and_walls() {
        let get = |q: IVec3| match q {
            q if q == IVec3::X => B::PlankFence,
            q if q == IVec3::NEG_Z => B::Stone,
            _ => B::Air,
        };
        let links = links(B::PlankFence, IVec3::ZERO, get);
        assert_eq!(links, 0b0110);
        assert_eq!(boxes(B::PlankFence, IVec3::ZERO, links).len(), 5);
        assert_eq!(base(B::BrickFence), Some(B::StoneBricks));
        assert_eq!(base(B::Tnt), None);
    }
}
