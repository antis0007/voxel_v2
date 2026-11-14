use axiom_civ_routine_shadow::{
    Action, Probe,
    routine::{Meeting, Phase, Routine},
    time::PULSE,
};

// Compile the current app policy, including its own tests. No source copy.
#[allow(dead_code, clippy::field_reassign_with_default)]
mod legacy {
    pub mod policy {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/app/src/world/farming/jobs/routine/policy.rs"
        ));
    }
}
use legacy::policy::{Phase as AppPhase, Resident};

fn app_phase(phase: Phase) -> AppPhase {
    match phase {
        Phase::Work => AppPhase::Work,
        Phase::Meal => AppPhase::Meal,
        Phase::Rest => AppPhase::Rest,
        Phase::Social => AppPhase::Social,
    }
}

fn snapshot(r: &Resident) -> Routine {
    Routine {
        phase: match r.phase {
            AppPhase::Work => Phase::Work,
            AppPhase::Meal => Phase::Meal,
            AppPhase::Rest => Phase::Rest,
            AppPhase::Social => Phase::Social,
        },
        due: r.due,
        fatigue: r.fatigue,
        social_need: r.social_need,
        needs_at: r.needs_at,
        since: r.since,
        last: r.last,
        dwell: r.dwell,
        rested_at: r.rested_at,
        social_at: r.social_at,
        social_partner: r.social_partner,
        rests: r.rests,
        socials: r.socials,
        interrupted: r.interrupted,
    }
}

fn legacy_apply(r: &mut Resident, now: u64, action: Action) {
    match action {
        Action::Enter(phase) => r.enter(app_phase(phase), now),
        Action::RequestMeal { hungry, threshold } => r.request_meal(now, hungry, threshold),
        Action::MealCommitted => r.meal_committed(now),
        Action::Observe { present, cap } => r.observe(now, present, cap),
        // Reference expressions from jobs/routine.rs and routine/social.rs.
        // Physical task/route/meeting mutations are outside the projection.
        Action::AdvanceNeeds => {
            let elapsed = if r.needs_at == 0 {
                30
            } else {
                now.saturating_sub(r.needs_at)
            };
            r.fatigue = r.fatigue.saturating_add(elapsed);
            r.social_need = r.social_need.saturating_add(elapsed);
            r.needs_at = now;
        }
        Action::BeginCycle { id } => {
            if r.due == 0 {
                r.due = now.saturating_add(60 * 30 + u64::from(id % 24) * 8 * 30);
                r.since = now;
            }
        }
        Action::Interrupt => {
            if r.phase != AppPhase::Work {
                r.interrupted = r.interrupted.saturating_add(1);
            }
            r.enter(AppPhase::Work, now);
            r.due = now.saturating_add(240 * 30);
        }
        Action::Rested => {
            r.rests = r.rests.saturating_add(1);
            r.fatigue = 0;
            r.rested_at = now;
            r.enter(AppPhase::Social, now);
        }
        Action::Socialized { id, partner } => {
            r.socials = r.socials.saturating_add(1);
            r.social_need = 0;
            r.social_partner = Some(partner);
            r.social_at = now;
            r.enter(AppPhase::Work, now);
            r.due = now.saturating_add(240 * 30 + u64::from(id % 8) * 30);
        }
    }
}

