//! Homogeneous buried soil may share a metre-sized reservoir. Any nearby edit
//! refines it BEFORE geometry changes. Refinement distributes every integer
//! stock/remainder once; collision and voxel occupancy are never coarsened.
use super::*;

impl Patch {
    pub fn scale(&self, world: crate::fluid::VoxelScalePreset) -> crate::fluid::VoxelScalePreset {
        if self.extent > 1 { crate::fluid::VoxelScalePreset::OneMeter } else { world }
    }
}

impl State {
    pub fn owner(&self, p: IVec3) -> Key {
        let n = self.cells_per_metre.max(1);
        let a = (p.x.div_euclid(n) * n, p.y.div_euclid(n) * n, p.z.div_euclid(n) * n);
        if self.patches.get(&a).is_some_and(|patch| patch.extent > 1) { a } else { key(p) }
    }

    pub fn can_refine(&self, positions: &[IVec3]) -> bool {
        let owners: BTreeSet<_> = positions.iter().map(|&p| self.owner(p)).collect();
        let additional: usize = owners
            .iter()
            .filter_map(|p| self.patches.get(p))
            .map(|patch| (patch.extent as usize).pow(3) - 1)
            .sum();
        self.patches.len() + additional <= PATCH_CEILING
            && self.scheduled.len() + additional + positions.len() <= MAX_EVENTS
    }

    pub fn refine(&mut self, p: IVec3) {
        let owner = self.owner(p);
        let Some(patch) = self.patches.get(&owner).filter(|p| p.extent > 1).cloned() else {
            return;
        };
        let n = i32::from(patch.extent);
        let count = (n * n * n) as u64;
        self.patches.remove(&owner);
        if let Some(due) = self.scheduled.remove(&owner) {
            self.due.remove(&(due, owner));
        }
        for y in 0..n {
            for z in 0..n {
                for x in 0..n {
                    let index = ((y * n + z) * n + x) as u64;
                    let part = |v: u64| v / count + u64::from(index < v % count);
                    let s = patch.soil;
                    let soil = Soil {
                        water: part(s.water),
                        infiltration_credit: part(s.infiltration_credit),
                        flux_carry: part(s.flux_carry),
                        erosion_work: part(s.erosion_work),
                        nutrient: part(s.nutrient),
                        biomass: part(s.biomass),
                        plant_nutrient: part(s.plant_nutrient),
                        litter: part(s.litter),
                        litter_nutrient: part(s.litter_nutrient),
                        growth_credit: part(s.growth_credit),
                    };
                    let p = position(owner) + IVec3::new(x, y, z);
                    self.patches.insert(
                        key(p),
                        Patch { soil, extent: 1, revision: patch.revision + 1, ..patch.clone() },
                    );
                    self.wake(p);
                }
            }
        }
    }
}

/// Exact face contacts, in stable order. Interior fine cells never run a rule.
pub fn faces(anchor: IVec3, extent: u8) -> Vec<(IVec3, IVec3)> {
    let n = i32::from(extent);
    let mut faces = Vec::with_capacity((6 * n * n) as usize);
    for direction in [IVec3::NEG_Y, IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
        for v in 0..n {
            for u in 0..n {
                let offset = if direction.y != 0 {
                    IVec3::new(u, if direction.y > 0 { n - 1 } else { 0 }, v)
                } else if direction.x != 0 {
                    IVec3::new(if direction.x > 0 { n - 1 } else { 0 }, u, v)
                } else {
                    IVec3::new(u, v, if direction.z > 0 { n - 1 } else { 0 })
                };
                faces.push((anchor + offset + direction, direction));
            }
        }
    }
    faces
}
