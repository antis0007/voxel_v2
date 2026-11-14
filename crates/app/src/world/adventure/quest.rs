//! Authored quests, three physical relics, and a home to return to.
use super::*;
use reliquaries::{AWAKENED, CLAIMED, RelicFocus};

pub(super) const VALVES: u32 = 3 << 12;
pub(super) const BYPASS: u32 = 1 << 14;
pub(super) const RESTORED: u32 = 1 << 15;
const ORDER: [usize; 3] = [2, 0, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum QuestId {
    Workshop,
    Foundry,
    Archive,
    Cathedral,
    Heart,
    BuriedGate,
    ReturnGate,
}

impl QuestId {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Workshop => "Workshop",
            Self::Foundry => "Foundry Relic",
            Self::Archive => "Archive Relic",
            Self::Cathedral => "Cathedral Relic",
            Self::Heart => "Workshop Heart",
            Self::BuriedGate => "Buried Gate",
            Self::ReturnGate => "Return Gate",
        }
    }
}

#[derive(Clone)]
pub struct QuestEntry {
    pub id: QuestId,
    pub stage: u8,
    pub complete: bool,
    pub available: bool,
    pub objective: &'static str,
}

#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
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
    /// Quest status comes from saved gameplay facts, independently of tracking.
    pub fn quest_entries(&self, cfg: &crate::world::WorldSampleConfig, workshop_prepared: bool) -> Vec<QuestEntry> {
        use crate::world::{WorldType, workshop};
        let mut entries = Vec::new();
        if matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged)
            && cfg.generator_version >= 4 && sites::workshop_heart(cfg).is_some() {
            if cfg.generator_version >= 48 && workshop::home(cfg).is_some() {
                let stage = if workshop::returned(self) && workshop::automated(self) { 4 }
                    else if workshop::returned(self) { 3 }
                    else if workshop::cleared(self) { 2 }
                    else if workshop_prepared { 1 } else { 0 };
                entries.push(QuestEntry {
                    id: QuestId::Workshop, stage, complete: stage == 4, available: true,
                    objective: ["Combat Equipment", "Guardian Vault · Free the Lanterncap",
                        if self.companion.is_none() { "Free the Lanterncap or return to the workbench" } else { "Return to the workbench" }, "Iron, Steel or Bio-ethanol → Storage",
                        "Workshop Complete"][stage as usize],
                });
            }
            for (id, bit, objective) in [
                (QuestId::Foundry, 1, "Claim the foundry relic"),
                (QuestId::Archive, 2, "Claim the archive relic"),
                (QuestId::Cathedral, 4, "Claim the cathedral relic"),
            ] {
                let complete = self.sigils & bit != 0;
                entries.push(QuestEntry { id, stage: complete as u8, complete,
                    available: true, objective });
            }
            entries.push(QuestEntry { id: QuestId::Heart, stage: self.heart_restored() as u8,
                complete: self.heart_restored(), available: self.sigils == 7 || self.heart_restored(),
                objective: if self.heart_restored() { "Heart Restored" }
                    else if self.sigils == 7 { "Restore the workshop heart" }
                    else { "Requires: 3 Relics" } });
        }
        if engine_worldgen::hell_gate::enabled(cfg) {
            if cfg.world_type == WorldType::Nether {
                entries.push(QuestEntry { id: QuestId::ReturnGate, stage: 0,
                    complete: false, available: true, objective: "Return through the gate" });
            } else if self.heart_restored() && self.knows(11) {
                entries.push(QuestEntry { id: QuestId::BuriedGate, stage: self.knows(13) as u8,
                    complete: self.knows(13), available: true, objective: "Reach the buried gate" });
            }
        }
        entries
    }

    /// Resolve only the selected quest. The log keeps this position for its stage.
    pub fn quest_destination(&self, id: QuestId,
        cfg: &crate::world::WorldSampleConfig, origin: Vec3, workshop_prepared: bool) -> Option<QuestMarker> {
        let (pos, kind) = match id {
            QuestId::Workshop => return super::super::workshop::marker(self, cfg, origin, workshop_prepared),
            QuestId::Foundry | QuestId::Archive | QuestId::Cathedral => {
                let collected = match id { QuestId::Foundry => 0, QuestId::Archive => 1, _ => 3 };
                let (kind, pos) = sites::next_relic(cfg, origin, collected)?;
                (pos, kind)
            }
            QuestId::Heart => (sites::workshop_heart(cfg)?, 3),
            QuestId::BuriedGate | QuestId::ReturnGate => {
                if !engine_worldgen::hell_gate::enabled(cfg) { return None; }
                let center = engine_worldgen::hell_gate::center(cfg, origin);
                (center + Vec3::Y * engine_worldgen::hell_gate::floor(cfg, center), 3)
            }
        };
        let delta = pos - origin;
        Some(QuestMarker { pos, label: id.name(), kind,
            bearing_deg: delta.x.atan2(-delta.z).to_degrees().rem_euclid(360.0),
            distance_m: delta.x.hypot(delta.z), vertical_m: delta.y })
    }

    /// Resolve the next authored expedition objective to a compass waypoint.
    /// This is derived from the same generator used by the relic trace, so the
    /// marker can never point at a fabricated or stale location.
    #[cfg(test)]
    pub fn quest_marker(
        &self,
        cfg: &crate::world::WorldSampleConfig,
        origin: Vec3,
    ) -> Option<QuestMarker> {
        // A known lamp route leads onward after the heart is repaired; in
        // the Nether the compass always offers a physical way back.
        if engine_worldgen::hell_gate::enabled(cfg)
            && (cfg.world_type == crate::world::WorldType::Nether
                || self.heart_restored() && self.knows(11) && !self.knows(13)) {
            let center = engine_worldgen::hell_gate::center(cfg, origin);
            let pos = center + Vec3::Y * engine_worldgen::hell_gate::floor(cfg, center);
            let delta = pos - origin;
            return Some(QuestMarker {
                pos, label: if cfg.world_type == crate::world::WorldType::Nether { "Return gate" } else { "Buried gate" },
                kind: 3,
                bearing_deg: delta.x.atan2(-delta.z).to_degrees().rem_euclid(360.0),
                distance_m: Vec3::new(delta.x, 0.0, delta.z).length(), vertical_m: delta.y,
            });
        }
        if let Some(marker) = super::super::workshop::marker(self, cfg, origin, false) {
            return Some(marker);
        }
        let (pos, label, kind) = if self.sigils == 7 && !self.heart_restored() {
            (sites::workshop_heart(cfg)?, "Workshop heart", 3)
        } else if let Some((kind, pos)) = sites::next_relic(cfg, origin, self.sigils) {
            (
                pos,
                match kind {
                    3 => "Foundry resonance",
                    5 => "Archive resonance",
                    _ => "Crystal cathedral",
                },
                kind,
            )
        } else if self.heart_restored() {
            let heart = sites::workshop_heart(cfg)?;
            if (heart - origin).length() < 64.0 {
                return None;
            }
            (heart, "Workshop heart", 3)
        } else {
            return None;
        };
        let delta = pos - origin;
        Some(QuestMarker {
            pos,
            label,
            kind,
            bearing_deg: delta.x.atan2(-delta.z).to_degrees().rem_euclid(360.0),
            distance_m: Vec3::new(delta.x, 0.0, delta.z).length(),
            vertical_m: delta.y,
        })
    }

    pub(super) fn update_stations(&mut self, world: &World) {
        self.stations.clear();
        let cfg = world.sample_config();
        for r in &self.reliquaries {
            if r.kind != 3 || observatory::is_site(r.site) {
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
                self.pickup_burst(pos + Vec3::Y * 0.7, [0.45, 0.95, 0.55]);
                world.expedition.say("Maintenance cache: restorative tonic. The bypass vents the steam trap safely; the valve sequence still controls the furnace.");
            }
        } else if sealed(3, state) {
            let progress = (state >> 12 & 3) as usize;
            let correct = kind == ORDER[progress];
            let next = if correct { progress + 1 } else { 0 };
            *self.records.entry(site).or_default() = (state & !VALVES) | ((next as u32) << 12);
            if correct {
                if next == 3 {
                    self.health = self.max_health();
                    self.mana = self.max_mana();
                }
                sound(&mut self.sounds, Cue::Clang, Some(pos), 0.65);
                self.effect(pos + Vec3::Y, [0.3, 1.0, 0.65], 0.6);
                world.expedition.say(if next == 3 { "Furnace activated. Health and mana restored." } else { "Valve set. Check the plate for the next valve." });
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
                        fire: false,
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
            self.route = "Heart restored. Use it to recover health and mana.".into();
            world.expedition.say(&self.route);
        } else if self.heart_restored() {
            let danger = self
                .enemies
                .iter()
                .any(|e| e.health > 0.0 && (e.pos - pos).length() < 24.0);
            if danger {
                world
                    .expedition
                    .say("Enemies nearby. Cannot heal.");
                return;
            }
            self.health = self.max_health();
            self.mana = self.max_mana();
            sound(&mut self.sounds, Cue::Mend, Some(pos), 0.7);
            world
                .expedition
                .say("Health and mana restored.");
        } else {
            self.chart_route(world, pos);
        }
    }
    pub(super) fn chart_route(&mut self, world: &mut World, origin: Vec3) {
        if self.sigils == 7 {
            self.route = "Return to the heart beneath the workshop.".into();
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
                "{}: {} {:.0} m. X {:.0}, Z {:.0}{}",
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
            self.route = "No relics nearby. Search farther away.".into();
        }
        world.expedition.say(&self.route);
    }
    /// A tall, bobbing diamond and beam at the authored objective. The HUD
    /// compass gives bearing; this is the diegetic counterpart visible through
    /// an open line of sight.
    pub fn quest_beacon_models(
        &self,
        marker: Option<&QuestMarker>,
        m: &mut Model,
    ) {
        let Some(marker) = marker else {
            return;
        };
        let color = match marker.kind {
            3 => [1.0, 0.72, 0.24],
            5 => [0.66, 0.52, 1.0],
            _ => [0.30, 0.94, 0.78],
        };
        let phase = self.time * 1.7 + marker.pos.x * 0.07 + marker.pos.z * 0.05;
        // Preserve a readable silhouette at several hundred metres without
        // turning a nearby marker into a giant obstruction.
        let far_scale = (marker.distance_m / 240.0).clamp(0.0, 3.0);
        let center = marker.pos + Vec3::Y * (2.1 + far_scale * 0.55 + phase.sin() * 0.18);
        let spin = Quat::from_rotation_y(self.time * 1.25) * Quat::from_rotation_z(0.62);
        m.glow_beam(
            marker.pos + Vec3::Y * 0.25,
            center + Vec3::Y * 0.18,
            0.022 + far_scale * 0.018,
            color,
            2.8,
            0.46,
        );
        m.glow_voxel(
            center,
            Vec3::splat(0.13 + far_scale * 0.10),
            spin,
            color,
            3.4,
            0.78,
        );
        m.glow_voxel(
            center,
            Vec3::splat(0.052),
            Quat::from_rotation_y(-self.time * 2.1),
            [0.92, 1.0, 0.94],
            4.4,
            0.88,
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{WorldSampleConfig, WorldType};

    fn cfg() -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version: 18,
            seed: 424242,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        }
    }

    #[test]
    fn compass_waypoint_uses_authored_relic_and_heart_targets() {
        let mut adventure = Adventure::default();
        let origin = Vec3::new(11.0, -8.0, -90.0);
        let relic = adventure
            .quest_marker(&cfg(), origin)
            .expect("foundry target");
        assert!(relic.distance_m.is_finite() && relic.distance_m > 0.0);
        assert!((0.0..360.0).contains(&relic.bearing_deg));

        adventure.sigils = 7;
        let heart = adventure
            .quest_marker(&cfg(), origin)
            .expect("heart target");
        assert_eq!(heart.label, "Workshop heart");
        assert!(heart.distance_m < 1.0);
    }
}
