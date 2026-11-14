//! Kernel micro-benchmarks. Numbers are per operation on one core.

use criterion::{Criterion, criterion_group, criterion_main};
use engine_civ::agenda::{Key, Ordered, Queue, Wheel};
use engine_civ::draw::draw;
use std::hint::black_box;

fn keyed_draw(c: &mut Criterion) {
    c.bench_function("draw_keyed", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            black_box(draw(0xA11CE, 3, i, 412))
        })
    });
}

/// Pop the minimum and reschedule it, the steady-state agenda operation.
fn churn<Q: Queue>(c: &mut Criterion, name: &str, people: u32) {
    let mut q = Q::default();
    for p in 0..people {
        q.insert(Key::new(draw(1, 0, p as u64, 0) % 4096, p / 64, p));
    }
    c.bench_function(name, |b| {
        b.iter(|| {
            let k = q.pop_due(u64::MAX).expect("populated");
            q.insert(Key::new(k.due + 1 + draw(1, 1, k.actor as u64, k.due) % 4096, k.site, k.actor));
            black_box(k)
        })
    });
}

fn agenda(c: &mut Criterion) {
    churn::<Ordered>(c, "agenda_ordered_churn_64", 64);
    churn::<Ordered>(c, "agenda_ordered_churn_100k", 100_000);
    churn::<Ordered>(c, "agenda_ordered_churn_1m", 1_000_000);
    churn::<Wheel>(c, "agenda_wheel_churn_64", 64);
    churn::<Wheel>(c, "agenda_wheel_churn_100k", 100_000);
    churn::<Wheel>(c, "agenda_wheel_churn_1m", 1_000_000);
}

criterion_group!(benches, keyed_draw, agenda);
criterion_main!(benches);
