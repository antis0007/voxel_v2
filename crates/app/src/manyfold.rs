//! Shared brass weight, counterweight socket, and bounded fold interactions.
use crate::{
    player::Player,
    world::{World, WorldType},
};
use glam::Vec3;

pub mod maze_portal;
pub mod undercroft;
pub mod weave;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weight {
    pub position: Vec3,
    pub velocity: Vec3,
    #[serde(default)]
    pub opened: bool,
    /// Held in the player's frame. Absent from older snapshots.
    #[serde(default)]
    pub carried: bool,
}
impl Default for Weight {
    fn default() -> Self {
        Self {
            position: Vec3::new(1.5, 13.0, 0.0),
            velocity: Vec3::ZERO,
            opened: false,
            carried: false,
        }
    }
}
impl Weight {
    pub fn valid(&self) -> bool {
        self.position.is_finite()
            && self.velocity.is_finite()
            && self.position.abs().max_element() < 1_000_000.0
            && self.velocity.length() < 1000.0
    }
}

/// Use lifts the loose weight. Use again throws it. No inventory item is created.
pub fn use_weight(world: &mut World, p: &Player, pressed: bool) -> bool {
    if world.world_type != WorldType::Manyfold
        || engine_worldgen::manyfold::flat_maze(world.generator_version)
    {
        return false;
    }
    if pressed
        && world.rules.mode.interacts()
        && world.adventure.health > 0.0
        && world.manyfold_weight.carried
        && !world.manyfold_weight.opened
    {
        let scale = world.voxel_scale.meters();
        world.manyfold_weight.carried = false;
        world.manyfold_weight.velocity = throw_velocity(p) + p.vel * scale;
        return true;
    }
    if pressed
        && world.rules.mode.interacts()
        && world.adventure.health > 0.0
        && world.manyfold_weight.opened
        && hint(world, p) == Some("Passage open")
    {
        if !world::adventure::chronicle::grant(world, 14) {
            world
                .expedition
                .say(world::adventure::chronicle::history(14).unwrap());
        }
        return true;
    }
    if pressed && world.rules.mode.interacts() && undercroft::interact(world, p) {
        return true;
    }
    if pressed && world.rules.mode.interacts() && weave::interact(world, p) {
        return true;
    }
    if !world.rules.mode.interacts() || !pressed || world.manyfold_weight.opened {
        return false;
    }
    let eye = p.eye(world.voxel_scale) * world.voxel_scale.meters();
    let delta = world.manyfold_weight.position - eye;
    if delta.length() < 3.5 && p.forward_dir().dot(delta.normalize_or_zero()) > 0.8 {
        let scale = world.voxel_scale.meters();
        if world::raycast_block(
            world,
            eye / scale,
            delta.normalize_or_zero(),
            (delta.length() - 0.3).max(0.0) / scale,
        )
        .is_none()
        {
            world.manyfold_weight.carried = true;
            world.manyfold_weight.velocity = Vec3::ZERO;
            return true;
        }
    }
    false
}

/// A look into the floor used to cancel the toss. Loft along the shell instead.
fn throw_velocity(p: &Player) -> Vec3 {
    let up = p.up_dir();
    let look = p.forward_dir();
    let along = look - up * look.dot(up);
    let dir = if look.dot(up) > 0.35 {
        look
    } else if along.length_squared() > 1e-4 {
        (along.normalize() * 0.85 + up * 0.55).normalize_or_zero()
    } else {
        up
    };
    dir * 16.0
}
use crate::world;

