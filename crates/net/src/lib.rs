//! Bounded host-ordered action exchange. Transport never mutates game state.
//! The game must validate actions and apply each returned frame at its fixed step.
pub mod admin;
pub mod commands;
pub mod identity;
pub mod invite;
pub mod lobby;
pub mod predict;
pub mod protocol;
pub mod punch;
pub mod session;
pub mod simnet;
pub mod social;
pub mod transfer;
#[cfg(all(feature = "steam", not(feature = "steamworks")))]
pub mod steam;
#[cfg(feature = "steamworks")]
#[path = "sdr.rs"]
pub mod steam;
