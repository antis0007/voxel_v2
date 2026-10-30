use super::*;

const TAKE: u64 = 0x7a6b;
const PUT: u64 = 0x9c01;

fn issue(s: &mut Sequencer) -> OperationId {
    s.issue().unwrap()
}

#[test]
fn duplicate_returns_stored_result() {
    let mut r = Receipts::new(8);
    assert_eq!(r.check(0, TAKE), Seen::<u8>::New);
    assert!(r.record(0, TAKE, 7));
    assert_eq!(r.check(0, TAKE), Seen::Done(7));
    assert!(!r.record(0, TAKE, 9));
    assert_eq!(r.check(0, TAKE), Seen::Done(7));
}

#[test]
fn reused_sequence_with_other_contents_conflicts() {
    let mut r = Receipts::new(8);
    assert!(r.record(0, TAKE, 1u8));
    assert_eq!(r.check(0, PUT), Seen::Conflict);
}

#[test]
fn out_of_order_sequences_are_independent() {
    let mut r = Receipts::new(8);
    assert!(r.record(3, TAKE, 'c'));
    assert_eq!(r.check(1, TAKE), Seen::New);
    assert!(r.record(1, TAKE, 'a'));
    assert_eq!(r.check(3, TAKE), Seen::Done('c'));
    assert_eq!(r.next_seq(), 4);
}

#[test]
fn evicted_sequences_expire_instead_of_reapplying() {
    let mut r = Receipts::new(2);
    for seq in 0..3 {
        assert!(r.record(seq, TAKE, seq));
    }
    assert_eq!(r.len(), 2);
    assert_eq!(r.check(0, TAKE), Seen::Expired);
    assert!(!r.record(0, TAKE, 0));
    assert_eq!(r.check(1, TAKE), Seen::Done(1));
}

#[test]
fn reconnect_resumes_instead_of_reusing_old_sequences() {
    let mut server = Receipts::new(8);
    let mut first = Sequencer::new(4);
    let old = issue(&mut first);
    assert!(server.record(old.seq, TAKE, true));

    // A fresh sequencer would send seq 0 again for a new put and be told
    // the old take's result; resuming gives it an unused sequence.
    let mut naive = Sequencer::new(4);
    assert_eq!(server.check(issue(&mut naive).seq, PUT), Seen::Conflict);
    let mut resumed = Sequencer::resume(4, server.next_seq());
    let next = issue(&mut resumed);
    assert_eq!(next.seq, 1);
    assert_eq!(server.check(next.seq, PUT), Seen::New);
}

#[test]
fn restart_keeps_applied_sequences_from_reapplying() {
    let mut before = Receipts::new(8);
    for seq in 0..5 {
        assert!(before.record(seq, TAKE, seq));
    }
    let saved = before.floor();

    let after = Receipts::<u32>::restore(8, saved);
    for seq in 0..5 {
        assert_eq!(after.check(seq, TAKE), Seen::Expired, "seq {seq} must not apply twice");
    }
    assert_eq!(after.check(5, TAKE), Seen::New);
    assert_eq!(after.next_seq(), 5);
}

#[test]
fn last_shared_item_moves_once_under_resends() {
    // Two clients each resend a take of the last item three times; the
    // server applies each id once, so exactly one take succeeds.
    let mut stock = 1u32;
    let mut held = [0u32; 2];
    let mut receipts = [Receipts::new(16), Receipts::new(16)];
    let mut seqs = [Sequencer::new(0), Sequencer::new(1)];
    let ids = [issue(&mut seqs[0]), issue(&mut seqs[1])];
    for _ in 0..3 {
        for id in ids {
            let table = &mut receipts[id.client as usize];
            if table.check(id.seq, TAKE) == Seen::New {
                let ok = stock > 0;
                if ok {
                    stock -= 1;
                    held[id.client as usize] += 1;
                }
                table.record(id.seq, TAKE, ok);
            }
        }
    }
    assert_eq!(stock, 0);
    assert_eq!(held.iter().sum::<u32>(), 1);
    assert_eq!(receipts[0].check(ids[0].seq, TAKE), Seen::Done(true));
    assert_eq!(receipts[1].check(ids[1].seq, TAKE), Seen::Done(false));
}

#[test]
fn last_shared_item_survives_a_restart_between_resends() {
    let mut stock = 1u32;
    let mut server = Receipts::new(16);
    let id = issue(&mut Sequencer::new(0));
    assert_eq!(server.check(id.seq, TAKE), Seen::New);
    stock -= 1;
    server.record(id.seq, TAKE, true);

    let server = Receipts::<bool>::restore(16, server.floor());
    // The reply was lost; the resend after the restart must not take again.
    assert_eq!(server.check(id.seq, TAKE), Seen::Expired);
    assert_eq!(stock, 0);
}

#[test]
fn sequencer_numbers_per_client_and_stops_before_wrapping() {
    let mut s = Sequencer::new(5);
    assert_eq!(s.issue(), Some(OperationId { client: 5, seq: 0 }));
    assert_eq!(s.issue(), Some(OperationId { client: 5, seq: 1 }));
    let mut end = Sequencer::resume(5, u32::MAX - 1);
    assert_eq!(end.issue().map(|id| id.seq), Some(u32::MAX - 1));
    assert_eq!(end.issue(), None);
}
