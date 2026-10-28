//! Client side of lockstep: buffers frames by tick and releases tick T only when it holds T.
use super::{ClientMsg, Config, Frame, HostMsg, LockstepError, PeerId, at_or_before};
use std::collections::VecDeque;

/// Where the client stands against the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientStatus {
    /// Ticks sealed by the host that this client has not stepped yet.
    Trailing(u64),
    /// Cut off or too far behind: load a checkpoint, then call `resync`.
    NeedsResync,
}

pub struct LockstepClient<T> {
    config: Config,
    id: PeerId,
    next_step: u64,
    /// `window[i]` is the frame for `next_step + i`; at most `resync_lag + input_delay` slots.
    window: VecDeque<Option<Frame<T>>>,
    sealed_until: u64,
    cut_off: bool,
    /// Resync flags from messages sealed no later than this predate the last checkpoint.
    resync_seal: u64,
    /// Inputs not yet acknowledged; `outbox[0]` has sequence `outbox_first`.
    outbox: VecDeque<T>,
    outbox_first: u32,
}

impl<T: Clone> LockstepClient<T> {
    /// A client whose world is the checkpoint taken at the start of `start`.
    pub fn new(config: Config, id: PeerId, start: u64) -> Self {
        Self {
            config,
            id,
            next_step: start,
            window: VecDeque::new(),
            sealed_until: start,
            cut_off: false,
            resync_seal: 0,
            outbox: VecDeque::new(),
            outbox_first: 1,
        }
    }

    pub fn id(&self) -> PeerId {
        self.id
    }

    /// Next tick this client steps.
    pub fn next_tick(&self) -> u64 {
        self.next_step
    }

    /// Inputs sent but not yet in a frame.
    pub fn unacked(&self) -> usize {
        self.outbox.len()
    }

    /// Queue a local input for the host. Returns its sequence.
    pub fn submit(&mut self, payload: T) -> Result<u32, LockstepError> {
        if self.outbox.len() >= self.config.max_pending_inputs {
            return Err(LockstepError::InputsFull);
        }
        self.outbox.push_back(payload);
        Ok(self.outbox_first.wrapping_add(self.outbox.len() as u32 - 1))
    }

    /// What to send the host now: the acknowledgement and the oldest unacknowledged inputs.
    pub fn message(&self) -> ClientMsg<T> {
        let held = self.window.iter().take_while(|f| f.is_some()).count() as u64;
        ClientMsg {
            frames_ack: self.next_step + held,
            first_sequence: self.outbox_first,
            inputs: self.outbox.iter().take(self.config.inputs_per_message).cloned().collect(),
        }
    }

    /// A host message, in any order and any number of times.
    pub fn receive(&mut self, msg: HostMsg<T>) {
        while !self.outbox.is_empty() && at_or_before(self.outbox_first.wrapping_add(1), msg.input_ack) {
            self.outbox.pop_front();
            self.outbox_first = self.outbox_first.wrapping_add(1);
        }
        let slots = (self.config.resync_lag + self.config.input_delay) as usize;
        // A seal past every frame the host could have sent would pin the client in resync.
        let reach = self.next_step.saturating_add(slots as u64 + self.config.frames_per_message as u64);
        if msg.sealed_until <= reach {
            self.sealed_until = self.sealed_until.max(msg.sealed_until);
        }
        self.cut_off |= msg.resync && msg.sealed_until > self.resync_seal;
        for frame in msg.frames {
            let Some(i) = frame.tick.checked_sub(self.next_step).map(|i| i as usize) else { continue };
            if i >= slots {
                continue;
            }
            if self.window.len() <= i {
                self.window.resize(i + 1, None);
            }
            if self.window[i].is_none() {
                self.window[i] = Some(frame);
            }
        }
    }

    /// Whether the next tick's input set is complete.
    pub fn ready(&self) -> bool {
        matches!(self.window.front(), Some(Some(_)))
    }

    /// The next tick's frame, once complete; the caller applies its inputs, then steps the tick.
    pub fn release(&mut self) -> Option<Frame<T>> {
        if !self.ready() {
            return None;
        }
        self.next_step += 1;
        self.window.pop_front().flatten()
    }

    pub fn status(&self) -> ClientStatus {
        let trailing = self.sealed_until.saturating_sub(self.next_step);
        if self.cut_off || trailing > self.config.resync_lag {
            ClientStatus::NeedsResync
        } else {
            ClientStatus::Trailing(trailing)
        }
    }

    /// The world was replaced by the checkpoint taken at the start of `from_tick`. Unsent inputs stay queued.
    pub fn resync(&mut self, from_tick: u64) {
        self.next_step = from_tick;
        self.window.clear();
        self.sealed_until = from_tick;
        self.cut_off = false;
        self.resync_seal = from_tick + self.config.input_delay;
    }
}
