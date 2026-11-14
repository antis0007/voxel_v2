//! Broad branch fans and layered needle pads instead of narrow fluted spikes.
//! Every visible piece is editable wood/foliage sampled on the world lattice.
use super::{BlockId as B, IVec3, TreeForm, TreeProfile, WorldSampleConfig, foliage, hash2, distance_to_segment};
use glam::{Vec3, Vec3Swizzles};

pub(super) fn applies(c: &WorldSampleConfig, t: &TreeProfile) -> bool {
    tall_redwood(c, t) || (c.generator_version >= 57 && t.conifer && t.form == TreeForm::Classic
        && t.trunk_h as f32 * c.voxel_meters() > 4.0)
}

pub(super) fn tall_redwood(c: &WorldSampleConfig, t: &TreeProfile) -> bool {
    t.form == TreeForm::Redwood && super::old_growth::active(c) && c.world_y(t.trunk_h) >= 43.0
}

pub(super) fn profile(c: &WorldSampleConfig, t: &mut TreeProfile, age: f32) {
    t.trunk_h = c.blocks_for_span(11.0 + age * 9.0);
    t.canopy_radius = c.blocks_for_span(3.8 + age * 1.6);
    t.branch_start_y = t.ground_y + c.blocks_for_span(2.8 + age);
    t.branch_len = t.canopy_radius;
}

