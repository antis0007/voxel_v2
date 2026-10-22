use super::*;
use crate::simulation::sandbox::ThermalCell;
use crate::world::BlockId as B;

#[derive(Clone, Copy)]
pub struct VoxelParticle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub color: [f32; 3],
    pub size: f32,
    pub glow: f32,
    pub alpha: f32,
    pub gravity: f32,
    pub life: f32,
    pub total: f32,
}

#[derive(Clone, Copy)]
pub struct LaserBeam {
    pub a: Vec3,
    pub b: Vec3,
    pub color: [f32; 3],
    pub radius: f32,
    pub life: f32,
    pub total: f32,
}

#[derive(Clone, Copy)]
pub struct PlasmaCross {
    pub pos: Vec3,
    pub normal: Vec3,
    pub life: f32,
    pub pulse: f32,
}

impl Adventure {
    /// An arcane pile ran out of coolant. Violent, local and survivable: a
    /// fireball, a shove, and damage that falls off with distance. The machine
    /// itself survives (disabled and hot) so the player can rebuild the cooling.
    pub(super) fn reactor_meltdown(&mut self, world: &mut World, at: Vec3, eye: Vec3) {
        let color = [0.55, 0.95, 1.0];
        self.blast_visual(at, 3.4, Vec3::Y, color);
        self.spawn_voxel_burst(at, color, 48, 6.5, 0.06);
        self.spawn_voxel_burst(at, [1.0, 0.55, 0.2], 24, 4.0, 0.05);
        sound(&mut self.sounds, Cue::Burst, Some(at), 1.0);
        world
            .sandbox
            .apply_radial_impulse(at, 4.5, 6.0, world.voxel_scale.meters());
        self.thermal_bloom(world, at, 3.0, true);
        let distance = (eye - at).length();
        if distance < 6.0 {
            self.damage(34.0 * (1.0 - distance / 6.0), eye - at);
        }
        for e in &mut self.enemies {
            if e.health > 0.0 && (e.center() - at).length() < 5.0 {
                e.hurt(30.0);
                e.flash = 0.2;
            }
        }
        world
            .expedition
            .say("Arcane pile meltdown! Pipe coolant water into it before restarting.");
    }

    pub fn spawn_voxel_particle(
        &mut self,
        pos: Vec3,
        vel: Vec3,
        color: [f32; 3],
        size: f32,
        glow: f32,
        alpha: f32,
        gravity: f32,
        life: f32,
    ) {
        if self.particles.len() >= 480 {
            self.particles.remove(0);
        }
        let fx = self.zone_fx;
        let life = life * fx.life_mul;
        self.particles.push(VoxelParticle {
            pos,
            vel,
            color: fx.tinted(color),
            size: size * fx.size_mul,
            glow: fx.glow(glow, pos.x * 1.3 + pos.z * 0.7 + self.time),
            alpha,
            gravity: gravity * fx.gravity_mul,
            life,
            total: life.max(0.05),
        });
    }

    pub fn spawn_voxel_burst(
        &mut self,
        pos: Vec3,
        color: [f32; 3],
        count: usize,
        speed: f32,
        size: f32,
    ) {
        for i in 0..count {
            let a = i as f32 * 2.39996 + self.time * 3.7 + pos.x * 0.41;
            let pitch = ((i as f32 * 1.73).sin()) * 0.75;
            let dir = Vec3::new(
                a.cos() * pitch.cos(),
                pitch.sin() + 0.25,
                a.sin() * pitch.cos(),
            )
            .normalize_or_zero();
            let v_mag = speed * (0.55 + (i % 5) as f32 * 0.18);
            self.spawn_voxel_particle(
                pos + dir * 0.04,
                dir * v_mag * 0.55,
                color,
                size * (0.85 + (i % 3) as f32 * 0.08),
                1.35,
                0.55,
                0.8,
                0.85 + (i % 4) as f32 * 0.12,
            );
        }
    }

