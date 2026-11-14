//! Generator 58: a single-storey smith's cottage. The plan owns solid AND void
//! space, so legacy furnishings cannot reappear through new openings.
use super::*;

#[derive(Clone, Copy)]
struct Opening { centre: f32, half_width: f32, bottom: f32, top: f32 }

#[derive(Clone, Copy)]
struct Gable { plate: f32, pitch: f32, eave: f32 }

/// Fixed benchmark grammar; dimensions are metres, not voxel counts.
struct HallPlan {
    half_width: f32,
    half_depth: f32,
    roof: Gable,
    door: Opening,
    windows: [Opening; 2],
}

const HALL: HallPlan = HallPlan {
    half_width: 4.0, half_depth: 4.0,
    roof: Gable { plate: 4.0, pitch: 0.5, eave: 0.35 },
    door: Opening { centre: 0.0, half_width: 1.1, bottom: 0.0, top: 3.0 },
    windows: [
        Opening { centre: -1.8, half_width: 0.65, bottom: 1.5, top: 2.7 },
        Opening { centre: 1.8, half_width: 0.65, bottom: 1.5, top: 2.7 },
    ],
};

impl Opening {
    fn contains(self, along: f32, y: f32, v: f32) -> bool {
        (along - self.centre).abs() < self.half_width.max(v * 0.7)
            && y >= self.bottom && y < self.top.max(self.bottom + v)
    }
}

impl HallPlan {
    fn roof_height(&self, x: f32) -> f32 {
        self.roof.plate + (self.half_width - x.abs()) * self.roof.pitch
    }

    fn sample(&self, x: f32, y: f32, z: f32, v: f32) -> Option<B> {
        let (hw, hd) = (self.half_width, self.half_depth);
        let t = v.max(0.25);
        let eave = self.roof.eave;
        if x.abs() > hw + eave || z.abs() > hd + eave || !(-6.0..10.0).contains(&y) {
            return None;
        }
        // The roof slab intersects each rotated grid column over the full
        // slope interval of a cell. Adjacent courses overlap in height instead
        // of merely touching at an edge (the old fixed-thickness sampler).
        let roof = self.roof_height(x);
        let under = roof - self.roof.pitch * v;
        if y >= under && y < roof + t {
            // The overlap is concealed behind the gable; the visible verge is
            // one course, rather than a double-thickness red stair at the ends.
            if y < roof && z.abs() >= hd - t {
                return Some(if z.abs() <= hd && x.abs() <= hw { B::Limestone } else { B::Air });
            }
            return Some(B::RoofTiles);
        }
        let inside = x.abs() <= hw && z.abs() <= hd;
        if !inside {
            // A short threshold joins the existing graded door spur. No slab
            // across the whole facade and no second roof hiding the gable.
            if z < -hd && x.abs() < 1.5 && y < v {
                return Some(if y < -v { B::StoneBricks } else { B::Cobblestone });
            }
            return if y >= v { Some(B::Air) } else { None };
        }
        if y < v { return Some(if y < 0.0 { B::StoneBricks } else { B::Planks }); }
        if y >= under { return Some(B::Air); }
        let front = z <= -hd + t;
        let back = z >= hd - t;
        let side = x.abs() >= hw - t;
        // The walls terminate at the eave plate; only gable ends reach ridge.
        if front || back || side && y < self.roof.plate {
            if front && self.door.contains(x, y, v) { return Some(B::Air); }
            let along = if front || back { x } else { z };
            if !front && self.windows.iter().any(|w| w.contains(along, y, v)) {
                return Some(B::Air);
            }
            let sill = !front && self.windows.iter().any(|w|
                (along-w.centre).abs() < w.half_width.max(v*0.7)+0.25
                && y >= w.bottom-t && y < w.bottom);
            let corner = x.abs() >= hw - t && z.abs() >= hd - t;
            let jamb = front && (x.abs() - self.door.half_width).abs() < t * 0.6
                && y < self.door.top + t;
            let lintel = front && x.abs() < self.door.half_width + t
                && y >= self.door.top && y < self.door.top + t;
            let plate = y >= self.roof.plate - t && y < self.roof.plate;
            // Vertical members only use Log (its grain axis is vertical).
            // Horizontal plates use boards until oriented timber is supported.
            return Some(if y < 1.0 { B::StoneBricks }
                else if corner || jamb { B::Log }
                else if lintel || plate || sill { B::Planks } else { B::Limestone });
        }
        // Furnishings stay in two perimeter bays, outside a 2.2 m centre aisle.
        if x < -2.4 && z > -0.5 && z < 1.0 && y < v + 1.0 { return Some(B::Workbench); }
        if x > 2.4 && z > 0.0 && z < 1.5 && y < v + 1.0 { return Some(B::Smelter); }
        if x < -2.4 && z > 2.0 && y < v + 1.0 { return Some(B::SalvageCrate); }
        // Supported, actual torch blocks illuminate the entry and work bays.
        if x.abs() > hw - 2.0*t && z > hd - 2.0*t {
            if y < 2.0 { return Some(B::StoneBricks); }
            if y < 2.0 + v { return Some(B::Torch); }
        }
        Some(B::Air)
    }
}

