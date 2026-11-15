use super::*;
use crate::simulation::sandbox::ThermalCell;
use crate::world::BlockId as B;

pub(crate) const STORM_SURGE: u32 = 98;
pub(crate) const DONT_PANIC: u32 = 100;
pub(crate) const DONT_PANIC_SECONDS: f32 = 4.2;
pub(super) const SURGE_CELLS: usize = 28;
pub(super) const SURGE_DAMAGE_BONUS: f32 = 16.0;
pub(super) const PLASMA_DIRECTIONS: [Vec3; 4] = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z];

#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct VoxelParticle {
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub seed: u32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub color: [f32; 3],
    pub size: f32,
    pub glow: f32,
    pub alpha: f32,
    pub gravity: f32,
    pub life: f32,
    pub total: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub(super) family: magic_fx::Family,
}

#[derive(Clone)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct LaserBeam {
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub path: Vec<Vec3>,
    pub a: Vec3,
    pub b: Vec3,
    pub color: [f32; 3],
    pub radius: f32,
    pub life: f32,
    pub total: f32,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct PlasmaCross {
    pub pos: Vec3,
    pub normal: Vec3,
    pub life: f32,
    pub pulse: f32,
}

pub(crate) const MAX_SPLASHES: usize = 6;

#[derive(Clone, Copy)]
pub(super) enum Spray {
    Water,
    Lava,
    Acid,
}

