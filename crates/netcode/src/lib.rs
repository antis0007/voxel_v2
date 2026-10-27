//! Multiplayer building blocks that need no socket and no world.
//!
//! Callers pass time in explicitly, so every result is a function of its inputs
//! and the network simulator replays identically from a seed.
pub mod chunk_pace;
pub mod container;
pub mod edits;
pub mod flood;
pub mod interp;
pub mod link;
pub mod movement;
pub mod pose;