    /// Emit the visible free-surface response to a water entry. The solver owns
    /// the volume; this only adds a bounded crown, droplets and a contact ring
    /// at the exposed top of the local water column.
    pub(crate) fn splash(&mut self, surface: Vec3, strength: f32) {
        if !surface.is_finite() {
            return;
        }
        let strength = strength.clamp(0.12, 1.0);
        if self.splashes.len() >= 24 {
            self.splashes.remove(0);
        }
        let life = 0.56 + 0.38 * strength;
        self.splashes.push(crate::tool_effects::Splash {
            pos: surface,
            radius: 0.34 + 1.05 * strength,
            age: 0.0,
            life,
            strength,
        });
        let count = (5.0 + strength * 19.0).round() as usize;
        for i in 0..count {
            let a = i as f32 * 2.399963 + self.time * 4.1 + surface.x * 0.37;
            let radial = Vec3::new(a.cos(), 0.0, a.sin());
            let lift = 1.15 + (i % 5) as f32 * 0.22 + strength * 1.8;
            let speed = (0.8 + strength * 2.5) * (0.7 + (i % 4) as f32 * 0.11);
            let tint = if i % 4 == 0 {
                [0.91, 0.97, 0.98]
            } else if i % 3 == 0 {
                [0.52, 0.83, 0.85]
            } else {
                [0.72, 0.93, 0.95]
            };
            self.spawn_voxel_particle(
                surface
                    + radial * (0.10 + (i % 3) as f32 * 0.06)
                    + Vec3::Y * (0.015 + (i % 2) as f32 * 0.025),
                radial * speed + Vec3::Y * lift * (0.7 + (i % 3) as f32 * 0.12),
                tint,
                (0.026 + (i % 3) as f32 * 0.012) * (0.8 + strength * 0.5),
                2.0 + strength * 1.6,
                0.74 + strength * 0.18,
                7.5,
                0.38 + (i % 5) as f32 * 0.075 + strength * 0.20,
            );
        }
        sound(
            &mut self.sounds,
            Cue::Splash,
            Some(surface),
            0.42 + strength * 0.38,
        );
    }

    /// The pixels of a pickup leave the object: outward, upward, then they thin out.
    pub fn pickup_burst(&mut self, pos: Vec3, color: [f32; 3]) {
        for i in 0..32 {
            let a = i as f32 * 2.399963 + pos.x;
            let lift = 0.25 + (i % 6) as f32 * 0.14;
            let dir = Vec3::new(a.cos(), lift, a.sin()).normalize_or_zero();
            let speed = 1.1 + (i % 8) as f32 * 0.28;
            let tint = if i % 4 == 0 { [1.0, 1.0, 1.0] } else { color };
            self.spawn_voxel_particle(
                pos + Vec3::Y * (0.35 + (i % 5) as f32 * 0.06),
                dir * speed,
                tint,
                0.028 + (i % 3) as f32 * 0.014,
                4.2,
                0.92,
                -0.15,
                0.7 + (i % 5) as f32 * 0.1,
            );
        }
    }

    pub(super) fn mark_voxel_dirty(&mut self, world: &mut World, cell: IVec3) {
        world.activity.wake(cell);
        if self.dirty_voxels.len() < 2048 {
            self.dirty_voxels.push(cell);
        }
    }

    /// Apply one spell's terrain changes as a single world transaction.
    ///
    /// Applying a `world.set` per voxel incremented the world revision and
    /// repeated chunk bookkeeping thousands of times inside one simulation
    /// tick. `set_many` keeps the same result while paying that bookkeeping a
    /// bounded number of times; the dirty list still drives the immediate mesh
    /// refresh.
    pub(super) fn apply_terrain_edits(&mut self, world: &mut World, edits: &[(IVec3, B)]) {
        if edits.is_empty() {
            return;
        }
        world.set_many(edits);
        for &(cell, _) in edits {
            self.mark_voxel_dirty(world, cell);
        }
    }

    pub(super) fn carve_cell(&mut self, world: &mut World, cell: IVec3, harvest: bool) -> bool {
        let cfg = world.sample_config();
        if cell.y <= cfg.min_world_y() + 1 || cell.y >= cfg.max_world_y() {
            return false;
        }
        let anchor = factory::anchor(cell, factory::step(world));
        if world.factory.nodes.contains_key(&anchor) {
            return false;
        }
        let old = world.get(cell);
        if old == B::Air || old.is_liquid() {
            return false;
        }
        let scale = world.voxel_scale.meters();
        let center = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
        let col = old.color(cell);
        // Drills and singularity carving should not run the mining inventory
        // pipeline for every stone voxel. Only genuinely valuable blocks are
        // harvested; ordinary terrain is simply removed in one write.
        let valuable = super::super::expedition::resource(old).is_some()
            || matches!(
                old,
                B::Log | B::SalvageCrate | B::SurveyCache | B::CultivatorCache
            );
        if harvest && valuable {
            let mut blocks = std::collections::HashSet::new();
            let mut chunks = std::collections::HashSet::new();
            if !super::super::expedition::blast_harvest(world, cell, &mut blocks, &mut chunks) {
                return false;
            }
            for touched in blocks {
                self.mark_voxel_dirty(world, touched);
            }
        } else {
            world.set(cell, B::Air);
            if world.get(cell) != B::Air {
                return false;
            }
        }
        self.mark_voxel_dirty(world, cell);
        // `blast_harvest` fells a tree when the removed block is a log, so the
        // caller does not run the canopy search a second time.
        self.spawn_voxel_burst(center, col, 4, 1.8, 0.036);
        true
    }

