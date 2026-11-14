use super::config::CONFIG;
use super::*;
use engine_core::hash::hash_u32;

fn share(mass: u64, fraction: f32, salt: u32) -> u64 {
    let exact = mass as f64 * f64::from(fraction.clamp(0.0, 0.85));
    let base = exact.floor() as u64;
    let round = f64::from(hash_u32(salt)) / f64::from(u32::MAX);
    (base + u64::from(round < exact.fract())).min(mass)
}

pub fn step(world: &World, state: &mut State, day: &DayNightCycle, dt: f32) {
    refine(world, state);
    let fine: Vec<_> = state.air.keys().copied().filter(|k| k.3 == 1).collect();
    for k in fine {
        let to = geometry::owner(world, state, pos(k));
        if to != k {
            if let Some(m) = state.air.remove(&k) {
                let dst = state.air.entry(to).or_default();
                for i in 0..SPECIES {
                    dst[i] += m[i];
                }
            }
        }
    }
    // Geometry queries use the derived caches, not the live air map. Move
    // the old map into the read snapshot instead of cloning it twice.
    let before = std::mem::take(&mut state.air);
    let mut next = before.clone();
    let scale = world.voxel_scale.meters();
    let controls = day.cloud_controls();
    let clock = super::super::WeatherClock::new(
        &controls,
        (day.day_index() as f32 + day.time_of_day()) * day.day_length_seconds(),
        super::super::synoptic::days(day),
    );
    let mut rainfall = BTreeMap::new();
    for (&k, mass) in &before {
        let size = k.3 as f32 * scale;
        let rain_key = (k.0.div_euclid(32), k.2.div_euclid(32));
        // Keep the first cell's sample point even if that cell cannot deposit.
        let rain_sample = rainfall.entry(rain_key).or_insert((k.0, k.2, None));
        if k.3 > 1 && geometry::owner(world, state, pos(k)) != k {
            continue;
        }
        let salt = world.seed
            ^ (state.tick as u32).wrapping_mul(0x9e3779b9)
            ^ (k.0 as u32).wrapping_mul(0x85ebca6b)
            ^ (k.1 as u32).rotate_left(11)
            ^ (k.2 as u32).rotate_left(22);
        let mut remaining = *mass;
        for side in 0..6 {
            if next.len() < MAX_CELLS && remaining.iter().all(|&m| m == 0) {
                break;
            }
            let direction = super::super::factory::DIRS[side].as_vec3();
            let neighbours = geometry::neighbours(world, state, k, side);
            for (to, area) in neighbours {
                if to == k {
                    continue;
                }
                if !next.contains_key(&to) && next.len() >= MAX_CELLS {
                    state.blocked = state.blocked.saturating_add(1);
                    continue;
                }
                let to_size = to.3 as f32 * scale;
                let distance = (size + to_size) * 0.5;
                let dest = before.get(&to);
                let volume_ratio = (size / to_size).powi(3) as f64;
                for i in 0..SPECIES {
                    if remaining[i] == 0 {
                        continue;
                    }
                    let species = &CONFIG.species[i];
                    let velocity = glam::Vec3::new(state.wind[0], species.rise_m_s, state.wind[1]);
                    let advection = velocity.dot(direction).max(0.0) * dt / size;
                    let dest_mass = dest.map_or(0.0, |m| m[i] as f64 * volume_ratio);
                    let gradient = ((mass[i] as f64 - dest_mass).max(0.0) / mass[i] as f64) as f32;
                    let diffusion = CONFIG.diffusion_m2_s * dt / (size * distance) * gradient;
                    let moved = share(
                        mass[i],
                        (advection + diffusion) * area,
                        salt ^ (side * 7 + i) as u32,
                    )
                    .min(remaining[i]);
                    if moved == 0 {
                        continue;
                    }
                    next.entry(to).or_default()[i] += moved;
                    remaining[i] -= moved;
                }
            }
        }
        // Gas that cannot deposit does not need a ray to the ground or a
        // weather sample. Keep decay and transport identical for those cells.
        let ground = remaining.iter().zip(&CONFIG.species)
            .any(|(&m, species)| m > 0 && species.deposition_s > 0.0)
            .then(|| geometry::ground(world, state, k)).flatten();
        let rain = ground.map_or(0.0, |_| {
            let (x, z, sample) = rain_sample;
            *sample.get_or_insert_with(|| {
                let q = glam::Vec2::new(*x as f32, *z as f32) * scale;
                super::super::precipitation_at(q, clock, controls, day.weather_seed())
            })
        });
        for i in 0..SPECIES {
            let species = &CONFIG.species[i];
            let decay =
                share(mass[i], species.decay_s * dt, salt ^ 0x3120 ^ i as u32).min(remaining[i]);
            if decay > 0 {
                remaining[i] -= decay;
                state.transformed[i] = state.transformed[i].saturating_add(decay);
            }
            let Some(p) = ground else {
                continue;
            };
            let near = (pos(k).y - p.y) as f32 * scale <= size + scale;
            let rate = species.deposition_s * if near { 1.0 } else { 0.0 }
                + if species.deposition_s > 0.0 {
                    rain * CONFIG.rain_capture_s
                } else {
                    0.0
                };
            let deposit = share(mass[i], rate * dt, salt ^ 0x7540 ^ i as u32).min(remaining[i]);
            let metre = super::super::factory::step(world);
            let p = p.div_euclid(IVec3::splat(metre)) * metre;
            let target = (p.x, p.y, p.z);
            if deposit > 0
                && (state.deposits.contains_key(&target) || state.deposits.len() < MAX_DEPOSITS)
            {
                remaining[i] -= deposit;
                state.deposits.entry(target).or_default()[i] += deposit;
            }
        }
        // Apply one debit per source. Incoming transfers already in `next`
        // remain available for the following tick, as with the old per-face debits.
        if remaining != *mass {
            let source = next.get_mut(&k).expect("air snapshot source exists in next map");
            for i in 0..SPECIES {
                source[i] -= mass[i] - remaining[i];
            }
        }
    }
    next.retain(|_, m| total(m) > 0);
    state.air = next;
}

fn refine(world: &World, state: &mut State) {
    let keys: Vec<_> = state.air.keys().copied().filter(|k| k.3 > 1).collect();
    for k in keys {
        if geometry::owner(world, state, pos(k)) == k {
            continue;
        }
        let count = (k.3 * k.3 * k.3) as usize;
        if state.air.len() + count > MAX_CELLS {
            continue;
        }
        let Some(mass) = state.air.remove(&k) else {
            continue;
        };
        let mut index = 0;
        for y in 0..k.3 {
            for z in 0..k.3 {
                for x in 0..k.3 {
                    let m = state.air.entry((k.0 + x, k.1 + y, k.2 + z, 1)).or_default();
                    for i in 0..SPECIES {
                        m[i] += mass[i] / count as u64
                            + u64::from((index as u64) < mass[i] % count as u64);
                    }
                    index += 1;
                }
            }
        }
    }
}
