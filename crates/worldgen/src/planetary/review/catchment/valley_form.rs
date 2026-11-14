//! Connected valley widening and sediment storage on the solved bed.
use super::{Budget, Material, SPAN_M, Stage, bedding, erosion, fracture_weakness};
use crate::planetary::tectonics::smooth;

/// Downstream memory of a resistant-to-weak transition. A strong lip can
/// concentrate erosion below it while the flow is still in the same reach.
fn plunge_load(stage: &Stage, flow: &erosion::Flow, area: &[u32], material: Material) -> Vec<f32> {
    let grid = stage.grid;
    let minimum = (45_000.0 / grid.cell_m.powi(2)).ceil() as u32;
    let mut wear = vec![0.0_f32; grid.len()];
    for (i, value) in wear.iter_mut().enumerate() {
        if area[i] < minimum {
            continue;
        }
        let [x, z] = grid.at(i);
        *value = (0.76 * bedding(material, f64::from(stage.rock[i]), x, z)
            + 0.24 * fracture_weakness(x, z)) as f32;
    }
    let mut plunge = vec![0.0_f32; grid.len()];
    let mut upstream_rock = wear.clone();
    for &i in flow.order.iter().rev() {
        let i = i as usize;
        let r = flow.receiver[i] as usize;
        if area[i] < minimum || i == r {
            continue;
        }
        let [x, z] = grid.at(i);
        let [rx, rz] = grid.at(r);
        let edge = (x - rx).hypot(z - rz);
        let upstream =
            f64::from(wear[i]) + f64::from(upstream_rock[i] - wear[i]) / (1.0 + edge / 25.0);
        let share = (f64::from(area[i]) / f64::from(area[r])).sqrt();
        let contrast = (f64::from(wear[r]) - upstream).max(0.0) * share;
        upstream_rock[r] = upstream_rock[r].min((f64::from(wear[r]) - contrast) as f32);
        let transition = smooth(0.025, 0.21, contrast);
        let drop = smooth(0.004, 0.072, (stage.height[i] - stage.height[r]) / edge);
        // The weak side of a contact receives the signal. At confluences a
        // smaller donor cannot apply the full cut to the larger channel.
        let transfer = (transition * drop).max(f64::from(plunge[i]) / (1.0 + edge / 38.0));
        plunge[r] = plunge[r].max((transfer * share) as f32);
    }
    plunge
}

#[derive(Clone, Copy)]
struct Reach {
    floor: f32,
    distance: f32,
    width: f32,
    cut: f32,
}

