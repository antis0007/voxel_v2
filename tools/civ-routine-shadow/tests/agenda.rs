use axiom_civ_routine_shadow::routine::next_wake;
use engine_civ::agenda::{COUNCIL, FOUNDING, Key, Ordered, Queue, Wheel};
use std::collections::{BTreeMap, BTreeSet};

const BASELINE_EVENTS_PER_TICK: usize = 8;

fn agenda_trace(budget: usize) -> usize {
    let mut reference = BTreeSet::new();
    let mut wakes = BTreeMap::new();
    let mut ordered = Ordered::default();
    let mut wheel = Wheel::with_bits(6);
    for site in (0..16u32).rev() {
        for actor in (0..64u32).chain([COUNCIL, FOUNDING]).rev() {
            let due = u64::from((site + actor % 11) % 5);
            reference.insert((due, site, actor));
            wakes.insert((site, actor), due);
            ordered.insert(Key::new(due, site, actor));
            wheel.insert(Key::new(due, site, actor));
        }
    }
    let mut serviced = 0;
    let mut oldest_delay = 0;
    for now in 0..2_000u64 {
        let mut count = 0;
        for _ in 0..budget {
            let expected = reference.first().copied().filter(|entry| entry.0 <= now);
            let key = expected.map(|(due, site, actor)| Key::new(due, site, actor));
            assert_eq!(ordered.pop_due(now), key, "ordered tick={now}");
            assert_eq!(wheel.pop_due(now), key, "wheel tick={now}");
            let Some((due, site, actor)) = expected else {
                break;
            };
            reference.remove(&(due, site, actor));
            oldest_delay = oldest_delay.max(now - due);
            // Reschedule from processing time, as communities::service does.
            let delay = match actor {
                FOUNDING => 120,
                COUNCIL => 600,
                _ => 30,
            };
            let next = now + delay;
            reference.insert((next, site, actor));
            wakes.insert((site, actor), next);
            ordered.insert(Key::new(next, site, actor));
            wheel.insert(Key::new(next, site, actor));
            serviced += 1;
            count += 1;
        }
        assert!(count <= budget);
        if now % 137 == 0 {
            ordered.clear();
            wheel.clear();
            // The actual save owns wakes, not queue order. This rebuild only
            // models that table; it is not an application save/load test.
            for (&(site, actor), &due) in wakes.iter().rev() {
                ordered.insert(Key::new(due, site, actor));
                wheel.insert(Key::new(due, site, actor));
            }
        }
        if now % 31 == 0 {
            let due = reference.iter().take_while(|entry| entry.0 <= now).count();
            assert_eq!(ordered.len(), reference.len());
            assert_eq!(wheel.len(), reference.len());
            assert_eq!(ordered.due_count(now), due);
            assert_eq!(wheel.due_count(now), due);
        }
    }
    assert!(oldest_delay > 0, "the fixture must exercise backlog");
    serviced
}

#[test]
fn cancellation_and_replacement_keep_one_live_wake() {
    let mut ordered = Ordered::default();
    let mut wheel = Wheel::with_bits(6);
    for due in [1, 63, 64, 65, 16384, u64::MAX - 1, u64::MAX] {
        let old = Key::new(due, 7, 8);
        ordered.insert(old);
        wheel.insert(old);
        assert_eq!(ordered.remove(old), wheel.remove(old));
        assert_eq!(ordered.remove(old), wheel.remove(old));
        let new = Key::new(due.saturating_add(30), 7, 8);
        ordered.insert(new);
        wheel.insert(new);
        assert_eq!(ordered.pop_due(u64::MAX), wheel.pop_due(u64::MAX));
        assert!(ordered.is_empty() && wheel.is_empty());
    }
}

#[test]
fn integrated_wheel_duplicate_insertion_is_idempotent() {
    let key = Key::new(1, 2, 3);
    let mut ordered = Ordered::default();
    let mut wheel = Wheel::with_bits(6);
    for _ in 0..2 {
        ordered.insert(key);
        wheel.insert(key);
    }
    assert_eq!(ordered.len(), 1);
    assert_eq!(ordered.pop_due(1), wheel.pop_due(1));
    assert_eq!(ordered.pop_due(1), None);
    assert_eq!(
        wheel.pop_due(1),
        None,
        "integrated wheel must preserve set semantics"
    );
}

#[test]
fn next_wake_matches_pre_activity_baseline_across_boundaries() {
    for now in [0u64, 1, 29, 30, 600, u64::MAX - 30, u64::MAX] {
        for busy in [false, true] {
            for has_homes in [false, true] {
                for retry in [0, now, now.saturating_add(20), now.saturating_add(900)] {
                    for routine in [0, now, now.saturating_add(10), now.saturating_add(400)] {
                        for meal in [0, now, now.saturating_add(15), now.saturating_add(300)] {
                            let earliest = now.saturating_add(30);
                            let expected = if busy {
                                earliest
                            } else {
                                let mut due =
                                    retry.max(now.saturating_add(if has_homes { 600 } else { 30 }));
                                if routine > now {
                                    due = due.min(routine.max(earliest));
                                }
                                if meal > now {
                                    due = due.min(meal.max(earliest));
                                }
                                due
                            };
                            assert_eq!(
                                next_wake(now, busy, has_homes, retry, routine, meal),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn eight_event_baseline_matches_tuple_reference_and_restored_wakes() {
    assert_eq!(agenda_trace(BASELINE_EVENTS_PER_TICK), 16_000);
}

#[test]
fn sixty_four_event_budget_keeps_the_same_total_order() {
    assert!(agenda_trace(64) > 16_000);
}

#[test]
fn activity_executor_wakes_are_not_legacy_equivalence() {
    let now = 100u64;
    // Current crew.rs default idle wait is 6000, rather than 600.
    assert_eq!(next_wake(now, false, true, 0, 0, 0), 700);
    assert_ne!(next_wake(now, false, true, 0, 0, 0), now + 6000);
    // A future retry now wins over the idle wait (subject to one pulse).
    assert_eq!(next_wake(now, false, true, 200, 0, 0), 700);
    assert_ne!(next_wake(now, false, true, 200, 0, 0), 200u64.max(now + 30));
    // Activity completion and overdue path service can wake after one tick.
    assert_eq!(next_wake(now, true, true, 0, 0, 0), 130);
    assert_ne!(next_wake(now, true, true, 0, 0, 0), 90u64.max(now + 1));
}
