//! Number formats shared by the flight HUD and the system map.
use engine_space::{BodyId, BodyKind, System};

/// Display name of a body: its data id, capitalised.
pub fn name(system: &System, body: BodyId) -> String {
    let id = system.name(body).replace('_', " ");
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => id,
    }
}

pub fn kind(kind: &BodyKind) -> &'static str {
    match kind {
        BodyKind::Star { .. } => "Star",
        BodyKind::Planet => "Planet",
        BodyKind::Moon => "Moon",
        BodyKind::GasGiant => "Gas Giant",
        BodyKind::Barycenter => "Barycenter",
    }
}

/// Metres as m, km or Mm with three or four significant figures.
pub fn range(m: f64) -> String {
    let m = m.max(0.0);
    if m < 10_000.0 {
        format!("{m:.0} m")
    } else if m < 1.0e6 {
        format!("{:.1} km", m / 1.0e3)
    } else if m < 1.0e7 {
        format!("{:.0} km", m / 1.0e3)
    } else if m < 1.0e9 {
        format!("{:.1} Mm", m / 1.0e6)
    } else {
        format!("{:.0} Mm", m / 1.0e6)
    }
}

pub fn speed(mps: f64) -> String {
    let abs = mps.abs();
    if abs >= 1.0e6 {
        format!("{:.0} km/s", mps / 1.0e3)
    } else if abs >= 1_000.0 {
        format!("{:.2} km/s", mps / 1.0e3)
    } else {
        format!("{mps:.0} m/s")
    }
}

/// Seconds as `42 s`, `12 m 30 s` or `3 h 12 m`.
pub fn duration(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    if s < 60 {
        format!("{s} s")
    } else if s < 3_600 {
        format!("{} m {:02} s", s / 60, s % 60)
    } else {
        format!("{} h {:02} m", s / 3_600, s % 3_600 / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readouts_keep_units_and_restrained_precision() {
        assert_eq!(range(812.4), "812 m");
        assert_eq!(range(18_140.0), "18.1 km");
        assert_eq!(range(2_400_000.0), "2400 km");
        assert_eq!(range(1.5e8), "150.0 Mm");
        assert_eq!(range(-3.0), "0 m");
        assert_eq!(speed(327.0), "327 m/s");
        assert_eq!(speed(-3_636.4), "-3.64 km/s");
        assert_eq!(speed(2.5e7), "25000 km/s");
        assert_eq!(duration(42.2), "42 s");
        assert_eq!(duration(750.0), "12 m 30 s");
        assert_eq!(duration(11_520.0), "3 h 12 m");
    }
}
