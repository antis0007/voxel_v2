//! One expedition, three physical relics, and a home to return to.
use super::*;
use reliquaries::{AWAKENED, CLAIMED, RelicFocus};

pub(super) const VALVES: u32 = 3 << 12;
pub(super) const BYPASS: u32 = 1 << 14;
pub(super) const RESTORED: u32 = 1 << 15;
const ORDER: [usize; 3] = [2, 0, 1];

pub(super) struct Station {
    site: IVec3,
    pos: Vec3,
    kind: usize,
    yaw: f32,
}
pub(super) fn sigil(kind: u32) -> u32 {
    match kind {
        3 => 1,
        5 => 2,
        8 => 4,
        _ => 0,
    }
}
pub(super) fn sealed(kind: u32, state: u32) -> bool {
    kind == 3 && state & (AWAKENED | CLAIMED | 8) == 0 && state & VALVES != VALVES
}
impl Adventure {
    pub(super) fn update_stations(&mut self, world: &World) {
        self.stations.clear();
        let cfg = world.sample_config();
        for r in &self.reliquaries {
            if r.kind != 3 {
                continue;
            }
            for (kind, center) in sites::foundry_stations(&cfg, r.site)
                .into_iter()
                .enumerate()
            {
                // Keep fixtures at their authored locations; only accommodate floor height.
                // A mined-away floor must be repaired, not silently relocate the puzzle.
                for j in 0..24 {
                    let pos = center + Vec3::Y * (2.0 - j as f32 * 0.125);
                    if free(world, pos, 0) && factory::solid_point(world, pos - Vec3::Y * 0.18) {
                        self.stations.push(Station {
                            site: r.site,
                            pos,
                            kind,
                            yaw: sites::foundry_rotation(&cfg, r.site),
                        });
                        break;
                    }
                }
            }
        }
    }
    fn focused_station(&self, world: &World, eye: Vec3, forward: Vec3) -> Option<usize> {
        self.stations
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let target = s.pos + Vec3::Y * 0.9;
                let d = target - eye;
                let along = d.dot(forward);
                (along > 0.0
                    && d.length() < 3.3
                    && (d - forward * along).length() < 0.48
                    && line(world, eye, target))
                .then_some((i, d.length()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    pub(super) fn station_focus(
        &self,
        world: &World,
        eye: Vec3,
        forward: Vec3,
    ) -> Option<RelicFocus> {
        let s = &self.stations[self.focused_station(world, eye, forward)?];
        let state = *self.records.get(&s.site).unwrap_or(&0);
        Some(RelicFocus {
            title: [
                "Intake valve · I",
                "Pressure valve · II",
                "Exhaust valve · III",
                "Foundry operating plate",
                "Hidden maintenance bypass",
            ][s.kind],
            action: if s.kind == 3 {
                "F  Read the operating sequence"
            } else if s.kind == 4 {
                if state & BYPASS != 0 {
                    "Steam vented · cache used"
                } else {
                    "F  Vent steam and recover the tonic"
                }
            } else if !sealed(3, state) {
                "Furnace balanced"
            } else {
                [
                    "F  Turn valve · 0 / 3",
                    "F  Turn valve · 1 / 3",
                    "F  Turn valve · 2 / 3",
                ][(state >> 12 & 3) as usize]
            },
        })
    }
    pub(super) fn interact_station(&mut self, world: &mut World, eye: Vec3, forward: Vec3) -> bool {
        let Some(i) = self.focused_station(world, eye, forward) else {
            return false;
        };
        if self.puzzle_cd > 0.0 {
            return true;
        }
        self.puzzle_cd = 0.65;
        let s = &self.stations[i];
        let (site, pos, kind) = (s.site, s.pos, s.kind);
        let state = *self.records.get(&site).unwrap_or(&0);
        if kind == 3 {
            world.expedition.say("Operating plate: III EXHAUST, then I INTAKE, then II PRESSURE. Wrong order vents scalding steam. Maintenance access is behind one of the furnace stacks.");
        } else if kind == 4 {
            if state & BYPASS == 0 {
                *self.records.entry(site).or_default() |= BYPASS;
                self.health = (self.health + 35.0).min(self.max_health());
                self.mana = (self.mana + 45.0).min(self.max_mana());
                sound(&mut self.sounds, Cue::Mend, Some(pos), 0.8);
                world.expedition.say("Maintenance cache: restorative tonic. The bypass vents the steam trap safely; the valve sequence still controls the furnace.");
            }
        } else if sealed(3, state) {
            let progress = (state >> 12 & 3) as usize;
            let correct = kind == ORDER[progress];
            let next = if correct { progress + 1 } else { 0 };
            *self.records.entry(site).or_default() = (state & !VALVES) | ((next as u32) << 12);
            if correct {
                if next==3 {self.health=self.max_health();self.mana=self.max_mana();}
                sound(&mut self.sounds, Cue::Clang, Some(pos), 0.65);
                self.effect(pos + Vec3::Y, [0.3, 1.0, 0.65], 0.6);
                world.expedition.say(if next == 3 { "The furnace settles into a steady beat. You recover health and mana before the guardian fight." } else { "Pressure stabilizing. The next valve is marked on the operating plate." });
            } else {
                if state & BYPASS == 0 && self.hazards.len() < 16 {
                    self.hazards.push(tactics::Hazard {
                        pos,
                        radius: 2.4,
                        delay: 1.1,
                        life: 0.25,
                        pulse: 0.0,
                        damage: 16.0,
                        spores: false,
                    });
                    sound(&mut self.sounds, Cue::ForgeWindup, Some(pos), 0.7);
                }
                world.expedition.say(if state & BYPASS == 0 {
                    "Pressure fault! Step out of the glowing steam vent. Sequence reset."
                } else {
                    "Pressure fault safely vented. Sequence reset."
                });
            }
        }
        true
    }
    pub(super) fn heart_focus(&self) -> RelicFocus {
        RelicFocus {
            title: "The workshop heart",
            action: if self.heart_restored() {
                "F  Rest at the restored heart"
            } else if self.sigils == 7 {
                "F  Restore the heart with the three relics"
            } else {
                "F  Trace the next guardian relic"
            },
        }
    }
    pub(super) fn heart_restored(&self) -> bool {
        self.records.get(&IVec3::ZERO).copied().unwrap_or(0) & RESTORED != 0
    }
    pub(super) fn use_heart(&mut self, world: &mut World, pos: Vec3) {
        if self.puzzle_cd > 0.0 {
            return;
        }
        self.puzzle_cd = 1.0;
        if self.sigils == 7 && !self.heart_restored() {
            *self.records.entry(IVec3::ZERO).or_default() |= RESTORED;
            self.reward = Some(Reward {
                name: "The workshop lives again",
                items: Vec::new(),
                seconds: 10.0,
                discovery: None,
            });
            self.effect(pos + Vec3::Y, [1.0, 0.7, 0.3], 3.0);
            sound(&mut self.sounds, Cue::Treasure, Some(pos), 1.0);
            self.route = "Expedition complete. The restored heart is a sanctuary; your workshop and the wider world remain yours to build.".into();
            world.expedition.say(&self.route);
        } else if self.heart_restored() {
            let danger = self
                .enemies
                .iter()
                .any(|e| e.health > 0.0 && (e.pos - pos).length() < 24.0);
            if danger {
                world
                    .expedition
                    .say("The heart cannot shelter you while enemies are nearby.");
                return;
            }
            self.health = self.max_health();
            self.mana = self.max_mana();
            sound(&mut self.sounds, Cue::Mend, Some(pos), 0.7);
            world
                .expedition
                .say("Home again. The workshop heart restores your health and mana.");
        } else {
            self.chart_route(world, pos);
        }
    }
    pub(super) fn chart_route(&mut self, world: &mut World, origin: Vec3) {
        if self.sigils == 7 {
            self.route = "Three relics recovered. Return down the starter workshop's mine to restore its heart.".into();
        } else if let Some((kind, point)) =
            sites::next_relic(&world.sample_config(), origin, self.sigils)
        {
            let d = point - origin;
            let direction =
                match ((d.x.atan2(-d.z).to_degrees() + 360.0 + 22.5) / 45.0) as usize % 8 {
                    0 => "north",
                    1 => "northeast",
                    2 => "east",
                    3 => "southeast",
                    4 => "south",
                    5 => "southwest",
                    6 => "west",
                    _ => "northwest",
                };
            self.route = format!(
                "{} lies {}: {:.0} m from this relic, near X {:.0}, Z {:.0}{}. Recover its guardian relic, then follow the next resonance.",
                if kind == 3 {
                    "A foundry"
                } else if kind == 5 {
                    "An archive"
                } else {
                    "A crystal cathedral"
                },
                direction,
                Vec3::new(d.x, 0.0, d.z).length(),
                point.x,
                point.z,
                if kind == 8 {
                    format!(
                        ", underground near Y {:.0}; seek a cave or dig a safe stair",
                        point.y
                    )
                } else {
                    String::new()
                }
            );
        } else {
            self.route = "No remaining guardian resonance in this district. Travel farther, then consult another claimed relic. Foundries, archives and crystal cathedrals hold the three keys.".into();
        }
        world.expedition.say(&self.route);
    }
    pub(super) fn station_models(&self, m: &mut Model) {
        for s in &self.stations {
            let mark = m.mark();
            let state = self.records.get(&s.site).copied().unwrap_or(0);
            let done = !sealed(3, state);
            let active = done
                || ORDER
                    .iter()
                    .take((state >> 12 & 3) as usize)
                    .any(|&k| k == s.kind);
            let metal = [0.28, 0.27, 0.23];
            let glow = if active {
                [0.3, 0.9, 0.65]
            } else {
                [0.95, 0.48, 0.13]
            };
            m.cuboid(
                s.pos + Vec3::Y * 0.43,
                Vec3::new(0.46, 0.86, 0.4),
                Quat::IDENTITY,
                metal,
                0.0,
            );
            if s.kind < 3 {
                let hub = s.pos + Vec3::Y * 0.95;
                m.orb(hub, 0.07, metal, 0.0);
                for j in 0..8 {
                    let a = j as f32 * std::f32::consts::TAU / 8.0 + if active { 0.4 } else { 0.0 };
                    let b = a + std::f32::consts::TAU / 8.0;
                    let at =
                        |angle: f32| hub + Vec3::new(angle.cos() * 0.3, angle.sin() * 0.3, 0.0);
                    m.beam(at(a), at(b), 0.035, metal, 0.0);
                    if j % 2 == 0 {
                        m.beam(hub, at(a), 0.025, metal, 0.0);
                    }
                }
                for j in 0..=s.kind {
                    m.orb(
                        s.pos + Vec3::new((j as f32 - s.kind as f32 * 0.5) * 0.12, 0.55, 0.22),
                        0.035,
                        glow,
                        1.0,
                    );
                }
            } else if s.kind == 3 {
                m.cuboid(
                    s.pos + Vec3::Y * 1.0,
                    Vec3::new(1.1, 0.5, 0.08),
                    Quat::IDENTITY,
                    metal,
                    0.0,
                );
                // Left-to-right pip groups repeat the valve markings: III, I, II.
                for (col, count) in [3, 1, 2].into_iter().enumerate() {
                    for row in 0..count {
                        m.orb(
                            s.pos
                                + Vec3::new(
                                    (col as f32 - 1.0) * 0.34,
                                    1.15 - row as f32 * 0.12,
                                    0.06,
                                ),
                            0.035,
                            [0.9, 0.72, 0.3],
                            0.7,
                        );
                    }
                }
            } else {
                m.beam(
                    s.pos + Vec3::new(-0.3, 0.8, 0.0),
                    s.pos + Vec3::new(0.3, 1.1, 0.0),
                    0.05,
                    metal,
                    0.0,
                );
                if state & BYPASS == 0 {
                    m.orb(
                        s.pos + Vec3::new(0.0, 0.6, 0.24),
                        0.07,
                        [0.35, 0.85, 0.5],
                        0.5,
                    );
                }
            }
            m.rotate_since(mark, s.pos, Quat::from_rotation_y(s.yaw));
        }
    }
}