fn draw(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

#[test]
fn routine_projection_matches_100000_recorded_steps() {
    for seed in [1, 74, 0x12345678, u64::MAX] {
        let mut rng = seed;
        let mut app = Resident::default();
        let mut probe = Probe::default();
        let mut now = 0u64;
        for step in 0..25_000 {
            let bits = draw(&mut rng);
            now = now.saturating_add(bits % 1_201);
            let action = match step % 9 {
                0 => Action::Enter(Phase::from_u8((bits % 4) as u8).unwrap()),
                1 => Action::AdvanceNeeds,
                2 => Action::BeginCycle { id: bits as u32 },
                3 => Action::RequestMeal {
                    hungry: bits & 1 != 0,
                    threshold: bits % 1_801,
                },
                4 => Action::MealCommitted,
                5 => Action::Observe {
                    present: bits & 1 != 0,
                    cap: bits % 61,
                },
                6 => Action::Interrupt,
                7 => Action::Rested,
                _ => Action::Socialized {
                    id: bits as u32,
                    partner: (bits >> 32) as u32,
                },
            };
            let before = snapshot(&app);
            legacy_apply(&mut app, now, action);
            assert!(
                probe.observe(&before, now, action, &snapshot(&app)),
                "seed={seed} step={step} {:?}",
                probe.first
            );
        }
        assert_eq!((probe.checked, probe.mismatches), (25_000, 0));
    }
}

#[test]
fn saturated_counters_and_tick_boundaries_match() {
    for now in [0, 1, 29, 30, u64::MAX - 1, u64::MAX] {
        let mut app = Resident {
            fatigue: u64::MAX,
            social_need: u64::MAX,
            rests: u32::MAX,
            socials: u32::MAX,
            interrupted: u32::MAX,
            ..Resident::default()
        };
        let mut probe = Probe::default();
        for action in [
            Action::AdvanceNeeds,
            Action::BeginCycle { id: u32::MAX },
            Action::Enter(Phase::Meal),
            Action::MealCommitted,
            Action::Observe {
                present: true,
                cap: PULSE,
            },
            Action::Rested,
            Action::Socialized {
                id: u32::MAX,
                partner: 1,
            },
            Action::Interrupt,
        ] {
            let before = snapshot(&app);
            legacy_apply(&mut app, now, action);
            assert!(
                probe.observe(&before, now, action, &snapshot(&app)),
                "{:?}",
                probe.first
            );
        }
    }
}

#[test]
fn probe_records_first_mismatch_without_mutating_input() {
    let before = Routine::default();
    let mut after = before.clone();
    after.rests = 1;
    let preserved = after.clone();
    let mut probe = Probe::default();
    assert!(!probe.observe(&before, 10, Action::MealCommitted, &after));
    assert!(!probe.observe(&before, 20, Action::MealCommitted, &after));
    assert_eq!(after, preserved);
    assert_eq!(before, Routine::default());
    assert_eq!((probe.checked, probe.mismatches), (2, 2));
    let mismatch = probe.first.unwrap();
    assert_eq!((mismatch.sample, mismatch.now), (1, 10));
    assert_eq!(mismatch.observed.rests, 1);
    assert_eq!(mismatch.expected.rests, 0);
}

#[test]
fn legacy_meeting_rule_matches_20000_observations() {
    let mut rng = 74;
    let mut now = 0u64;
    let mut expected = legacy::policy::Meeting {
        ids: [1, 2],
        points: [[0; 3], [1; 3]],
        started: 0,
        last: 0,
        dwell: 0,
        done: [false; 2],
    };
    let mut candidate = Meeting::new([1, 2], 0);
    for i in 0..20_000 {
        let bits = draw(&mut rng);
        now += bits % 121;
        let present = !bits.is_multiple_of(8);
        expected.observe(now, present, 30, 360);
        candidate.observe(now, present, 30, 360, false);
        assert_eq!(
            (candidate.last, candidate.dwell),
            (expected.last, expected.dwell),
            "step={i}"
        );
        if i % 100 == 99 {
            expected.dwell = 0;
            candidate.dwell = 0;
        }
    }
}

#[test]
fn exact_meeting_mode_is_explicitly_not_legacy_equivalence() {
    let mut app = legacy::policy::Meeting {
        ids: [1, 2],
        points: [[0; 3], [1; 3]],
        started: 0,
        last: 0,
        dwell: 0,
        done: [false; 2],
    };
    let mut candidate = Meeting::new([1, 2], 0);
    app.observe(500, true, 30, 360);
    candidate.observe(500, true, 30, 360, true);
    assert_eq!((app.dwell, candidate.dwell), (30, 0));
}

#[test]
fn meal_request_guards_match_at_each_boundary() {
    for phase in [Phase::Work, Phase::Meal, Phase::Rest, Phase::Social] {
        for now in [0, 1, 30, 1_800, u64::MAX] {
            for due in [now.saturating_sub(1), now, now.saturating_add(1)] {
                for fatigue in [1_799, 1_800, 1_801] {
                    for hungry in [false, true] {
                        let mut app = Resident {
                            phase: app_phase(phase),
                            due,
                            fatigue,
                            target: Some([1, 2, 3]),
                            dwell: 25,
                            ..Resident::default()
                        };
                        let before = snapshot(&app);
                        let action = Action::RequestMeal {
                            hungry,
                            threshold: 1_800,
                        };
                        legacy_apply(&mut app, now, action);
                        assert!(Probe::default().observe(&before, now, action, &snapshot(&app)));
                        if before.phase == Phase::Work && now >= due && fatigue >= 1_800 && hungry {
                            assert_eq!(app.target, None);
                        }
                    }
                }
            }
        }
    }
}
