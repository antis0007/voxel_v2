//! Diegetic, deterministic discovery objects. Record flags survive saves and streaming.
use super::*;
pub(super) const AWAKENED: u32 = 512;
pub(super) const CLAIMED: u32 = 1024;
pub(super) const PHYSICAL: u32 = 2048;
#[derive(Clone)]
pub struct Reliquary {
    pub site: IVec3,
    pub pos: Vec3,
    pub kind: u32,
    pub state: u32,
}
#[derive(Clone)]
pub struct RelicFocus {
    pub title: &'static str,
    pub action: &'static str,
}
#[derive(Clone)]
pub struct BossView {
    pub name: &'static str,
    pub health: f32,
    pub maximum: f32,
}
pub(super) fn place(world: &World, center: Vec3) -> Option<Vec3> {
    for i in 0..20 {
        let a = i as f32 * 2.4;
        let r = (i / 4) as f32 * 1.5;
        let xz = center + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        for j in 0..24 {
            let p = xz + Vec3::Y * (3.0 - j as f32 * 0.25);
            if free(world, p, 0) && factory::solid_point(world, p - Vec3::Y * 0.18) {
                return Some(p);
            }
        }
    }
    None
}
impl Adventure {
    pub(super) fn update_reliquaries(&mut self, world: &World, feet: Vec3) {
        self.reliquaries.clear();
        let cfg = world.sample_config();
        for (site, center, _) in sites::encounters(&cfg, feet) {
            if self.records.len() >= MAX_RECORDS && !self.records.contains_key(&site) {
                continue;
            }
            let state = *self.records.entry(site).or_insert(PHYSICAL);
            if let Some(pos) = place(world, center) {
                self.reliquaries.push(Reliquary {
                    site,
                    pos,
                    kind: sites::reward_kind(&cfg, site),
                    state,
                });
            }
        }
        self.update_stations(world);
    }
    fn focused_relic(&self, world: &World, eye: Vec3, forward: Vec3) -> Option<usize> {
        self.reliquaries
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let d = r.pos + Vec3::Y * 0.85 - eye;
                let t = d.dot(forward);
                (t > 0.0
                    && d.length() < 3.8
                    && (d - forward * t).length() < 0.75
                    && line(world, eye, r.pos + Vec3::Y * 0.85))
                .then_some((i, d.length()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    pub(super) fn refresh_focus(&mut self, world: &World, eye: Vec3, forward: Vec3) {
        self.relic_focus = self.focused_relic(world, eye, forward).map(|i| {
            let r = &self.reliquaries[i];
            let state = *self.records.get(&r.site).unwrap_or(&0);
            let boss = bosses::kind(r.kind);
            if r.site == IVec3::ZERO && state & CLAIMED != 0 {
                return self.heart_focus();
            }
            RelicFocus {
                title: if boss > 0 {
                    bosses::name(boss)
                } else {
                    inscriptions::INSCRIPTIONS[r.kind as usize].name
                },
                action: if state & CLAIMED != 0 {
                    "F - Trace the next guardian relic"
                } else if state & 256 == 0 {
                    "Defeat the guardians to unseal"
                } else if boss > 0 && state & 8 == 0 {
                    if state & AWAKENED == 0 {
                        "F - Awaken guardian"
                    } else {
                        "Defeat the awakened guardian"
                    }
                } else {
                    "F - Claim relic"
                },
            }
        });
        if let Some(focus) = self.station_focus(world, eye, forward) {
            self.relic_focus = Some(focus);
        }
        self.boss_view = self
            .enemies
            .iter()
            .filter(|e| e.kind >= 6 && e.health > 0.0 && (e.center() - eye).length() < 32.0)
            .min_by(|a, b| {
                (a.center() - eye)
                    .length_squared()
                    .total_cmp(&(b.center() - eye).length_squared())
            })
            .map(|e| BossView {
                name: bosses::name(e.kind),
                health: e.health,
                maximum: bosses::health(e.kind),
            });
        self.enemy_focus = self
            .enemies
            .iter()
            .filter_map(|e| {
                if e.health <= 0.0 {
                    return None;
                }
                let to_enemy = e.center() - eye;
                let dist = to_enemy.length();
                if dist <= 0.1 || dist > 26.0 {
                    return None;
                }
                let align = forward.dot(to_enemy / dist);
                if align > 0.94 && super::line(world, eye, e.center()) {
                    Some((e, dist * (1.05 - align)))
                } else {
                    None
                }
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(e, _)| super::EnemyFocus {
                kind: e.kind,
                name: super::enemy_name(e.kind),
                health: e.health,
                maximum: super::enemy_health(e.kind),
                frozen: e.slow > 0.0,
            });
    }
    pub(super) fn interact_relic(&mut self, world: &mut World, eye: Vec3, forward: Vec3) -> bool {
        if self.interact_station(world, eye, forward) {
            return true;
        }
        let Some(i) = self.focused_relic(world, eye, forward) else {
            return false;
        };
        let r = self.reliquaries[i].clone();
        let state = *self.records.get(&r.site).unwrap_or(&0);
        if state & CLAIMED != 0 {
            if r.site == IVec3::ZERO {
                self.use_heart(world, r.pos);
            } else if self.puzzle_cd <= 0.0 {
                self.puzzle_cd = 1.0;
                self.chart_route(world, r.pos);
            }
            return true;
        }
        if state & 256 == 0 {
            return true;
        }
        let boss = bosses::kind(r.kind);
        if boss > 0 && state & 8 == 0 {
            if state & AWAKENED == 0 {
                // Refuse to awaken into solid terrain or the player's body.
                if self.spawn_boss(world, &r, eye) {
                    *self.records.entry(r.site).or_default() |= AWAKENED;
                    sound(&mut self.sounds, Cue::Warning, Some(r.pos), 1.0);
                } else {
                    world.expedition.say("The guardian needs open ground around the reliquary. Clear more space and try again.");
                }
            }
            return true;
        }
        let weapon = match r.kind {
            3 => 3,
            8 => 4,
            _ => 0,
        };
        if weapon > 0 && !self.owns_weapon(weapon) {
            use super::super::inventory::{Item, ItemStack};
            let item = Item::Weapon(weapon as u8);
            let (inserted, _) = world.inventory.try_insert(ItemStack::new(item, 1));
            if inserted == 0 {
                world
                    .expedition
                    .say("Make one inventory slot available before claiming this weapon.");
                return true;
            }
            world.inventory.notify_pickup(item, 1);
            self.add_weapon(weapon);
        }
        *self.records.entry(r.site).or_default() |= CLAIMED | PHYSICAL;
        self.discoveries |= 1 << r.kind;
        self.sigils |= quest::sigil(r.kind);
        if r.kind == 5 {
            self.rare_spells |= 1 << 6;
        }
        if r.kind == 8 {
            self.rare_spells |= 1 << 7;
        }
        self.reward = Some(Reward {
            name: match r.kind {
                3 => "Cinderbrand recovered",
                5 => "Rift well recovered",
                8 => "Winterglass and Echo step recovered",
                _ => "Inscription recovered",
            },
            items: Vec::new(),
            seconds: 10.0,
            discovery: Some(r.kind),
        });
        sound(&mut self.sounds, Cue::Treasure, None, 0.9);
        self.effect(
            r.pos + Vec3::Y,
            spell_color(inscriptions::INSCRIPTIONS[r.kind as usize].spell),
            1.5,
        );
        self.reliquaries[i].state |= CLAIMED;
        true
    }
    pub(super) fn spawn_boss(&mut self, world: &World, r: &Reliquary, eye: Vec3) -> bool {
        if self.enemies.iter().any(|e| e.site == r.site && e.slot == 3) {
            return true;
        }
        if self.enemies.len() >= MAX_ENEMIES {
            return false;
        }
        let kind = bosses::kind(r.kind);
        if kind == 0 {
            return false;
        }
        for i in 0..24 {
            let a = i as f32 * 2.4;
            let p = r.pos + Vec3::new(a.cos(), 0.0, a.sin()) * (4.0 + (i / 8) as f32);
            for dy in [0.0, 0.5, 1.0, -0.5, -1.0] {
                let q = p + Vec3::Y * dy;
                if (q - eye).length() > 2.5
                    && free(world, q, kind)
                    && factory::solid_point(world, q - Vec3::Y * 0.2)
                {
                    self.enemies.push(Enemy::new(r.site, 3, kind, q));
                    return true;
                }
            }
        }
        false
    }
    pub(super) fn relic_models(&self, m: &mut Model) {
        self.station_models(m);
        for r in &self.reliquaries {
            let state = *self.records.get(&r.site).unwrap_or(&r.state);
            let recovered = state & CLAIMED != 0;
            let color = if r.site == IVec3::ZERO && self.heart_restored() {
                [1.0, 0.7, 0.3]
            } else if recovered {
                [0.20, 0.23, 0.25]
            } else {
                spell_color(inscriptions::INSCRIPTIONS[r.kind as usize].spell)
            };
            m.cuboid(
                r.pos + Vec3::Y * 0.15,
                Vec3::new(0.8, 0.3, 0.8),
                Quat::IDENTITY,
                [0.22, 0.23, 0.25],
                0.0,
            );
            m.cuboid(
                r.pos + Vec3::Y * 0.45,
                Vec3::new(0.48, 0.3, 0.48),
                Quat::IDENTITY,
                [0.35, 0.32, 0.26],
                0.0,
            );
            for i in 0..4 {
                let a = i as f32 * 1.57;
                let p = r.pos + Vec3::new(a.cos() * 0.35, 0.64, a.sin() * 0.35);
                m.orb(p, 0.045, color, if recovered { 0.0 } else { 1.2 });
            }
            if r.site == IVec3::ZERO && recovered {
                // Three sockets make the expedition progress tangible in the workshop.
                for (i, bit) in [1, 2, 4].into_iter().enumerate() {
                    let c = bosses::color(6 + i as u32);
                    m.orb(
                        r.pos + Vec3::new((i as f32 - 1.0) * 0.27, 0.85, 0.0),
                        0.10,
                        c,
                        if self.sigils & bit != 0 { 1.8 } else { 0.0 },
                    );
                }
                if self.heart_restored() {
                    m.orb(r.pos + Vec3::Y * 1.2, 0.2, color, 2.5);
                }
            }
            if !recovered {
                let p = r.pos + Vec3::Y * (0.96 + (self.time * 1.6).sin() * 0.045);
                let rot = Quat::from_rotation_y(self.time * 0.4);
                if r.kind == 3 || r.kind == 8 {
                    m.cuboid(p, Vec3::new(0.07, 0.63, 0.04), rot, color, 0.6);
                    m.cuboid(
                        p - Vec3::Y * 0.23,
                        Vec3::new(0.3, 0.05, 0.08),
                        rot,
                        [0.7, 0.48, 0.19],
                        0.0,
                    );
                } else {
                    for side in [-1.0, 1.0] {
                        let q = p + rot * Vec3::new(side * 0.14, 0.0, 0.0);
                        m.cuboid(
                            q,
                            Vec3::new(0.27, 0.045, 0.34),
                            rot * Quat::from_rotation_z(side * 0.18),
                            [0.8, 0.76, 0.62],
                            0.0,
                        );
                    }
                    m.beam(p - Vec3::Z * 0.18, p + Vec3::Z * 0.18, 0.025, color, 1.0);
                }
                // Noita-inspired floating 3D pixel-voxel temple glyphs and rising sanctum motes
                for g in 0..3 {
                    let ga = self.time * 0.65 + g as f32 * 2.094;
                    let gp = r.pos
                        + Vec3::new(
                            ga.cos() * 0.56,
                            1.18 + (self.time * 1.9 + g as f32).sin() * 0.07,
                            ga.sin() * 0.56,
                        );
                    m.temple_glyph(
                        gp,
                        Quat::from_rotation_y(-ga + std::f32::consts::FRAC_PI_2),
                        r.kind + g,
                        0.024,
                        color,
                        3.2,
                        0.72,
                    );
                }
                for m_idx in 0..12 {
                    let ma = self.time * 1.1 + m_idx as f32 * 2.39996;
                    let rise = (self.time * 0.48 + m_idx as f32 * 0.17).rem_euclid(1.65);
                    let mp = r.pos
                        + Vec3::new(
                            ma.cos() * (0.32 + (m_idx % 3) as f32 * 0.12),
                            0.45 + rise,
                            ma.sin() * (0.32 + (m_idx % 3) as f32 * 0.12),
                        );
                    let fade = 1.0 - (rise / 1.65 - 0.5).abs() * 1.8;
                    m.glow_voxel(
                        mp,
                        Vec3::splat(0.028 * fade.clamp(0.25, 1.0)),
                        Quat::from_rotation_y(ma),
                        color,
                        3.4,
                        (0.75 * fade).clamp(0.18, 0.82),
                    );
                }
                if state & 256 == 0 || bosses::kind(r.kind) > 0 && state & 8 == 0 {
                    for i in 0..4 {
                        let a = i as f32 * 1.57 + self.time * 0.3;
                        m.glow_beam(
                            r.pos + Vec3::new(a.cos() * 0.4, 0.55, a.sin() * 0.4),
                            r.pos + Vec3::Y * 1.45,
                            0.016,
                            color,
                            1.8,
                            0.56,
                        );
                    }
                }
            }
        }
    }
}
pub fn interact(world: &mut World, player: &Player) -> bool {
    let scale = world.voxel_scale.meters();
    let mut adventure = std::mem::take(&mut world.adventure);
    let used = adventure.interact_relic(
        world,
        player.eye(world.voxel_scale) * scale,
        player.forward_dir(),
    );
    world.adventure = adventure;
    used
}
