//! Integer simulation time. One tick is one 120 Hz fixed step of the app.
//!
//! Additions saturate. A tick value never wraps.

/// Count of fixed steps since the world began.
pub type Tick = u64;

/// "No wake-up scheduled".
pub const NEVER: Tick = u64::MAX;
/// The executor's base service period, 0.25 s.
pub const PULSE: Tick = 30;
/// One travel day, 480 s.
pub const DAY: Tick = 57_600;