pub fn hint(world: &World, p: &Player) -> Option<&'static str> {
    if world.world_type != WorldType::Manyfold
        || engine_worldgen::manyfold::flat_maze(world.generator_version)
    {
        return None;
    }
    if world.manyfold_weight.carried && !world.manyfold_weight.opened {
        return Some("Throw weight");
    }
    let scale = world.voxel_scale.meters();
    let eye = p.eye(world.voxel_scale) * scale;
    let focus = |point: Vec3| {
        let d = point - eye;
        d.length() < 3.5
            && p.forward_dir().dot(d.normalize_or_zero()) > 0.8
            && world::raycast_block(
                world,
                eye / scale,
                d.normalize_or_zero(),
                (d.length() - 0.3).max(0.0) / scale,
            )
            .is_none()
    };
    if !world.manyfold_weight.opened && focus(world.manyfold_weight.position) {
        return Some("Pick up weight");
    }
    let socket = engine_worldgen::manyfold::socket(&world.sample_config())?;
    focus(socket).then_some(if world.manyfold_weight.opened {
        "Passage open"
    } else {
        "Weight socket"
    })
}

/// Host/solo fixed step only. Positions and velocities are in metres.
pub fn tick(
    world: &mut World,
    player: &Player,
    dt: f32,
    blocks: &mut std::collections::HashSet<glam::IVec3>,
    chunks: &mut std::collections::HashSet<glam::IVec3>,
) {
    if world.world_type != WorldType::Manyfold
        || engine_worldgen::manyfold::flat_maze(world.generator_version)
    {
        return;
    }
    if world.manyfold_weight.opened {
        return;
    }
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }
    if world.manyfold_weight.carried {
        let scale = world.voxel_scale.meters();
        let hold = player.eye(world.voxel_scale) * scale + player.forward_dir() * 1.15
            - player.up_dir() * 0.35;
        world.manyfold_weight.position = hold;
        world.manyfold_weight.velocity = Vec3::ZERO;
        if dock(world, blocks, chunks) {
            world.manyfold_weight.carried = false;
        }
        return;
    }
    if !world.manyfold_weight.valid()
        || !engine_worldgen::manyfold::contains_weight(
            world.generator_version,
            world.manyfold_weight.position,
        )
    {
        // Recovery relocates the same object. It neither mints an inventory item
        // nor clears construction around its cradle.
        let scale = world.voxel_scale.meters();
        let base = engine_worldgen::manyfold::cradle(&world.sample_config())
            .unwrap_or(Weight::default().position);
        for i in 0..16 {
            let point = base + Vec3::Y * (i as f32 * 0.25);
            if !world::collides_world(
                world,
                (point - Vec3::splat(0.2)) / scale,
                (point + Vec3::splat(0.2)) / scale,
            ) {
                world.manyfold_weight.position = point;
                world.manyfold_weight.velocity = Vec3::ZERO;
                return;
            }
        }
        return;
    }
    if dock(world, blocks, chunks) {
        return;
    }
    let mut weight = world.manyfold_weight;
    let gravity =
        engine_worldgen::manyfold::acceleration_for(world.generator_version, weight.position);
    weight.velocity += gravity * dt;
    let floor_normal = gravity.normalize_or_zero();
    let scale = world.voxel_scale.meters();
    // Bound both work and displacement; a cap alone could tunnel at high speed.
    weight.velocity = weight
        .velocity
        .clamp_length_max((64.0 * 0.1 * scale / dt).min(40.0));
    let steps = (weight.velocity.length() * dt / (0.1 * scale))
        .ceil()
        .clamp(1.0, 64.0) as u32;
    let h = dt / steps as f32;
    for _ in 0..steps {
        match weave::weight_transit(
            world,
            weight.position,
            weight.position + weight.velocity * h,
            weight.velocity,
        ) {
            weave::WeightTransit::Crossed(position, velocity) => {
                weight.position = position;
                weight.velocity = velocity;
                continue;
            }
            weave::WeightTransit::Blocked => {
                weight.velocity = Vec3::ZERO;
                break;
            }
            weave::WeightTransit::None => {}
        }
        for axis in 0..3 {
            let mut next = weight.position;
            next[axis] += weight.velocity[axis] * h;
            let radius = Vec3::splat(0.2);
            if world::collides_world(world, (next - radius) / scale, (next + radius) / scale) {
                // Slide along the shell. Zeroing a world axis left only a flat shove.
                if floor_normal.length_squared() > 0.0 {
                    let into = weight.velocity.dot(floor_normal);
                    if into > 0.0 {
                        weight.velocity -= floor_normal * into;
                    }
                    next = weight.position;
                    next[axis] += weight.velocity[axis] * h;
                    if !world::collides_world(
                        world,
                        (next - radius) / scale,
                        (next + radius) / scale,
                    ) {
                        weight.position = next;
                        continue;
                    }
                }
                weight.velocity[axis] = if weight.velocity[axis].abs() > 0.5 {
                    -weight.velocity[axis] * 0.2
                } else {
                    0.0
                };
            } else {
                weight.position = next;
            }
        }
    }
    if let Some(curve) = engine_worldgen::manyfold::shell_curve(world.generator_version) {
        let pinned = curve.project(weight.position);
        if pinned.distance_squared(weight.position) > 1e-8 && shell_floor_clear(world, curve, pinned) {
            let radial = pinned - curve.center;
            let outward = Vec3::new(radial.x, radial.y, 0.0).normalize_or_zero();
            let into = weight.velocity.dot(outward);
            if into > 0.0 {
                weight.velocity -= outward * into;
            }
            weight.position = pinned;
        }
    }
    world.manyfold_weight = weight;
}

