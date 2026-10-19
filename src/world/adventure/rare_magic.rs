use super::*;
pub(super) struct Well {
    pub pos: Vec3,
    pub life: f32,
    pub pulse: f32,
}
impl Adventure {
    /// Relic finishers reward the existing combo/freeze loop, without another keybind.
    pub(super) fn relic_hit(
        &mut self,
        world: &World,
        eye: Vec3,
        point: Vec3,
        forward: Vec3,
        primary: usize,
        finisher: bool,
        shatter: bool,
    ) {
        let cinder = self.weapon == 3 && finisher;
        let winter = self.weapon == 4 && shatter;
        if !cinder && !winter {
            return;
        }
        let color = if cinder {
            [1.0, 0.32, 0.045]
        } else {
            spell_color(2)
        };
        for (i, e) in self.enemies.iter_mut().enumerate() {
            if i == primary || e.health <= 0.0 {
                continue;
            }
            let d = e.center() - point;
            if d.length() > if cinder { 3.2 } else { 2.4 } || !line(world, point, e.center()) {
                continue;
            }
            if cinder && (e.center() - eye).normalize_or_zero().dot(forward) < 0.45 {
                continue;
            }
            e.health -= if cinder { 20.0 } else { 12.0 };
            e.flash = 0.2;
            if winter {
                e.slow = e.slow.max(2.5);
            }
            if e.kind < 6 {
                e.knock = d.normalize_or_zero() * 4.0;
            }
        }
        for j in 0..7 {
            let a = (j as f32 - 3.0) * 0.3;
            let d = Quat::from_rotation_y(a) * forward;
            let p = if cinder {
                point + d * 1.1
            } else {
                point + Vec3::new((a * 3.0).cos(), 0.2, (a * 3.0).sin()) * 1.3
            };
            if line(world, point, p) {
                self.effect(p, color, 0.45);
            }
        }
        sound(
            &mut self.sounds,
            if cinder {
                Cue::CinderStrike
            } else {
                Cue::Shatter
            },
            Some(point),
            0.9,
        );
    }
    pub(super) fn echo_step(&mut self, world: &World, player: &mut Player) -> bool {
        let scale = world.voxel_scale.meters();
        let origin = player.pos * scale;
        let dir = player.forward_dir();
        let mut last = origin;
        for i in 1..=24 {
            let candidate = origin + dir * (i as f32 * 0.25);
            let (min, max) = super::super::player_aabb(
                candidate / scale,
                world.voxel_scale,
                player.scale,
                player.crouch_t,
            );
            if collides_world(world, min, max) {
                break;
            }
            if player.flying
                || factory::solid_point(world, candidate - Vec3::Y * 0.5)
                || factory::solid_point(world, candidate - Vec3::Y * 1.0)
            {
                last = candidate;
            }
        }
        if (last - origin).length() < 0.75 {
            return false;
        }
        player.pos = last / scale;
        player.vel = Vec3::ZERO;
        self.invulnerable = self.invulnerable.max(0.25);
        self.effect(origin + Vec3::Y, spell_color(7), 1.2);
        self.effect(last + Vec3::Y, spell_color(7), 1.2);
        // A brief trail makes the actual travelled path legible without obscuring aim.
        for i in 1..5 {
            self.effect(
                origin.lerp(last, i as f32 / 5.0) + Vec3::Y * 0.8,
                spell_color(7),
                0.35,
            );
        }
        sound(&mut self.sounds, Cue::Echo, None, 0.8);
        true
    }
    pub(super) fn tick_wells(&mut self, world: &World, dt: f32) {
        for well in &mut self.wells {
            well.life -= dt;
            well.pulse -= dt;
            let pulse = well.pulse <= 0.0;
            if pulse {
                well.pulse = 0.6;
            }
            for e in &mut self.enemies {
                let d = well.pos - e.center();
                if e.health <= 0.0 || d.length() > 4.5 || !line(world, well.pos, e.center()) {
                    continue;
                }
                let strength = if e.kind >= 6 { 1.4 } else { 5.0 };
                e.knock += d.normalize_or_zero() * dt * strength * 5.0;
                e.knock = e.knock.clamp_length_max(strength);
                if pulse {
                    e.health -= if e.slow > 0.0 { 12.0 } else { 7.0 };
                    e.flash = 0.12;
                    self.hit = 0.12;
                }
            }
        }
        self.wells.retain(|w| w.life > 0.0);
    }
    pub(super) fn well_models(&self, m: &mut Model) {
        for w in &self.wells {
            let fade = w.life.min(1.0);
            // Dark event horizon core + glowing inner shell
            m.orb(w.pos, 0.17 * fade, [0.06, 0.02, 0.12], 0.25);
            m.glow_orb(
                w.pos,
                0.24 * fade,
                spell_color(6),
                3.4 * fade,
                0.55 * fade,
            );
            m.temple_glyph(
                w.pos,
                Quat::from_rotation_y(self.time * 1.6) * Quat::from_rotation_x(0.45),
                3,
                0.036 * fade,
                spell_color(10),
                3.8 * fade,
                0.76 * fade,
            );
            for plane in 0..2 {
                for j in 0..16 {
                    let a = j as f32 * std::f32::consts::TAU / 16.0;
                    let b = a + std::f32::consts::TAU / 16.0;
                    let radius = (0.55 + plane as f32 * 0.16) * fade;
                    let turn = Quat::from_rotation_y(self.time * 0.65 + plane as f32 * 1.4)
                        * Quat::from_rotation_z(plane as f32 * 0.52 - 0.26);
                    let at = |angle: f32| {
                        w.pos + turn * Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0)
                    };
                    m.glow_beam(
                        at(a),
                        at(b),
                        0.022 * fade,
                        if plane == 0 {
                            spell_color(6)
                        } else {
                            spell_color(10)
                        },
                        3.0 * fade,
                        0.66 * fade,
                    );
                }
            }
            // Inward-spiraling accretion disk of 3D translucent micro-voxels
            for i in 0..24 {
                let a = self.time * 3.4 + i as f32 * 2.39996;
                let r = 0.22 + (i as f32 * 0.17 - self.time * 0.95).rem_euclid(1.65);
                let p = w.pos + Vec3::new(a.cos() * r, (a * 0.7).sin() * 0.36, a.sin() * r);
                m.glow_voxel(
                    p,
                    Vec3::splat((0.036 + (i % 3) as f32 * 0.012) * fade),
                    Quat::from_rotation_y(a) * Quat::from_rotation_z(self.time * 2.0),
                    if i % 3 == 0 {
                        [0.45, 0.92, 1.0]
                    } else {
                        spell_color(6)
                    },
                    3.6 * fade,
                    0.74 * fade,
                );
            }
        }
    }
}
