//! Time-scaled biological response and erosion caps for island evolution.
pub(super) fn relaxation(dt_myr: f64) -> f64 {
    -((dt_myr / 0.25) * 0.65_f64.ln()).exp_m1()
}

pub(super) fn growth_cap(dt_myr: f64) -> f64 { 3.0 * dt_myr / 0.25 }
pub(super) fn weathering_cap(dt_myr: f64) -> f64 { 0.5 * dt_myr / 0.25 }

pub(super) fn elapsed(restored_height: f64, own_age: f64, neighbour_age: f64) -> f64 {
    if restored_height > 0.1 { own_age } else { own_age.max(neighbour_age) }
}
