//! Isolated candidates; the game and original kernel remain unchanged.
pub use engine_civ::agenda::{Key, Ordered, Queue};
pub mod wake;
mod wheel;
pub use wheel::Wheel;
