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
        self.particles.push(VoxelParticle {
            pos,
            vel,
            color,
            size,
            glow,
            alpha,
            gravity,
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
            let tint = if i % 3 == 0 {
                [
                    (color[0] * 1.25 + 0.12).min(1.0),
                    (color[1] * 1.25 + 0.12).min(1.0),
                    (color[2] * 1.25 + 0.12).min(1.0),
                ]
            } else {
                color
            };
            self.spawn_voxel_particle(
                pos + dir * 0.05,
                dir * v_mag,
                tint,
                size * (0.8 + (i % 3) as f32 * 0.22),
                3.2,
                0.76,
                if i % 2 == 0 { 3.2 } else { -0.6 },
                0.45 + (i % 4) as f32 * 0.14,
            );
        }
    }

    pub(super) fn mark_voxel_dirty(&mut self, world: &mut World, cell: IVec3) {
        world.activity.wake(cell);
        if self.dirty_voxels.len() < 2048 {
            self.dirty_voxels.push(cell);
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
        let mut blocks = std::collections::HashSet::new();
        let mut chunks = std::collections::HashSet::new();
        if harvest {
            super::super::expedition::blast_harvest(world, cell, &mut blocks, &mut chunks);
        } else {
            world.set(cell, B::Air);
        }
        self.mark_voxel_dirty(world, cell);
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
                e.health -= (26.0 + rank as f32 * 3.5 + if shatter { 16.0 } else { 0.0 })
                    * mods.damage_mul;
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
                4.2,
                0.82,
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
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    if (dx * dx + dy * dy + dz * dz) as f32 > r_sq {
                        continue;
                    }
                    let cell = c_cell + IVec3::new(dx, dy, dz);
                    let b = world.get(cell);
                    if b == B::Water {
                        world.set(cell, B::Ice);
                        world.sandbox.set(
                            cell,
                            ThermalCell {
                                temperature: -5.0,
                                fuel: 0.0,
                                gas: 0,
                            },
                        );
                        self.mark_voxel_dirty(world, cell);
                    } else if b == B::Fire {
                        world.set(cell, B::Ash);
                        world.sandbox.set(cell, ThermalCell::default());
                        self.mark_voxel_dirty(world, cell);
                    } else if b == B::Lava {
                        world.set(cell, B::Basalt);
                        world.sandbox.set(cell, ThermalCell::default());
                        self.mark_voxel_dirty(world, cell);
                    }
                }
            }
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
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    if (dx * dx + dy * dy + dz * dz) as f32 > r_sq {
                        continue;
                    }
                    let cell = c_cell + IVec3::new(dx, dy, dz);
                    let b = world.get(cell);
                    if b == B::Ice {
                        world.set_water_volume(cell, 8);
                        self.mark_voxel_dirty(world, cell);
                    } else if b == B::SnowGrass {
                        world.set(cell, B::Dirt);
                        self.mark_voxel_dirty(world, cell);
                    } else if b == B::Water {
                        world.set(cell, B::Steam);
                        world.sandbox.set(
                            cell,
                            ThermalCell {
                                temperature: 135.0,
                                fuel: 0.0,
                                gas: 8,
                            },
                        );
                        self.mark_voxel_dirty(world, cell);
                    } else if b == B::Air
                        && spawn_fire
                        && fires_placed < 6
                        && world.solid(cell - IVec3::Y)
                    {
                        world.set(cell, B::Fire);
                        world.sandbox.set(
                            cell,
                            ThermalCell {
                                temperature: 820.0,
                                fuel: 75.0,
                                gas: 0,
                            },
                        );
                        self.mark_voxel_dirty(world, cell);
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
                    if let Some(node) = world.factory.nodes.get_mut(&anchor) {
                        if node.kind.powered() {
                            node.energy = 64;
                        }
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
                        world.set(cell, nb);
                        self.mark_voxel_dirty(world, cell);
                    }
                }
            }
        }
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
                let dir = Vec3::new(ang.cos(), (ang * 0.5).sin() * 0.35, ang.sin()).normalize_or_zero();
                self.spawn_voxel_particle(
                    p + dir * 1.1,
                    dir * 1.8 + Vec3::Y * 0.4,
                    spell_color(11),
                    0.034,
                    4.0,
                    0.80,
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
            p.vel *= (1.0 - dt * 1.8).max(0.0);
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
                if e.kind >= 6 {
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
            if (eye - bloom.pos).length() < 3.2 && line(world, bloom.pos, eye) {
                if self.health > 0.0 {
                    self.health = (self.health + healing).min(max_health);
                }
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

    pub fn magic_models(&self, m: &mut Model, eye: Vec3, forward: Vec3) {
        self.danger_models(m);
        self.relic_models(m);
        self.well_models(m);

        // 3D Glowing Translucent Voxel Particles (Noita-style living 3D pixel sparks & motes)
        for p in &self.particles {
            let t = (p.life / p.total).clamp(0.0, 1.0);
            let s = p.size * (0.45 + 0.55 * t);
            let rot = Quat::from_rotation_y(self.time * 2.2 + p.pos.x * 1.7)
                * Quat::from_rotation_x(p.pos.z * 1.3);
            m.glow_voxel(
                p.pos,
                Vec3::splat(s),
                rot,
                p.color,
                p.glow * (0.5 + 0.5 * t),
                (p.alpha * t).clamp(0.14, 0.92),
            );
        }

        // Luminous Drill 3D Laser Beams with helical voxel rings
        for laser in &self.lasers {
            let fade = (laser.life / laser.total).clamp(0.0, 1.0);
            m.beam(
                laser.a,
                laser.b,
                laser.radius * 0.52 * fade,
                [0.92, 1.0, 0.96],
                5.2 * fade,
            );
            m.glow_beam(
                laser.a,
                laser.b,
                laser.radius * 1.45 * fade,
                laser.color,
                3.4 * fade,
                0.58 * fade,
            );
            let axis = (laser.b - laser.a).normalize_or_zero();
            let right = axis
                .cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X })
                .normalize_or_zero();
            let up = axis.cross(right);
            for i in 0..14 {
                let u = i as f32 / 14.0;
                let ang = u * 18.0 - self.time * 24.0;
                let ring = (right * ang.cos() + up * ang.sin()) * (laser.radius * 2.2 * fade);
                m.glow_voxel(
                    laser.a.lerp(laser.b, u) + ring,
                    Vec3::splat(0.032 * fade),
                    Quat::from_rotation_y(ang),
                    laser.color,
                    4.0 * fade,
                    0.75 * fade,
                );
            }
        }

        // Plasma Beam Crosses (4 rotating 3D laser blades + floating temple core)
        for cross in &self.crosses {
            let fade = cross.life.min(1.0);
            let c_col = spell_color(11);
            let rot = Quat::from_rotation_y(self.time * 2.6)
                * Quat::from_rotation_z((self.time * 1.3).sin() * 0.45);
            m.glow_orb(cross.pos, 0.16 * fade, [0.92, 0.98, 1.0], 4.8 * fade, 0.88);
            m.temple_glyph(
                cross.pos,
                Quat::from_rotation_y(-self.time * 1.8),
                2,
                0.032 * fade,
                c_col,
                3.8 * fade,
                0.78 * fade,
            );
            for dir in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let tip = cross.pos + rot * (dir * 1.75 * fade);
                m.beam(cross.pos, tip, 0.022 * fade, [0.95, 0.99, 1.0], 4.5 * fade);
                m.glow_beam(cross.pos, tip, 0.056 * fade, c_col, 3.0 * fade, 0.52 * fade);
                for k in 1..=5 {
                    let p = cross.pos.lerp(tip, k as f32 / 5.0);
                    m.glow_voxel(
                        p,
                        Vec3::splat(0.042 * fade),
                        rot,
                        c_col,
                        3.6 * fade,
                        0.72 * fade,
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
                            (j as f32 * 13.7 + self.time * 32.0).sin() * 0.13
                        };
                m.beam(previous, point, 0.015, [1.0, 0.96, 0.82], 4.2 * fade);
                m.glow_beam(
                    previous,
                    point,
                    0.038,
                    spell_color(0),
                    2.6 * fade,
                    0.55 * fade,
                );
                m.glow_voxel(
                    point,
                    Vec3::splat(0.038 * fade),
                    Quat::IDENTITY,
                    spell_color(14),
                    3.8 * fade,
                    0.78 * fade,
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
                3.2 * fade,
                0.74 * fade,
            );
            for i in 0..24 {
                let a = self.time * 0.8 + i as f32 * 2.39996;
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
                    2.6 * fade,
                    0.66 * fade,
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
                m.glow_voxel(
                    p,
                    Vec3::splat(0.028),
                    rot,
                    [0.92, 0.99, 1.0],
                    4.0,
                    0.88,
                );
            }
        }
    }
}
