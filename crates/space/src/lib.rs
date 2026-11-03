//! Star systems on rails: orbits and spin evaluated from data per tick.
//!
//! A system is authored in `assets/star_systems.ron`, checked by `validate` and
//! compiled by `System::compile` into columns with every parent before its
//! children. Nothing is stepped. `System::evaluate` and `System::orientation` are
//! pure functions of that data and the integer tick; `orbit_path` depends on
//! the data alone. The phase
//! of an orbit or a spin comes from `tick % period_ticks` before any float
//! division, so a tick and the same tick plus one period give identical bits
//! and a long run never drifts.
//!
//! Positions, velocities and angles are `f64` rails for rendering and the map
//! only. Anything the simulation reads, such as sun elevation for heat, light
//! and growth, must come from a fixed-point path built with its first consumer,
//! so that replay stays deterministic across machines.
//!
//! Frame: the system frame has +Y north and its root at the origin. Lengths are
//! metres, times are seconds, and angles in code are radians (degrees in data).
mod data;
#[cfg(test)]
mod fixtures;
mod kepler;
mod path;
mod spin;
mod system;
mod transfer;
mod validate;

pub use data::{
    BinaryDef, BodyDef, BodyKind, OrbitDef, RotationDef, RotationPeriod, SystemDef, SystemsFile,
    parse,
};
pub use kepler::{Elements, G, solve_kepler};
pub use path::orbit_path;
pub use system::{BodyId, Snapshot, Spin, System};
pub use transfer::{
    Hohmann, Plan, circular_speed, escape_speed, hill_radius, hyperbolic_burn, lead_angle,
    soi_radius, synodic_period,
};
pub use validate::{SystemError, validate};
