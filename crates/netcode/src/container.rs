//! Server-side view of what one client believes a container holds.
//!
//! Each viewer has a revision that advances whenever the server sends it slots.
//! A client action names the revision it was made against and the slot values it
//! now shows. If the revision is current the server trusts those values as the
//! client's view and later sends only the slots its own result disagrees with. If
//! it is stale the client acted on contents it no longer had, so the server sends
//! the whole container. The server's own result is always what the container
//! holds; the client's values only decide what needs sending. This closes the
//! desync duplications where a client keeps showing items the server took away.
//!
//! The item on the cursor is one more slot, so it follows the same rule.

#[derive(Clone, Debug, PartialEq)]
pub enum Sync<T> {
    Full { revision: u32, slots: Vec<T> },
    Slots { revision: u32, changed: Vec<(u16, T)> },
}

pub struct ContainerView<T> {
    revision: u32,
    /// What the client shows; `None` until a full send.
    remote: Option<Vec<T>>,
}

impl<T: Clone + PartialEq> Default for ContainerView<T> {
    fn default() -> Self {
        Self { revision: 0, remote: None }
    }
}

impl<T: Clone + PartialEq> ContainerView<T> {
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Records a client action; returns false when it was stale or malformed and
    /// the next sync will be full.
    pub fn client_action(&mut self, revision: u32, shown: &[(u16, T)]) -> bool {
        let Some(remote) = self.remote.as_mut() else { return false };
        let fits = shown.iter().all(|(i, _)| usize::from(*i) < remote.len());
        if revision != self.revision || !fits {
            self.remote = None;
            return false;
        }
        for (i, value) in shown {
            remote[usize::from(*i)] = value.clone();
        }
        true
    }

    /// Compares the server's slots with the client's view; `None` when they agree.
    pub fn sync(&mut self, slots: &[T]) -> Option<Sync<T>> {
        let message = match self.remote.as_mut() {
            Some(remote) if remote.len() == slots.len() => {
                let changed: Vec<(u16, T)> = remote
                    .iter_mut()
                    .zip(slots)
                    .enumerate()
                    .filter(|(_, (r, s))| r != s)
                    .map(|(i, (r, s))| {
                        *r = s.clone();
                        (i as u16, s.clone())
                    })
                    .collect();
                if changed.is_empty() {
                    return None;
                }
                Sync::Slots { revision: self.revision.wrapping_add(1), changed }
            }
            _ => {
                self.remote = Some(slots.to_vec());
                Sync::Full { revision: self.revision.wrapping_add(1), slots: slots.to_vec() }
            }
        };
        self.revision = self.revision.wrapping_add(1);
        Some(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (item, count); 0 is empty.
    type Stack = (u16, u8);
    const EMPTY: Stack = (0, 0);

    fn opened(slots: &[Stack]) -> (ContainerView<Stack>, u32) {
        let mut view = ContainerView::default();
        let Some(Sync::Full { revision, .. }) = view.sync(slots) else { panic!() };
        (view, revision)
    }

    #[test]
    fn a_correct_prediction_sends_nothing() {
        let mut server = vec![(7, 64), EMPTY, EMPTY];
        let (mut view, rev) = opened(&server);
        // The client moves the stack from slot 0 to slot 2 and shows that.
        assert!(view.client_action(rev, &[(0, EMPTY), (2, (7, 64))]));
        server.swap(0, 2);
        assert_eq!(view.sync(&server), None);
        assert_eq!(view.revision(), rev);
    }

    #[test]
    fn a_wrong_prediction_corrects_only_the_slots_that_differ() {
        let mut server = vec![(7, 64), EMPTY, (3, 1)];
        let (mut view, rev) = opened(&server);
        // The client thinks it split the stack evenly; the server split 33/31.
        assert!(view.client_action(rev, &[(0, (7, 32)), (1, (7, 32))]));
        server[0] = (7, 33);
        server[1] = (7, 31);
        assert_eq!(view.sync(&server), Some(Sync::Slots { revision: rev + 1, changed: vec![(0, (7, 33)), (1, (7, 31))] }));
    }

    #[test]
    fn a_stale_action_cannot_keep_items_the_server_removed() {
        let mut server = vec![(7, 64), EMPTY];
        let (mut view, rev) = opened(&server);
        // A hopper empties slot 0 and the server tells the client.
        server[0] = EMPTY;
        let Some(Sync::Slots { revision: now, .. }) = view.sync(&server) else { panic!() };
        // A click made before that update arrives, claiming the stack moved to slot 1.
        assert!(!view.client_action(rev, &[(0, EMPTY), (1, (7, 64))]));
        assert_eq!(view.sync(&server), Some(Sync::Full { revision: now + 1, slots: vec![EMPTY, EMPTY] }));
    }

    #[test]
    fn out_of_range_slots_force_a_full_send() {
        let server = vec![EMPTY; 2];
        let (mut view, rev) = opened(&server);
        assert!(!view.client_action(rev, &[(9, (1, 1))]));
        assert!(matches!(view.sync(&server), Some(Sync::Full { .. })));
    }
}