pub(super) fn block(c: &WorldSampleConfig, t: &TreeProfile, p: IVec3) -> B {
    let v = c.voxel_meters();
    let q = (p - IVec3::new(t.anchor_x, t.ground_y, t.anchor_z)).as_vec3() * v;
    let h = t.trunk_h as f32 * v;
    let r = t.canopy_radius as f32 * v;
    if q.y <= 0.0 || q.y > h + v || q.xz().length() > r * 1.15 + v + foliage::pad(c) { return B::Air; }
    let relief = foliage::Relief::of(c, t);
    let age = hash2(t.cell_x, t.cell_z, c.seed.wrapping_add(12011));
    let spin = hash2(t.cell_x, t.cell_z, c.seed.wrapping_add(57117)) * std::f32::consts::TAU;
    let redwood = tall_redwood(c, t);
    let tall_spruce = !redwood && super::old_growth::active(c) && h >= 22.0;
    let (stem_radius, flare, flare_height, tiers) = if redwood {
        (1.52 + age * 0.55, 1.4, 4.5, 9)
    } else if tall_spruce {
        (0.51 + age * 0.32, 0.68, 2.6, 8)
    } else {
        (0.32 + age * 0.22, 0.45, 1.8, 6)
    };
    let stem = (stem_radius + (1.0-q.y/flare_height).max(0.0)*flare) * (1.0 - 0.5*q.y/h);
    if q.y < h * 0.95 && q.xz().length() < stem.max(v * 0.56) { return B::Log; }
    let base = (t.branch_start_y-t.ground_y) as f32*v;
    let spacing = (h-base)/tiers as f32;
    let fan_variation = if redwood { 0.20 } else { 0.12 };
    let branch_radius = if redwood { 0.30 } else { 0.16 };
    // Only adjacent tiers can contain this point. Wide unequal fans leave
    // clear negative space beneath their tips, with a connected central crown.
    let nearest = ((q.y-base)/spacing).floor() as i32;
    let mut leaves = false;
    for tier in (nearest-1).max(0)..=(nearest+1).min(tiers - 1) {
        let f = tier as f32 / tiers as f32;
        let cy = base + tier as f32*spacing;
        let reach = r * if redwood { (1.0 - f * f).powf(0.74) } else { (1.0-f).powf(0.85) };
        if (q.y-cy).abs() > spacing.max(1.6) + foliage::pad(c) { continue; }
        let leader = Vec3::new(0.0,cy,0.0);
        for fan in 0..4 {
            let a = spin + fan as f32*std::f32::consts::FRAC_PI_2 + tier as f32*0.63;
            let dir = Vec3::new(a.cos(),0.0,a.sin());
            let side = Vec3::new(-dir.z,0.0,dir.x);
            let asymmetry = 0.82 + fan_variation * (a*1.7+age*5.0).sin();
            let tip = leader + dir*reach*asymmetry + Vec3::Y*0.18;
            // The branch ends inside a pad; fine worlds expose its underside.
            let (distance,u) = distance_to_segment(q,leader,tip*0.72+leader*0.28);
            if distance < (branch_radius*(1.0-u*0.5)).max(v*0.90) { return B::Log; }
            let centre = leader + dir*reach*0.48 + Vec3::Y*0.36;
            let d = q-centre;
            let radii = Vec3::new(reach*0.60+v*0.25, 0.65+spacing*0.22, reach*0.40+v*0.25);
            let pad = Vec3::new(d.dot(dir), d.y, d.dot(side)) / radii;
            leaves |= match relief {
                Some(relief) => relief.leaf(q, v, pad.length_squared(), radii.min_element(), foliage::NEEDLES),
                None => pad.length_squared() < 1.0,
            };
        }
    }
    // Dense upper leader prevents a bare log antenna while preserving a point.
    let tip_r = ((h+v-q.y)*0.35).min(r*0.32).max(v*0.56);
    let outside = q.xz().length() - tip_r;
    let tip_start = if redwood { 0.70 } else { 0.60 };
    leaves |= q.y > h * tip_start && match relief {
        Some(relief) => relief.shell(q, v, outside, tip_r, foliage::NEEDLES),
        None => outside < 0.0,
    };
    if leaves { B::PineLeaves } else { B::Air }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;
    use std::collections::{HashSet, VecDeque};
    #[test]
    fn conifer_wood_is_connected_and_fits_lod_bounds() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
            let c = WorldSampleConfig {
    planet: None, seed: 424242, generator_version: 57,
                world_type: crate::WorldType::Hills, voxel_scale: scale };
            let mut t = TreeProfile { conifer: true, cell_x: -7, cell_z: 11,
                anchor_x: 0, anchor_z: 0, ground_y: 0, trunk_h: 12,
                canopy_radius: 4, canopy_height: 1, branch_count: 6, branch_len: 4,
                branch_start_y: 3, lean_x: 0, lean_z: 0, form: TreeForm::Classic };
            profile(&c, &mut t, 0.5);
            let v = c.voxel_meters();
            let bound = crate::trees::crown_reach(&c, &t);
            let n = (bound / v).ceil() as i32 + 1;
            let top = crate::trees::crown_top(&c, &t);
            let mut wood = HashSet::new();
            let mut leaves = 0;
            for y in 1..=(top/v).ceil() as i32+1 {
                for z in -n..=n { for x in -n..=n {
                    let p = IVec3::new(x,y,z);
                    match block(&c,&t,p) {
                        B::Log => { wood.insert(p); },
                        B::PineLeaves => {
                            leaves += 1;
                            assert!((x as f32).hypot(z as f32)*v <= bound);
                            assert!(y as f32*v <= top);
                        },
                        _ => {},
                    }
                }}
            }
            assert!(leaves > 100);
            let mut reached: HashSet<_> = wood.iter().filter(|p| p.y == 1).copied().collect();
            let mut queue: VecDeque<_> = reached.iter().copied().collect();
            while let Some(p) = queue.pop_front() {
                for d in [IVec3::X,-IVec3::X,IVec3::Y,-IVec3::Y,IVec3::Z,-IVec3::Z] {
                    let q = p+d;
                    if wood.contains(&q) && reached.insert(q) { queue.push_back(q); }
                }
            }
            assert_eq!(wood.len(),reached.len(),"floating wood at {v} m: {:?}", wood.difference(&reached).collect::<Vec<_>>());
        }
    }
}


