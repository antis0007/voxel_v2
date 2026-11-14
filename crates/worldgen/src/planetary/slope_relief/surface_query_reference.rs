//! Frozen V13 Stage; renamed entry/path qualifiers and literal old Ground/history.
use super::*;
impl Stage {
    pub(super) fn solve_surface_query_reference(world: &World, cfg: &WorldSampleConfig, key: Key, spacing: f64, passes: usize, parent: Option<&Self>, distant: bool) -> Self {
        let focused = crate::planetary::watershed_erosion(cfg.generator_version);
        let composed = crate::planetary::composed_drainage(cfg.generator_version);
        let connected = crate::planetary::connected_relief(cfg.generator_version);
        let grid = Grid {
            n: ((CORE_M + 2.0 * PAD_M) / spacing) as usize,
            cell_m: spacing,
            min_m: [f64::from(key.x) * CORE_M - PAD_M, f64::from(key.z) * CORE_M - PAD_M],
        };
        let mesh = Mesh::with_alignment(grid, connected);
        let face = Face::ALL[key.face as usize];
        let mut base = Vec::with_capacity(grid.len());
        let mut rock = Vec::with_capacity(grid.len());
        let mut rain = Vec::with_capacity(grid.len());
        let mut fixed = Vec::with_capacity(grid.len());
        let mut strength = Vec::with_capacity(grid.len());
        let mut rock_limit = Vec::with_capacity(grid.len());
        let mut soil = Vec::with_capacity(grid.len());
        let mut uplift = Vec::with_capacity(grid.len());
        let strike = DVec3::new(0.35, -0.69, 0.63).normalize();
        let rifted = crate::planetary::rifted_margins(cfg.generator_version);
        let coupled = crate::planetary::coupled_history(cfg.generator_version);
        let canyons = cfg.generator_version == crate::planetary::LAYERED_CANYON_GENERATOR;
        let mut beds = canyons.then(|| Vec::with_capacity(grid.len()));
        let branching = crate::planetary::branching_watersheds(cfg.generator_version);
        let historical = crate::planetary::geological_history(cfg.generator_version);
        let evolved = crate::planetary::evolved_watersheds(cfg.generator_version);
        let regional = evolved.then(|| {
            let regions = if distant { &world.slope_tiles.distant_regions } else { &world.slope_tiles.regions };
            regions.window(world, cfg, face, grid)
        });
        let mut outlets = evolved.then(|| Vec::with_capacity(grid.len()));
        let mut history_beds = historical.then(|| Vec::with_capacity(grid.len()));
        let mut repose = vec![0.0; if canyons || historical { grid.len() } else { 0 }];
        for (i, point) in mesh.points.iter().enumerate() {
            let dir = Projection::Tangent.to_sphere(face, point.x / atlas::HALF_M, point.y / atlas::HALF_M);
            let volcanic = world.volcano_shape(dir);
            let intact = volcanic.active_caldera || world.edifice_lift(dir) > 12.0;
            // Every stage measures residual height against the same parent
            // surface. Coarse sampling must not reintroduce filtered river beds.
            let ground = world.ground_query_reference(cfg, dir, CELL_M as f32, volcanic);
            let openness = exposure(&ground, focused, rifted || connected, canyons, evolved);
            let boundary = grid.on_border(i) || openness == 0.0 || intact;
            let body = dir * atlas::RADIUS_M;
            let along = strike * body.dot(strike);
            let across = body - along;
            let joint = if branching { noise(cfg.seed, body / 240.0, 14383) }
                else { noise(cfg.seed, along / 168.0 + across / 61.0, 14383) };
            let lens = noise(cfg.seed, body / 186.0, 14387);
            let erodible = smooth(0.23, 0.76, 0.64 * joint + 0.36 * lens);
            let h = f64::from(ground.top_m);
            if let Some(outlets) = &mut outlets {
                outlets.push(ground.water_m.is_some() || h < SEA + 3.0);
            }
            let collision = world.mesh.sample(&world.collision, dir);
            if let Some(beds) = &mut beds { beds.push(world.strata(cfg, dir)); }
            if let Some(beds) = &mut history_beds { beds.push(world.history_query_reference(cfg, dir)); }
            let bedding = crate::planetary::bedding_height(cfg, body, collision, h);
            let weak_bed = if canyons || historical { 0.0 } else {
                1.0 - smooth(8.0, 20.0, (bedding - 6.0).abs().min(crate::planetary::BED_INTERVAL - (bedding - 6.0).abs()))
            };
            let history_soil = if coupled { world.mesh.sample(&world.sediment_cover, dir) } else { 0.0 };
            let (relief, cover) = if let Some(regional) = &regional {
                if openness == 0.0 || intact {
                    (-history_soil, history_soil)
                } else if let Some(old) = parent {
                    (old.grid.bilinear(&old.rock_delta, point.x, point.y),
                        old.grid.bilinear(&old.soil, point.x, point.y))
                } else {
                    regional.surface(*point)
                }
            } else if boundary {
                (-history_soil, history_soil)
            } else if let Some(old) = parent {
                let inherited = old.grid.bilinear(&old.rock_delta, point.x, point.y);
                let inherited_soil = old.grid.bilinear(&old.soil, point.x, point.y);
                let local = if rifted || connected { 0.0 } else { (noise(cfg.seed, body / (spacing * 4.0), 14393 + spacing as u32) - 0.5)
                    * spacing * 0.15 * openness };
                (inherited + local, inherited_soil)
            } else {
                (if rifted || connected { -history_soil } else { (8.0 * (joint - 0.5) + 18.0 * (lens - 0.5)) * openness }, history_soil)
            };
            base.push(h);
            rock.push(h + relief);
            soil.push(cover);
            rain.push(0.25 + 0.75 * f64::from(ground.moisture));
            fixed.push(boundary);
            let damage = if focused { world.mesh.sample(&world.fault_damage, dir) } else { 0.0 };
            strength.push(openness * (0.35 + 0.85 * erodible + 0.45 * weak_bed + 0.60 * damage));
            rock_limit.push(0.57 + 1.45 * (1.0 - erodible) * (1.0 - 0.73 * weak_bed) * (1.0 - 0.6 * damage));
            // Differential uplift follows broad rock blocks. The fine stage
            // inherits the new divides and carves them without further uplift.
            let fold = noise(cfg.seed, along / 285.0 + across / 112.0, 14389);
            let uplift_rate = if !rifted && parent.is_none() && !boundary {
                4.0 * openness.sqrt() * (0.53 + 0.47 * collision.clamp(0.0, 1.0))
                    * smooth(0.37, 0.74, fold) * (0.48 + 0.52 * (1.0 - weak_bed))
                    * smooth(24.0, 120.0, h)
            } else {
                0.0
            };
            uplift.push(uplift_rate);
        }
        let mut height = vec![0.0; grid.len()];
        let mut load = vec![0.0; grid.len()];
        let local_runoff: Vec<_> = if branching {
            rain.iter().zip(&mesh.area).map(|(rain, area)| rain * area).collect()
        } else { Vec::new() };
        let mut bank_support = if branching { vec![1.0_f64; grid.len()] } else { Vec::new() };
        if let Some(outlets) = &mut outlets {
            watershed::open_perimeter(grid, &rock, &soil, outlets);
        }
        let drainage_outlets = outlets.as_deref().unwrap_or(&fixed);
        let mut momentum = branching.then(|| vec![DVec2::ZERO; grid.len()]);
        if let Some(regional) = &regional {
            regional.supply(&mesh, &fixed, &mut rain, momentum.as_deref_mut());
        } else if coupled {
            inflow::add(world, face, &mesh, &fixed, &mut rain, None);
        }
        for _ in 0..passes {
            for i in 0..height.len() {
                rock[i] += uplift[i];
                height[i] = rock[i] + soil[i];
            }
            let flow = if let Some(momentum) = &momentum { mesh.route_momentum(&height, drainage_outlets, &rain, momentum) }
                else { mesh.route(&height, drainage_outlets, &rain) };
            load.fill(0.0);
            bank_support.fill(1.0);
            for &i in flow.order.iter().rev() {
                let i = i as usize;
                let routes = flow.links[i].as_slice();
                if fixed[i] {
                    if evolved { transport::forward(routes, load[i], &mut load); }
                    continue;
                }
                let grade = flow.grade[i];
                let runoff = (flow.q[i] / 900.0).sqrt();
                let resistance = history_beds.as_ref().map_or_else(
                    || beds.as_ref().map_or(1.0, |beds| beds[i].resistance(rock[i])),
                    |beds| beds[i].as_ref().map_or(1.0, |b| b.resistance(rock[i])));
                // Routing and other nodes have not changed this rock height.
                // Keep repose at the same pre-incision height; fixed-node limits
                // are never read by move_talus, including its neighbour loop.
                if canyons || historical {
                    repose[i] = (rock_limit[i] * resistance).clamp(0.42, 2.9);
                }
                let channel = if branching {
                    let area = flow.q[i] - local_runoff[i];
                    let onset = (520.0 + 650.0 * resistance) / (5.0 * (grade + 0.05)).clamp(0.55, 2.8);
                    smooth(0.32 * onset, 1.45 * onset, area)
                } else { 1.0 };
                let threshold = if connected { 0.043 } else { 0.055 };
                let shear = (runoff * grade - threshold).max(0.0);
                let scour = (0.68 * shear * (0.30 + 0.70 * channel)).min(soil[i]);
                soil[i] -= scour;
                let transport_speed = if branching { (0.60 + 0.36 * flow.speed[i]).clamp(0.6, 1.55) } else { 1.0 };
                let capacity = mesh.area[i] * 2.8 * runoff * grade * transport_speed;
                let abrasion = if crate::planetary::resolved_history(cfg.generator_version) {
                    transport::exposed_abrasion(load[i], capacity, mesh.area[i], soil[i], strength[i], grade, spacing)
                } else if composed {
                    transport::abrasion(load[i], capacity, mesh.area[i], soil[i], strength[i], grade, spacing)
                } else { 0.0 };
                let mut power = if focused { 3.8 + 1.8 * smooth(1.0, 5.0, runoff) } else { 3.0 };
                if connected {
                    power *= 1.0 + 0.30 * smooth(2.0, 6.0, runoff) * smooth(0.045, 0.16, grade);
                }
                let detachment = power * strength[i] * shear * (-soil[i] / 1.3).exp();
                let incision = if canyons || evolved { 0.74 } else { 0.45 };
                let limit = if evolved { 120.0 } else { 42.0 };
                let rock_cut = ((detachment + abrasion) * channel / resistance)
                    .min(grade * incision * spacing)
                    .min((rock[i] - base[i] + limit).max(0.0));
                rock[i] -= rock_cut;
                if branching && channel > 0.65 && rock_cut > 0.08 {
                    // A cut can undermine a steep adjacent bank. The talus
                    // step below transfers the failed rock and soil downhill.
                    let support = 1.0 - 0.40 * channel * smooth(0.08, 0.65, rock_cut);
                    for edge in mesh.links[i].as_slice() {
                        let j = edge.to as usize;
                        if height[j] - height[i] > 0.48 * f64::from(edge.distance) {
                            bank_support[j] = bank_support[j].min(support);
                        }
                    }
                }
                let sediment = load[i] + (scour + rock_cut) * mesh.area[i];
                let deposit = ((sediment - capacity).max(0.0) * 0.27).min(mesh.area[i] * 0.45);
                soil[i] += deposit / mesh.area[i];
                transport::forward(routes, sediment - deposit, &mut load);
            }
            for (repose, support) in repose.iter_mut().zip(&bank_support) { *repose *= support; }
            let limits = if canyons || historical { &repose } else { &rock_limit };
            move_talus(&mesh, &fixed, limits, &mut rock, &mut soil, &mut height, &mut load);
        }
        Self {
            grid,
            base: composed.then(|| mesh.regularize(|i| base[i])),
            rock_delta: mesh.regularize(|i| rock[i] - base[i]),
            soil: mesh.regularize(|i| soil[i]),
        }
    }
}