impl Spray {
    pub(super) fn viscous_liquid(block: B) -> Option<Self> {
        match block {
            B::Lava => Some(Self::Lava),
            B::SulfuricAcid => Some(Self::Acid),
            _ => None,
        }
    }
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
        self.spawn_styled_particle(pos, vel, color, size, glow, alpha, gravity, life, magic_fx::Family::Plain);
    }

    pub(super) fn spawn_styled_particle(
        &mut self, pos: Vec3, vel: Vec3, color: [f32; 3], size: f32, glow: f32,
        alpha: f32, gravity: f32, life: f32, family: magic_fx::Family,
    ) {
        self.make_particle_room(1);
        let fx = self.zone_fx;
        let life = life * fx.life_mul;
        self.particles.push(VoxelParticle {
            seed: pos.x.to_bits() ^ pos.y.to_bits().rotate_left(11)
                ^ pos.z.to_bits().rotate_left(7) ^ vel.x.to_bits().rotate_left(5)
                ^ vel.y.to_bits().rotate_left(23) ^ vel.z.to_bits().rotate_left(19)
                ^ self.time.to_bits(),
            pos,
            vel,
            color: fx.tinted(color),
            size: size * fx.size_mul,
            glow: fx.glow(glow, pos.x * 1.3 + pos.z * 0.7 + self.time),
            alpha,
            gravity: gravity * fx.gravity_mul,
            life,
            total: life.max(0.05),
            family,
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
        self.make_particle_room(count);
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

    fn make_particle_room(&mut self, count: usize) {
        const MAX_PARTICLES: usize = 480;
        let excess = (self.particles.len() + count.min(MAX_PARTICLES)).saturating_sub(MAX_PARTICLES);
        if excess > 0 {
            self.particles.drain(..excess);
        }
    }

    pub(crate) fn preview_spell_contact(&mut self, name: &str, eye: Vec3, forward: Vec3) -> bool {
        use magic_fx::{Contact, Family};
        let family = match name {
            "spark" => Family::Spark,
            "ember" => Family::Ember,
            "frost" => Family::Frost,
            "shock" => Family::Shock,
            "rift" => Family::Rift,
            "lumen" => Family::Lumen,
            _ => return false,
        };
        self.spell_contact(eye + forward * 2.4, forward * 4.0, family.look().hot,
            family, Contact::Surface(-forward));
        true
    }

    pub(super) fn spell_contact(
        &mut self, at: Vec3, velocity: Vec3, color: [f32; 3], family: magic_fx::Family,
        contact: magic_fx::Contact,
    ) {
        use magic_fx::{Contact, Family};
        let normal = match contact {
            Contact::Surface(normal) => normal,
            _ => (-velocity).normalize_or(Vec3::Y),
        };
        let tangent = if matches!(contact, Contact::Surface(_)) {
            let incoming = velocity.normalize_or_zero();
            incoming - normal * incoming.dot(normal)
        } else {
            Vec3::ZERO
        };
        let side = normal.any_orthonormal_vector();
        let up = normal.cross(side);
        let look = family.look();
        let count = match contact {
            Contact::Actor if matches!(family, Family::Shock | Family::Frost) => 7,
            Contact::Actor => 5,
            Contact::Surface(_) => 4,
            Contact::Glance => 2,
        };
        let origin = at + normal * 0.04;
        self.make_particle_room(count);
        for i in 0..count {
            let angle = i as f32 * 2.39996 + self.time * 8.0;
            let (sin, cos) = angle.sin_cos();
            let radial = side * cos + up * sin;
            let variation = ((i * 7 + 3) % 11) as f32 / 10.0;
            let speed = look.speed * (0.65 + variation * 0.55);
            let recoil = if matches!(contact, Contact::Surface(_)) { 1.35 } else { 0.42 };
            let mut spray = radial * speed + normal * recoil + tangent * speed * 0.6
                + Vec3::Y * look.lift * (0.55 + variation);
            if matches!(contact, Contact::Surface(_)) {
                spray += normal * (0.12 - spray.dot(normal)).max(0.0);
            }
            self.spawn_styled_particle(origin + radial * 0.025, spray, color,
                0.017 + variation * 0.014, look.glow, 0.68, look.gravity,
                look.life * (0.78 + variation * 0.36), family);
        }
        if self.effects.len() < 64 {
            let (radius, duration_mul) = match contact {
                Contact::Actor => (0.42, 1.0),
                Contact::Surface(_) => (0.27, 0.80),
                Contact::Glance => (0.21, 0.55),
            };
            let duration = (look.life * 0.73).clamp(0.14, 0.36) * duration_mul;
            self.effects.push(Effect {
                pos: origin,
                color,
                radius,
                life: duration,
                total: duration,
                impact_normal: Some(normal),
                family,
            });
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
        let radius = 0.34 + 1.05 * strength;
        if let Some(existing) = self.splashes.iter_mut().find(|splash| {
            splash.age < 0.18 && splash.pos.distance_squared(surface) < 0.75 * 0.75
        }) {
            existing.strength = existing.strength.max(strength);
            existing.radius = existing.radius.max(radius);
            existing.life = existing.life.max(0.56 + 0.38 * strength);
            return;
        }
        if self.splashes.len() >= MAX_SPLASHES {
            self.splashes.remove(0);
        }
        let life = 0.56 + 0.38 * strength;
        self.splashes.push(crate::tool_effects::Splash {
            pos: surface,
            radius,
            age: 0.0,
            life,
            strength,
        });
        self.splash_droplets(surface, strength, Spray::Water, Vec3::ZERO);
        sound(
            &mut self.sounds,
            Cue::Splash,
            Some(surface),
            0.42 + strength * 0.38,
        );
    }

    pub(super) fn splash_droplets(&mut self, surface: Vec3, strength: f32, spray: Spray, push: Vec3) {
        let strength = strength.clamp(0.12, 1.0);
        let family = match spray {
            Spray::Water => magic_fx::Family::Water,
            Spray::Lava => magic_fx::Family::Ember,
            Spray::Acid => magic_fx::Family::Acid,
        };
        let (body, highlight, speed_scale, size_scale, glow, lifetime) = match spray {
            Spray::Water => ([0.65, 0.87, 0.91], [0.91, 0.97, 0.98], 1.0, 1.0, 2.0, 1.0),
            Spray::Lava => ([0.84, 0.11, 0.015], [1.0, 0.46, 0.055], 0.57, 1.35, 2.7, 1.15),
            Spray::Acid => ([0.34, 0.74, 0.08], [0.76, 1.0, 0.23], 0.85, 0.92, 1.0, 0.8),
        };
        let count = (3.0 + strength * 9.0).round() as usize;
        self.make_particle_room(count);
        for i in 0..count {
            let a = i as f32 * 2.399963 + self.time * 4.1 + surface.x * 0.37;
            let radial = Vec3::new(a.cos(), 0.0, a.sin());
            let lift = 1.15 + (i % 5) as f32 * 0.22 + strength * 1.8;
            let speed = (0.8 + strength * 2.5) * (0.7 + (i % 4) as f32 * 0.11);
            let tint = if i % 4 == 0 { highlight } else { body };
            self.spawn_styled_particle(
                surface
                    + radial * (0.10 + (i % 3) as f32 * 0.06)
                    + Vec3::Y * (0.015 + (i % 2) as f32 * 0.025),
                (radial * speed + Vec3::Y * lift * (0.7 + (i % 3) as f32 * 0.12)) * speed_scale + push,
                tint,
                (0.026 + (i % 3) as f32 * 0.012) * (0.8 + strength * 0.5) * size_scale,
                glow + strength * 1.2,
                0.74 + strength * 0.18,
                7.5,
                (0.38 + (i % 5) as f32 * 0.075 + strength * 0.20) * lifetime,
                family,
            );
        }
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

    /// Intentionally partial spell batch. Reserve dirty-list room before any
    /// writes, and publish only successful cells without per-cell revisions.
    pub(super) fn apply_terrain_edits(
        &mut self, world: &mut World, edits: &[(IVec3, B)],
    ) -> std::collections::HashSet<IVec3> {
        let room = 2048usize.saturating_sub(self.dirty_voxels.len());
        let admitted: Vec<_> = edits.iter().copied()
            .filter(|(cell, _)| super::super::orbital::habitat::ordinary_spell_allowed(world, *cell))
            .filter(|(cell, _)| super::super::effects::admit(world, *cell).is_ok())
            .take(room).collect();
        let mut changed = std::collections::HashSet::new();
        world.set_many_committed(&admitted, |cell, _, _| {
            self.dirty_voxels.push(cell);
            changed.insert(cell);
        });
        changed
    }

    fn replace_terrain_cell(&mut self, world: &mut World, cell: IVec3, old: B, new: B) -> bool {
        if !super::super::orbital::habitat::ordinary_spell_allowed(world, cell) { return false; }
        if self.dirty_voxels.len() >= 2048 {
            return false;
        }
        let Ok(commit) = super::super::effects::replace(world, cell, old, new) else {
            return false;
        };
        commit.publish_voxel(world, &mut self.dirty_voxels);
        true
    }

    pub(super) fn carve_cell(&mut self, world: &mut World, cell: IVec3, harvest: bool) -> bool {
        self.carve_cell_with(world, cell, harvest, true).is_some()
    }

    /// Removes terrain at `cell` with the same guards as spell carving and
    /// returns the block it held; `burst` spawns the per-cell dust puff.
    pub(super) fn carve_cell_with(&mut self, world: &mut World, cell: IVec3, harvest: bool, burst: bool) -> Option<B> {
        if !super::super::orbital::habitat::ordinary_spell_allowed(world, cell) { return None; }
        let cfg = world.sample_config();
        if (cell.y <= cfg.min_world_y() + 1 || cell.y >= cfg.max_world_y())
            && !super::super::orbital::habitat::allows_cell(world, cell) {
            return None;
        }
        let anchor = factory::anchor(cell, factory::step(world));
        if world.factory.nodes.contains_key(&anchor) {
            return None;
        }
        let old = world.get(cell);
        if old == B::Air || old.is_liquid() {
            return None;
        }
        // Tree harvesting can publish the removed cell plus a 1024-cell canopy.
        // Defer before changing terrain when that bounded result cannot fit.
        let required = if harvest && old == B::Log { 1025 } else { 1 };
        if self.dirty_voxels.len().saturating_add(required) > 2048 {
            return None;
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
                return None;
            }
            for touched in blocks {
                self.mark_voxel_dirty(world, touched);
            }
        } else if !self.replace_terrain_cell(world, cell, old, B::Air) {
            return None;
        }
        // `blast_harvest` fells a tree when the removed block is a log, so the
        // caller does not run the canopy search a second time.
        if burst { self.spawn_voxel_burst(center, col, 4, 1.8, 0.036); }
        Some(old)
    }

    /// Sets an air cell to `block` under the spell-edit guards (habitat rules,
    /// dirty-voxel budget). Used by creatures that move terrain around.
    pub(super) fn place_terrain_cell(&mut self, world: &mut World, cell: IVec3, block: B) -> bool {
        if world.get(cell) != B::Air {
            return false;
        }
        self.replace_terrain_cell(world, cell, B::Air, block)
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
        // Admission precedes excavation: every damaging path must be visible.
        if self.lasers.len() >= 16 {
            return;
        }
        use magic_fx::{Contact, Family};
        let (family, tint) = Family::for_cast(9, spell_color(9), mods.element);
        let scale = world.voxel_scale.meters();
        let max_dist = wands::card_rules::DRILL_REACH * mods.speed_mul.clamp(0.7, 1.5);
        let steps = (max_dist / (scale * 0.35).clamp(0.08, 0.25)).ceil() as usize;
        let step = max_dist / steps as f32;
        let muzzle = eye + dir * 0.36;
        let start = if trace::solid(world, eye, muzzle).is_some() { eye } else { muzzle };
        let mut path = Vec::with_capacity(steps + 1);
        path.push(start);
        let mut flight = trajectory::Flight::new(dir);
        let mut tangent = dir.normalize_or(Vec3::Z);
        let mut carve_budget = if mods.bore { 10 } else { 5 };
        let mut wall = None;
        let mut revealed_mirage = false;
        for _ in 0..steps {
            let previous = *path.last().unwrap();
            let start = flight.position(previous);
            let delta = flight.advance(&mut tangent, step, mods.path);
            let mut next = flight.translate(previous, delta);
            // Trace every crossed voxel, including at sub-voxel world scales.
            while let Some(hit) = trace::solid_contact_precise(world, start, flight.position(next)) {
                if carve_budget == 0 || !self.carve_cell(world, hit.cell, true) {
                    next = hit.pos;
                    wall = Some(hit);
                    break;
                }
                carve_budget -= 1;
            }
            if wall.is_none() && !revealed_mirage {
                let cell = (next / scale).floor().as_ivec3();
                if world.get(cell).is_mirage() {
                    let changed = crate::world::mirage::reveal(world, cell);
                    if !changed.is_empty() {
                        revealed_mirage = true;
                        for p in changed { self.mark_voxel_dirty(world, p); }
                        self.spell_contact(next, tangent, tint, Family::Lumen, Contact::Glance);
                        self.spawn_voxel_burst(next, [0.62, 0.42, 0.95], 12, 2.6, 0.03);
                    }
                }
            }
            path.push(next);
            if wall.is_some() { break; }
        }
        let end = *path.last().unwrap();
        let rank = self.rank();
        #[cfg(feature = "multiplayer")]
        self.player_beam_path(world, &path, (26.0 + rank as f32 * 3.5) * mods.damage_mul);
        let mut glances = [None; 3];
        let mut glance_count = 0;
        for e in &mut self.enemies {
            if e.health <= 0.0 { continue; }
            // A looping head still delivers only one paid hit per target.
            let contact = path.windows(2).find_map(|segment| {
                let delta = segment[1] - segment[0];
                let t = ((e.center() - segment[0]).dot(delta) / delta.length_squared().max(0.000001)).clamp(0.0, 1.0);
                let closest = segment[0] + delta * t;
                ((e.center() - closest).length() < if tall(e.kind) { 0.85 } else { 0.65 })
                    .then_some((closest, delta.normalize_or_zero()))
            });
            if let Some((point, direction)) = contact {
                let shatter = e.slow > 0.0;
                e.health -= (26.0 + rank as f32 * 3.5 + if shatter { 16.0 } else { 0.0 }) * mods.damage_mul;
                if shatter { e.slow = 0.0; }
                e.flash = 0.16;
                e.stagger = 0.22;
                e.knock = direction * 2.2;
                self.hit = 0.16;
                if glance_count < glances.len() {
                    glances[glance_count] = Some((point, direction));
                    glance_count += 1;
                }
            }
        }
        for (at, direction) in glances.into_iter().flatten() {
            self.spell_contact(at, direction, tint, family, Contact::Glance);
        }
        if let Some(hit) = wall {
            self.spell_contact(hit.pos, tangent, tint, family, Contact::Surface(hit.normal));
        }
        for i in 1..=8 {
            let p = path[(i * (path.len() - 1) / 8).min(path.len() - 1)];
            self.spawn_styled_particle(p, Vec3::Y * 0.8, tint, 0.035, 1.3, 0.45, 0.5, 0.28, family);
        }
        self.lasers.push(LaserBeam {
            a: start, b: end, path, color: tint, radius: 0.042, life: 0.18, total: 0.18,
        });
        if let Some(pay) = payload {
            self.emit_spell_modified(pay, end - tangent * 0.12, tangent, None, mods);
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
        let committed = self.apply_terrain_edits(world, &edits);
        for (cell, state) in thermal {
            if committed.contains(&cell) {
                world.sandbox.set(cell, state);
            }
        }
        for e in &mut self.enemies {
            if e.health > 0.0 && (e.center() - center).length() < radius_m + 0.8 {
                e.slow = e.slow.max(6.0);
                e.flash = 0.16;
            }
        }
        let motes = (5.0 + radius_m * 1.5) as usize;
        self.spawn_voxel_burst(center, spell_color(13), motes.min(10), 1.0, 0.028);
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
        let committed = self.apply_terrain_edits(world, &edits);
        for (cell, state) in thermal {
            if committed.contains(&cell) {
                world.sandbox.set(cell, state);
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
        self.spawn_voxel_burst(center, spell_color(12), 8, 1.1, 0.03);
    }

    pub(super) fn electrify_pool(&mut self, world: &mut World, center: Vec3, rank: u32, surge: bool) {
        let scale = world.voxel_scale.meters();
        let c_cell = (center / scale).floor().as_ivec3();
        let point = |cell: IVec3| (cell.as_vec3() + Vec3::splat(0.5)) * scale;
        // Find one visible entry into the liquid, rather than linking every
        // water patch in the search box through intervening walls.
        let mut seed = None;
        for dy in -2..=2 {
            for dx in -4..=4 {
                for dz in -4..=4 {
                    let cell = c_cell + IVec3::new(dx, dy, dz);
                    let distance = dx * dx + dy * dy + dz * dz;
                    if world.get(cell) == B::Water
                        && seed.is_none_or(|(_, best)| distance < best)
                        && line(world, center, point(cell)) {
                        seed = Some((cell, distance));
                    }
                }
            }
        }
        let Some((seed, _)) = seed else { return; };
        const DIRS: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
        let limit = if surge { SURGE_CELLS } else { wands::card_rules::CONDUCTIVE_CELLS };
        let horizontal = if surge { (8.0 / scale).ceil() as i32 } else { 4 };
        let vertical = if surge { (2.0 / scale).ceil() as i32 } else { 2 };
        let mut wet_cells = vec![(seed, center)];
        let mut cursor = 0;
        while cursor < wet_cells.len() && wet_cells.len() < limit {
            let cell = wet_cells[cursor].0;
            for offset in DIRS {
                let next = cell + offset;
                let delta = (next - c_cell).abs();
                if delta.x <= horizontal && delta.z <= horizontal && delta.y <= vertical
                    && world.get(next) == B::Water
                    && !wet_cells.iter().any(|(p, _)| *p == next) {
                    wet_cells.push((next, point(cell)));
                    if wet_cells.len() == limit { break; }
                }
            }
            cursor += 1;
        }
        let visual_limit = 32usize.saturating_sub(self.arcs.len()) / 2;
        for &(cell, parent) in wet_cells.iter().take(visual_limit) {
            self.arcs.push(Arc { a: parent, b: point(cell), life: if surge { 0.35 } else { 0.22 } });
        }
        if !surge && !self.creative && wet_cells.len() > 1 && crate::world::progression::study::available(world)
            && world.progression.lessons & crate::world::progression::study::CONDUCTION_OBSERVED == 0
        {
            let observation = crate::world::progression::study::CONDUCTION_OBSERVED;
            world.progression.lessons |= observation;
            self.study_lessons |= observation;
            sound(&mut self.sounds, Cue::Treasure, Some(center), 0.7);
            world.expedition.say("Storm Surge discovered · Charge crossed connected water");
        }
        // One finite electrical grant per cast, shared by touching machinery.
        // A charged machine is never drained by a weaker incoming pulse.
        let mut energy = wands::card_rules::CONDUCTIVE_ENERGY;
        let mut visited = std::collections::HashSet::new();
        for &(cell, _) in &wet_cells {
            for offset in DIRS {
                let anchor = factory::anchor(cell + offset, factory::step(world));
                if visited.insert(anchor)
                    && let Some(node) = world.factory.nodes.get_mut(&anchor)
                    && node.kind.powered() {
                    let accepted = energy.min(wands::card_rules::CONDUCTIVE_ENERGY.saturating_sub(node.energy));
                    node.energy += accepted;
                    energy -= accepted;
                }
            }
        }
        let damage = wands::card_rules::CONDUCTIVE_DAMAGE
            + rank as f32 * wands::card_rules::CONDUCTIVE_DAMAGE_PER_RANK
            + if surge { SURGE_DAMAGE_BONUS } else { 0.0 };
        for e in &mut self.enemies {
            if e.health <= 0.0 { continue; }
            let contact = wet_cells.iter().map(|(cell, _)| point(*cell))
                .filter(|p| p.distance(e.center()) < wands::card_rules::CONDUCTIVE_REACH
                    && line(world, *p, e.center()))
                .min_by(|a, b| a.distance_squared(e.center()).total_cmp(&b.distance_squared(e.center())));
            if let Some(from) = contact {
                e.hurt(damage * e.affixes.damage_taken());
                e.flash = 0.22;
                e.stagger = 0.35;
                if self.arcs.len() < 32 { self.arcs.push(Arc { a: from, b: e.center(), life: 0.22 }); }
                self.hit = 0.2;
            }
        }
        #[cfg(feature = "multiplayer")]
        self.player_conductive_field(world, &wet_cells.iter().map(|(cell, _)| point(*cell)).collect::<Vec<_>>(), damage);
        self.spawn_voxel_burst(center, spell_color(14), 16, 3.0, 0.042);
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
        let sustained = self.concentration.as_mut().and_then(super::concentration::Channel::plasma_mut);
        for (cross, temporary) in self.crosses.iter_mut().map(|cross| (cross, true))
            .chain(sustained.map(|cross| (cross, false)))
        {
            if temporary { cross.life -= dt; }
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
        use magic_fx::Family;
        let scale = world.voxel_scale.meters();
        let curved_world = world.world_type == super::super::WorldType::Manyfold;
        for p in &mut self.particles {
            p.life -= dt;
            if p.life <= 0.0 { continue; }
            p.vel = p.family.velocity(p.pos, p.vel, p.gravity, self.time, dt);
            if p.vel == Vec3::ZERO { continue; }
            let next = p.pos + p.vel * dt;
            let cell = (next / scale).floor().as_ivec3();
            if p.family == Family::Plain {
                if world.solid(cell) {
                    p.vel = p.family.rebound(p.vel, Vec3::ZERO);
                } else {
                    p.pos = next;
                }
                continue;
            }
            let from = (p.pos / scale).floor().as_ivec3();
            let hit = if curved_world || from != cell || world.solid(cell) {
                trace::solid_contact(world, p.pos, next)
            } else {
                None
            };
            if p.family == Family::Water && p.vel.y < 0.0 {
                let end = hit.as_ref().map_or(next, |contact| contact.pos);
                let end_cell = (end / scale).floor().as_ivec3();
                // Water occupies only part of a surface voxel. Check the
                // actual waterline and stop at any intervening solid.
                if world.get(end_cell) == B::Water
                    && let Some(surface) = world.water_surface_at(end)
                    && end.y <= surface.y
                {
                    static REVIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
                    if *REVIEW.get_or_init(|| std::env::var_os("VOXEL_WATER_SURFACE_REVIEW").is_some()) {
                        println!("[water-review] absorbed y={:.4} waterline={:.4}", end.y, surface.y);
                    }
                    p.life = 0.0;
                    continue;
                }
            }
            if let Some(hit) = hit {
                p.pos = hit.pos;
                match p.family {
                    Family::Acid => {
                        p.vel = Vec3::ZERO;
                        p.gravity = 0.0;
                        p.life = p.life.min(0.13);
                    }
                    _ => {
                        p.vel = p.family.rebound(p.vel, hit.normal);
                        if p.family == Family::Water {
                            p.life = p.life.min(0.10);
                        }
                    }
                }
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
                    e.slow = e.slow.max(0.9);
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
            let old = world.get(cell);
            if matches!(old, B::Dirt | B::Grass) {
                self.replace_terrain_cell(world, cell, old, B::FungalSoil);
            }
            self.spawn_voxel_burst(pos + Vec3::Y * 0.2, spell_color(5), 2, 0.35, 0.024);
        }
        self.blooms.retain(|b| b.life > 0.0);
    }

    pub(super) fn cast_timberfall(&mut self, world: &mut World, eye: Vec3, dir: Vec3) {
        if self.dirty_voxels.len().saturating_add(1024) > 2048 {
            return;
        }
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
        super::super::expedition::fell_tree(world, target_cell, dir);
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

    pub fn magic_models(&self, m: &mut Model, eye: Vec3, forward: Vec3, shell: Option<u32>, lanterncap: bool) {
        self.danger_models(m);
        self.relic_models(m, shell, eye, lanterncap);
        self.well_models(m);
        self.aegis_model(m);
        self.circle_model(m);
        self.recall_model(m);

        // 3D Glowing Translucent Voxel Particles (Noita-style living 3D pixel sparks & motes)
        for p in &self.particles {
            let t = (p.life / p.total).clamp(0.0, 1.0);
            let apparent_size = p.size / p.pos.distance(eye).max(0.05);
            let coverage = ((apparent_size - 0.00035) / 0.00045).clamp(0.0, 1.0);
            let opacity = (p.alpha * (t * 8.0).min(1.0) * coverage).min(0.7);
            if opacity < 0.06 {
                continue;
            }
            // Seed and pose belong to the particle, not its current world cell.
            let angle = std::f32::consts::TAU / 1024.0;
            let rot = Quat::from_rotation_y((p.seed & 1023) as f32 * angle)
                * Quat::from_rotation_x(((p.seed >> 10) & 1023) as f32 * angle);
            use magic_fx::Family;
            if p.family != Family::Plain {
                let color = p.family.color(p.color, t, p.seed);
                let glow = p.glow * (0.35 + 0.65 * t * t);
                match p.family {
                    Family::Water => m.glow_orb(p.pos, p.size * (0.36 + t * 0.28), color, glow, opacity),
                    Family::Spore => m.glow_voxel(
                        p.pos, Vec3::splat(p.size * (0.74 + (1.0 - t) * 0.42)), rot,
                        color, glow, opacity * 0.75,
                    ),
                    _ => {
                        let direction = if p.family == Family::Frost {
                            rot * (Quat::from_rotation_x((1.0 - t) * 5.2) * Vec3::Y)
                        } else {
                            p.vel.normalize_or(rot * Vec3::Y)
                        };
                        let stretch = p.family.look().stretch * (0.67 + 0.33 * t);
                        m.spark_shard(p.pos, direction, p.size * (0.54 + 0.46 * t),
                            stretch, color, glow, opacity);
                    }
                }
                continue;
            }
            let erosion = ((1.0 - t - 0.30) / 0.70).clamp(0.0, 1.0);
            // Subcells smaller than roughly two pixels use one fading cube.
            // The existing 480-particle cap also bounds near-field geometry.
            let detail = if apparent_size >= 0.004 { 1.0 } else { 0.0 };
            m.eroding_particle(
                p.pos, p.size, rot, p.color,
                p.glow * (0.65 + 0.35 * t),
                opacity,
                erosion, p.seed, detail,
            );
        }
        // Luminous Drill 3D Laser Beams with helical voxel rings
        for laser in &self.lasers {
            let fade = (laser.life / laser.total).clamp(0.0, 1.0);
            let endpoints = [laser.a, laser.b];
            let path = if laser.path.len() >= 2 { laser.path.as_slice() } else { &endpoints };
            let mut distance = 0.0;
            for segment in path.windows(2) {
                m.beam(segment[0], segment[1], laser.radius * 0.52 * fade, [0.72, 0.78, 0.74], 1.6 * fade);
                m.spell_current(segment[0], segment[1], laser.radius * 1.45 * fade, laser.color, 1.3 * fade, 0.35 * fade, distance);
                distance += segment[0].distance(segment[1]);
            }
            if distance > 0.001 {
                let spacing = distance / 6.0;
                let mut next_mote = ((1.0 - fade) * 2.7).fract() * spacing;
                let mut walked = 0.0;
                let mut left = 6;
                for segment in path.windows(2) {
                    let span = segment[1] - segment[0];
                    let length = span.length();
                    if length <= 0.0 { continue; }
                    while left > 0 && next_mote <= walked + length {
                        let axis = span / length;
                        let right = axis.any_orthonormal_vector();
                        let up = axis.cross(right);
                        let angle = next_mote / distance * 8.0 - self.time * 3.0;
                        let (sin, cos) = angle.sin_cos();
                        let ring = (right * cos + up * sin) * (laser.radius * 2.2 * fade);
                        m.glow_voxel(
                            segment[0] + axis * (next_mote - walked) + ring,
                            Vec3::splat(0.032 * fade), Quat::from_rotation_y(angle),
                            laser.color, 1.2 * fade, 0.4 * fade,
                        );
                        next_mote += spacing;
                        left -= 1;
                    }
                    walked += length;
                }
            }
        }

        // Plasma Beam Crosses (4 rotating 3D laser blades + floating temple core)
        for cross in self.crosses.iter().chain(self.sustained_plasma()) {
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
            for dir in PLASMA_DIRECTIONS {
                let tip = cross.pos + rot * (dir * 1.75 * fade);
                m.beam(cross.pos, tip, 0.016 * fade, c_col, 1.2 * fade);
                m.spell_current(cross.pos, tip, 0.04 * fade, c_col, 1.15 * fade, 0.32 * fade, 0.0);
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
            let axis = delta.normalize_or(Vec3::Z);
            let side = axis.cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize_or_zero();
            let fade = (arc.life / 0.18).clamp(0.0, 1.0);
            let mut previous = arc.a;
            let mut distance = 0.0;
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
                m.spell_current(previous, point, 0.038, spell_color(14), 1.15 * fade, 0.32 * fade, distance);
                distance += previous.distance(point);
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
