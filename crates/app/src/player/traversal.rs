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
pub const SLING_SECONDS: f32 = 1.0;
pub const SLING_SPEED: f32 = 22.0;
pub const BOUNCE_CAP: f32 = 20.0;

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
    if t.sling {
        let held = input.place_block;
        if held && grounded && t.sling_cooldown <= 0.0 {
            t.sling_charge = (t.sling_charge + dt / SLING_SECONDS).min(1.0);
        }
        if !held && t.sling_was_down {
            if grounded && t.sling_charge >= 0.15 {
                let aim = (direction + up * (0.3 - direction.dot(up)).max(0.0)).normalize_or_zero();
                let impulse = aim * speed_blocks(world.voxel_scale, SLING_SPEED * t.sling_charge);
                player.vel += impulse;
                player.on_ground = false;
                player.jump_buffer = 0.0;
                player.coyote_time = 0.0;
                player.movement.raise(movement::EVENT_AIR_JUMP);
                t.sling_cooldown = 0.35;
            }
            t.sling_charge = 0.0;
        }
        if !grounded {
            t.sling_charge = 0.0;
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

pub fn bounce(player: &mut Player, input: &Input, world: &World) {
    if !player.traversal.slime_boots || input.sneak || player.swimming {
        return;
    }
    let speed = player.movement.fall_speed;
    if speed < 4.0 {
        return;
    }
    let up = player.up_dir();
    player.vel +=
        up * (speed_blocks(world.voxel_scale, (speed * 0.82).min(BOUNCE_CAP)) - player.vel.dot(up));
    player.on_ground = false;
    player.movement.raise(movement::EVENT_AIR_JUMP);
}
