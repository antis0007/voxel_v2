//! Reduced mantle-driven continental cycle. Material remains in the conservative
//! transport solver; changing Euler velocities never teleports or paints land.
use super::*;

pub(super) struct Assembly {
    sink: DVec3,
    dispersing: bool,
    residence: f64,
    heat: f64,
}

impl Assembly {
    pub fn new(seed: u32) -> Self {
        let a = f64::from(crate::hash_u32(seed ^ 0x287a)) / f64::from(u32::MAX);
        Self { sink: DVec3::new((a * 6.28).cos(), 0.35, (a * 6.28).sin()).normalize(),
            dispersing: false, residence: 0.0, heat: 0.0 }
    }
}

impl Plates {
    pub(in crate::planetary) fn breakup_axis(&self, dir: DVec3) -> Option<DVec3> {
        self.assembly.as_ref().filter(|c| c.dispersing).map(|c| {
            (dir * dir.dot(c.sink) - c.sink).normalize_or(dir.any_orthonormal_vector())
        })
    }

    pub(in crate::planetary) fn evolve_continents(&mut self, mesh: &super::super::topology::Mesh,
        crust: &[Crust], warped: &[DVec3], dt: f64)
    {
        let Some(cycle) = &mut self.assembly else { return; };
        let mut parcels = vec![(DVec3::ZERO, 0.0, 0.0); self.plates.len()];
        // Mass-weighted crust centres, rather than the original plate labels:
        // daughters inherit crust, and sutures retain their separate ancestry.
        for (i, c) in crust.iter().enumerate() {
            let dir = mesh.directions[i];
            let owner = self.plates.iter().enumerate()
                .max_by(|(_, a), (_, b)| warped[i].dot(a.centre).total_cmp(&warped[i].dot(b.centre)))
                .map_or(0, |(id, _)| id);
            let mass = c.continental_km * mesh.area[i];
            parcels[owner].0 += dir * mass;
            parcels[owner].1 += mass;
            parcels[owner].2 += mesh.area[i];
        }
        let total: f64 = parcels.iter().map(|p| p.1).sum();
        let moment: DVec3 = parcels.iter().map(|p| p.0).sum();
        let concentration = moment.length() / total.max(1.0);
        cycle.residence += dt;
        // A compact assemblage retains heat; fragmented continents cool it.
        let insulation = smooth(0.32, 0.72, concentration);
        cycle.heat += (insulation - cycle.heat) * -(-dt / 65.0).exp_m1();
        if !cycle.dispersing && cycle.heat > 0.48 && cycle.residence > 100.0 {
            cycle.dispersing = true;
            cycle.residence = 0.0;
            cycle.sink = moment.normalize_or(cycle.sink);
            eprintln!("[continental-cycle] breakup concentration={concentration:.3} heat={:.3}", cycle.heat);
        } else if cycle.dispersing && cycle.residence > 120.0 && concentration < 0.38 {
            cycle.dispersing = false;
            eprintln!("[continental-cycle] assembly concentration={concentration:.3}");
            cycle.residence = 0.0;
            // The next downwelling need not lie beneath the previous continent.
            cycle.sink = DQuat::from_axis_angle(cycle.sink.any_orthonormal_vector(), 1.15) * cycle.sink;
        }
        for (plate, (moment, mass, area)) in self.plates.iter_mut().zip(parcels) {
            let centre = moment.normalize_or(plate.centre);
            let continental = smooth(2.0, 16.0, mass / area.max(1.0));
            let toward = centre.cross(cycle.sink);
            let direction = if cycle.dispersing { -toward } else { toward };
            let mantle = direction.normalize_or(centre.any_orthonormal_vector())
                * (0.65 + 0.65 * direction.length().min(1.0));
            // Oceanic plates retain independent circulation. Continental plates
            // progressively entrain into assembly or radial breakup flow.
            let target = plate.spin.lerp(mantle, continental * 0.85);
            let turn = (target - plate.spin).clamp_length_max(
                PLATE_ACCELERATION_CM_PER_YEAR_PER_MYR * dt / PLATE_REFERENCE_CM_PER_YEAR);
            plate.spin = (plate.spin + turn).clamp_length_max(PLATE_MAX_CM_PER_YEAR / PLATE_REFERENCE_CM_PER_YEAR);
        }
    }
}

mod snapshot;
