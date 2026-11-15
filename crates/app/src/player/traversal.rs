//! Ballast working harnesses, Founders' propulsion and Keepers' elastic gear.
use super::{Input, Player, movement};
use crate::world::{
    World,
    inventory::{ArmorTier, EquipSlot, Item},
    speed_blocks, try_move_local,
};
use glam::{IVec3, Vec3};

pub const ARM_REACH: f32 = 3.0;
pub const BURN_SECONDS: f32 = 12.0;
pub const CHARGE_CAPACITY: f32 = 120_000.0;
pub const POWER_PER_SECOND: f32 = 4_000.0;
pub const SLING_SECONDS: f32 = 0.8;
pub const SLING_SPEED: f32 = 22.0;
/// Below this stretch a release is a fumble, not a launch.
const SLING_MIN_CHARGE: f32 = 0.15;
/// One airborne release per flight, at this share of the ground launch.
const SLING_AIR_POWER: f32 = 0.6;
/// Releasing within this many seconds of full stretch snaps harder.
pub const SLING_SNAP_WINDOW: f32 = 0.25;
const SLING_SNAP_BONUS: f32 = 1.15;
/// Landing rebound share, and the cap for an untimed bounce (m/s).
const BOUNCE_KEEP: f32 = 0.82;
pub const BOUNCE_CAP: f32 = 20.0;
/// Falls slower than this settle instead of rebounding (m/s).
const BOUNCE_MIN: f32 = 4.0;
/// Jump pressed just before touchdown pumps the rebound: more kept, a kick added, a higher cap.
const PUMP_KEEP: f32 = 0.96;
const PUMP_KICK: f32 = 3.0;
const PUMP_CAP: f32 = 24.0;
/// Airborne wall hits faster than this rebound off slime soles (m/s into the wall).
const WALL_BOUNCE_MIN: f32 = 7.0;
const WALL_BOUNCE_KEEP: f32 = 0.72;
/// Upward pop added by a wall rebound (m/s), so wall-to-wall chains climb a little.
const WALL_BOUNCE_LIFT: f32 = 3.0;

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Traversal {
    // Reservoirs belong to the wearer, so changing packs never refills them.
    pub fuel_seconds: f32,
    pub charge_fe: f32,
    #[serde(skip)]
    pub octo: bool,
    #[serde(skip)]
    pub jet: u8,
    #[serde(skip)]
    pub slime_boots: bool,
    #[serde(skip)]
    pub sling: bool,
    #[serde(skip)]
    pub attached: bool,
    #[serde(skip)]
    pub contacts: [Option<Vec3>; 8],
    #[serde(skip)]
    pub thrust: bool,
    #[serde(skip)]
    pub hover: bool,
    #[serde(skip)]
    hover_was_down: bool,
    #[serde(skip)]
    pub sling_charge: f32,
    #[serde(skip)]
    sling_was_down: bool,
    #[serde(skip)]
    sling_cooldown: f32,
    /// Seconds held at full stretch; inside the snap window a release hits harder.
    #[serde(skip)]
    pub sling_full: f32,
    /// The single airborne release has been spent; landing restores it.
    #[serde(skip)]
    pub sling_air_used: bool,
}

impl Traversal {
    pub(crate) fn checkpoint_valid(&self) -> bool {
        (0.0..=BURN_SECONDS).contains(&self.fuel_seconds)
            && (0.0..=CHARGE_CAPACITY).contains(&self.charge_fe)
    }
}

