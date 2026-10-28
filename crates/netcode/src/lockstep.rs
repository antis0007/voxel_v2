//! Deterministic lockstep input scheduling: the host stamps every matter-changing input with a
//! simulation tick, and every peer steps tick T only once it holds T's complete input set.
//!
//! The host seals tick `stepped + input_delay` each time it steps, so a frame (the inputs of one
//! tick, possibly none) exists `input_delay` ticks before anyone needs it. Frames travel
//! redundantly: each host message repeats every frame since the peer's last acknowledged tick, up
//! to a cap, so loss, duplication and reordering cost latency, never correctness, and no
//! retransmit timer is needed. Client inputs travel the same way in the other direction, keyed by
//! a per-client sequence. A peer that falls `resync_lag` ticks behind stops being fed frames and
//! must load a checkpoint (docs/design/MULTIPLAYER_LOCKSTEP_SIM.md).
//!
//! Generic over the payload; no sockets, no world and no clock.
mod client;
mod host;
pub mod desync;
pub mod wire;

pub use client::{ClientStatus, LockstepClient};
pub use host::LockstepHost;

/// A peer in the session. The host is `HOST`.
pub type PeerId = u16;
pub const HOST: PeerId = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Ticks between the host's current tick and the tick it stamps a new input with.
    pub input_delay: u64,
    /// Frames one host message carries at most.
    pub frames_per_message: usize,
    /// Inputs one client message carries at most.
    pub inputs_per_message: usize,
    /// Inputs a client may hold unacknowledged, and the host may hold unsealed.
    pub max_pending_inputs: usize,
    /// Inputs one frame holds at most; the rest roll over to the next tick.
    pub inputs_per_frame: usize,
    /// Ticks a peer may trail the newest sealed tick before it must resync from a checkpoint.
    pub resync_lag: u64,
}

impl Config {
    /// 60 Hz: 67 ms of input delay, five seconds of lag before a resync.
    pub const DEFAULT: Self = Self {
        input_delay: 4,
        frames_per_message: 32,
        inputs_per_message: 32,
        max_pending_inputs: 256,
        inputs_per_frame: 64,
        resync_lag: 300,
    };
}

/// One input as every peer applies it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input<T> {
    pub peer: PeerId,
    /// The peer's own sequence for this input.
    pub sequence: u32,
    /// Session-wide order the host accepted it in: the replay log key for a late joiner.
    pub order: u64,
    pub payload: T,
}

/// Everything that changes matter in one tick, in the order peers apply it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame<T> {
    pub tick: u64,
    pub inputs: Vec<Input<T>>,
}

/// Host to one client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostMsg<T> {
    /// First tick the host has not sealed yet.
    pub sealed_until: u64,
    /// Next sequence the host expects from this client: everything before it is in a frame.
    pub input_ack: u32,
    /// The client trails too far and must load a checkpoint; `frames` is empty.
    pub resync: bool,
    /// Consecutive frames from the client's acknowledged tick.
    pub frames: Vec<Frame<T>>,
}

/// One client to the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientMsg<T> {
    /// First tick the client does not hold, counting only frames held without a gap.
    pub frames_ack: u64,
    /// Sequence of `inputs[0]`; the rest follow without gaps.
    pub first_sequence: u32,
    pub inputs: Vec<T>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockstepError {
    UnknownPeer,
    DuplicatePeer,
    /// The frames a late joiner or resync needs were already dropped.
    TickNotRetained,
    /// Too many inputs waiting; the rest of the message was ignored and will be resent.
    InputsFull,
}

/// Whether sequence `a` is at or before `b`, across `u32` wrap.
pub(crate) fn at_or_before(a: u32, b: u32) -> bool {
    b.wrapping_sub(a) < 1 << 31
}

#[cfg(test)]
mod tests;
