use super::Wheel;
use crate::agenda::{Key, Ordered, Queue};

fn check(wheel: &Wheel, reference: &Ordered, now: u64) {
    assert_eq!(wheel.first(), reference.first());
    assert_eq!(wheel.len(), reference.len());
    assert_eq!(wheel.due_count(now), reference.due_count(now));
}

#[test]
fn duplicate_keys_match_in_unsorted_draining_late_and_far_regions() {
    let (mut wheel, mut reference) = (Wheel::with_bits(6), Ordered::default());
    for actor in [5, 1, 9, 9, 5] {
        let key = Key::new(3, 0, actor);
        wheel.insert(key);
        reference.insert(key);
        check(&wheel, &reference, 3);
    }
    assert_eq!(wheel.pop_due(3), reference.pop_due(3));
    for actor in [5, 7, 7, 2, 9] {
        let key = Key::new(3, 0, actor);
        wheel.insert(key);
        reference.insert(key);
        check(&wheel, &reference, 3);
    }
    while let Some(key) = reference.pop_due(3) {
        assert_eq!(wheel.pop_due(3), Some(key));
    }
    for due in [1000, 1000, 1001, 1001] {
        wheel.insert(Key::new(due, 2, 3));
        reference.insert(Key::new(due, 2, 3));
        check(&wheel, &reference, 1000);
    }
    assert_eq!(wheel.pop_due(1000), reference.pop_due(1000));
    for due in [2, 2, 1001, 1001, 1063, 1063, u64::MAX, u64::MAX] {
        wheel.insert(Key::new(due, 2, 3));
        reference.insert(Key::new(due, 2, 3));
        check(&wheel, &reference, 1000);
    }
    while let Some(key) = reference.pop_due(u64::MAX) {
        assert_eq!(wheel.pop_due(u64::MAX), Some(key));
    }
    check(&wheel, &reference, u64::MAX);
}