/// Called by local gameplay and the client's prediction before movement.
pub fn prepare(world: &mut World, player: &mut Player, input: &Input, dt: f32) {
    player.glide.available = world.inventory.glide_frame();
    let worn = |slot, tier| {
        world.inventory.equipment[slot as usize].is_some_and(|s| s.item == Item::Armor(slot, tier))
    };
    let t = &mut player.traversal;
    t.octo = worn(EquipSlot::Chest, ArmorTier::Ballast);
    t.jet = if worn(EquipSlot::Chest, ArmorTier::Induction) {
        2
    } else if worn(EquipSlot::Chest, ArmorTier::Burner) {
        1
    } else {
        0
    };
    t.slime_boots = worn(EquipSlot::Feet, ArmorTier::Slime);
    t.sling = world.adventure.wand == 0
        && world
            .inventory
            .hotbar
            .get(world.adventure.hand_slot)
            .copied()
            .flatten()
            .is_some_and(|s| s.item == Item::Tool(19));
    t.thrust = false;
    if !t.octo || player.flying || player.swimming {
        t.attached = false;
        t.contacts = [None; 8];
    }
    if !t.sling || player.flying || player.swimming {
        t.sling_charge = 0.0;
        t.sling_full = 0.0;
        t.sling_was_down = false;
    }
    let hover_down = input.alt_down && input.sneak;
    if t.jet != 0 && hover_down && !t.hover_was_down {
        t.hover = !t.hover;
    }
    t.hover_was_down = hover_down;
    if t.jet == 0 {
        t.hover = false;
    }
    let requesting = t.jet != 0
        && !player.flying
        && !player.swimming
        && !world.adventure.is_dead()
        && (input.jump || t.hover && !player.on_ground);
    if !requesting || world.adventure.creative {
        return;
    }
    if t.jet == 1
        && t.fuel_seconds < dt
        && (crate::world::factory::industry::use_fuel_cell(&mut world.inventory)
            || world.inventory.remove_item(Item::Resource(crate::world::factory::ETHANOL as u8), 1))
    {
        t.fuel_seconds = BURN_SECONDS;
        world.sync_from_inventory();
    }
    if t.jet == 2 && t.charge_fe < POWER_PER_SECOND * dt {
        // Only a charged portable battery pays; an ordinary battery block is empty.
        let mut changed = false;
        for stack in world
            .inventory
            .pack
            .iter_mut()
            .chain(world.inventory.hotbar.iter_mut())
            .chain(std::iter::once(&mut world.inventory.offhand))
            .flatten()
        {
            if let Item::PowerKit(mut kit) = stack.item
                && kit.kind == crate::world::factory::Kind::Battery
                && kit.energy > 0
            {
                let transfer = kit
                    .energy
                    .min((CHARGE_CAPACITY - t.charge_fe).floor() as u32);
                kit.energy -= transfer;
                t.charge_fe += transfer as f32;
                stack.item = Item::PowerKit(kit);
                changed |= transfer > 0;
                if t.charge_fe >= POWER_PER_SECOND * dt {
                    break;
                }
            }
        }
        if changed {
            world.sync_from_inventory();
        }
    }
}

/// Eight bounded rays: feet on the floor, claws on walls or overhead.
pub fn octo_step(
    player: &mut Player,
    input: &Input,
    world: &World,
    dt: f32,
    origin: IVec3,
    fresh_jump: bool,
) -> bool {
    if !octo_velocity(player, input, world, dt, origin, fresh_jump) {
        return false;
    }
    player.on_ground = false;
    for delta in [
        Vec3::new(player.vel.x, 0.0, player.vel.z) * dt,
        Vec3::Y * player.vel.y * dt,
    ] {
        try_move_local(
            origin,
            world,
            player.scale,
            player.crouch_t,
            &mut player.pos,
            delta,
            &mut player.on_ground,
            false,
        );
    }
    true
}

