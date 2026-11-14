//! Simulation-owned action clock. Rendering reads this clock; it never emits contacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) enum Phase {
    Prepare,
    Active,
    Recover,
    Done,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct Run<A> {
    pub kind: A,
    pub sequence: u32,
    pub elapsed: f32,
    pub prepare: f32,
    pub active: f32,
    pub recover: f32,
    /// One player contact per action, including multi-frame swept movement.
    pub hit: bool,
}
impl<A> Run<A> {
    pub fn new(kind: A, sequence: u32, prepare: f32, active: f32, recover: f32) -> Self {
        Self {
            kind,
            sequence,
            elapsed: 0.0,
            prepare,
            active,
            recover,
            hit: false,
        }
    }
    pub fn phase(&self) -> Phase {
        if self.elapsed < self.prepare {
            Phase::Prepare
        } else if self.elapsed < self.prepare + self.active {
            Phase::Active
        } else if self.elapsed < self.prepare + self.active + self.recover {
            Phase::Recover
        } else {
            Phase::Done
        }
    }
    pub fn wind(&self) -> f32 {
        (self.elapsed / self.prepare.max(0.01)).clamp(0.0, 1.0)
    }
    pub fn advance(&mut self, dt: f32) -> bool {
        let old = self.elapsed;
        self.elapsed += dt.max(0.0);
        old < self.prepare && self.elapsed >= self.prepare
    }
    pub fn interrupt(&mut self) {
        self.elapsed = self.prepare + self.active;
        self.recover = self.recover.max(0.9);
        self.hit = true;
    }
}
