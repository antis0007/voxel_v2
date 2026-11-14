//! Headless civilization kernel.
//!
//! Integer time, one total order over pending work, keyed random draws and a
//! canonical state encoding. The app keeps `World`, terrain, actors, network
//! transport and save files; this crate never sees them.

pub mod agenda;
pub mod canon;
pub mod draw;
