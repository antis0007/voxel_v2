//! Diegetic, deterministic discovery objects. Record flags survive saves and streaming.
use super::*;

mod access;

// These values belong to the existing save format; do not renumber them.
const BOSS_DEFEATED: u32 = 1 << 3;
const GUARDIANS_CLEARED: u32 = 1 << 8;
pub(super) const AWAKENED: u32 = 1 << 9;
pub(super) const CLAIMED: u32 = 1 << 10;
pub(super) const PHYSICAL: u32 = 1 << 11;

const RELIC_REACH: f32 = 3.8;
const RELIC_AIM_RADIUS: f32 = 0.75;
const RELIC_FOCUS_HEIGHT: f32 = 0.85;
const BOSS_VIEW_RANGE: f32 = 32.0;
const ENEMY_FOCUS_RANGE: f32 = 26.0;
const ENEMY_MIN_ALIGNMENT: f32 = 0.94;

#[derive(Clone)]
pub struct Reliquary {
    pub site: IVec3,
    pub pos: Vec3,
    pub kind: u32,
    pub state: u32,
}

/// One prompt shape is shared with world mechanisms so the HUD has one path.
pub type RelicFocus = crate::world::interaction::Prompt;

#[derive(Clone)]
pub struct BossView {
    pub name: &'static str,
    pub action: Option<&'static str>,
    pub health: f32,
    pub maximum: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RelicStage {
    Sealed,
    Dormant,
    Awakened,
    Ready,
    Claimed,
}

fn relic_stage(kind: u32, state: u32) -> RelicStage {
    if state & CLAIMED != 0 {
        RelicStage::Claimed
    } else if state & GUARDIANS_CLEARED == 0 {
        RelicStage::Sealed
    } else if bosses::kind(kind) > 0 && state & BOSS_DEFEATED == 0 {
        if state & AWAKENED == 0 {
            RelicStage::Dormant
        } else {
            RelicStage::Awakened
        }
    } else {
        RelicStage::Ready
    }
}

fn finite_position(p: Vec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

fn unit_direction(direction: Vec3) -> Option<Vec3> {
    let length_squared = direction.length_squared();
    if length_squared.is_finite() && length_squared > 0.0 {
        Some(direction / length_squared.sqrt())
    } else {
        None
    }
}

pub(super) fn place(world: &World, center: Vec3) -> Option<Vec3> {
    if !finite_position(center) {
        return None;
    }
    for i in 0..20 {
        // All four initial indices have radius zero. Probe that column only once,
        // preserving the order and coordinates of every distinct candidate.
        if (1..4).contains(&i) {
            continue;
        }
        let a = i as f32 * 2.4;
        let r = (i / 4) as f32 * 1.5;
        let xz = center + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        for j in 0..24 {
            let p = xz + Vec3::Y * (3.0 - j as f32 * 0.25);
            if factory::solid_point(world, p - Vec3::Y * 0.18) && free(world, p, 0) {
                return Some(p);
            }
        }
    }
    None
}

impl Adventure {
    fn valid_relic_kind(&self, kind: u32) -> bool {
        // Use the actual field width; the parent owns the discovery mask's type.
        let discovery_bits = (std::mem::size_of_val(&self.discoveries) * 8) as u32;
        kind < discovery_bits && (kind as usize) < inscriptions::INSCRIPTIONS.len()
    }

    // Persisted records are authoritative; streamed objects retain a snapshot.
    fn relic_state(&self, relic: &Reliquary) -> u32 {
        self.records
            .get(&relic.site)
            .copied()
            .unwrap_or(relic.state)
    }

    // Call only after checking record capacity and completing fallible prerequisites.
    fn add_relic_flags(&mut self, index: usize, flags: u32) {
        let site = self.reliquaries[index].site;
        let snapshot = self.reliquaries[index].state;
        let record = self.records.entry(site).or_insert(snapshot);
        *record |= flags;
        let state = *record;
        self.reliquaries[index].state = state;
    }

    pub(super) fn update_reliquaries(&mut self, world: &mut World, feet: Vec3) {
        self.reliquaries.clear();
        let cfg = world.sample_config();
        let encounters = sites::encounters(&cfg, feet);
        let nearby: Vec<IVec3> = encounters.iter().map(|e| e.0).collect();
        for (site, center, _) in encounters {
            let kind = sites::reward_kind(&cfg, site);
            if !self.valid_relic_kind(kind) {
                continue;
            }
            let Some(pos) = place(world, center) else {
                continue;
            };
            if !self.admit_record(site, &nearby) {
                continue;
            }
            // A failed placement must not consume a new persistent record slot.
            // Existing records keep their saved flags; only a new slot starts as PHYSICAL.
            let state = *self.records.entry(site).or_insert(PHYSICAL);
            self.reliquaries.push(Reliquary {
                site,
                pos,
                kind,
                state,
            });
        }
        access::tend(self, world);
        self.update_stations(world);
    }

    fn focused_relic(&self, world: &World, eye: Vec3, forward: Vec3) -> Option<usize> {
        if !finite_position(eye) {
            return None;
        }
        let forward = unit_direction(forward)?;
        let mut closest = None;
        let mut closest_distance_squared = RELIC_REACH * RELIC_REACH;
        for (i, relic) in self.reliquaries.iter().enumerate() {
            if !self.valid_relic_kind(relic.kind) {
                continue;
            }
            let target = relic.pos + Vec3::Y * RELIC_FOCUS_HEIGHT;
            let offset = target - eye;
            let distance_squared = offset.length_squared();
            let along = offset.dot(forward);
            if !(distance_squared < closest_distance_squared && along > 0.0) {
                continue;
            }
            if (offset - forward * along).length_squared() >= RELIC_AIM_RADIUS * RELIC_AIM_RADIUS {
                continue;
            }
            // Occluded candidates never hide a farther visible candidate.
            if line(world, eye, target) {
                closest = Some(i);
                closest_distance_squared = distance_squared;
            }
        }
        closest
    }

    pub(super) fn refresh_focus(&mut self, world: &World, eye: Vec3, forward: Vec3) {
        if !finite_position(eye) {
            self.relic_focus = None;
            self.undercrown_focus = None;
            self.boss_view = None;
            self.enemy_focus = None;
            return;
        }
        let direction = unit_direction(forward);
        self.relic_focus = direction
            .and_then(|forward| self.focused_relic(world, eye, forward))
            .map(|i| {
                let relic = &self.reliquaries[i];
                let state = self.relic_state(relic);
                if relic.site == IVec3::ZERO && state & CLAIMED != 0 {
                    return self.heart_focus();
                }
                let boss = bosses::kind(relic.kind);
                RelicFocus {
                    title: if boss > 0 {
                        bosses::name(boss)
                    } else {
                        inscriptions::INSCRIPTIONS[relic.kind as usize].name
                    },
                    action: match relic_stage(relic.kind, state) {
                        RelicStage::Claimed => "F - Attune sanctum & trace next relic",
                        RelicStage::Sealed => "Defeat the guardians to unseal",
                        RelicStage::Dormant => "F - Awaken guardian",
                        RelicStage::Awakened => "Defeat the awakened guardian",
                        RelicStage::Ready => "F - Claim relic",
                    },
                }
            });
        if let Some(forward) = direction
            && let Some(focus) = self.station_focus(world, eye, forward) {
                self.relic_focus = Some(focus);
            }
        if let Some(ray) = direction
            && let Some(prompt) = self.enemies.iter().find_map(|e| engine::focus(e, world, eye, ray)) {
            self.relic_focus = Some(prompt);
        }

        let mut closest_boss = None;
        let mut closest_boss_distance_squared = BOSS_VIEW_RANGE * BOSS_VIEW_RANGE;
        let mut focused_enemy = None;
        let mut best_enemy_score = f32::INFINITY;
        for enemy in &self.enemies {
            // NaN health counts as dead; keep the negated form on purpose.
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if !(enemy.health > 0.0) {
                continue;
            }
            let center = enemy.center();
            let offset = center - eye;
            let distance_squared = offset.length_squared();
            if is_boss(enemy.kind) && distance_squared < closest_boss_distance_squared {
                closest_boss = Some(enemy);
                closest_boss_distance_squared = distance_squared;
            }
            let Some(forward) = direction else {
                continue;
            };
            // A disguise must not announce its species and health across the room.
            // Its physical seam/glint and player attacks remain valid counterplay.
            if enemy.encounter.concealed { continue; }
            if !(distance_squared > 0.1 * 0.1
                && distance_squared <= ENEMY_FOCUS_RANGE * ENEMY_FOCUS_RANGE)
            {
                continue;
            }
            let distance = distance_squared.sqrt();
            let alignment = (forward.dot(offset) / distance).clamp(-1.0, 1.0);
            if alignment <= ENEMY_MIN_ALIGNMENT {
                continue;
            }
            let score = distance * (1.05 - alignment);
            if score < best_enemy_score && line(world, eye, center) {
                focused_enemy = Some(enemy);
                best_enemy_score = score;
            }
        }
        self.boss_view = closest_boss.map(|enemy| BossView {
            action: if enemy.kind == engine::KIND {
                engine::action_name(enemy)
            } else if enemy.kind == glassweaver::KIND {
                glassweaver::action_name(enemy)
            } else if enemy.kind==matron::KIND {matron::action_name(enemy)} else {None},
            name: bosses::title(enemy),
            health: enemy.health,
            maximum: enemy.max_health(),
        });
        self.enemy_focus = focused_enemy.map(|enemy| super::EnemyFocus {
            kind: enemy.kind,
            name: super::encounters::name(enemy),
            health: enemy.health,
            maximum: enemy.max_health(),
            frozen: enemy.slow > 0.0,
            affixes: enemy.affixes,
            status: enemy.status,
            shield: enemy.shield,
        });
        // Reliquaries keep priority; mechanisms only advertise themselves when
        // no relic or station is already focused.
        self.undercrown_focus = if self.relic_focus.is_some() {
            None
        } else {
            direction.and_then(|forward| undercrown::focus(world, eye, forward))
        };
    }

    pub(super) fn interact_relic(&mut self, world: &mut World, eye: Vec3, forward: Vec3) -> bool {
        if !finite_position(eye) {
            return false;
        }
        let Some(forward) = unit_direction(forward) else {
            return false;
        };
        if let Some(i) = self.enemies.iter().position(|e| engine::focus(e, world, eye, forward).is_some()) {
            engine::secure(&mut self.enemies[i], world);
            return true;
        }
        if self.interact_station(world, eye, forward) {
            return true;
        }
        let Some(i) = self.focused_relic(world, eye, forward) else {
            return false;
        };
        let relic = self.reliquaries[i].clone();
        let state = self.relic_state(&relic);
        let stage = relic_stage(relic.kind, state);
        match stage {
            RelicStage::Claimed => {
                self.respawn_point = Some(relic.pos + Vec3::Y * 0.45);
                self.health = (self.health + 18.0).min(self.max_health());
                self.mana = (self.mana + 25.0).min(self.max_mana());
                sound(&mut self.sounds, Cue::Mend, Some(relic.pos), 0.65);
                self.pickup_burst(relic.pos + Vec3::Y * 0.6, [0.45, 0.95, 0.55]);
                if relic.site == IVec3::ZERO {
                    self.use_heart(world, relic.pos);
                } else if self.puzzle_cd <= 0.0 {
                    self.puzzle_cd = 1.0;
                    self.chart_route(world, relic.pos);
                }
                return true;
            }
            RelicStage::Sealed | RelicStage::Awakened => return true,
            RelicStage::Dormant | RelicStage::Ready => {}
        }
        if self.records.len() >= MAX_RECORDS && !self.records.contains_key(&relic.site) {
            world
                .expedition
                .say("The expedition cannot record another relic yet.");
            return true;
        }
        if stage == RelicStage::Dormant {
            // Keep actor lifetime and death cleanup in the parent adventure system.
            if self.spawn_boss(world, &relic, eye) {
                self.add_relic_flags(i, AWAKENED);
                sound(&mut self.sounds, Cue::Warning, Some(relic.pos), 1.0);
            } else {
                world.expedition.say("The guardian cannot awaken here yet. Clear open ground around the reliquary and try again.");
            }
            return true;
        }

        // focused_relic validated the table index and the discovery bit width.
        let color = spell_color(inscriptions::INSCRIPTIONS[relic.kind as usize].spell);
        let weapon = match relic.kind {
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
        self.add_relic_flags(i, CLAIMED | PHYSICAL);
        self.respawn_point = Some(relic.pos + Vec3::Y * 0.45);
        self.health = self.max_health();
        self.mana = self.max_mana();
        self.discoveries |= 1 << relic.kind;
        self.sigils |= quest::sigil(relic.kind);
        if relic.kind == 5 {
            self.rare_spells |= 1 << 6;
        }
        if relic.kind == 8 {
            self.rare_spells |= 1 << 7;
        }
        self.reward = Some(Reward {
            name: match relic.kind {
                3 => "Cinderbrand recovered",
                5 => "Rift well recovered",
                8 => "Winterglass and Echo step recovered",
                10 => "Keeper's passage recovered",
                _ => "Inscription recovered",
            },
            items: Vec::new(),
            seconds: 10.0,
            discovery: Some(relic.kind),
        });
        sound(&mut self.sounds, Cue::Treasure, None, 0.9);
        self.pickup_burst(relic.pos + Vec3::Y * 0.8, color);
        self.effect(relic.pos + Vec3::Y, color, 1.5);
        true
    }

    pub(super) fn spawn_boss(&mut self, world: &World, relic: &Reliquary, eye: Vec3) -> bool {
        if !self.valid_relic_kind(relic.kind)
            || !finite_position(relic.pos)
            || !finite_position(eye)
        {
            return false;
        }
        let kind = bosses::kind(relic.kind);
        if kind == 0 {
            return false;
        }
        if let Some(enemy) = self
            .enemies
            .iter()
            .find(|enemy| enemy.site == relic.site && enemy.slot == 3)
        {
            // A corpse or unrelated occupant is not a successful awakening.
            // Leave its removal and defeat bookkeeping to the existing owner.
            return enemy.kind == kind && enemy.health > 0.0;
        }
        if self.enemies.iter().filter(|e|e.habitat.is_none()).count() >= MAX_ENEMIES {
            return false;
        }
        if kind == tollkeeper::KIND
            && let Some(saved) = self.tollkeepers.get(&relic.site) {
                let mut enemy=Enemy::new(relic.site,3,kind,relic.pos,0);
                tollkeeper::restore(&mut enemy,saved);
                if !free(world,enemy.pos,kind) { return false; }
                self.enemies.push(enemy);
                return true;
            }
        if kind == engine::KIND && let Some(saved) = self.engines.get(&relic.site) {
            let mut enemy = Enemy::new(relic.site, 3, kind, relic.pos, 0);
            engine::restore(&mut enemy, saved);
            // Never move a trapped or blocked saved machine to a fresh spawn.
            self.enemies.push(enemy);
            return true;
        }
        for i in 0..24 {
            let a = i as f32 * 2.4;
            let p = relic.pos + Vec3::new(a.cos(), 0.0, a.sin()) * (4.0 + (i / 8) as f32);
            for dy in [0.0, 0.5, 1.0, -0.5, -1.0] {
                let q = p + Vec3::Y * dy;
                if (q - eye).length_squared() > 2.5 * 2.5
                    && factory::solid_point(world, q - Vec3::Y * 0.2)
                    && free(world, q, kind)
                {
                    let menace = if kind == tollkeeper::KIND || kind == engine::KIND { 0 } else if kind == 8 {
                        (relic
                            .site
                            .x
                            .unsigned_abs()
                            .wrapping_add(relic.site.y.unsigned_abs())
                            % 3) as u8
                    } else {
                        3
                    };
                    self.enemies
                        .push(Enemy::new(relic.site, 3, kind, q, menace));
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn relic_models(&self, m: &mut Model) {
        use std::f32::consts::{FRAC_PI_2, TAU};

        self.station_models(m);
        for r in &self.reliquaries {
            if !self.valid_relic_kind(r.kind) {
                continue;
            }
            let state = self.relic_state(r);
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
                let a = i as f32 * FRAC_PI_2;
                let (sin_a, cos_a) = a.sin_cos();
                let p = r.pos + Vec3::new(cos_a * 0.35, 0.64, sin_a * 0.35);
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
                // A loose fog of voxels waiting on the relic. Claiming it throws them outward.
                for i in 0..14 {
                    let seed = i as f32 * 2.399963;
                    let t = self.time * (0.28 + (i % 4) as f32 * 0.05);
                    let rad = 0.12 + (seed.sin() * 0.5 + 0.5) * 0.5;
                    let y = 0.45 + ((t * 1.7 + seed).sin() * 0.5 + 0.5) * 0.7;
                    let a = seed + t * (0.4 + (i % 3) as f32 * 0.15);
                    let (sin_a, cos_a) = a.sin_cos();
                    let at = r.pos + Vec3::new(cos_a * rad, y, sin_a * rad);
                    let spin = Quat::from_rotation_y(t + seed) * Quat::from_rotation_x(seed);
                    m.glow_voxel(
                        at,
                        Vec3::splat(0.026 + (i % 3) as f32 * 0.01),
                        spin,
                        color,
                        2.2,
                        0.7,
                    );
                }
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
                    let spine = rot * (Vec3::Z * 0.18);
                    m.beam(p - spine, p + spine, 0.025, color, 1.0);
                }
                // Noita-inspired floating 3D pixel-voxel temple glyphs and rising sanctum motes
                for g in 0..3 {
                    let ga = self.time * 0.65 + g as f32 * (TAU / 3.0);
                    let (sin_ga, cos_ga) = ga.sin_cos();
                    let gp = r.pos
                        + Vec3::new(
                            cos_ga * 0.56,
                            1.18 + (self.time * 1.9 + g as f32).sin() * 0.07,
                            sin_ga * 0.56,
                        );
                    m.temple_glyph(
                        gp,
                        Quat::from_rotation_y(-ga + FRAC_PI_2),
                        r.kind + g,
                        0.024,
                        color,
                        3.2,
                        0.72,
                    );
                }
                for m_idx in 0..12 {
                    let ma = self.time * 1.1 + m_idx as f32 * 2.39996;
                    let (sin_ma, cos_ma) = ma.sin_cos();
                    let radius = 0.32 + (m_idx % 3) as f32 * 0.12;
                    let rise = (self.time * 0.48 + m_idx as f32 * 0.17).rem_euclid(1.65);
                    let mp = r.pos + Vec3::new(cos_ma * radius, 0.45 + rise, sin_ma * radius);
                    // Fade geometry and glow together at both ends of the height wrap.
                    let phase = rise / 1.65;
                    let envelope = (1.0 - (phase * 2.0 - 1.0).abs()).clamp(0.0, 1.0);
                    let fade = envelope * envelope * (3.0 - 2.0 * envelope);
                    if fade <= f32::EPSILON {
                        continue;
                    }
                    m.glow_voxel(
                        mp,
                        Vec3::splat(0.028 * fade),
                        Quat::from_rotation_y(ma),
                        color,
                        3.4,
                        0.75 * fade,
                    );
                }
                if matches!(
                    relic_stage(r.kind, state),
                    RelicStage::Sealed | RelicStage::Dormant | RelicStage::Awakened
                ) {
                    for i in 0..4 {
                        let a = i as f32 * FRAC_PI_2 + self.time * 0.3;
                        let (sin_a, cos_a) = a.sin_cos();
                        m.glow_beam(
                            r.pos + Vec3::new(cos_a * 0.4, 0.55, sin_a * 0.4),
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
