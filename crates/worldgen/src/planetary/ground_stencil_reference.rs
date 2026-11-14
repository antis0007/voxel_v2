//! Frozen V8 ground_base; function name only changed for independent comparison.
use super::*;
impl World {
    pub(super) fn ground_base_reference(&self, cfg: &WorldSampleConfig, dir: DVec3, footprint: f32, volcanic: crate::volcano::Shaped) -> geo::Ground {
        let layered = cfg.generator_version >= engine_voxel::BEDDING_PLANET_GENERATOR;
        let valley_guided = cfg.generator_version >= engine_voxel::RIVER_MORPHOLOGY_GENERATOR;
        let bare = self.mesh.sample(&self.bare_height, dir);
        let (local, weights) = self.mesh.stencil(dir);
        let sample = |field: &[f64]| (0..4).map(|i| field[local[i]] * weights[i]).sum::<f64>();
        let coarse = sample(&self.height);
        let spill = sample(&self.flow.spill);
        let inland = if valley_guided || spill <= coarse + 0.75 {
            smooth(SEA + 2.0, SEA + 24.0, coarse)
        } else { 0.0 };
        // At sea level the same tangent reconstruction removes bends tied
        // to the edges of solve cells. Keep lake basins and active craters
        // on their existing elevation and let the river model cut its bed.
        let coastal = if cfg.generator_version >= engine_voxel::CONTINUOUS_COAST_PLANET_GENERATOR
            && !volcanic.active_caldera && volcanic.lake.is_none() {
            (1.0 - smooth(4.0, 24.0, (coarse - SEA).abs()))
                * (1.0 - smooth(0.25, 0.75, spill - coarse))
        } else { 0.0 };
        let relief_delta = if cfg.generator_version >= engine_voxel::CONTINUOUS_RELIEF_PLANET_GENERATOR
            && (inland > 0.0 || coastal > 0.0) {
            let mut blended = self.mesh.reconstruct_height(&self.height, &self.gradient, dir);
            let edifice = self.edifice_lift(dir);
            if edifice > 0.0 {
                // Use the same reconstruction for volcanic relief and terrain.
                // The analytic edifice correction below replaces its linear part.
                blended -= self.mesh.reconstruct_height(&self.edifice_lift, &self.edifice_gradient, dir) - edifice;
            }
            blended - coarse
        } else { 0.0 };
        let relief = coarse + relief_delta * inland;
        let volcanic_base = bare + self.mesh.sample(&self.volcanic_delta, dir);
        let h = if uses_headwater_relief(cfg.generator_version) || cfg.generator_version == TECTONIC_TRAVERSE_GENERATOR {
            // Apply the analytic volcanic correction in the same height
            // space as the routed mesh. The continuous fit is applied once.
            (relief + fit_mountain_height(volcanic.metres) - fit_mountain_height(volcanic_base))
                .clamp(-58.0, 503.0)
        } else {
            // Retain the historical operation order for saved generators.
            (relief + volcanic.metres - bare - self.mesh.sample(&self.volcanic_delta, dir))
                .clamp(-58.0, 495.0)
        };
        let c = self.climate(dir);
        let lake = volcanic.lake.map(|(head, _)| head).or_else(|| {
            if coarse <= SEA || c.wet <= 0.12 {
                return None;
            }
            if !valley_guided {
                return (spill > coarse + 0.75 && h < spill).then_some(spill);
            }
            // Terrain intersects a lake's level. Interpolating the routing
            // elevations instead tilts the lake toward the dry corner of a cell.
            local.into_iter().zip(weights).filter_map(|(i, weight)| {
                let level = self.flow.spill[i];
                (weight > 0.0 && self.height[i] > SEA && level > self.height[i] + 0.75
                    && level > h).then_some((weight, level))
            }).max_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, level)| level)
        });
        let p = dir * atlas::RADIUS_M;
        let coastal_relief = cfg.generator_version >= engine_voxel::COASTAL_RELIEF_PLANET_GENERATOR;
        let alluvial = self.alluvium.as_ref().filter(|_| !volcanic.active_caldera)
            .map(|network| network.sample(p, h, footprint));
        let riverbed = alluvial.as_ref().and_then(|a| a.riverbed);
        let channel = if alluvial.is_none() && !volcanic.active_caldera { self.channel(dir) } else { None };
        let calm = alluvial.as_ref().map_or_else(
            || channel.map_or(1.0, |c| smooth(c.width + 5.0, c.width + 40.0, c.distance)),
            |a| a.calm * (1.0 - a.delta) * (1.0 - a.morph.map_or(0.0, |m| {
                if coastal_relief { m.blend * smooth(0.3, 3.0, m.deposited.abs()) * 0.45 }
                else { m.blend }
            })),
        ) * if lake.is_some() || volcanic.active_caldera { 0.0 } else { 1.0 };
        let (ids, weights) = self.mesh.stencil(dir);
        let gradient: DVec3 = (0..4).map(|i| self.gradient[ids[i]] * weights[i]).sum();
        // Establish the river bed and floodplain against the solved grade.
        // Limit the displacement on shallow shelves as well as on cliffs.
        let h = if coastal > 0.0 {
            let shore_shift = relief_delta * coastal * (1.0 - inland) * calm.powi(4);
            let limit = (gradient.length() * 64.0).clamp(0.55, 6.0);
            (h + limit * (shore_shift / limit).tanh()).clamp(-58.0, 495.0)
        } else { h };
        let land = smooth(SEA, SEA + 12.0, h);
        let carbonate = self.mesh.sample(&self.carbonate, dir);
        let collision = self.mesh.sample(&self.collision, dir).min(1.0);
        let rift = self.mesh.sample(&self.rift, dir).min(1.0);
        let filtered = |w: f64| 1.0 - smooth(w * 0.25, w * 2.0, f64::from(footprint));
        // Province-conditioned relief: rounded uplands, resistant crags,
        // limestone terraces and sinkholes, volcanic ridges. All wavelengths are metres.
        let round = (noise(cfg.seed, p / 150.0, 1001) - 0.5) * (10.0 + collision * 22.0) * filtered(150.0);
        let crag = (1.0 - (noise(cfg.seed, p / 48.0, 1009) * 2.0 - 1.0).abs()).powi(3) * collision * 12.0 * filtered(48.0);
        let dissolution = if valley_guided {
            // Develop hollows in wet carbonate depressions, where water can
            // collect and enter the rock. Carbonate by itself is insufficient.
            carbonate * smooth(0.2, 0.65, c.wet) * smooth(0.3, 3.0, spill - coarse)
        } else {
            carbonate
        };
        let sink = smooth(0.65, 0.88, noise(cfg.seed, p / 85.0, 1013)) * dissolution * 10.0 * filtered(85.0);
        let basalt = (noise(cfg.seed, p / 110.0, 1019) - 0.5) * rift * 16.0 * filtered(110.0);
        let micro = (noise(cfg.seed, p / 7.0, 1021) - 0.5) * 0.9 * filtered(7.0);
        // Leave the highlands' sharp relief to the routed erosion and beds.
        let crag_weight = if cfg.generator_version == LAYERED_CANYON_GENERATOR { 0.18 }
            else if connected_relief(cfg.generator_version) { 0.36 } else { 1.0 };
        let detail = if geological_history(cfg.generator_version) || cfg.generator_version == COUPLED_RIFT_GENERATOR {
            micro
        } else { round + crag * crag_weight - sink + basalt + micro };
        let detail = detail.min(0.0) + detail.max(0.0) * (1.0 - smooth(470.0, 500.0, h));
        let mut top = h + detail * calm * land;
        let lava_lake = cfg.generator_version >= engine_voxel::VOLCANIC_LANDFORM_PLANET_GENERATOR
            && volcanic.lake.is_some_and(|(_, fluid)| fluid == BlockId::Lava);
        // The coarse watershed may see the active caldera as a freshwater
        // catchment. Its visible basin is instead ash, exposed rock and lava.
        let mut water = if lava_lake || volcanic.active_caldera { None } else { lake };
        let mut riparian = 0.0;
        let mut valley_wall = 0.0;
        let mut delta = 0.0;
        let mut delta_mud = 1.0;
        let mut suspended = 0.0;
        let mut intertidal_exposure = 0.0;
        let mut outlook_grade = None;
        let mut placer = 0.0;
        if let Some(a) = alluvial {
            delta = a.delta;
            if delta > 0.0 && h < SEA + 6.0 {
                top += (a.deposit - top).clamp(0.0, 34.0) * delta;
            }
            if let Some(plain) = a.floodplain {
                top -= (top - plain.ceiling).max(0.0) * plain.weight;
                if a.bed.is_none() && lake.is_none()
                    && !(landscape_cycles(cfg.generator_version) && a.channel_head.is_some()) {
                    top += plain.fill.min((plain.ceiling - top).max(0.0));
                }
                valley_wall = plain.wall;
            }
            if let Some((bed, head, weight)) = a.bed {
                top -= (top - bed).max(0.0) * weight;
                if top < head {
                    water = Some(water.map_or(head, |l| l.max(head)));
                }
            }
            if let Some(head) = a.overbank.filter(|head| *head > top) {
                water = Some(water.map_or(head, |l| l.max(head)));
            }
            if landscape_cycles(cfg.generator_version)
                && let Some(head) = a.channel_head.filter(|head| *head > top) {
                // Filtering narrow river geometry cannot make the physical
                // water disappear and abruptly enable dry-slope erosion.
                water = Some(water.map_or(head, |old| old.max(head)));
            }
            placer = a.placer.filter(|&(ceiling, _)| top <= ceiling).map_or(0.0, |(_, grade)| grade);
            riparian = a.riparian.max(delta * 0.7);
            if let Some(m) = a.morph {
                outlook_grade = m.outlook;
                let deposited = m.deposited - m.deposited.max(0.0) * m.island_share;
                delta = m.blend * smooth(0.08, 0.7, m.deposited);
                if m.mobile {
                    let wet = m.blend * smooth(0.08, 0.70, m.depth);
                    top += m.deposited * m.blend;
                    let intertidal = if tidal_coasts(cfg) {
                        m.blend * (1.0 - smooth(1.2, 5.0, (top - SEA).abs()))
                    } else { 0.0 };
                    top += (m.bed - top) * wet.max(intertidal);
                    if wet > 0.0 {
                        let old = water.unwrap_or(SEA);
                        let stage = old + (m.bed + m.depth - old) * wet;
                        water = (stage > top && stage > SEA + 0.02).then_some(stage);
                    }
                    delta_mud = m.mud;
                    suspended = m.suspended * m.blend * (1.0 - smooth(48.0, 240.0, f64::from(footprint)));
                    intertidal_exposure = m.blend * smooth(0.04, 0.22, m.inundation)
                        * (1.0 - smooth(0.76, 0.98, m.inundation))
                        * smooth(0.008, 0.065, m.fine_accretion);
                } else if let Some(form) = m.estuary {
                    let active = m.blend * form.weight;
                    let bar_weight = m.blend * form.sediment;
                    let relief = if bar_weight > 0.02 {
                        (noise(cfg.seed, p / 16.0, 1491) - 0.5) * 1.05 * filtered(16.0)
                    } else { 0.0 };
                    // The solved deposit supplies the shelf. The smaller bars
                    // and channel cross-sections reshape only the active reach.
                    top += deposited * m.blend * (1.0 - 0.78 * active.max(bar_weight));
                    top += (form.bar + relief - top).clamp(0.0, 9.0) * bar_weight;
                    let bank = if form.bed >= form.water { 1.0 - bar_weight * 0.87 } else { 1.0 };
                    let bed = if form.water <= SEA + 0.01 && top < SEA - 3.0 && form.bed < SEA {
                        top.min(form.bed)
                    } else { form.bed };
                    top += (bed - top) * active * bank;
                    if active > 0.0 {
                        let previous = water.unwrap_or(SEA);
                        let head = previous + (form.water - previous) * active;
                        water = (head > top && head > SEA + 0.01).then_some(head);
                    }
                    if let Some((bed, head, resolved)) = a.bed.filter(|_| form.inlet_tie > 0.0) {
                        // Both routes share a wet cross-section before they
                        // diverge. A flood deposit cannot plug the handoff.
                        let inlet = head - (head - bed).clamp(0.0, 2.0) * 0.8;
                        top -= (top - inlet).max(0.0) * resolved * form.inlet_tie;
                        if top < head { water = Some(water.map_or(head, |level| level.max(head))); }
                    }
                    delta = delta.max(bar_weight * 0.96);
                    delta_mud = m.mud + (form.fine - m.mud) * bar_weight;
                } else if coastal_relief {
                    // Transfer erosion and deposition onto the walked terrain.
                    // The coarse grid's baseline would flatten the coast in a rectangle.
                    let detail = delta * 0.75 * filtered(11.0);
                    let texture = if detail > 0.0 {
                        (noise(cfg.seed, p / 11.0, 1469) - 0.5) * detail
                    } else { 0.0 };
                    let shore = m.blend * a.calm * (1.0 - smooth(1.0, 9.0, (top - SEA).abs()))
                        * filtered(72.0);
                    let beach_relief = if shore > 0.0 {
                        let banks = noise(cfg.seed, p / 71.0, 1481) - 0.5;
                        let shoals = noise(cfg.seed, p / 33.0, 1483) - 0.5;
                        (3.0 * banks + 1.4 * shoals) * shore
                    } else { 0.0 };
                    top += deposited * m.blend + texture + beach_relief;
                    if let Some((bed, river_head, resolved)) = a.bed {
                        // The short depositional event can strand its inflow.
                        // Retain a low-flow outlet through the active thalweg.
                        let relief = (river_head - bed).clamp(0.0, 1.8) * 0.68;
                        let outlet = river_head - relief;
                        top -= (top - outlet).max(0.0) * resolved * smooth(0.0, 0.15, m.blend);
                        if top < river_head {
                            water = Some(water.map_or(river_head, |head| head.max(river_head)));
                        }
                    }
                    if let Some(bed) = m.outlet_bed {
                        top -= (top - bed).max(0.0) * m.blend;
                    }
                    let head = m.bed + m.depth;
                    let wet = m.blend * smooth(0.12, 0.6, m.depth);
                    let old = water.unwrap_or(SEA);
                    let stage = old + (head - old).max(0.0) * wet;
                    if stage > (SEA + 0.08).max(top) {
                        water = Some(stage);
                    }
                } else {
                    let old = (water.unwrap_or(SEA) - top).max(0.0);
                    top += (m.bed - top) * m.blend;
                    let wet = if m.depth > 0.04 { m.depth } else { 0.0 };
                    let depth = old + (wet - old) * m.blend;
                    water = (depth > 0.0).then_some(top + depth);
                }
                if m.estuary.is_none() { delta_mud = m.mud; }
                if let Some(island) = m.island {
                    let weight = island.weight * m.blend * (1.0 - smooth(SEA + 3.2, SEA + 7.5, h));
                    let shift = island.level - top;
                    top += (shift.min(0.0) + shift.max(0.0) * (1.0 - island.channel)) * weight;
                    let open = smooth(0.40, 0.94, island.channel) * m.blend
                        * (1.0 - smooth(SEA + 4.0, SEA + 9.0, h));
                    top -= (top - (SEA - 1.6)).max(0.0) * open;
                    delta = delta.max(weight);
                    delta_mud += (island.mud - delta_mud) * weight;
                }
                riparian = riparian.max(delta * 0.65);
            }
        }
        if let Some(c) = channel {
            let bank = c.width + 12.0;
            let valley = 1.0 - smooth(bank, REACH, c.distance);
            // Valleys belong to the drainage solve at every viewing distance.
            // Only the narrow bed below is subpixel detail. Keep historical
            // distant surfaces unchanged for existing saves.
            let weight = if cfg.generator_version >= engine_voxel::WATERSHED_LOD_GENERATOR {
                1.0
            } else {
                1.0 - smooth(45.0, 400.0, f64::from(footprint))
            };
            let floor = c.level + 0.7 + (c.distance - c.width).max(0.0) * 0.055;
            if layered && top > floor {
                // Rise across the carved bank from the derivative of its blend.
                let t = ((c.distance - bank) / (REACH - bank)).clamp(0.0, 1.0);
                valley_wall = (top - floor) * 6.0 * t * (1.0 - t) / (REACH - bank);
            }
            top += (top.min(floor) - top) * valley * weight;
            let bed_weight = 1.0 - smooth((c.width * 2.0).max(8.0), (c.width * 8.0).max(32.0), f64::from(footprint));
            if c.distance < c.width {
                let bed = c.level - c.depth * (1.0 - (c.distance / c.width).powi(2));
                top += (top.min(bed) - top) * bed_weight;
                if (landscape_cycles(cfg.generator_version) || bed_weight > 0.0) && top < c.level {
                    water = Some(water.map_or(c.level, |l| l.max(c.level)));
                }
            }
            if cfg.generator_version >= engine_voxel::COASTAL_PLANET_GENERATOR {
                // Backwater widens toward the coast. The same filtered head
                // drives the ground column and the distant water surface.
                let tidal = (1.0 - smooth(SEA + 0.5, SEA + 4.0, c.level))
                    * (1.0 - smooth(SEA + 1.0, SEA + 8.0, h));
                let mouth = 1.0 - smooth(c.width, c.width * 2.8 + 7.0, c.distance);
                let resolve = filtered((c.width * 5.0).max(30.0));
                top -= (top - (SEA - 1.4)).max(0.0) * tidal * mouth * resolve;
            }
            riparian = valley;
        }
        let area = self.mesh.sample(&self.flow.discharge, dir);
        let mut slope = (gradient.length() * (1.0 - delta * 0.85)).max(valley_wall);
        let mut scree_m = 0.0;
        // The tributary solve uses these beds as erodibility and repose
        // limits, without imposing contour ripples on its parent surface.
        if layered && !composed_drainage(cfg.generator_version) {
            // Folded beds follow the same datum as the exposed and buried rock.
            // Stretch the profile through resistant rock and flatten it between
            // beds. The monotone remap preserves the order of the underlying slope.
            let province = (1.0 - smooth(0.12, 0.55, collision)).max(carbonate)
                * (1.0 - smooth(0.12, 0.25, rift))
                * (1.0 - smooth(8.0, 35.0, self.edifice_lift(dir)));
            let strength = 0.78 * province * calm * smooth(0.09, 0.30, slope)
                * smooth(SEA + 6.0, SEA + 28.0, top) * (1.0 - smooth(460.0, 495.0, top))
                * (1.0 - smooth(0.0, 0.3, volcanic.slope)) * filtered(95.0);
            if strength > 0.0 {
                let offset = if self.fault_offset.is_empty() { 0.0 } else { sample(&self.fault_offset) };
                let bed = bedding_height(cfg, p, collision, top - offset);
                if cfg.generator_version >= ERODED_RELIEF_GENERATOR {
                    // The resistant band projects from the slope, with its softer
                    // contact recessed. A continuous profile leaves no flat steps.
                    let phase = (bed - 10.0) * std::f64::consts::TAU / BED_INTERVAL;
                    let ledge = 2.8 * strength;
                    top += ledge * phase.sin();
                    slope *= 1.0 + ledge * (std::f64::consts::TAU / BED_INTERVAL) * phase.cos();
                    if water.is_none() && footprint < 125.0 {
                        let (shift, debris) = weathered_bed(cfg.seed, p, bed, f64::from(footprint));
                        let talus = 1.0 - smooth(0.35, 0.80, slope);
                        scree_m = strength * debris * talus;
                        top += scree_m + strength * (shift - debris) * 0.65;
                        // Sub-metre joint relief is limited to exposed, steep beds.
                        let resolved = 1.0 - smooth(0.25, 0.9, f64::from(footprint));
                        let exposed = smooth(0.36, 0.85, slope) * smooth(10.0, 18.0, bed)
                            * (1.0 - smooth(31.0, 38.0, bed));
                        if resolved * exposed > 0.0 {
                            let joint = (noise(cfg.seed, p / 2.8, 1259) - 0.5).abs();
                            top -= 0.48 * strength * resolved * exposed * (1.0 - smooth(0.02, 0.19, joint));
                        }
                    }
                } else {
                    let riser = ((bed - 12.0) / 24.0).clamp(0.0, 1.0);
                    top += strength * (BED_INTERVAL * smooth(12.0, 36.0, bed) - bed);
                    slope *= 1.0 + strength * (12.0 * riser * (1.0 - riser) - 1.0);
                    if cfg.generator_version >= engine_voxel::SCREE_PLANET_GENERATOR
                        && water.is_none() && footprint < 125.0
                    {
                        let (shift, debris) = weathered_bed(cfg.seed, p, bed, f64::from(footprint));
                        top += strength * shift;
                        scree_m = strength * debris;
                    }
                }
            }
        }
        if crate::trees::rainforest::forest_floor(cfg) && cfg.world_type == WorldType::Planet
            && footprint < 9.0 && water.is_none() && volcanic.lake.is_none() && scree_m < 0.5
        {
            let t = c.lifted((top - SEA).max(0.0));
            let tropical = smooth(17.0, 22.0, t.annual_c) * smooth(0.66, 0.79, c.wet + riparian * 0.2);
            if tropical > 0.0 {
                let (swale, mound) = rainforest_floor(cfg, p);
                let shift = crate::trees::rainforest::floor_relief(swale, mound, top as f32, slope as f32, footprint);
                top += tropical * f64::from(shift);
            }
        }
        if !self.fault_damage.is_empty() && water.is_none() && top > SEA + 8.0 {
            let damage = sample(&self.fault_damage);
            let apron = smooth(0.12, 0.30, slope) * (1.0 - smooth(0.43, 0.72, slope));
            // Loose blocks accumulate below damaged rock; cliff geometry was
            // displaced and eroded before this surface-cover classification.
            scree_m = scree_m.max(3.4 * damage * apron * calm);
        }
        let mut outlook_clearing = 0.0;
        if let Some(grade) = outlook_grade
            && water.is_none() && volcanic.lake.is_none() && top > SEA + 4.0
        {
            let weight = grade.weight * smooth(SEA + 4.0, SEA + 6.0, top);
            top += (grade.level - top).clamp(-3.0, 3.0) * weight;
            outlook_clearing = grade.clearing as f32;
        }
        let mut river_gravel = false;
        let mut river_bedrock = false;
        if let Some(river) = riverbed.filter(|_| lake.is_none() && delta < 0.1) {
            let old_depth = water.map_or(0.0, |head| (head - top).max(0.0));
            if old_depth > 0.0 || river.depth > 0.08 || river.shift.abs() > 0.035 {
                top += (river.bed - top) * river.blend;
                let depth = old_depth + (river.depth - old_depth) * river.blend;
                water = (depth > 0.11).then_some(top + depth);
                river_gravel = if clean_riverbeds(cfg.generator_version) {
                    // Pilot fill and flood spread raised `shift` across whole
                    // straight tiles; keep gravel on the wet bed and real bars.
                    let deposit = river.deposited * river.blend;
                    (depth > 0.11 && deposit > 0.16) || deposit > 0.5
                } else {
                    river.shift.max(river.deposited) * river.blend > 0.16
                };
                river_bedrock = river.rock_exposure * river.blend > 0.62
                    && river.shift < -0.25 && depth > 0.11;
            }
        }
        let mut sediment = ((area.sqrt() / 5000.0).min(1.0) * (1.0 - collision)
            + riparian * 0.4).max(delta).min(1.0);
        if !self.sediment_cover.is_empty() {
            sediment = sediment.max(smooth(0.1, 2.0, self.mesh.sample(&self.sediment_cover, dir)));
        }
        if let Some(region) = self.eroded_region.as_ref().and_then(|region| region.sample(dir)) {
            let mix = |parent: f64, local: f64| parent + region.weight * (local - parent);
            // Keep parent water at its solved grade where it crosses the apron.
            // The regional runoff is annual volume and supplies no water stage.
            let wet = water.is_some_and(|head| head > top);
            let weight = if wet { region.weight * region.weight } else { region.weight };
            let old_top = top;
            top += weight * (region.height - top);
            slope = mix(slope, region.slope);
            riparian = mix(riparian, region.drainage);
            sediment = mix(sediment, smooth(0.15, 2.0, region.soil));
            scree_m = mix(scree_m, 0.0);
            if wet {
                // A legacy stage may otherwise inundate an independently cut gorge.
                water = water.filter(|head| old_top - top < 1.0 && *head > top);
            }
        }
        if water.is_none() && top > SEA + 5.0 && slope < 0.38 && riparian < 0.25 {
            let erratic = glacial::erratic(&self.mesh, &self.former_ice, dir, cfg.seed, footprint);
            top += erratic;
            scree_m = scree_m.max(erratic * 1.45);
        }
        if changing_seas(cfg.generator_version) && coarse < SEA - 8.0
            && water.is_none() && !volcanic.active_caldera && delta < 0.1
        {
            // Shape only submerged open-ocean beds, after the shoreline,
            // rivers, volcanic relief and local sediment have been solved.
            let weight = smooth(8.0, 20.0, SEA - coarse) * (1.0 - smooth(0.0, 0.1, delta));
            let trench = self.mesh.sample(&self.offshore_trench, dir);
            top += weight * (self.plates.offshore_bed(dir, top, SEA, trench) - top);
        }
        let t = c.lifted((top - SEA).max(0.0));
        geo::Ground { deposit_material: crate::BlockId::Stone, deposit_bed_top: 0.0, deposit_bed_slope: 0.0, deposit_m: 0.0, top_m: top as f32,
            slope: slope as f32,
            temperature: t.heat01() as f32,
            moisture: (c.wet + riparian * 0.2).min(1.0) as f32,
            sediment: sediment as f32,
            maat_c: t.annual_c as f32, permafrost: t.annual_c < geo::PERMAFROST_C,
            water_m: water.filter(|w| !lava_lake && *w > top).map(|w| w as f32), riparian: riparian as f32,
            delta: delta as f32, delta_mud: delta_mud as f32, suspended: suspended as f32,
            intertidal: intertidal_exposure as f32,
            scree_m: scree_m as f32, outlook_clearing,
            placer: placer as f32, river_gravel, river_bedrock }
    }

}