    pub(super) fn carve_sphere(
        &mut self,
        world: &mut World,
        center: Vec3,
        radius_m: f32,
        mut budget: usize,
    ) {
        let scale = world.voxel_scale.meters();
        let c_cell = (center / scale).floor().as_ivec3();
        let r = (radius_m / scale).ceil().clamp(1.0, 4.0) as i32;
        let r_sq = (radius_m / scale) * (radius_m / scale);
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    if budget == 0 {
                        return;
                    }
                    let d_sq = (dx * dx + dy * dy + dz * dz) as f32;
                    if d_sq <= r_sq {
                        let cell = c_cell + IVec3::new(dx, dy, dz);
                        if self.carve_cell(world, cell, true) {
                            budget -= 1;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn cast_luminous_drill(
        &mut self,
        world: &mut World,
        eye: Vec3,
        dir: Vec3,
        mods: wands::ShotMods,
        payload: Option<u32>,
    ) {
        let scale = world.voxel_scale.meters();
        let max_dist = 6.8 * mods.speed_mul.clamp(0.7, 1.5);
        let step = (scale * 0.35).clamp(0.08, 0.25);
        let steps = (max_dist / step).ceil() as usize;
        let start = eye + dir * 0.36;
        let mut end = start + dir * max_dist;
        let mut carve_budget = if mods.bore { 10 } else { 5 };
        for i in 1..=steps {
            let p = start + dir * (i as f32 * step);
            let cell = (p / scale).floor().as_ivec3();
            if world.solid(cell) {
                if carve_budget > 0 && self.carve_cell(world, cell, true) {
                    carve_budget -= 1;
                    self.spawn_voxel_burst(p, spell_color(9), 5, 2.2, 0.032);
                } else {
                    end = p;
                    break;
                }
            }
        }
        let rank = self.rank();
        let seg = end - start;
        let seg_len = seg.length().max(0.01);
        let seg_dir = seg / seg_len;
        for e in &mut self.enemies {
            if e.health <= 0.0 {
                continue;
            }
            let rel = e.center() - start;
            let t = rel.dot(seg_dir).clamp(0.0, seg_len);
            let closest = start + seg_dir * t;
            if (e.center() - closest).length() < if tall(e.kind) { 0.85 } else { 0.65 } {
                let shatter = e.slow > 0.0;
                e.health -=
                    (26.0 + rank as f32 * 3.5 + if shatter { 16.0 } else { 0.0 }) * mods.damage_mul;
                if shatter {
                    e.slow = 0.0;
                }
                e.flash = 0.16;
                e.stagger = 0.22;
                e.knock = seg_dir * 2.2;
                self.hit = 0.16;
            }
        }
        for i in 1..=8 {
            let p = start.lerp(end, i as f32 / 8.0);
            self.spawn_voxel_particle(
                p,
                Vec3::new(
                    (i as f32 * 3.1).cos() * 0.7,
                    0.8,
                    (i as f32 * 3.1).sin() * 0.7,
                ),
                spell_color(9),
                0.035,
                1.3,
                0.45,
                0.5,
                0.28,
            );
        }
        if self.lasers.len() < 16 {
            self.lasers.push(LaserBeam {
                a: start,
                b: end,
                color: spell_color(9),
                radius: 0.042,
                life: 0.18,
                total: 0.18,
            });
        }
        if let Some(pay) = payload {
            self.emit_spell_modified(pay, end - dir * 0.12, dir, None, mods);
        }
    }

    pub(super) fn cryo_bloom(&mut self, world: &mut World, center: Vec3, radius_m: f32) {
        let scale = world.voxel_scale.meters();
        let c_cell = (center / scale).floor().as_ivec3();
        let r = (radius_m / scale).ceil().clamp(1.0, 5.0) as i32;
        let r_sq = (radius_m / scale) * (radius_m / scale);
        let mut edits = Vec::new();
        let mut thermal = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    if (dx * dx + dy * dy + dz * dz) as f32 > r_sq {
                        continue;
                    }
                    let cell = c_cell + IVec3::new(dx, dy, dz);
                    let b = world.get(cell);
                    if b == B::Water {
                        edits.push((cell, B::Ice));
                        thermal.push((
                            cell,
                            ThermalCell {
                                temperature: -5.0,
                                fuel: 0.0,
                                gas: 0,
                            },
                        ));
                    } else if b == B::Fire {
                        edits.push((cell, B::Ash));
                        thermal.push((cell, ThermalCell::default()));
                    } else if b == B::Lava {
                        edits.push((cell, B::Basalt));
                        thermal.push((cell, ThermalCell::default()));
                    }
                }
            }
        }
        self.apply_terrain_edits(world, &edits);
        for (cell, state) in thermal {
            world.sandbox.set(cell, state);
        }
        for e in &mut self.enemies {
            if e.health > 0.0 && (e.center() - center).length() < radius_m + 0.8 {
                e.slow = e.slow.max(6.0);
                e.flash = 0.16;
            }
        }
        self.spawn_voxel_burst(center, spell_color(13), 22, 2.6, 0.044);
    }

