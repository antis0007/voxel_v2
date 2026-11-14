use axiom_civ_routine_shadow::candidate::Wheel as Candidate;
use engine_civ::agenda::{Key, Ordered, Queue, Wheel};
use std::{hint::black_box, time::Instant};

fn run<Q: Queue>(keys: &[Key], repeats: usize) -> (u128, u64) {
    let started = Instant::now();
    let mut digest = 0u64;
    for _ in 0..repeats {
        let mut queue = Q::default();
        for &key in keys {
            queue.insert(black_box(key));
        }
        while let Some(key) = queue.pop_due(u64::MAX) {
            digest = digest
                .wrapping_mul(31)
                .wrapping_add(key.due ^ u64::from(key.site) ^ u64::from(key.actor));
        }
        black_box(queue.len());
    }
    (started.elapsed().as_nanos(), black_box(digest))
}

fn measure(keys: &[Key], name: &str, measure_kernel: bool) {
    let reference = run::<Ordered>(keys, 1).1;
    assert_eq!(run::<Candidate>(keys, 1).1, reference);
    if measure_kernel {
        assert_eq!(run::<Wheel>(keys, 1).1, reference);
    }
    let (mut ordered, mut candidate, mut kernel) = (Vec::new(), Vec::new(), Vec::new());
    // Rotate all implementations to reduce order bias on the shared machine.
    for sample in 0..9 {
        if measure_kernel {
            for slot in 0..3 {
                match (sample + slot) % 3 {
                    0 => ordered.push(run::<Ordered>(keys, 1).0),
                    1 => candidate.push(run::<Candidate>(keys, 1).0),
                    _ => kernel.push(run::<Wheel>(keys, 1).0),
                }
            }
        } else if sample % 2 == 0 {
            candidate.push(run::<Candidate>(keys, 1).0);
            ordered.push(run::<Ordered>(keys, 1).0);
        } else {
            ordered.push(run::<Ordered>(keys, 1).0);
            candidate.push(run::<Candidate>(keys, 1).0);
        }
    }
    ordered.sort_unstable();
    candidate.sort_unstable();
    kernel.sort_unstable();
    println!(
        "{{\"case\":\"{name}\",\"inserts\":{},\"samples\":9,\"ordered_median_ns\":{},\"candidate_median_ns\":{},\"kernel_median_ns\":{}}}",
        keys.len(),
        ordered[4],
        candidate[4],
        kernel.get(4).map_or("null".into(), ToString::to_string)
    );
}

fn main() {
    for (name, count, span) in [
        ("same_tick_8192", 8192u32, 1),
        ("spread_8192", 8192, 4096),
        ("spread_65536", 65536, 16384),
    ] {
        let keys: Vec<_> = (0..count)
            .map(|i| {
                let actor = i.wrapping_mul(2654435761);
                Key::new(u64::from(i % span), i % 64, actor)
            })
            .collect();
        measure(&keys, name, true);
    }
    let keys: Vec<_> = (0..8192u32)
        .flat_map(|i| [Key::new(u64::from(i % 30), i % 64, i); 3])
        .collect();
    measure(&keys, "duplicate_burst_8192", true);
}
