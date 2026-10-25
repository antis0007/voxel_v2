//! Generator 45 broadleaf anatomy: bent scaffold, unequal crown lobes and
//! flared roots. All wood and foliage remain ordinary editable world voxels.
use super::{BlockId as B, IVec3, TreeForm, TreeProfile, WorldSampleConfig, distance_to_segment, hash2, hash3};
use glam::{Vec3, Vec3Swizzles};

mod v47;
mod v52;

#[cfg(test)]
mod tests;

pub(super) fn applies(cfg: &WorldSampleConfig, t: &TreeProfile) -> bool {
    cfg.generator_version >= 45 && !t.conifer && t.form == TreeForm::Classic
        && t.trunk_h as f32 * cfg.voxel_meters() >= 4.0
}

pub(super) fn profile(cfg: &WorldSampleConfig, t: &mut TreeProfile, age: f32) {
    if !applies(cfg, t) { return; }
    if cfg.generator_version >= 47 { return v47::profile(cfg, t, age); }
    let (h, r, c) = if age > 0.76 {
        (9.0 + age * 3.0, 6.0 + age * 1.2, 5.2)
    } else if age < 0.22 {
        (4.5 + age * 9.0, 2.0 + age * 5.0, 3.0)
    } else {
        (9.0 + age * 8.0, 3.4 + age * 2.0, 4.5)
    };
    t.trunk_h = cfg.blocks_for_span(h);
    t.canopy_radius = cfg.blocks_for_span(r);
    t.canopy_height = cfg.blocks_for_span(c);
    t.branch_start_y = t.ground_y + cfg.blocks_for_span(h * if age > 0.76 { 0.38 } else { 0.64 });
}

/// Exact leaf reach and top (metres) where the crown is a skeleton.
pub(super) fn bounds(cfg: &WorldSampleConfig, t: &TreeProfile) -> Option<(f32, f32)> {
    (cfg.generator_version >= 52 && applies(cfg, t)).then(|| v52::bounds(cfg, t))
}

pub(super) fn block(cfg: &WorldSampleConfig, t: &TreeProfile, p: IVec3) -> B {
    if cfg.generator_version >= 52 { return v52::block(cfg, t, p); }
    if cfg.generator_version >= 47 { return v47::block(cfg, t, p); }
    let v = cfg.voxel_meters();
    let q = (p - IVec3::new(t.anchor_x, t.ground_y, t.anchor_z)).as_vec3() * v;
    let h = t.trunk_h as f32 * v;
    let r = t.canopy_radius as f32 * v;
    let ch = t.canopy_height as f32 * v;
    if q.y <= 0.0 || q.y > h + ch || q.xz().length() > r * 1.35 { return B::Air; }
    let age = hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(12011));
    let old = age > 0.76;
    let angle = hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(12049)) * std::f32::consts::TAU;
    let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
    let bend = dir * if old { 1.2 } else { 0.5 };
    let fork = bend + Vec3::Y * h * if old { 0.46 } else { 0.67 };
    let stem = if old { 0.85 + (age - 0.76) * 1.8 } else { 0.28 + age * 0.30 };
    let (dist, u) = distance_to_segment(q, Vec3::ZERO, fork);
    let flare = (1.0 - q.y / 2.5).max(0.0) * stem * 0.6;
    if dist < (stem * (1.0 - u * 0.38) + flare).max(v * 0.56) { return B::Log; }
    let mut canopy = f32::MAX;
    for i in 0..5 {
        let k = hash3(t.cell_x, i, t.cell_z, cfg.seed.wrapping_add(45117));
        let a = angle + i as f32 * 2.399963 + (k - 0.5) * 0.35;
        let axis = Vec3::new(a.cos(), 0.0, a.sin());
        // Sparse tapered roots leave gaps for movement between older trees.
        if old && q.y < 2.5 {
            let root_tip = axis * (2.2 + k * 1.2) + Vec3::Y * v * 0.5;
            let (rd, ru) = distance_to_segment(q, Vec3::Y * 1.5, root_tip);
            if rd < (stem * (0.75 - ru * 0.55)).max(v * 0.48) { return B::Log; }
        }
        let tip = axis * r * (0.42 + k * 0.20) + Vec3::Y * (h + (k - 0.45) * ch * 0.6);
        let elbow = fork.lerp(tip, 0.52) + Vec3::Y * (0.5 + k);
        for (start, end, width) in [(fork, elbow, stem * 0.70), (elbow, tip, stem * 0.42)] {
            let (d, along) = distance_to_segment(q, start, end);
            if d < (width * (1.0 - along * 0.45)).max(v * 0.82) { return B::Log; }
        }
        let lobe = Vec3::new(r * (0.39 + k * 0.09), ch * (0.50 + k * 0.15), r * (0.34 + k * 0.10));
        canopy = canopy.min(((q - tip) / lobe).length_squared());
    }
    let grain = cfg.blocks_for_span(1.2).max(1);
    let edge = hash3(p.x.div_euclid(grain), p.y.div_euclid(grain), p.z.div_euclid(grain), cfg.seed.wrapping_add(45119));
    if canopy < 0.90 + edge * 0.16 { B::Leaves } else { B::Air }
}
