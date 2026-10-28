//! Host side of lockstep: stamps inputs, seals frames, feeds each client from its acknowledgement.
use super::{ClientMsg, Config, Frame, HOST, HostMsg, Input, LockstepError, PeerId, at_or_before};
use std::collections::VecDeque;

struct HostPeer {
    id: PeerId,
    next_input: u32,
    /// First tick the peer does not hold.
    frames_from: u64,
    resync: bool,
}

/// The authority: accepts inputs, seals frames, feeds each client from its acknowledgement.
pub struct LockstepHost<T> {
    config: Config,
    /// Retained frames; `frames[i].tick == base + i`, the last one is `sealed_until - 1`.
    frames: VecDeque<Frame<T>>,
    base: u64,
    sealed_until: u64,
    /// Next tick the host itself steps.
    stepped: u64,
    open: VecDeque<Input<T>>,
    next_order: u64,
    own_sequence: u32,
    peers: Vec<HostPeer>,
}

impl<T: Clone> LockstepHost<T> {
    /// A session whose first stepped tick is `start`; ticks up to `start + input_delay` are sealed empty.
    pub fn new(config: Config, start: u64) -> Self {
        let frames = (start..start + config.input_delay).map(|tick| Frame { tick, inputs: Vec::new() }).collect();
        Self {
            config,
            frames,
            base: start,
            sealed_until: start + config.input_delay,
            stepped: start,
            open: VecDeque::new(),
            next_order: 0,
            own_sequence: 0,
            peers: Vec::new(),
        }
    }

    pub fn config(&self) -> Config {
        self.config
    }

    /// Next tick the host steps; a checkpoint taken now is the state at the start of this tick.
    pub fn next_tick(&self) -> u64 {
        self.stepped
    }

    pub fn sealed_until(&self) -> u64 {
        self.sealed_until
    }

    /// Frames held for peers that have not acknowledged them.
    pub fn retained(&self) -> usize {
        self.frames.len()
    }

    /// Admit a peer whose world is the checkpoint taken at the start of `from_tick`.
    pub fn add_peer(&mut self, id: PeerId, from_tick: u64) -> Result<(), LockstepError> {
        if id == HOST || self.peers.iter().any(|p| p.id == id) {
            return Err(LockstepError::DuplicatePeer);
        }
        if from_tick < self.base || from_tick > self.sealed_until {
            return Err(LockstepError::TickNotRetained);
        }
        self.peers.push(HostPeer { id, next_input: 1, frames_from: from_tick, resync: false });
        Ok(())
    }

    pub fn remove_peer(&mut self, id: PeerId) {
        self.peers.retain(|p| p.id != id);
        self.trim();
    }

    /// The peer has been sent the checkpoint taken at the start of `from_tick` and resumes from it.
    pub fn resynced(&mut self, id: PeerId, from_tick: u64) -> Result<(), LockstepError> {
        if from_tick < self.base || from_tick > self.sealed_until {
            return Err(LockstepError::TickNotRetained);
        }
        let peer = self.peers.iter_mut().find(|p| p.id == id).ok_or(LockstepError::UnknownPeer)?;
        peer.frames_from = from_tick;
        peer.resync = false;
        Ok(())
    }

    /// Whether the peer was cut off and needs a checkpoint.
    pub fn needs_resync(&self, id: PeerId) -> bool {
        self.peers.iter().any(|p| p.id == id && p.resync)
    }

    /// The host's own input; it is stamped like everyone else's. Returns its sequence.
    pub fn submit(&mut self, payload: T) -> Result<u32, LockstepError> {
        if self.open.len() >= self.config.max_pending_inputs {
            return Err(LockstepError::InputsFull);
        }
        self.own_sequence = self.own_sequence.wrapping_add(1);
        self.accept(HOST, self.own_sequence, payload);
        Ok(self.own_sequence)
    }

    fn accept(&mut self, peer: PeerId, sequence: u32, payload: T) {
        self.open.push_back(Input { peer, sequence, order: self.next_order, payload });
        self.next_order += 1;
    }

    /// A client message. Duplicate and stale inputs are skipped; a gap stops acceptance until the
    /// client resends from its oldest unacknowledged input.
    pub fn receive(&mut self, id: PeerId, msg: ClientMsg<T>) -> Result<(), LockstepError> {
        let limit = self.config.max_pending_inputs;
        let index = self.peers.iter().position(|p| p.id == id).ok_or(LockstepError::UnknownPeer)?;
        let peer = &mut self.peers[index];
        if !peer.resync {
            peer.frames_from = peer.frames_from.max(msg.frames_ack.min(self.sealed_until));
        }
        let mut expected = peer.next_input;
        let mut accepted = Vec::new();
        let mut full = false;
        for (k, payload) in msg.inputs.into_iter().enumerate() {
            let sequence = msg.first_sequence.wrapping_add(k as u32);
            if sequence != expected {
                if at_or_before(sequence, expected) {
                    continue;
                }
                break;
            }
            if self.open.len() + accepted.len() >= limit {
                full = true;
                break;
            }
            accepted.push((sequence, payload));
            expected = expected.wrapping_add(1);
        }
        peer.next_input = expected;
        for (sequence, payload) in accepted {
            self.accept(id, sequence, payload);
        }
        self.trim();
        if full { Err(LockstepError::InputsFull) } else { Ok(()) }
    }

    /// Seal the next tick and return the frame the host steps now.
    pub fn advance(&mut self) -> Frame<T> {
        let take = self.open.len().min(self.config.inputs_per_frame);
        let inputs: Vec<Input<T>> = self.open.drain(..take).collect();
        self.frames.push_back(Frame { tick: self.sealed_until, inputs });
        self.sealed_until += 1;
        let frame = self.frame(self.stepped).cloned().unwrap_or(Frame { tick: self.stepped, inputs: Vec::new() });
        self.stepped += 1;
        self.trim();
        frame
    }

    /// A retained frame.
    pub fn frame(&self, tick: u64) -> Option<&Frame<T>> {
        tick.checked_sub(self.base).and_then(|i| self.frames.get(i as usize))
    }

    /// What to send this client now.
    pub fn message_for(&self, id: PeerId) -> Option<HostMsg<T>> {
        let peer = self.peers.iter().find(|p| p.id == id)?;
        let frames = if peer.resync {
            Vec::new()
        } else {
            let from = peer.frames_from.max(self.base);
            let to = self.sealed_until.min(from + self.config.frames_per_message as u64);
            (from..to).filter_map(|t| self.frame(t).cloned()).collect()
        };
        Some(HostMsg { sealed_until: self.sealed_until, input_ack: peer.next_input, resync: peer.resync, frames })
    }

    /// Cut off peers past `resync_lag` and drop frames nobody needs.
    fn trim(&mut self) {
        let lag = self.config.resync_lag;
        let mut keep = self.stepped;
        for p in &mut self.peers {
            if self.sealed_until - p.frames_from > lag {
                p.resync = true;
            }
            if !p.resync {
                keep = keep.min(p.frames_from);
            }
        }
        while self.base < keep && !self.frames.is_empty() {
            self.frames.pop_front();
            self.base += 1;
        }
    }
}
