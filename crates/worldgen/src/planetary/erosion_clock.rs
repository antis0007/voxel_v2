//! Allocate a surface interval between stripping alluvium and cutting bedrock.
pub(super) fn exposed_time(cover_m: f64, stripping_capacity_m: f64) -> f64 {
    if cover_m <= 0.0 { 1.0 }
    else if stripping_capacity_m <= 0.0 { 0.0 }
    else { (1.0 - cover_m / stripping_capacity_m).clamp(0.0, 1.0) }
}