pub(super) fn sample(x: f32, y: f32, z: f32, v: f32) -> Option<B> {
    HALL.sample(x,y,z,v)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circulation_openings_and_roof_columns_at_three_scales() {
        for v in [1.0_f32, 0.5, 0.25] {
            for iz in 0..((8.0-v.max(0.25))/v) as i32 {
                for iy in 0..(1.8/v).ceil() as i32 {
                    assert_eq!(sample(0.0,v+iy as f32*v,-4.0+iz as f32*v,v),Some(B::Air));
                }
            }
            for w in HALL.windows {
                assert_eq!(sample(4.0,w.bottom+0.2,w.centre,v),Some(B::Air));
            }
            // Check overlapping roof intervals at rotated world-grid phases.
            for angle in [0.0_f32,0.3,0.785,1.2] {
                let (s,c) = angle.sin_cos();
                for ix in -16..16 { for iz in -16..16 {
                    let x = (ix as f32+0.37)*v*c-(iz as f32+0.19)*v*s;
                    let z = (ix as f32+0.37)*v*s+(iz as f32+0.19)*v*c;
                    if x.abs()>4.0 || z.abs()>=4.0-v.max(0.25) { continue; }
                    let ys: Vec<_> = (0..40).filter(|iy| sample(x,*iy as f32*v,z,v)==Some(B::RoofTiles)).collect();
                    assert!(!ys.is_empty(),"missing roof v={v} x={x} z={z}");
                    for (dx,dz) in [(v*c,v*s),(-v*s,v*c)] {
                        if (x+dx).abs() <= 4.0 && (z+dz).abs() < 4.0-v.max(0.25) {
                            assert!(ys.iter().any(|iy| sample(x+dx,*iy as f32*v,z+dz,v)==Some(B::RoofTiles)),"disconnected roof v={v} x={x} z={z}");
                        }
                    }
                }}
            }
            assert_eq!(sample(6.1,2.0,0.0,v),None);
            assert_eq!(sample(0.0,-2.0,0.0,v),Some(B::StoneBricks));
        }
    }

    #[test]
    fn bounded_deterministic_volume_and_sampler_cost() {
        for v in [1.0_f32,0.5,0.25] {
            let start = std::time::Instant::now();
            let mut counts = [0_usize; 2];
            for count in &mut counts {
                for ix in (-6.0/v) as i32..(6.0/v) as i32 {
                    for iz in (-6.0/v) as i32..(6.0/v) as i32 {
                        for iy in (-6.0/v) as i32..(10.0/v) as i32 {
                            let p = (ix as f32*v,iy as f32*v,iz as f32*v);
                            let b = sample(p.0,p.1,p.2,v);
                            assert_eq!(b,sample(p.0,p.1,p.2,v));
                            if b.is_some_and(|b| b!=B::Air) { *count += 1; }
                        }
                    }
                }
            }
            assert_eq!(counts[0],counts[1]);
            assert!(counts[0] as f32*v.powi(3)<1200.0);
            println!("hall58 voxel_m={v} occupied={} solid_volume_m3={:.2} four_sweeps_ms={:.3}",
                counts[0],counts[0] as f32*v.powi(3),start.elapsed().as_secs_f64()*1000.0);
        }
    }
}
