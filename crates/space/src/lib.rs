//! Star systems on rails: schema, validation and (in later modules) evaluation.
mod data;
#[cfg(test)]
mod fixtures;
mod kepler;
mod validate;

pub use data::{
    BinaryDef, BodyDef, BodyKind, OrbitDef, RotationDef, RotationPeriod, SystemDef, SystemsFile,
    parse,
};
pub use kepler::{Elements, G, solve_kepler};
pub use validate::{SystemError, validate};
