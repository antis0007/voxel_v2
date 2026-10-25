//! Generator 47 woodland: deep, joined crown masses over a low woody scaffold.
//! Dimensions stay inside the existing resident/LOD/search bounds. These are
//! editable logs and leaves, sampled through the same path as collision.
use super::{
    B, IVec3, TreeProfile, Vec3, Vec3Swizzles, WorldSampleConfig, distance_to_segment, hash2, hash3,
};

pub(super) fn profile(cfg: &WorldSampleConfig, t: &mut TreeProfile, age: f32) {
    // Retain slender groves selected by the species pass. The old woodland
    // override replaced those proportions with the same spreading age forms.
    let slender = t.canopy_radius as f32 * cfg.voxel_meters() <= 2.8
        && t.trunk_h as f32 * cfg.voxel_meters() >= 8.5;
    let (height, radius, crown) = if slender {
        (8.0 + age * 4.5, 2.4 + age * 0.9, 5.5 + age * 2.0)
    } else if age < 0.24 {
        (4.5 + age * 12.0, 2.3 + age * 6.0, 4.5 + age * 8.0)
    } else if age > 0.78 {
        (7.5 + age * 3.0, 5.4 + age * 1.2, 8.5)
    } else {
        (7.2 + age * 5.6, 3.8 + age * 2.0, 6.0 + age * 2.6)
    };
    t.trunk_h = cfg.blocks_for_span(height);
    t.canopy_radius = cfg.blocks_for_span(radius);
    t.canopy_height = cfg.blocks_for_span(crown);
    t.branch_start_y = t.ground_y + cfg.blocks_for_span(height * 0.43);
    t.branch_len = (t.canopy_radius as f32 * 0.76) as i32;
}

pub(super) fn block(cfg: &WorldSampleConfig, t: &TreeProfile, p: IVec3) -> B {
    let scale = cfg.voxel_meters();
    let q = (p - IVec3::new(t.anchor_x, t.ground_y, t.anchor_z)).as_vec3() * scale;
    let height = t.trunk_h as f32 * scale;
    let radius = t.canopy_radius as f32 * scale;
    let crown = t.canopy_height as f32 * scale;
    if q.y <= 0.0 || q.y > height + crown || q.xz().length_squared() > (radius * 1.35).powi(2) {
        return B::Air;
    }
    let age = hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(12011));
    let rotation = hash2(t.cell_x, t.cell_z, cfg.seed.wrapping_add(12049)) * std::f32::consts::TAU;
    let axis = Vec3::new(rotation.cos(), 0.0, rotation.sin());
    let bend = axis * (0.30 + age * 0.65);
    let fork = bend + Vec3::Y * height * 0.43;
    let stem = (0.28 + age * 0.78) * (radius / 5.0).clamp(0.65, 1.15);
    let (distance, along) = distance_to_segment(q, Vec3::ZERO, fork);
    let flare = (1.0 - q.y / 2.4).max(0.0) * stem * 0.7;
    if distance < (stem * (1.0 - along * 0.28) + flare).max(scale * 0.78) {
        return B::Log;
    }

    // A connected central mass keeps the crown from reading as five thin pads.
    // Unequal side lobes extend both above and below this mass; the underside
    // remains high enough for a route beneath mature woodland.
    let center = bend * 0.8 + Vec3::Y * (height - crown * 0.10);
    let mut field =
        ((q - center) / Vec3::new(radius * 0.59, crown * 0.60, radius * 0.53)).length_squared();
    for limb in 0..5 {
        let variation = hash3(t.cell_x, limb, t.cell_z, cfg.seed.wrapping_add(47117));
        let angle = rotation + limb as f32 * 2.399963 + (variation - 0.5) * 0.65;
        let direction = Vec3::new(angle.cos(), 0.0, angle.sin());
        if age > 0.40 && q.y < 2.4 {
            let root_tip = direction * (1.2 + age + variation) + Vec3::Y * scale * 0.65;
            let (distance, along) = distance_to_segment(q, Vec3::Y * 1.5, root_tip);
            if distance < (stem * (0.70 - along * 0.46)).max(scale * 0.78) {
                return B::Log;
            }
        }
        let tip = bend * 0.5
            + direction * radius * (0.30 + variation * 0.31)
            + Vec3::Y * (height + crown * (variation - 0.55) * 0.46);
        let elbow = fork.lerp(tip, 0.53) + Vec3::Y * (0.25 + variation * 0.60);
        for (start, end, width) in [(fork, elbow, stem * 0.72), (elbow, tip, stem * 0.44)] {
            let (distance, along) = distance_to_segment(q, start, end);
            if distance < (width * (1.0 - along * 0.42)).max(scale * 0.82) {
                return B::Log;
            }
        }
        let lobe = Vec3::new(
            radius * (0.44 + variation * 0.12),
            crown * (0.47 + variation * 0.15),
            radius * (0.40 + variation * 0.13),
        );
        field = field.min(((q - tip) / lobe).length_squared());
    }
    // Edge changes operate in metre-sized groups, leaving the interior solid.
    let grain = cfg.blocks_for_span(1.6);
    let edge = hash3(
        p.x.div_euclid(grain),
        p.y.div_euclid(grain),
        p.z.div_euclid(grain),
        cfg.seed.wrapping_add(47119),
    );
    if field < 0.94 + edge * 0.12 {
        B::Leaves
    } else {
        B::Air
    }
}