/// A wall in the hollow keeps the weight. Shell crust does not: that box is the stair.
fn shell_floor_clear(world: &World, curve: engine_worldgen::manyfold::ShellCurve, point: Vec3) -> bool {
    let scale = world.voxel_scale.meters();
    let radius = Vec3::splat(0.2);
    let lo = ((point - radius) / scale).floor().as_ivec3();
    let hi = ((point + radius) / scale).floor().as_ivec3();
    for z in lo.z..=hi.z {
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let cell = glam::IVec3::new(x, y, z);
                if !world.solid(cell) {
                    continue;
                }
                let centre = (cell.as_vec3() + glam::Vec3::splat(0.5)) * scale;
                if curve.offset(centre) <= 0.05 {
                    return false;
                }
            }
        }
    }
    true
}

fn dock(
    world: &mut World,
    blocks: &mut std::collections::HashSet<glam::IVec3>,
    chunks: &mut std::collections::HashSet<glam::IVec3>,
) -> bool {
    let cfg = world.sample_config();
    let Some(socket) = engine_worldgen::manyfold::socket(&cfg) else {
        return false;
    };
    let delta = socket - world.manyfold_weight.position;
    if delta.length() > 0.85 {
        return false;
    }
    let scale = world.voxel_scale.meters();
    if delta.length() > 0.05
        && world::raycast_block(
            world,
            world.manyfold_weight.position / scale,
            delta.normalize_or_zero(),
            delta.length() / scale,
        )
        .is_some()
    {
        return false;
    }
    world.manyfold_weight.position = socket;
    world.manyfold_weight.velocity = Vec3::ZERO;
    world.manyfold_weight.carried = false;
    let mut pending = false;
    let mut edits = 0;
    for &(cell, expected) in engine_worldgen::manyfold::shortcut_cells_cached(&cfg).iter() {
        let actual = world.get(cell);
        if actual != expected {
            pending |= actual.is_solid();
            continue;
        }
        if edits >= 64 {
            pending = true;
            break;
        }
        if !world.admit_edit_chunk(world::chunk_coord_from_world_block(cell)) {
            pending = true;
            continue;
        }
        match world::effects::replace(world, cell, expected, world::BlockId::Air) {
            Ok(commit) => {
                commit.publish(world, blocks, chunks);
                edits += 1;
            }
            Err(_) => pending = true,
        }
    }
    if !pending {
        world.manyfold_weight.opened = true;
        if !world::adventure::chronicle::grant(world, 14) {
            world.expedition.say("Passage open.");
        }
    }
    true
}
