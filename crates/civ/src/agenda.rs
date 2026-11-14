//! The serial agenda: one total order over pending wake-ups.
//!
//! The order is `(due, site, actor)`, the order the app's community agenda
//! already uses. It is the reference semantics: every faster queue must pop
//! the same keys in the same sequence before its speed is considered.

use std::collections::BTreeSet;

mod wheel;
pub use wheel::Wheel;

/// Actor id of a site's founding step. Sorts after every resident.
pub const FOUNDING: u32 = u32::MAX;
/// Actor id of a site's council. Sorts after residents, before founding.
pub const COUNCIL: u32 = u32::MAX - 1;

/// Position of one pending wake-up in the total order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub due: u64,
    pub site: u32,
    pub actor: u32,
}

impl Key {
    pub const fn new(due: u64, site: u32, actor: u32) -> Self { Self { due, site, actor } }
}

/// A set of keys that yields its minimum.
///
/// Callers keep at most one live key per `(site, actor)`; the owner of the
/// wake table removes the old key before inserting a new one.
pub trait Queue: Default {
    fn insert(&mut self, key: Key);
    /// Returns whether the key was present.
    fn remove(&mut self, key: Key) -> bool;
    fn first(&self) -> Option<Key>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
    fn clear(&mut self);

    /// Removes and returns the minimum if it is due at `now`.
    fn pop_due(&mut self, now: u64) -> Option<Key> {
        let key = self.first().filter(|k| k.due <= now)?;
        self.remove(key);
        Some(key)
    }

    /// Keys due at `now`, without removing them.
    fn due_count(&self, now: u64) -> usize;
}

/// Reference queue: an ordered set.
#[derive(Clone, Debug, Default)]
pub struct Ordered(BTreeSet<Key>);

impl Queue for Ordered {
    fn insert(&mut self, key: Key) { self.0.insert(key); }
    fn remove(&mut self, key: Key) -> bool { self.0.remove(&key) }
    fn first(&self) -> Option<Key> { self.0.first().copied() }
    fn len(&self) -> usize { self.0.len() }
    fn clear(&mut self) { self.0.clear(); }
    fn due_count(&self, now: u64) -> usize { self.0.range(..=Key::new(now, u32::MAX, u32::MAX)).count() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_is_due_then_site_then_actor() {
        let mut q = Ordered::default();
        for k in [Key::new(5, 1, 9), Key::new(5, 1, COUNCIL), Key::new(5, 0, FOUNDING), Key::new(4, 7, 7), Key::new(5, 1, 2)] {
            q.insert(k);
        }
        let mut popped = Vec::new();
        while let Some(k) = q.pop_due(5) { popped.push((k.due, k.site, k.actor)); }
        assert_eq!(popped, [(4, 7, 7), (5, 0, FOUNDING), (5, 1, 2), (5, 1, 9), (5, 1, COUNCIL)]);
    }

    #[test]
    fn nothing_pops_before_it_is_due() {
        let mut q = Ordered::default();
        q.insert(Key::new(10, 0, 0));
        assert_eq!(q.pop_due(9), None);
        assert_eq!(q.due_count(9), 0);
        assert_eq!(q.due_count(10), 1);
        assert_eq!(q.pop_due(10), Some(Key::new(10, 0, 0)));
        assert!(q.is_empty());
    }

    #[test]
    fn remove_reports_presence() {
        let mut q = Ordered::default();
        q.insert(Key::new(1, 2, 3));
        assert!(q.remove(Key::new(1, 2, 3)));
        assert!(!q.remove(Key::new(1, 2, 3)));
    }
}