pub(super) fn octo_velocity(
    player: &mut Player,
    input: &Input,
    world: &World,
    dt: f32,
    origin: IVec3,
    fresh_jump: bool,
) -> bool {
    if !player.traversal.octo || player.flying || player.swimming {
        return false;
    }
    if fresh_jump && input.alt_down {
        player.traversal.attached = !player.traversal.attached;
    }
    if !player.traversal.attached {
        return false;
    }
    let m = world.voxel_scale.meters();
    let up = player.up_dir();
    let centre = player.pos + up * (0.85 * player.scale / m);
    let yaw = player.gravity_frame * glam::Quat::from_rotation_y(-player.yaw);
    let mut contacts = [None; 8];
    for (i, contact) in contacts.iter_mut().enumerate() {
        let angle = (i % 4) as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4;
        let dir =
            yaw * Vec3::new(angle.sin(), if i < 4 { -0.85 } else { 0.85 }, angle.cos()).normalize();
        if let Some((cell, _)) = crate::world::raycast_block_precise(
            world,
            origin.as_dvec3() + centre.as_dvec3(),
            dir,
            ARM_REACH / m,
        ) {
            if !world.solid(cell) {
                continue;
            }
            // Intersect the hit cell's near face, rather than sinking a claw into its centre.
            let lo = (cell - origin).as_vec3();
            let hi = lo + Vec3::ONE;
            let mut near = 0.0_f32;
            for axis in 0..3 {
                if dir[axis].abs() > 1e-5 {
                    near = near.max(
                        ((lo[axis] - centre[axis]) / dir[axis])
                            .min((hi[axis] - centre[axis]) / dir[axis]),
                    );
                }
            }
            *contact = Some((centre + dir * near - player.pos) * m);
        }
    }
    player.traversal.contacts = contacts;
    // A lone claw cannot carry the body; loss of the span returns to normal falling.
    if contacts.iter().flatten().count() < 2 {
        player.traversal.attached = false;
        return false;
    }
    let forward = player.flat_forward();
    let side = player.right_dir();
    let axis = |a: bool, b: bool| i32::from(a) as f32 - i32::from(b) as f32;
    let wish = forward * axis(input.forward, input.back)
        + side * axis(input.right, input.left)
        + up * axis(input.jump && !input.alt_down, input.sneak);
    player.vel = wish.normalize_or_zero() * speed_blocks(world.voxel_scale, 3.6);
    player.movement.settle(false, dt);
    player.movement.fall_speed = 0.0;
    player.glide.deployed = false;
    player.traversal.sling_charge = 0.0;
    player.traversal.sling_full = 0.0;
    player.traversal.sling_was_down = input.place_block;
    true
}

/// Applied after walking gravity, before the shared collision sweep.
pub fn propel(player: &mut Player, input: &Input, world: &World, dt: f32) {
    let direction = player.forward_dir();
    let up = player.up_dir();
    let grounded = player.on_ground || player.coyote_time > 0.0;
    let t = &mut player.traversal;
    t.sling_cooldown = (t.sling_cooldown - dt).max(0.0);
    if grounded {
        t.sling_air_used = false;
    }
    if t.sling {
        let held = input.place_block;
        // Stretch wherever a release could still fire: on the ground, or once per flight.
        let can_fire = grounded || !t.sling_air_used;
        if held && can_fire && t.sling_cooldown <= 0.0 {
            if t.sling_charge >= 1.0 {
                t.sling_full += dt;
            }
            t.sling_charge = (t.sling_charge + dt / SLING_SECONDS).min(1.0);
        }
        if !held && t.sling_was_down {
            if can_fire && t.sling_charge >= SLING_MIN_CHARGE {
                let snap = t.sling_charge >= 1.0 && t.sling_full <= SLING_SNAP_WINDOW;
                let (aim, power) = if grounded {
                    // A ground launch always leaves the ground, however low the aim.
                    ((direction + up * (0.3 - direction.dot(up)).max(0.0)).normalize_or_zero(), 1.0)
                } else {
                    // An airborne fling goes exactly where aimed: up to recover, down to slam.
                    // Aiming upward out of a fall arrests the fall first, so the fling is a real save.
                    let along = player.vel.dot(up);
                    if direction.dot(up) > 0.0 && along < 0.0 {
                        player.vel -= up * along;
                    }
                    t.sling_air_used = true;
                    (direction, SLING_AIR_POWER)
                };
                let bonus = if snap { SLING_SNAP_BONUS } else { 1.0 };
                let speed = SLING_SPEED * t.sling_charge * bonus * power;
                player.vel += aim * speed_blocks(world.voxel_scale, speed);
                player.on_ground = false;
                player.jump_buffer = 0.0;
                player.coyote_time = 0.0;
                player.movement.raise(if snap { movement::EVENT_SLING_SNAP } else { movement::EVENT_SLING });
                if direction.dot(up) > 0.0 {
                    // Rising out of a fall: the next landing measures only the new descent.
                    player.movement.fall_speed = 0.0;
                }
                t.sling_cooldown = 0.35;
            }
            t.sling_charge = 0.0;
            t.sling_full = 0.0;
        }
        if !can_fire {
            t.sling_charge = 0.0;
            t.sling_full = 0.0;
        }
        t.sling_was_down = held;
    }
    let paid = burn_jet(player, input, world, dt);
    if paid <= 0.0 {
        return;
    }
    let m = world.voxel_scale.meters();
    let target = if input.sneak {
        -2.5
    } else if input.jump {
        if player.traversal.jet == 2 { 9.0 } else { 7.0 }
    } else {
        0.0
    };
    let vy = player.vel.dot(up) * m;
    player.vel += up * ((target - vy).clamp(-50.0 * paid, 50.0 * paid) / m);
    // Damage tracks the speed actually arrested, not the pre-ignition peak.
    player.movement.fall_speed = (-player.vel.dot(up) * m).max(0.0);
}