    pub(super) fn thermal_bloom(
        &mut self,
        world: &mut World,
        center: Vec3,
        radius_m: f32,
        spawn_fire: bool,
    ) {
        let scale = world.voxel_scale.meters();
        let c_cell = (center / scale).floor().as_ivec3();
        let r = (radius_m / scale).ceil().clamp(1.0, 4.0) as i32;
        let r_sq = (radius_m / scale) * (radius_m / scale);
        let mut fires_placed = 0usize;
        let mut edits = Vec::new();
        let mut thermal = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    if (dx * dx + dy * dy + dz * dz) as f32 > r_sq {
                        continue;
                    }
                    let cell = c_cell + IVec3::new(dx, dy, dz);
                    let b = world.get(cell);
                    if b == B::Ice {
                        edits.push((cell, B::Water));
                    } else if b == B::SnowGrass {
                        edits.push((cell, B::Dirt));
                    } else if b == B::Water {
                        edits.push((cell, B::Steam));
                        thermal.push((
                            cell,
                            ThermalCell {
                                temperature: 135.0,
                                fuel: 0.0,
                                gas: 8,
                            },
                        ));
                    } else if b == B::Air
                        && spawn_fire
                        && fires_placed < 6
                        && world.solid(cell - IVec3::Y)
                    {
                        edits.push((cell, B::Fire));
                        thermal.push((
                            cell,
                            ThermalCell {
                                temperature: 820.0,
                                fuel: 75.0,
                                gas: 0,
                            },
                        ));
                        fires_placed += 1;
                    } else if world.sandbox.can_admit(&[cell]) {
                        let mut tc = world.sandbox.cell(cell);
                        tc.temperature = tc.temperature.max(680.0);
                        world.sandbox.set(cell, tc);
                        world.activity.wake(cell);
                    }
                }
            }
        }
        self.apply_terrain_edits(world, &edits);
        for (cell, state) in thermal {
            world.sandbox.set(cell, state);
        }
        for e in &mut self.enemies {
            let d = (e.center() - center).length();
            if e.health > 0.0 && d < radius_m + 0.8 {
                e.health -= 24.0 * (1.0 - d / (radius_m + 1.2)).max(0.25);
                e.flash = 0.2;
                e.slow = 0.0;
                self.hit = 0.16;
            }
        }
        self.spawn_voxel_burst(center, spell_color(12), 24, 3.1, 0.046);
    }

    pub(super) fn electrify_pool(&mut self, world: &mut World, center: Vec3, rank: u32) {
        let scale = world.voxel_scale.meters();
        let c_cell = (center / scale).floor().as_ivec3();
        let mut wet_cells = Vec::new();
        for dy in -2..=2 {
            for dx in -4..=4 {
                for dz in -4..=4 {
                    let cell = c_cell + IVec3::new(dx, dy, dz);
                    if world.get(cell) == B::Water && wet_cells.len() < 12 {
                        wet_cells.push(cell);
                    }
                    let anchor = factory::anchor(cell, factory::step(world));
                    if let Some(node) = world.factory.nodes.get_mut(&anchor)
                        && node.kind.powered() {
                            node.energy = 64;
                        }
                }
            }
        }
        for &wcell in &wet_cells {
            let wp = (wcell.as_vec3() + Vec3::new(0.5, 0.85, 0.5)) * scale;
            if self.arcs.len() < 32 {
                self.arcs.push(Arc {
                    a: center,
                    b: wp,
                    life: 0.22,
                });
            }
            for e in &mut self.enemies {
                if e.health > 0.0 && (e.center() - wp).length() < 2.6 {
                    e.health -= 18.0 + rank as f32 * 2.5;
                    e.flash = 0.22;
                    e.stagger = 0.35;
                    self.hit = 0.2;
                }
            }
        }
        self.spawn_voxel_burst(center, spell_color(14), 24, 3.6, 0.042);
    }

    pub(super) fn alchemical_transmute(&mut self, world: &mut World, center: Vec3, radius_m: f32) {
        let scale = world.voxel_scale.meters();
        let c_cell = (center / scale).floor().as_ivec3();
        let r = (radius_m / scale).ceil().clamp(1.0, 3.0) as i32;
        let r_sq = (radius_m / scale) * (radius_m / scale);
        let mut edits = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    if (dx * dx + dy * dy + dz * dz) as f32 > r_sq {
                        continue;
                    }
                    let cell = c_cell + IVec3::new(dx, dy, dz);
                    let old = world.get(cell);
                    let transmuted = match old {
                        B::Stone | B::Cobblestone | B::Limestone | B::Basalt => {
                            let h = (cell.x.wrapping_mul(37)
                                ^ cell.y.wrapping_mul(19)
                                ^ cell.z.wrapping_mul(73))
                            .unsigned_abs()
                                % 5;
                            if h == 0 {
                                Some(B::CopperOre)
                            } else if h == 1 {
                                Some(B::IronOre)
                            } else {
                                Some(B::Crystal)
                            }
                        }
                        B::Dirt | B::Grass | B::Mud | B::Clay | B::Sand | B::Gravel => {
                            Some(B::GlowCap)
                        }
                        _ => None,
                    };
                    if let Some(nb) = transmuted {
                        edits.push((cell, nb));
                    }
                }
            }
        }
        self.apply_terrain_edits(world, &edits);
        self.spawn_voxel_burst(center, spell_color(15), 26, 2.4, 0.045);
    }

    pub(super) fn tick_crosses(&mut self, world: &mut World, dt: f32) {
        let rank = self.rank();
        let mut heat_points = Vec::new();
        for cross in &mut self.crosses {
            cross.life -= dt;
            cross.pulse -= dt;
            if cross.pulse <= 0.0 {
                cross.pulse = 0.25;
                heat_points.push(cross.pos);
                for e in &mut self.enemies {
                    if e.health > 0.0
                        && (e.center() - cross.pos).length() < 2.2
                        && line(world, cross.pos, e.center())
                    {
                        e.health -= 12.0 + rank as f32 * 2.0;
                        e.flash = 0.14;
                        self.hit = 0.14;
                    }
                }
            }
        }
        for p in heat_points {
            let a = self.time * 4.2;
            for i in 0..4 {
                let ang = a + i as f32 * std::f32::consts::FRAC_PI_2;
                let dir =
                    Vec3::new(ang.cos(), (ang * 0.5).sin() * 0.35, ang.sin()).normalize_or_zero();
                self.spawn_voxel_particle(
                    p + dir * 1.1,
                    dir * 1.8 + Vec3::Y * 0.4,
                    spell_color(11),
                    0.034,
                    1.2,
                    0.4,
                    -0.3,
                    0.35,
                );
            }
            let cell = (p / world.voxel_scale.meters()).floor().as_ivec3();
            if world.sandbox.can_admit(&[cell]) {
                let mut tc = world.sandbox.cell(cell);
                tc.temperature = tc.temperature.max(460.0);
                world.sandbox.set(cell, tc);
                world.activity.wake(cell);
            }
        }
        self.crosses.retain(|c| c.life > 0.0);
    }

    pub(super) fn tick_particles(&mut self, world: &World, dt: f32) {
        let scale = world.voxel_scale.meters();
        for p in &mut self.particles {
            p.life -= dt;
            p.vel.y -= p.gravity * dt;
            if p.gravity < 0.5 {
                // Rising pickup fog tumbles as it spreads instead of flying in a straight line.
                let curl = Vec3::new(
                    (p.pos.y * 1.4 + self.time * 0.7).sin(),
                    0.2,
                    (p.pos.x * 1.4 + self.time * 0.5).cos(),
                );
                p.vel += curl * dt * 0.35;
            }
            p.vel *= (1.0 - dt * 1.4).max(0.0);
            let next = p.pos + p.vel * dt;
            let cell = (next / scale).floor().as_ivec3();
            if world.solid(cell) {
                p.vel = -p.vel * 0.42 + Vec3::Y * 0.35;
            } else {
                p.pos = next;
            }
        }
        self.particles.retain(|p| p.life > 0.0);
        for l in &mut self.lasers {
            l.life -= dt;
        }
        self.lasers.retain(|l| l.life > 0.0);
    }

    pub(super) fn chain(&mut self, world: &World, mut from: Vec3, first: (IVec3, u32), rank: u32) {
        let mut visited = vec![first];
        for _ in 0..if self.knows(1) { 3 } else { 2 } {
            let target = self
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0.0 && !visited.contains(&(e.site, e.slot)))
                .filter_map(|(i, e)| {
                    let d = (e.center() - from).length();
                    (d < if self.knows(1) { 5.5 } else { 4.5 } && line(world, from, e.center()))
                        .then_some((i, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((i, _)) = target else {
                break;
            };
            let e = &mut self.enemies[i];
            let frozen = e.slow > 0.0;
            e.health -= 10.0 + rank as f32 * 2.0 + if frozen { 12.0 } else { 0.0 };
            e.slow = 0.0;
            e.flash = 0.2;
            e.stagger = 0.18;
            if e.kind < 6 || frozen && e.poise <= 0.0 {
                e.tell = 0.0;
                if is_boss(e.kind) {
                    e.poise = 3.0;
                }
            }
            visited.push((e.site, e.slot));
            let to = e.center();
            if self.arcs.len() < 32 {
                self.arcs.push(Arc {
                    a: from,
                    b: to,
                    life: 0.18,
                });
            }
            sound(
                &mut self.sounds,
                if frozen { Cue::Shatter } else { Cue::Spark },
                Some(to),
                0.4,
            );
            self.hit = 0.18;
            from = to;
        }
    }

    pub(super) fn tick_blooms(&mut self, world: &mut World, eye: Vec3, dt: f32) {
        let max_health = self.max_health();
        let healing = if self.knows(4) { 4.0 } else { 2.0 };
        let scale = world.voxel_scale.meters();
        let mut spore_seeds = Vec::new();
        for bloom in &mut self.blooms {
            bloom.life -= dt;
            bloom.pulse -= dt;
            if bloom.pulse > 0.0 {
                continue;
            }
            bloom.pulse = 0.7;
            spore_seeds.push(bloom.pos);
            let mut hit = false;
            for e in &mut self.enemies {
                if e.health > 0.0
                    && (e.center() - bloom.pos).length() < 3.2
                    && line(world, bloom.pos, e.center())
                {
                    e.health -= 6.0;
                    e.flash = 0.15;
                    hit = true;
                }
            }
            if (eye - bloom.pos).length() < 3.2 && line(world, bloom.pos, eye)
                && self.health > 0.0 {
                    self.health = (self.health + healing).min(max_health);
                }
            if hit {
                self.hit = 0.14;
            }
        }
        for pos in spore_seeds {
            let cell = ((pos - Vec3::Y * 0.25) / scale).floor().as_ivec3();
            if matches!(world.get(cell), B::Dirt | B::Grass) {
                world.set(cell, B::FungalSoil);
                self.mark_voxel_dirty(world, cell);
            }
            self.spawn_voxel_burst(pos + Vec3::Y * 0.35, spell_color(5), 6, 0.9, 0.038);
        }
        self.blooms.retain(|b| b.life > 0.0);
    }

    pub(super) fn cast_timberfall(&mut self, world: &mut World, eye: Vec3, dir: Vec3) {
        let hit = wands::aim_cell(world, eye, dir, 28.0);
        let Some((hit_cell, _)) = hit else {
            self.effect(eye + dir * 1.5, spell_color(24), 0.5);
            return;
        };
        let mut target = if world.get(hit_cell) == B::Log {
            Some(hit_cell)
        } else {
            // Aiming at leaves should still find the supporting trunk nearby.
            let mut found = None;
            for dy in -2..=2 {
                for dz in -2..=2 {
                    for dx in -2..=2 {
                        let p = hit_cell + IVec3::new(dx, dy, dz);
                        if world.get(p) == B::Log {
                            found = Some(p);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            found
        };
        let Some(target_cell) = target.take() else {
            self.effect(eye + dir * 1.5, spell_color(24), 0.5);
            return;
        };
        self.effect(
            (target_cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters(),
            spell_color(24),
            1.2,
        );
        if let Some(cells) = super::super::expedition::fell_tree(world, target_cell) {
            for cell in cells {
                self.mark_voxel_dirty(world, cell);
            }
        }
        self.spawn_voxel_burst(
            (target_cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters(),
            spell_color(24),
            18,
            2.2,
            0.04,
        );
    }

    pub(super) fn cast_gale(&mut self, world: &mut World, eye: Vec3, dir: Vec3) {
        let center = eye + dir * 2.2;
        self.kinetic_pulse(world, center, 6.0, 5.0);
        self.effect(center, spell_color(25), 1.6);
        self.spawn_voxel_burst(center, spell_color(25), 28, 3.4, 0.042);
    }

    /// Screen-space magic envelope for the resolve. This is deliberately
    /// additive and short-lived: ordinary exploration stays clean, while a cast,
    /// ward impact, singularity or laser storm visibly bends the image.
    pub fn screen_magic(&self) -> (f32, [f32; 3]) {
        let cast = (self.cast_pose / 0.4).clamp(0.0, 1.0).powf(1.35) * 0.36;
        let mana = (self.mana_flash * 1.6).clamp(0.0, 1.0) * 0.20;
        let ward = (self.ward * 0.20).clamp(0.0, 0.18);
        let projectiles = (self.projectiles.len() as f32 * 0.022).min(0.20);
        let particles = (self.particles.len() as f32 / 480.0 * 0.03).min(0.04);
        let fields = ((self.crosses.len() + self.wells.len()) as f32 * 0.11).min(0.24);
        let lasers = (self.lasers.len() as f32 * 0.10).min(0.20);
        let intensity =
            (cast + mana + ward + projectiles + particles + fields + lasers).clamp(0.0, 0.88);
        let mut color = spell_color(if self.last_cast == 8 {
            self.spell
        } else {
            self.last_cast
        });
        if self.ward > 0.0 {
            let ward = spell_color(4);
            color = [
                color[0] * 0.65 + ward[0] * 0.35,
                color[1] * 0.65 + ward[1] * 0.35,
                color[2] * 0.65 + ward[2] * 0.35,
            ];
        }
        (intensity, color)
    }

    /// A short-lived light at the wand tip. The emissive mesh alone lit nothing
    /// around it, so a cast in a cave read as a glowing prop rather than an
    /// event. This piggybacks on the existing dynamic torch-light array.
    pub fn cast_light(&self, eye: Vec3, forward: Vec3) -> Option<(Vec3, f32, [f32; 3], f32)> {
        if self.cast_pose <= 0.01 || self.wand == 0 {
            return None;
        }
        let t = self.cast_motion().clamp(0.0, 1.0);
        let spell = if self.last_cast == 8 {
            self.spell
        } else {
            self.last_cast
        };
        Some((
            eye + forward * 0.82,
            5.8,
            spell_color(spell),
            (1.65 * t * t).max(0.0),
        ))
    }

    pub fn magic_models(&self, m: &mut Model, eye: Vec3, forward: Vec3) {
        self.danger_models(m);
        self.relic_models(m);
        self.well_models(m);

        // 3D Glowing Translucent Voxel Particles (Noita-style living 3D pixel sparks & motes)
        for p in &self.particles {
            let t = (p.life / p.total).clamp(0.0, 1.0);
            let s = p.size * (0.35 + 0.65 * t);
            // Fixed pose. A spinning cube catches a new edge every frame.
            let rot = Quat::from_rotation_y(p.pos.x * 1.7) * Quat::from_rotation_x(p.pos.z);
            let dissolve = ((1.0 - t) * 1.35 - 0.55).clamp(0.0, 1.0);
            m.glow_voxel_with_dissolve(
                p.pos,
                Vec3::splat(s),
                rot,
                p.color,
                p.glow * t,
                (p.alpha * t).clamp(0.0, 0.7),
                dissolve,
            );
        }

        // Luminous Drill 3D Laser Beams with helical voxel rings
        for laser in &self.lasers {
            let fade = (laser.life / laser.total).clamp(0.0, 1.0);
            m.beam(
                laser.a,
                laser.b,
                laser.radius * 0.52 * fade,
                [0.72, 0.78, 0.74],
                1.6 * fade,
            );
            m.glow_beam(
                laser.a,
                laser.b,
                laser.radius * 1.45 * fade,
                laser.color,
                1.3 * fade,
                0.35 * fade,
            );
            let axis = (laser.b - laser.a).normalize_or_zero();
            let right = axis
                .cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X })
                .normalize_or_zero();
            let up = axis.cross(right);
            for i in 0..6 {
                let u = i as f32 / 6.0;
                let ang = u * 8.0 - self.time * 0.8;
                let ring = (right * ang.cos() + up * ang.sin()) * (laser.radius * 2.2 * fade);
                m.glow_voxel(
                    laser.a.lerp(laser.b, u) + ring,
                    Vec3::splat(0.032 * fade),
                    Quat::from_rotation_y(ang),
                    laser.color,
                    1.2 * fade,
                    0.4 * fade,
                );
            }
        }

        // Plasma Beam Crosses (4 rotating 3D laser blades + floating temple core)
        for cross in &self.crosses {
            let fade = cross.life.min(1.0);
            let c_col = spell_color(11);
            let rot = Quat::from_rotation_y(self.time * 0.55)
                * Quat::from_rotation_z((self.time * 0.4).sin() * 0.18);
            m.glow_orb(cross.pos, 0.14 * fade, c_col, 1.5 * fade, 0.45);
            m.temple_glyph(
                cross.pos,
                Quat::from_rotation_y(-self.time * 0.4),
                2,
                0.028 * fade,
                c_col,
                1.3 * fade,
                0.4 * fade,
            );
            for dir in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let tip = cross.pos + rot * (dir * 1.75 * fade);
                m.beam(cross.pos, tip, 0.016 * fade, c_col, 1.2 * fade);
                m.glow_beam(cross.pos, tip, 0.04 * fade, c_col, 1.15 * fade, 0.32 * fade);
                for k in 1..=5 {
                    let p = cross.pos.lerp(tip, k as f32 / 5.0);
                    m.glow_voxel(
                        p,
                        Vec3::splat(0.042 * fade),
                        rot,
                        c_col,
                        1.15 * fade,
                        0.38 * fade,
                    );
                }
            }
        }

        // Branching Lightning Arcs with translucent plasma sheath & voxel nodes
        for arc in &self.arcs {
            let delta = arc.b - arc.a;
            let side = delta.cross(Vec3::Y).normalize_or_zero();
            let fade = (arc.life / 0.18).clamp(0.0, 1.0);
            let mut previous = arc.a;
            for j in 1..=8 {
                let t = j as f32 / 8.0;
                let point = arc.a
                    + delta * t
                    + side
                        * if j == 8 {
                            0.0
                        } else {
                            (j as f32 * 2.17 + arc.a.x * 1.7 + arc.a.z * 0.9).sin() * 0.06
                        };
                m.beam(previous, point, 0.012, [0.86, 0.78, 0.58], 1.4 * fade);
                m.glow_beam(
                    previous,
                    point,
                    0.038,
                    spell_color(0),
                    1.15 * fade,
                    0.32 * fade,
                );
                m.glow_voxel(
                    point,
                    Vec3::splat(0.026 * fade),
                    Quat::IDENTITY,
                    spell_color(14),
                    1.25 * fade,
                    0.4 * fade,
                );
                previous = point;
            }
        }

        // Spore Blooms with floating translucent micro-voxel cloud & temple vigour glyph
        for bloom in &self.blooms {
            let fade = bloom.life.min(1.0);
            m.temple_glyph(
                bloom.pos + Vec3::Y * 0.42,
                Quat::from_rotation_y(self.time * 0.9),
                1,
                0.034 * fade,
                spell_color(5),
                1.2 * fade,
                0.4 * fade,
            );
            for i in 0..8 {
                let a = self.time * 0.28 + i as f32 * 2.39996;
                let radius = 0.55 + (i % 6) as f32 * 0.42;
                let p = bloom.pos
                    + Vec3::new(
                        a.cos() * radius,
                        (a * 1.3 + i as f32).sin() * 0.42 + 0.34,
                        a.sin() * radius,
                    );
                m.glow_voxel(
                    p,
                    Vec3::splat((0.048 + (i % 3) as f32 * 0.016) * fade),
                    Quat::from_rotation_y(a) * Quat::from_rotation_x(a * 0.7),
                    if i % 3 == 0 {
                        spell_color(1)
                    } else {
                        spell_color(5)
                    },
                    1.05 * fade,
                    0.38 * fade,
                );
            }
            m.glow_orb(bloom.pos, 0.18 * fade, spell_color(5), 2.4 * fade, 0.72);
        }

        // Mirror Ward: peripheral floating 3D translucent crystal runes
        if self.ward > 0.0 && self.ward_hits > 0 {
            let right = forward.cross(Vec3::Y).normalize_or_zero();
            let up = right.cross(forward);
            for i in 0..self.ward_hits {
                let a = i as f32 * 2.094 + self.time * 0.85;
                let p = eye + forward * 0.72 + right * a.cos() * 0.38 + up * a.sin() * 0.27;
                let rot = Quat::from_rotation_z(a) * Quat::from_rotation_y(self.time * 1.4);
                m.glow_voxel(
                    p,
                    Vec3::new(0.048, 0.13, 0.036),
                    rot,
                    spell_color(4),
                    2.8,
                    0.68,
                );
                m.glow_voxel(p, Vec3::splat(0.02), rot, spell_color(4), 1.2, 0.4);
            }
        }
    }
}
