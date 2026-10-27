//! Client-predicted cell edits with sequence acknowledgement, and the server's reach check.
//!
//! The client applies a dig or place at once and tags the request with an action
//! sequence. Server writes to a predicted cell are held back instead of flickering
//! the prediction, and when the server acknowledges the sequence the client puts
//! back whatever the server last said for each cell. A refused or altered edit
//! therefore cannot leave a ghost cell behind.
//!
//! Ordering invariant: the server sends its cell writes for an action before that
//! action's acknowledgement, on the same ordered channel.
use glam::{IVec3, Vec3};
use std::collections::HashMap;

/// Whether sequence `a` is at or before `b`, across `u32` wrap.
fn at_or_before(a: u32, b: u32) -> bool {
    b.wrapping_sub(a) < 1 << 31
}

struct Held<T> {
    sequence: u32,
    authoritative: T,
}

/// Client side: predicted cells waiting for the server.
pub struct EditPredictor<T> {
    sequence: u32,
    held: HashMap<IVec3, Held<T>>,
}

impl<T: Copy> Default for EditPredictor<T> {
    fn default() -> Self {
        Self { sequence: 0, held: HashMap::new() }
    }
}

impl<T: Copy> EditPredictor<T> {
    /// Starts one player action (a dig, a place, a wand blast) and returns its sequence.
    pub fn begin(&mut self) -> u32 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    /// Records a cell the current action changes locally; `before` is its value
    /// prior to the change.
    pub fn predict(&mut self, cell: IVec3, before: T) {
        let sequence = self.sequence;
        self.held.entry(cell).and_modify(|h| h.sequence = sequence).or_insert(Held { sequence, authoritative: before });
    }

    /// A server write; returns it when the caller should apply it now, `None` while held.
    pub fn server_write(&mut self, cell: IVec3, value: T) -> Option<T> {
        match self.held.get_mut(&cell) {
            Some(h) => {
                h.authoritative = value;
                None
            }
            None => Some(value),
        }
    }

    /// Releases every cell predicted at or before `sequence`, sorted by cell, with
    /// the value the caller writes back.
    pub fn acknowledge(&mut self, sequence: u32) -> Vec<(IVec3, T)> {
        let mut out: Vec<(IVec3, T)> = Vec::new();
        self.held.retain(|&cell, h| {
            let done = at_or_before(h.sequence, sequence);
            if done {
                out.push((cell, h.authoritative));
            }
            !done
        });
        out.sort_by_key(|(c, _)| (c.x, c.y, c.z));
        out
    }

    pub fn held(&self) -> usize {
        self.held.len()
    }
}

/// Server side: the newest action sequence processed this tick, sent once per tick.
#[derive(Default)]
pub struct AckBatch {
    newest: Option<u32>,
}

impl AckBatch {
    pub fn processed(&mut self, sequence: u32) {
        self.newest = Some(match self.newest {
            Some(n) if at_or_before(sequence, n) => n,
            _ => sequence,
        });
    }

    pub fn take(&mut self) -> Option<u32> {
        self.newest.take()
    }
}

/// Whether the eye is within `reach` of the nearest point of `cell`.
pub fn within_reach(eye: Vec3, cell: IVec3, reach: f32) -> bool {
    let lo = cell.as_vec3();
    let nearest = eye.clamp(lo, lo + Vec3::ONE);
    eye.distance_squared(nearest) <= reach * reach
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIR: u8 = 0;
    const STONE: u8 = 1;

    #[test]
    fn a_refused_dig_restores_the_cell_after_the_ack() {
        let mut p = EditPredictor::default();
        let cell = IVec3::new(-33, 4, 7);
        let seq = p.begin();
        p.predict(cell, STONE);
        // The server refuses and sends nothing for the cell, then acknowledges.
        assert_eq!(p.acknowledge(seq), vec![(cell, STONE)]);
        assert_eq!(p.held(), 0);
    }

    #[test]
    fn server_writes_are_held_until_the_ack_then_applied() {
        let mut p = EditPredictor::default();
        let (a, b) = (IVec3::new(0, 0, 0), IVec3::new(1, 0, 0));
        let first = p.begin();
        p.predict(a, STONE);
        let second = p.begin();
        p.predict(b, STONE);
        assert_eq!(p.server_write(a, AIR), None);
        assert_eq!(p.server_write(IVec3::new(9, 9, 9), AIR), Some(AIR));
        assert_eq!(p.acknowledge(first), vec![(a, AIR)]);
        // The second action is still pending, so the server's echo stays held.
        assert_eq!(p.server_write(b, AIR), None);
        assert_eq!(p.acknowledge(second), vec![(b, AIR)]);
    }

    #[test]
    fn a_cell_predicted_twice_waits_for_the_later_action() {
        let mut p = EditPredictor::default();
        let cell = IVec3::new(2, 2, 2);
        let first = p.begin();
        p.predict(cell, STONE);
        p.begin();
        p.predict(cell, AIR);
        assert!(p.acknowledge(first).is_empty());
        assert_eq!(p.held(), 1);
    }

    #[test]
    fn sequences_compare_across_wrap() {
        let mut p: EditPredictor<u8> = EditPredictor { sequence: u32::MAX - 1, held: HashMap::new() };
        let old = p.begin();
        p.predict(IVec3::ZERO, STONE);
        let new = p.begin();
        assert_eq!(new, 0);
        assert_eq!(p.acknowledge(new), vec![(IVec3::ZERO, STONE)]);
        let mut batch = AckBatch::default();
        batch.processed(new);
        batch.processed(old);
        assert_eq!(batch.take(), Some(new));
        assert_eq!(batch.take(), None);
    }

    #[test]
    fn reach_measures_to_the_nearest_face_of_the_cell() {
        let eye = Vec3::new(0.5, 1.6, 0.5);
        assert!(within_reach(eye, IVec3::new(5, 1, 0), 4.5));
        assert!(!within_reach(eye, IVec3::new(6, 1, 0), 4.5));
        assert!(within_reach(eye, IVec3::new(0, 1, 0), 0.0));
        assert!(!within_reach(Vec3::new(-100.0, 0.0, -100.0), IVec3::new(-120, 0, -100), 6.0));
    }
}
