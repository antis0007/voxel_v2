use super::*;
use crate::world::BlockId;

pub(crate) const RECALL: u32 = 96;
pub(super) const SALVAGE: u32 = 97;
pub(super) const RECALL_RANGE: f32 = 24.0;
pub(super) const RECALL_SECONDS: f32 = 30.0;
pub(super) const SALVAGE_RANGE: f32 = 8.0;
const CIRCLE_RADIUS: f32 = 0.8;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct CircleAnchor {
    feet: Vec3,
    up: Vec3,
}

impl CircleAnchor {
    fn contains(self, feet: Vec3) -> bool {
        let delta = feet - self.feet;
        delta.length_squared() <= CIRCLE_RADIUS * CIRCLE_RADIUS && delta.dot(self.up).abs() <= 0.4
    }

    fn valid(self) -> bool {
        self.feet.is_finite() && self.feet.abs().max_element() < 100_000_000.0
            && self.up.is_finite() && self.up.is_normalized()
    }

    fn landing(self, world: &World, player: &Player) -> Option<Vec3> {
        let metres = world.voxel_scale.meters();
        let at = self.feet / metres;
        let foot_cell = (at + self.up * 0.1).floor().as_ivec3();
        let clear = if self.up.dot(Vec3::Y) > 0.999 {
            let (lo, hi) = super::super::player_aabb(at, world.voxel_scale, player.scale, 0.0);
            !collides_world(world, lo, hi)
        } else {
            crate::player::radial::clear_landing(world, player, at, self.up)
        };
        (world.rules.border_clamp(self.feet).is_none()
            && factory::solid_point(world, self.feet - self.up * (metres * 0.15))
            && !matches!(world.get(foot_cell), BlockId::Water | BlockId::Lava)
            && clear).then_some(at)
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum CircleRoute {
    #[default]
    Empty,
    Marked(CircleAnchor),
    Linked(CircleAnchor, CircleAnchor),
}

impl CircleRoute {
    pub(crate) fn is_empty(self) -> bool { matches!(self, Self::Empty) }

    pub(crate) fn validated(self) -> Self {
        match self {
            Self::Marked(a) if a.valid() => self,
            Self::Linked(a, b) if a.valid() && b.valid()
                && a.feet.distance_squared(b.feet) > (2.0 * CIRCLE_RADIUS).powi(2) => self,
            _ => Self::Empty,
        }
    }

    pub(crate) fn draw(self, model: &mut Model, eye: Vec3) {
        let draw = |model: &mut Model, anchor: CircleAnchor, linked| {
            if anchor.feet.distance_squared(eye) < 48.0 * 48.0 {
                draw_circle(model, anchor.feet, anchor.up, 1.0, if linked { 1.6 } else { 0.55 });
            }
        };
        match self {
            Self::Empty => {}
            Self::Marked(a) => draw(model, a, false),
            Self::Linked(a, b) => {
                draw(model, a, true);
                draw(model, b, true);
            }
        }
    }
}

pub(super) struct Recall {
    feet: Vec3,
    up: Vec3,
    realm: u64,
    expires: f32,
    armed: bool,
}

impl Adventure {
    pub(super) fn update_recall(&mut self, world: &World, alive: bool, casting: bool) {
        if !alive || !casting {
            self.circle_cast_latched = false;
        }
        if let Some(mark) = self.recall.as_mut() {
            if !alive || mark.realm != world.regions.instance_id || mark.expires <= self.time {
                self.recall = None;
            } else {
                mark.armed |= !casting;
            }
        }
    }

    pub(super) fn recall_thread(&mut self, world: &mut World, player: &mut Player) -> bool {
        let metres = world.voxel_scale.meters();
        let feet = player.pos * metres;
        if self.circle_cast_latched {
            return false;
        }
        if let Some(success) = self.permanent_circle(world, player, feet) {
            self.circle_cast_latched = success;
            return success;
        }
        let Some(mark) = self.recall.as_ref() else {
            if !player.on_ground {
                world.expedition.say("Teleportation Circle · Solid Footing Required");
                return false;
            }
            self.recall = Some(Recall {
                feet,
                up: player.up_dir(),
                realm: world.regions.instance_id,
                expires: self.time + RECALL_SECONDS,
                armed: false,
            });
            world.expedition.say("Circle Set · Release and Cast to Return");
            return true;
        };
        if !mark.armed {
            return false;
        }
        if feet.distance_squared(mark.feet) > RECALL_RANGE * RECALL_RANGE {
            world.expedition.say("Circle · Out of Range");
            return false;
        }
        let at = mark.feet / metres;
        let (lo, hi) = super::super::player_aabb(at, world.voxel_scale, player.scale, player.crouch_t);
        if collides_world(world, lo, hi)
            || !factory::solid_point(world, mark.feet - mark.up * (metres * 0.15))
            || world.rules.border_clamp(mark.feet).is_some()
        {
            world.expedition.say("Circle · Landing Blocked");
            return false;
        }
        player.pos = at;
        player.precise_pos = at.as_dvec3();
        player.vel = Vec3::ZERO;
        self.recall = None;
        self.spawn_voxel_burst(feet, spell_color(RECALL), 12, 1.0, 0.04);
        self.spawn_voxel_burst(at * metres, spell_color(RECALL), 12, 1.0, 0.04);
        true
    }

    fn permanent_circle(&mut self, world: &mut World, player: &mut Player, feet: Vec3) -> Option<bool> {
        use CircleRoute::{Empty, Linked, Marked};
        let route = world.portals.circles;
        let standing = player.on_ground && !player.flying && !player.swimming;
        if player.movement.sneak_was_down {
            if !standing {
                world.expedition.say("Circle · Solid footing required");
                return Some(false);
            }
            let anchor = CircleAnchor { feet, up: player.up_dir() };
            let next = match route {
                Marked(a) if a.contains(feet) => Empty,
                Linked(a, b) if a.contains(feet) || b.contains(feet) => Empty,
                _ if !anchor.valid() || anchor.landing(world, player).is_none() => {
                    world.expedition.say("Circle · Clear ground required");
                    return Some(false);
                }
                Empty => Marked(anchor),
                Marked(home) | Linked(home, _)
                    if home.feet.distance_squared(feet) > (2.0 * CIRCLE_RADIUS).powi(2) =>
                {
                    if home.landing(world, player).is_none() {
                        world.expedition.say("Home Circle · Landing blocked");
                        return Some(false);
                    }
                    Linked(home, anchor)
                }
                Marked(_) | Linked(..) => {
                    world.expedition.say("Circle · Move farther from the first mark");
                    return Some(false);
                }
            };
            world.portals.circles = next;
            self.recall = None;
            world.expedition.say(match next {
                Empty => "Circle Link Cleared",
                Marked(_) => "First Circle Drawn · Sneak-cast at the other end",
                Linked(..) => "Circles Linked · Cast while standing inside",
            });
            return Some(true);
        }
        let destination = match route {
            Linked(a, b) if a.contains(feet) => b,
            Linked(a, b) if b.contains(feet) => a,
            Marked(a) if a.contains(feet) => {
                world.expedition.say("Unlinked Circle · Sneak-cast at the other end");
                return Some(false);
            }
            _ => return None,
        };
        if !standing {
            return Some(false);
        }
        let Some(at) = destination.landing(world, player) else {
            world.expedition.say("Circle · Landing blocked");
            return Some(false);
        };
        player.pos = at;
        player.precise_pos = at.as_dvec3();
        player.vel = Vec3::ZERO;
        player.gravity_frame = (glam::Quat::from_rotation_arc(player.up_dir(), destination.up)
            * player.gravity_frame).normalize();
        player.movement.fall_speed = 0.0;
        self.recall = None;
        self.spawn_voxel_burst(feet, spell_color(RECALL), 12, 1.0, 0.04);
        self.spawn_voxel_burst(destination.feet, spell_color(RECALL), 12, 1.0, 0.04);
        Some(true)
    }

    pub(super) fn salvage_draw(&mut self, world: &mut World, eye: Vec3, forward: Vec3) -> bool {
        let pulls: Vec<_> = world.inventory.dropped.iter().enumerate().filter_map(|(i, stack)| {
            let delta = stack.pos - eye;
            let distance = delta.length();
            if !(1.0..=SALVAGE_RANGE).contains(&distance)
                || delta.dot(forward) < distance * 0.75 || !line(world, eye, stack.pos)
            {
                return None;
            }
            Some((i, stack.pos - delta / distance * (distance - 0.75).min(3.0)))
        }).take(16).collect();
        for &(i, destination) in &pulls {
            world.inventory.dropped[i].pos = destination;
            self.spawn_voxel_burst(destination, spell_color(SALVAGE), 3, 0.7, 0.026);
        }
        !pulls.is_empty()
    }

    pub(super) fn recall_model(&self, model: &mut Model) {
        let Some(mark) = &self.recall else { return };
        let remaining = ((mark.expires - self.time) / RECALL_SECONDS).clamp(0.0, 1.0);
        draw_circle(model, mark.feet, mark.up, remaining, if mark.armed { 1.3 } else { 0.45 });
    }
}

fn draw_circle(model: &mut Model, feet: Vec3, up: Vec3, remaining: f32, glow: f32) {
    let (right, forward) = up.any_orthonormal_pair();
    let center = feet + up * 0.045;
    let point = |angle: f32, radius: f32| {
        center + (right * angle.cos() + forward * angle.sin()) * radius
    };
    let color = spell_color(RECALL);
    let step = std::f32::consts::TAU / 12.0;
    for i in 0..12 {
        let start = i as f32 * step;
        model.beam(point(start, 0.48), point(start + step, 0.48), 0.012, color, 0.35);
        let lit = (remaining * 12.0 - i as f32).clamp(0.0, 1.0);
        if lit > 0.0 {
            model.beam(point(start, 0.62), point(start + step * 0.84 * lit, 0.62),
                0.018, color, glow);
        }
    }
    for i in [0, 3, 6, 9] {
        let angle = i as f32 * step;
        let tip = point(angle, 0.26);
        for side in [-1.0, 1.0] {
            model.beam(tip, point(angle + side * 0.22, 0.42), 0.012, color, glow);
        }
    }
}