/// Also runs on the host for client-owned bodies: prediction cannot refill fuel.
pub fn burn_jet(player: &mut Player, input: &Input, world: &World, dt: f32) -> f32 {
    let t = &mut player.traversal;
    if t.jet == 0 || player.flying || player.swimming || world.adventure.is_dead()
        || !(input.jump || t.hover && !player.on_ground) {
        return 0.0;
    }
    let paid = if world.adventure.creative { dt } else if t.jet == 1 {
        let burn = dt.min(t.fuel_seconds);
        t.fuel_seconds = (t.fuel_seconds - burn).max(0.0);
        burn
    } else {
        let burn = dt.min(t.charge_fe / POWER_PER_SECOND);
        t.charge_fe = (t.charge_fe - burn * POWER_PER_SECOND).max(0.0);
        burn
    };
    t.thrust = paid > 0.0;
    paid
}

/// Slime soles return a landing as a bounce. Sneak settles it; jump pressed
/// just before touchdown pumps it higher, so timed hops climb.
pub fn bounce(player: &mut Player, input: &Input, world: &World) {
    if !player.traversal.slime_boots || input.sneak || player.swimming {
        return;
    }
    let speed = player.movement.fall_speed;
    let pumped = player.jump_buffer > 0.0;
    if speed < BOUNCE_MIN && !pumped {
        return;
    }
    let rebound = if pumped {
        player.jump_buffer = 0.0;
        (speed * PUMP_KEEP + PUMP_KICK).min(PUMP_CAP)
    } else {
        (speed * BOUNCE_KEEP).min(BOUNCE_CAP)
    };
    let up = player.up_dir();
    let rebound = speed_blocks(world.voxel_scale, rebound);
    let current = player.vel.dot(up);
    if rebound > current {
        player.vel += up * (rebound - current);
    }
    player.on_ground = false;
    player.coyote_time = 0.0;
    player.movement.raise(movement::EVENT_BOUNCE);
}

/// Slime soles rebound off a wall struck at speed in the air. `intended` is the
/// horizontal step the sweep was asked for and `moved` what it allowed.
pub fn wall_bounce(player: &mut Player, input: &Input, world: &World, intended: Vec3, moved: Vec3) {
    if !player.traversal.slime_boots || input.sneak || player.swimming || player.flying
        || player.on_ground || player.movement.wallrunning {
        return;
    }
    let m = world.voxel_scale.meters();
    let mut bounced = false;
    for axis in [0, 2] {
        let step = intended[axis];
        // A blocked axis kept less than half of its step.
        if step.abs() < 1e-6 || moved[axis] / step >= 0.5 {
            continue;
        }
        if player.vel[axis].abs() * m < WALL_BOUNCE_MIN || player.vel[axis].signum() != step.signum() {
            continue;
        }
        player.vel[axis] = -player.vel[axis] * WALL_BOUNCE_KEEP;
        bounced = true;
    }
    if bounced {
        player.vel.y = player.vel.y.max(0.0) + speed_blocks(world.voxel_scale, WALL_BOUNCE_LIFT);
        // The wall broke the fall; the next landing measures only what follows.
        player.movement.fall_speed = 0.0;
        player.movement.raise(movement::EVENT_BOUNCE);
    }
}
