//! Player-designed microcontrollers (first slice, H1): a verified macro-module
//! floorplan, typed nets, multi-layer wires with explicit vias, a bounded
//! deterministic router, design checks with fixes, a first-order timing and
//! power characterisation, and the event-driven runtime of an installed part.
pub mod characterize;
pub mod compile;
pub mod design;
pub mod edit;
pub mod extract;
pub mod fab;
pub mod firmware;
pub mod library;
mod loops;
pub mod presets;
pub mod registry;
pub mod route;
pub mod runtime;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_runtime;
