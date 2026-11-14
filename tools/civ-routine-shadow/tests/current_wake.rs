use axiom_civ_routine_shadow::{
    candidate::wake::{Facts, next_wake},
    routine::Phase,
};

// The function body is extracted unchanged from the current crew.rs at build time.
// Stubs model supplied fields only, not physical state collection or execution.
#[allow(private_interfaces)]
mod current {
    use super::{Facts, Phase};
    use std::{collections::BTreeMap, ops::Deref};
    mod routine {
        pub use axiom_civ_routine_shadow::routine::Phase;
    }
    mod vehicle {
        #[derive(Clone, Copy)]
        pub struct ResidentBody(pub bool);
        impl ResidentBody {
            pub fn needs_tick(self) -> bool {
                self.0
            }
        }
    }
    #[derive(Clone, Copy)]
    struct Activity {
        end: u64,
    }
    struct Routine {
        phase: Phase,
        due: u64,
    }
    struct Worker {
        worker: Option<u32>,
        activity: Option<Activity>,
        navigation_wait: bool,
        retry_at: u64,
        motion_at: u64,
        body: Option<vehicle::ResidentBody>,
        task: Option<()>,
        path: Vec<()>,
        carry: Option<()>,
        batch: bool,
        mission: bool,
        routine: Routine,
    }
    struct Production {
        next_meal: BTreeMap<u32, u64>,
    }
    struct State {
        lead: Worker,
        helpers: Vec<Worker>,
        homes: BTreeMap<u32, ()>,
        production: Option<Production>,
    }
    impl Deref for State {
        type Target = Worker;
        fn deref(&self) -> &Worker {
            &self.lead
        }
    }
    fn view(j: &State, id: u32) -> Option<&Worker> {
        if j.worker == Some(id) {
            Some(&j.lead)
        } else {
            j.helpers.iter().find(|w| w.worker == Some(id))
        }
    }
    include!(concat!(env!("OUT_DIR"), "/current_next_wake.rs"));

    pub fn evaluate(now: u64, facts: Facts, helper: bool) -> u64 {
        let worker = |id| Worker {
            worker: facts.present.then_some(id),
            activity: facts.activity_end.map(|end| Activity { end }),
            navigation_wait: facts.navigation_wait,
            retry_at: facts.retry_at,
            motion_at: facts.motion_at,
            body: Some(vehicle::ResidentBody(facts.body_needs_tick)),
            task: facts.has_task.then_some(()),
            path: if facts.has_path { vec![()] } else { Vec::new() },
            carry: facts.carrying.then_some(()),
            batch: facts.batch,
            mission: facts.mission,
            routine: Routine {
                phase: facts.phase,
                due: facts.routine_due,
            },
        };
        let j = State {
            lead: worker(if helper { 99 } else { 7 }),
            helpers: if helper { vec![worker(7)] } else { Vec::new() },
            homes: if facts.has_homes {
                BTreeMap::from([(99, ())])
            } else {
                BTreeMap::new()
            },
            production: facts.next_meal.map(|meal| Production {
                next_meal: BTreeMap::from([(7, meal)]),
            }),
        };
        next_wake(&j, 7, now)
    }
}

fn check(now: u64, facts: Facts) {
    for helper in [false, true] {
        assert_eq!(
            next_wake(now, facts),
            current::evaluate(now, facts, helper),
            "now={now} helper={helper} facts={facts:?}"
        );
    }
}

#[test]
fn precedence_and_saturation_match_compiled_app_wake() {
    for now in [
        0u64,
        1,
        29,
        30,
        31,
        100,
        6000,
        u64::MAX - 30,
        u64::MAX - 1,
        u64::MAX,
    ] {
        for time in [
            0,
            now.saturating_sub(31),
            now.saturating_sub(30),
            now,
            now.saturating_add(1),
            now.saturating_add(30),
            now.saturating_add(6000),
        ] {
            for phase in [Phase::Work, Phase::Meal, Phase::Rest, Phase::Social] {
                let base = Facts {
                    present: true,
                    phase,
                    has_homes: true,
                    retry_at: time,
                    routine_due: time,
                    next_meal: Some(time),
                    ..Facts::default()
                };
                check(now, base);
                check(
                    now,
                    Facts {
                        present: false,
                        activity_end: Some(time),
                        ..base
                    },
                );
                check(
                    now,
                    Facts {
                        activity_end: Some(time),
                        has_path: true,
                        motion_at: 0,
                        ..base
                    },
                );
                check(
                    now,
                    Facts {
                        has_path: true,
                        motion_at: time,
                        body_needs_tick: true,
                        ..base
                    },
                );
                check(
                    now,
                    Facts {
                        body_needs_tick: true,
                        ..base
                    },
                );
                check(
                    now,
                    Facts {
                        has_task: true,
                        ..base
                    },
                );
                check(
                    now,
                    Facts {
                        carrying: true,
                        ..base
                    },
                );
                check(
                    now,
                    Facts {
                        batch: true,
                        ..base
                    },
                );
                check(
                    now,
                    Facts {
                        mission: true,
                        ..base
                    },
                );
            }
        }
    }
}

#[test]
fn wake_candidate_matches_200000_compiled_app_evaluations() {
    for seed in [1u64, 74, 9127, u64::MAX] {
        let mut rng = seed;
        for step in 0..25_000u64 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let now = step * 13;
            let future = now + rng % 8000;
            let past = now.saturating_sub((rng >> 32) % 80);
            let mut facts = Facts {
                present: rng & 1 != 0,
                activity_end: (rng & 2 != 0).then_some(if rng & 4 != 0 { future } else { past }),
                navigation_wait: rng & 65536 != 0,
                has_path: rng & 8 != 0,
                motion_at: past,
                body_needs_tick: rng & 16 != 0,
                has_task: rng & 32 != 0,
                carrying: rng & 64 != 0,
                batch: rng & 128 != 0,
                mission: rng & 256 != 0,
                phase: Phase::from_u8((rng >> 10) as u8 % 4).unwrap(),
                has_homes: rng & 512 != 0,
                retry_at: if rng & 4096 != 0 { future } else { past },
                routine_due: if rng & 8192 != 0 { future } else { past },
                next_meal: (rng & 16384 != 0).then_some(if rng & 32768 != 0 {
                    future / 2
                } else {
                    past
                }),
            };
            if step % 3 == 0 {
                facts.present = true;
                facts.activity_end = None;
                facts.navigation_wait = false;
                facts.has_path = false;
                facts.body_needs_tick = false;
                facts.has_task = false;
                facts.carrying = false;
                facts.batch = false;
                facts.mission = false;
                facts.phase = Phase::Work;
            }
            check(now, facts);
        }
    }
}

#[test]
fn navigation_wait_respects_activity_and_missing_worker_precedence() {
    for now in [0u64, 100, u64::MAX - 1, u64::MAX] {
        let waiting = Facts {
            present: true,
            navigation_wait: true,
            has_path: true,
            motion_at: 0,
            body_needs_tick: true,
            next_meal: Some(now.saturating_add(1)),
            ..Facts::default()
        };
        check(now, waiting);
        assert_eq!(next_wake(now, waiting), now.saturating_add(600));
        check(
            now,
            Facts {
                present: false,
                ..waiting
            },
        );
        check(
            now,
            Facts {
                activity_end: Some(now.saturating_add(5)),
                ..waiting
            },
        );
    }
}
