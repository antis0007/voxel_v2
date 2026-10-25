//! Star systems on rails: schema, validation and per-tick evaluation.
mod data;
#[cfg(test)]
mod fixtures;
mod kepler;
mod system;
mod validate;

pub use data::{
    BinaryDef, BodyDef, BodyKind, OrbitDef, RotationDef, RotationPeriod, SystemDef, SystemsFile,
    parse,
};
pub use kepler::{Elements, G, solve_kepler};
pub use system::{BodyId, Snapshot, Spin, System};
pub use validate::{SystemError, validate};
