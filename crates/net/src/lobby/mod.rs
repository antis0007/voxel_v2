//! Online sessions through a lobby. The lobby lists hosts, answers searches and relays bytes.

pub mod relay;
pub mod wire;

/// Public lobby used when no `--multiplayer-lobby` address is given.
pub const DEFAULT_LOBBY: &str = "172.81.178.223:27021";
