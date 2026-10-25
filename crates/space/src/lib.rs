//! Star systems on rails: schema, validation and (in later modules) evaluation.
mod data;
#[cfg(test)]
mod fixtures;
mod validate;

pub use data::{
    BinaryDef, BodyDef, BodyKind, OrbitDef, RotationDef, RotationPeriod, SystemDef, SystemsFile,
    parse,
};
pub use validate::{SystemError, validate};