pub(super) fn evolve(
    stage: &mut Stage,
    flow: &erosion::Flow,
    material: Material,
    seed: u32,
    budget: &mut Budget,
) -> Vec<u8> {
    let grid = stage.grid;
    let cell_area = grid.cell_m.powi(2);
    let probe = (48.0 / grid.cell_m).round() as usize;
    let opposing = [
        probe,
        grid.n * probe,
        (grid.n - 1) * probe,
        (grid.n + 1) * probe,
    ];
    // Use physical contributing area for valley size. Rainfall controls the
    // preceding incision solve and can vary across the same watershed.
    let mut area = vec![1_u32; grid.len()];
    for &i in flow.order.iter().rev() {
        let i = i as usize;
        let r = flow.receiver[i] as usize;
        if i != r {
            area[r] += area[i];
        }
    }
    let plunge = material
        .sill_erosion
        .then(|| plunge_load(stage, flow, &area, material));
    let channel_area = if material.sill_erosion {
        45_000.0
    } else {
        100_000.0
    };
    let none = Reach {
        floor: 0.0,
        distance: f32::INFINITY,
        width: 0.0,
        cut: 0.0,
    };
    let mut reach = vec![none; grid.len()];
    let mut lateral_cut = vec![0.0_f32; grid.len()];
    stage.flux.fill(0.0);
    let mut transported = 0.0;
    for &i in &flow.order {
        let i = i as usize;
        let r = flow.receiver[i] as usize;
        if i == r || stage.fixed[i] {
            continue;
        }
        let [x, z] = grid.at(i);
        let [rx, rz] = grid.at(r);
        let edge = (x - rx).hypot(z - rz);
        let rim = x.min(z).min(SPAN_M - x).min(SPAN_M - z);
        let fade = smooth(340.0, 630.0, rim);
        if fade == 0.0 {
            continue;
        }

        let a = f64::from(area[i]) * cell_area;
        let grade = ((stage.height[i] - stage.height[r]) / edge).max(0.0);
        if a >= channel_area && flow.spill[i] - stage.height[i] < 0.8 {
            let floor = stage.height[i];
            let relief = opposing
                .into_iter()
                .map(|d| stage.height[i - d].min(stage.height[i + d]))
                .fold(floor, f64::max)
                - floor;
            // Filled flats have receivers even when there is no valley to cut.
            let supported = smooth(0.35, 1.6, relief);
            if supported == 0.0 {
                continue;
            }
            let soft = smooth(
                0.50,
                1.55,
                bedding(material, f64::from(stage.rock[i]), x, z),
            );
            let scale = if material.sill_erosion {
                smooth(230_000.0, 400_000.0, a)
            } else {
                smooth(100_000.0, 340_000.0, a)
            };
            let opening = if material.sill_erosion {
                smooth(280_000.0, 490_000.0, a) * (1.0 - smooth(0.027, 0.14, grade))
            } else {
                0.0
            };
            let width = 19.0 * opening
                + 16.0
                + (34.0 + 22.0 * soft) * scale * (1.0 - 0.75 * smooth(0.07, 0.32, grade));
            // Weak beds retreat into pools. Strong ribs remain as steps.
            // Bound the cut so the floor can still drain to its receiver.
            let available = (stage.height[i] - stage.height[r] + f64::from(reach[r].cut)
                - edge * 0.001)
                .max(0.0);
            let jointed = 0.65 * soft + 0.35 * fracture_weakness(x, z);
            let depth = if let Some(plunge) = &plunge {
                let headward = 0.9
                    * smooth(channel_area, 105_000.0, a)
                    * (1.0 - smooth(180_000.0, 300_000.0, a));
                let pool = 7.5 * f64::from(plunge[i]) * smooth(110_000.0, 275_000.0, a);
                headward + 3.2 * jointed * smooth(100_000.0, 185_000.0, a) + pool
            } else {
                (0.20 + 6.5 * jointed) * smooth(100_000.0, 185_000.0, a)
            };
            let cut = (depth * fade * supported).min(available);
            reach[i] = Reach {
                floor: stage.height[i] as f32,
                distance: 0.0,
                width: width as f32,
                cut: cut as f32,
            };
            if scale > 0.3 && width > 30.0 {
                let (ix, iz) = ((i % grid.n) as isize, (i / grid.n) as isize);
                let reach_cells = (width / grid.cell_m).ceil() as isize;
                for jz in -reach_cells..=reach_cells {
                    for jx in -reach_cells..=reach_cells {
                        let (mx, mz) = (jx as f64 * grid.cell_m, jz as f64 * grid.cell_m);
                        let along = (mx * (rx - x) + mz * (rz - z)).abs() / edge;
                        if along >= 13.0 {
                            continue;
                        }
                        let distance = mx.hypot(mz);
                        if !(5.0..width).contains(&distance) {
                            continue;
                        }
                        let j = (iz + jz) as usize * grid.n + (ix + jx) as usize;
                        if f64::from(area[j]) * cell_area >= 100_000.0 {
                            continue;
                        }
                        let bank = stage.height[j] - stage.height[i];
                        let shoulder = smooth(0.2, 3.5, bank) * (1.0 - smooth(10.0, 19.0, bank));
                        let edge_fade = 1.0 - smooth(width * 0.78, width, distance);
                        let target =
                            stage.height[i] - cut + 0.9 + (0.035 - 0.008 * opening) * distance;
                        let shave = (stage.height[j] - target).clamp(0.0, 8.5)
                            * shoulder
                            * edge_fade
                            * fade
                            * supported;
                        lateral_cut[j] = lateral_cut[j].max(shave as f32);
                    }
                }
            }
        } else {
            let down = reach[r];
            let distance = down.distance + edge as f32;
            if distance >= down.width {
                continue;
            }
            reach[i] = Reach { distance, ..down };
        }

        let here = reach[i];
        let dist = f64::from(here.distance);
        let floor = f64::from(here.floor);
        let rise = stage.height[i] - floor;
        let cut = if dist == 0.0 {
            f64::from(here.cut)
        } else {
            let bank = smooth(2.0, 11.0, rise) * (1.0 - smooth(11.0, 23.0, rise));
            let shoulder = smooth(2.0, 10.0, dist)
                * (1.0 - smooth(f64::from(here.width) * 0.55, f64::from(here.width), dist));
            (rise - 0.044 * dist - 1.7).clamp(0.0, 6.0) * bank * shoulder * fade
        };
        lateral_cut[i] = lateral_cut[i].max(cut as f32);
    }
    for (i, &cut) in lateral_cut.iter().enumerate() {
        if cut <= 0.0 {
            continue;
        }
        let cut = f64::from(cut);
        let old_rock = f64::from(stage.rock[i]);
        let old_soil = f64::from(stage.soil[i]);
        stage.soil[i] = (old_soil - cut).max(0.0) as f32;
        stage.rock[i] = (old_rock - (cut - old_soil).max(0.0)) as f32;
        let rock = old_rock - f64::from(stage.rock[i]);
        stage.flux[i] = rock + old_soil - f64::from(stage.soil[i]);
        transported += stage.flux[i];
        budget.eroded_rock_m3 += rock * cell_area;
    }

    let mut deposits = vec![0_u8; grid.len()];
    let mut stored = 0.0;
    for &i in flow.order.iter().rev() {
        let i = i as usize;
        let r = flow.receiver[i] as usize;
        let mut load = stage.flux[i];
        if load <= 0.0 || i == r {
            continue;
        }
        let d = f64::from(reach[i].distance);
        let drop = stage.height[i] - stage.height[r];
        if (5.0..43.0).contains(&d) && (0.0..grid.cell_m * 0.24).contains(&drop) {
            // Angular patches are supplied by local bank failure. Other
            // material settles as shallow sand in gentler pockets.
            let clasts = crate::hash2((i % grid.n) as i32, (i / grid.n) as i32, seed) > 0.78;
            let room =
                if clasts { 1.7 } else { 0.62 } * (1.0 - smooth(0.0, grid.cell_m * 0.24, drop));
            let soil = f64::from(stage.soil[i]);
            stage.soil[i] = (soil + load.min(room)) as f32;
            let retained = f64::from(stage.soil[i]) - soil;
            load -= retained;
            stored += retained;
            if retained > 0.11 {
                deposits[i] = if clasts { 2 } else { 1 };
            }
        }
        stage.flux[r] += load;
    }
    budget.exported_sediment_m3 += (transported - stored) * cell_area;
    eprintln!(
        "[worldgen] valley_storage detached_m3={:.1} stored_m3={:.1}",
        transported * cell_area,
        stored * cell_area
    );
    deposits
}
