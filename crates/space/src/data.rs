//! Star-system schema as it appears in `assets/star_systems.ron`.
use crate::validate::SystemError;
use serde::{Deserialize, Serialize};

/// Every system a data file defines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemsFile {
    pub systems: Vec<SystemDef>,
}

/// One star system: a body hierarchy with a single root.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemDef {
    pub id: String,
    /// Multiplier from tick time to system time.
    pub time_scale: f64,
    pub bodies: Vec<BodyDef>,
}

/// One body. A body without a parent is the root; a binary's components
/// have the barycenter as parent and no orbit of their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BodyDef {
    pub id: String,
    pub kind: BodyKind,
    /// A barycenter's mass is derived from its components, so it gives 0.
    pub mass_kg: f64,
    pub radius_m: f64,
    pub parent: Option<String>,
    pub orbit: Option<OrbitDef>,
    pub binary: Option<BinaryDef>,
    pub rotation: Option<RotationDef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BodyKind {
    Star { temperature_k: f64 },
    Planet,
    Moon,
    GasGiant,
    Barycenter,
}

/// Keplerian elements about the parent. The semi-major axis is not stored:
/// it follows from the period and the combined mass of parent and body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrbitDef {
    pub period_s: f64,
    /// Eccentricity, 0 to 0.999.
    pub e: f64,
    pub i_deg: f64,
    /// Longitude of the ascending node.
    pub node_deg: f64,
    /// Argument of periapsis.
    pub peri_deg: f64,
    pub mean_anomaly_deg: f64,
}

/// The two components of a barycenter and the elements they share.
/// Component `b` sits at `peri_deg + 180` on the same orbit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BinaryDef {
    pub a: String,
    pub b: String,
    pub orbit: OrbitDef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RotationPeriod {
    Seconds(f64),
    /// Spin period equals the body's own orbital period, or its binary's for
    /// a component. The axis is that orbit's normal, so `tilt_deg` must be 0
    /// and `north_deg` is ignored. +X faces the parent (a component's
    /// barycenter) at tick 0; the spin runs at the mean motion, so an
    /// eccentric orbit librates.
    Locked,
}

/// Spin axis and period. The body turns positively about its axis, so a
/// retrograde spin is a tilt past 90 degrees.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RotationDef {
    pub period: RotationPeriod,
    /// Angle of the spin axis from system +Y, degrees. Must be 0 for
    /// `Locked`, whose axis is the orbit normal.
    pub tilt_deg: f64,
    /// Azimuth the axis tilts toward: degrees in the XZ plane from +X toward
    /// -Z, a positive turn about +Y. Ignored for `Locked`.
    pub north_deg: f64,
}

/// Reads a systems file. Syntax and types only; call `validate` per system.
pub fn parse(text: &str) -> Result<SystemsFile, SystemError> {
    ron::from_str(text).map_err(|e| SystemError::Parse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{binary, two_body};
    use crate::validate::validate;

    #[test]
    fn home_file_parses_validates_and_round_trips() {
        let text = include_str!("../../../assets/star_systems.ron");
        let file = parse(text).unwrap();
        assert!(!file.systems.is_empty());
        for s in &file.systems {
            validate(s).unwrap();
        }
        let again = ron::ser::to_string_pretty(&file, Default::default()).unwrap();
        assert_eq!(parse(&again).unwrap(), file);
    }

    #[test]
    fn binary_system_round_trips_with_locked_spin() {
        let mut sys = binary();
        sys.bodies[3].rotation = Some(RotationDef {
            period: RotationPeriod::Locked,
            tilt_deg: 0.0,
            north_deg: 90.0,
        });
        sys.bodies[1].rotation = Some(RotationDef {
            period: RotationPeriod::Seconds(480.0),
            tilt_deg: 0.0,
            north_deg: 0.0,
        });
        let file = SystemsFile {
            systems: vec![sys, two_body()],
        };
        let text = ron::ser::to_string_pretty(&file, Default::default()).unwrap();
        assert_eq!(parse(&text).unwrap(), file);
    }
}
