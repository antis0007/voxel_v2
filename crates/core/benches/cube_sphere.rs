//! Cost of the three cube-sphere projections: one chunk of columns out, 1,024 directions back.
use criterion::{Criterion, criterion_group, criterion_main};
use engine_core::cube_sphere::{Face, FaceGrid, Projection};
use glam::{DVec3, IVec3};
use std::hint::black_box;
use std::time::Duration;

const PROJECTIONS: [(&str, Projection); 3] = [
    ("gnomonic", Projection::Gnomonic),
    ("tangent", Projection::Tangent),
    ("spherified", Projection::Spherified),
];

/// Face coordinates of the 32 x 32 columns of one chunk away from the face centre.
fn chunk_columns() -> Vec<(f64, f64)> {
    let grid = FaceGrid::new(16_000).expect("16,000 is a multiple of 32");
    let mut columns = Vec::with_capacity(32 * 32);
    for z in -1600..-1568 {
        for x in 3200..3232 {
            columns.push(grid.cell_face_coords(IVec3::new(x, 0, z)));
        }
    }
    columns
}

/// 1,024 directions spread evenly over the whole sphere, so every face is hit.
fn directions() -> Vec<DVec3> {
    const COUNT: usize = 1024;
    let golden_angle = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    (0..COUNT)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / COUNT as f64;
            let ring = (1.0 - y * y).sqrt();
            let phi = golden_angle * i as f64;
            DVec3::new(ring * phi.cos(), y, ring * phi.sin())
        })
        .collect()
}

fn projections(c: &mut Criterion) {
    let columns = chunk_columns();
    let dirs = directions();
    let mut group = c.benchmark_group("cube_sphere");
    for (name, p) in PROJECTIONS {
        group.bench_function(format!("to_sphere_32x32_{name}"), |b| {
            b.iter(|| {
                let mut sum = DVec3::ZERO;
                for &(s, t) in &columns {
                    sum += p.to_sphere(Face::PosY, black_box(s), black_box(t));
                }
                black_box(sum)
            })
        });
        group.bench_function(format!("from_sphere_1024_{name}"), |b| {
            b.iter(|| {
                let mut sum = 0.0;
                for &d in &dirs {
                    let (_, s, t) = p.from_sphere(black_box(d));
                    sum += s + t;
                }
                black_box(sum)
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    targets = projections
}
criterion_main!(benches);