fn draw(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn duplicate_heavy_scripts_match_reference_for_200000_operations() {
    for bits in [6, 10] {
        for seed in [1, 74, 91, 98765, u64::MAX] {
            let (mut wheel, mut reference) = (Wheel::with_bits(bits), Ordered::default());
            let mut history = Vec::new();
            let mut rng = seed;
            let mut now = 0u64;
            for step in 0..20_000u64 {
                let roll = draw(&mut rng);
                let fresh = Key::new(
                    now.saturating_add((roll >> 9) % 600)
                        .saturating_sub((roll >> 22) % 100),
                    (roll >> 32) as u32 % 8,
                    (roll >> 40) as u32 % 128,
                );
                let key = if !history.is_empty() && roll & 1 == 0 {
                    history[roll as usize % history.len()]
                } else {
                    fresh
                };
                match roll % 7 {
                    0..=2 => {
                        for _ in 0..=roll % 3 {
                            wheel.insert(key);
                            reference.insert(key);
                        }
                        history.push(key);
                        if history.len() > 128 {
                            history.remove(0);
                        }
                    }
                    3 => assert_eq!(
                        wheel.remove(key),
                        reference.remove(key),
                        "seed={seed} step={step}"
                    ),
                    4 => assert_eq!(
                        wheel.pop_due(now),
                        reference.pop_due(now),
                        "seed={seed} step={step}"
                    ),
                    5 => now = now.saturating_add((roll >> 11) % 80),
                    _ if step % 137 == 0 => {
                        wheel.clear();
                        reference.clear();
                    }
                    _ => (),
                }
                assert_eq!(wheel.first(), reference.first(), "seed={seed} step={step}");
                assert_eq!(wheel.len(), reference.len(), "seed={seed} step={step}");
                if step % 16 == 0 {
                    check(&wheel, &reference, now);
                }
            }
            while let Some(key) = reference.pop_due(u64::MAX) {
                assert_eq!(wheel.pop_due(u64::MAX), Some(key));
            }
            check(&wheel, &reference, u64::MAX);
        }
    }
}

#[test]
fn clone_remove_clear_and_tick_ceiling_keep_set_semantics() {
    let (mut wheel, mut reference) = (Wheel::with_bits(6), Ordered::default());
    for due in [
        u64::MAX - 65,
        u64::MAX - 64,
        u64::MAX - 63,
        u64::MAX - 1,
        u64::MAX,
    ] {
        for actor in [0, 1, u32::MAX] {
            let key = Key::new(due, u32::MAX, actor);
            for _ in 0..3 {
                wheel.insert(key);
                reference.insert(key);
            }
        }
    }
    let mut copied = wheel.clone();
    while let Some(key) = reference.pop_due(u64::MAX) {
        assert_eq!(wheel.pop_due(u64::MAX), Some(key));
        assert!(copied.remove(key));
        assert!(!copied.remove(key));
    }
    assert!(wheel.is_empty() && copied.is_empty());
    copied.insert(Key::new(0, 0, 0));
    copied.clear();
    assert_eq!(
        (copied.first(), copied.len(), copied.due_count(u64::MAX)),
        (None, 0, 0)
    );
}

#[test]
fn dense_membership_survives_removal_and_transition_to_sorted_drain() {
    for count in [63u32, 64, 65, 128, 1024] {
        let (mut wheel, mut reference) = (Wheel::with_bits(6), Ordered::default());
        for actor in (0..count).rev() {
            let key = Key::new(7, 1, actor);
            for _ in 0..3 {
                wheel.insert(key);
                reference.insert(key);
            }
        }
        for actor in 0..count.saturating_sub(3) {
            let key = Key::new(7, 1, actor);
            assert_eq!(wheel.remove(key), reference.remove(key));
        }
        for actor in [0, 1, count - 1, count, count] {
            let key = Key::new(7, 1, actor);
            wheel.insert(key);
            reference.insert(key);
        }
        check(&wheel, &reference, 7);
        assert_eq!(wheel.pop_due(7), reference.pop_due(7));
        for actor in [count, count + 1, count + 1] {
            let key = Key::new(7, 1, actor);
            wheel.insert(key);
            reference.insert(key);
        }
        while let Some(key) = reference.pop_due(7) {
            assert_eq!(wheel.pop_due(7), Some(key));
        }
        check(&wheel, &reference, 7);
    }
}

#[test]
fn dense_far_bucket_migration_and_slot_reuse_match_reference() {
    let (mut wheel, mut reference) = (Wheel::with_bits(6), Ordered::default());
    for due in [5000, 5001, 5064] {
        for actor in (0..256u32).rev() {
            let key = Key::new(due, 4, actor);
            wheel.insert(key);
            wheel.insert(key);
            reference.insert(key);
        }
    }
    assert_eq!(wheel.pop_due(5000), reference.pop_due(5000));
    for due in [4999, 5000, 5001, 5064] {
        for actor in [128, 256, 256] {
            let key = Key::new(due, 4, actor);
            wheel.insert(key);
            reference.insert(key);
        }
    }
    let mut copy = wheel.clone();
    while let Some(key) = reference.pop_due(u64::MAX) {
        assert_eq!(wheel.pop_due(u64::MAX), Some(key));
        assert_eq!(copy.pop_due(u64::MAX), Some(key));
    }
    check(&wheel, &reference, u64::MAX);
    check(&copy, &reference, u64::MAX);
}

fn assert_membership_index(wheel: &Wheel) {
    assert_eq!(
        wheel.in_slots,
        wheel.slots.iter().map(Vec::len).sum::<usize>()
    );
    for (&slot, members) in &wheel.dense {
        assert!(!Wheel::bit(&wheel.sorted, slot));
        assert!(!wheel.slots[slot].is_empty());
        assert_eq!(members.len(), wheel.slots[slot].len());
        assert!(wheel.slots[slot].iter().all(|key| members.contains(key)));
    }
}

#[test]
fn membership_index_matches_slot_contents_through_each_transition() {
    let mut wheel = Wheel::with_bits(6);
    for actor in 0..256 {
        let key = Key::new(1, 3, actor);
        wheel.insert(key);
        wheel.insert(key);
        assert_membership_index(&wheel);
    }
    for actor in (0..256).step_by(2) {
        assert!(wheel.remove(Key::new(1, 3, actor)));
        assert_membership_index(&wheel);
    }
    let mut copy = wheel.clone();
    assert_membership_index(&copy);
    assert_eq!(copy.pop_due(1), Some(Key::new(1, 3, 1)));
    assert_membership_index(&copy);
    assert!(
        copy.dense.is_empty(),
        "sorted draining no longer needs membership storage"
    );
    while copy.pop_due(1).is_some() {
        assert_membership_index(&copy);
    }
    wheel.clear();
    assert_membership_index(&wheel);
    assert!(wheel.dense.is_empty());
}
