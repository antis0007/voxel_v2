//! Generator 52 woodland: the generator 47 proportions grown as a branching
//! skeleton. A flared, leaning bole forks low into 2-4 tapering limbs, each
//! ending in clumped lobes with open sky between them.
use super::{B, IVec3, TreeProfile, Vec3Swizzles, WorldSampleConfig, hash2};
use crate::trees::foliage;
use crate::trees::skeleton::{self, Design, Hit, Skeleton};

fn design(cfg: &WorldSampleConfig, t: &TreeProfile) -> Design {
    let scale = cfg.voxel_meters();
    let height = t.trunk_h as f32 * scale;
    let radius = t.canopy_radius as f32 * scale;
    let crown = t.canopy_height as f32 * scale;
    let h = |salt: u32| hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(salt));
    let age = crate::trees::stand_age(cfg, t);
    let slender = height > radius * 3.2;
    let base = Design {
        top: height + crown * 0.55,
        reach: radius * 1.15,
        fork: height * 0.40,
        trunk_r: (0.22 + age * 0.55) * (radius / 5.0).clamp(0.65, 1.15),
        flare: 0.50 + age * 0.30,
        roots: if age > 0.45 { 3 + (h(52_061) * 3.0) as u32 } else { 0 },
        limbs: 2 + (h(52_063) * 3.0) as u32,
        splits: 2,
        lean: 0.04 + h(52_067) * 0.16,
        rise: (0.55, 0.95),
        droop: 0.25,
        lobe: 0.36,
        low_limbs: 0.3,
    };
    if slender {
        Design { fork: height * 0.5, limbs: 2 + (h(52_063) * 2.0) as u32, rise: (0.95, 1.25), lobe: 0.44, low_limbs: 0.0, roots: 0, ..base }
    } else {
        base
    }
}

fn build(cfg: &WorldSampleConfig, t: &TreeProfile) -> Skeleton {
    let scale = cfg.voxel_meters();
    let top = (t.trunk_h + t.canopy_height) as f32 * scale;
    let reach = t.canopy_radius as f32 * scale * 1.35;
    if foliage::enabled(cfg) {
        return skeleton::build_clumped(cfg, t, &design(cfg, t), reach, top);
    }
    skeleton::build(cfg, t, &design(cfg, t), reach, top)
}

pub(super) fn block(cfg: &WorldSampleConfig, t: &TreeProfile, p: IVec3) -> B {
    let scale = cfg.voxel_meters();
    let q = (p - IVec3::new(t.anchor_x, t.ground_y, t.anchor_z)).as_vec3() * scale;
    let outset = if foliage::enabled(cfg) { foliage::OUTSET } else { 0.0 };
    let reach = t.canopy_radius as f32 * scale * 1.35 + scale + outset;
    let top = (t.trunk_h + t.canopy_height) as f32 * scale + scale + outset;
    if q.y <= 0.0 || q.y > top || q.xz().length_squared() > reach * reach {
        return B::Air;
    }
    let edge = skeleton::edge(cfg, p, 52_121);
    skeleton::with(cfg, t, || build(cfg, t), |s| match s.sample(q, scale, edge) {
        Hit::Wood => B::Log,
        Hit::Leaf => B::Leaves,
        Hit::Air => B::Air,
    })
}

/// Leaf reach and top in metres, from the same skeleton `block` samples.
pub(super) fn bounds(cfg: &WorldSampleConfig, t: &TreeProfile) -> (f32, f32) {
    let s = build(cfg, t);
    let scale = cfg.voxel_meters();
    (s.reach() + scale, s.top() + 0.5 * scale)
}
