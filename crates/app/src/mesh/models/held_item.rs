//! The selected hotbar item held low on the right: blocks as textured cubes,
//! other items as their hotbar sprite extruded to suit their shape.
//! Texels come from `egui_ui::held_art`, so the hand always matches the slot.
use super::{Model, Vertex3};
use crate::egui_ui::held_art::{self, Art, FACE};
use crate::egui_ui::icons::Icon;
use crate::world::inventory::Item;
use crate::world::{BlockId, World};
use glam::{Mat3, Quat, Vec3};

mod geometry;
mod review;
#[cfg(test)]
mod tests;

use geometry::{Out, cube, pan, sprite, torch};
use review::REVIEW;
pub(crate) use review::{pinned, review_bank};

/// Viewmodel material: dims with `ao_strength` under a roof like the hand and wand.
const KIND: f32 = 55.0;
const GLOW_KIND: f32 = 40.0;
const CUBE_EDGE: f32 = 0.125;
/// Held block turn in radians: modest yaw, pitch and roll so the hand carries it rather than a gizmo.
const BLOCK_YAW: f32 = -0.28;
const BLOCK_PITCH: f32 = 0.12;
const BLOCK_ROLL: f32 = -0.15;
/// Camera-space nudge of the cube toward the hand: right, up, forward (metres).
const BLOCK_SEAT: Vec3 = Vec3::new(0.035, -0.03, 0.02);
const SPRITE_TEXEL: f32 = 0.016;
/// Placed standing torch, scaled into the hand. The fist closes on the upper leather wrap.
const HELD_TORCH_SCALE: f32 = 0.42;
const HELD_GRIP_Y: f32 = 0.43;
/// Flame centre height above the fist along the torch, in metres.
const TORCH_FLAME: f32 = (0.80 - HELD_GRIP_Y) * HELD_TORCH_SCALE;
/// Hold point in camera space: right, up, forward (metres).
const REST: Vec3 = Vec3::new(0.34, -0.22, 0.62);
/// Empty hand: a loose fist low on the right, knuckles forward, so it does not read as gripping an unseen shaft (captured 2026-11-04).
const BARE_SEAT: Vec3 = Vec3::new(0.06, -0.14, 0.0);
const BARE_PITCH: f32 = -0.4;
const BARE_ROLL: f32 = 0.5;
/// Multiple of the rig's "open" finger turn applied to the empty hand.
pub(super) const BARE_RELAX: f32 = 2.0;
/// Slight finger opening for the torch shaft.
pub(super) const TORCH_RELAX: f32 = 0.25;
/// Camera-space nudge that seats the torch shaft in the rig's closed fist (captured 2026-10-30).
const TORCH_SEAT: Vec3 = Vec3::new(0.03, 0.012, 0.0);
/// Shift the shaft across the fist without moving the hand's grip socket.
const TORCH_GRIP_SIDE: f32 = 0.012;

pub struct View {
    pub eye: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub time: f32,
    /// Camera-lag offset in metres (right, up), from the viewmodel spring.
    pub sway: glam::Vec2,
    /// Walk cycle phase in radians and ground speed in m/s.
    pub gait: f32,
    pub speed: f32,
    /// Clip pose about the hold point, from `viewmodel::State::pose`.
    pub clip: crate::player::viewmodel::clips::Pose,
    /// 0 held, 1 fully below the screen (while a wand is drawn).
    pub lowered: f32,
    pub torch_lit: bool,
}


/// Flight does not change inventory ownership or the selected item.
pub(crate) fn held(world: &World, _player: &crate::player::Player) -> Option<Item> {
    if let Some(item) = *REVIEW {
        return Some(item);
    }
    if *review::EMPTY {
        return None;
    }
    world.inventory.hotbar.get(world.adventure.hand_slot).copied().flatten().map(|stack| stack.item)
}

/// Height above the planet surface past which the hands go away: a torch or
/// block held out over open sea, a mile up or in orbit, reads as a bug.
pub(crate) const STOWED_ABOVE_M: f64 = 500.0;

/// Whether the held items and hands are stowed because the player is far above a planet's surface.
pub(crate) fn stowed(world: &World, player: &crate::player::Player) -> bool {
    if pinned() { return false; }
    let eye_m = player.precise_eye(world.voxel_scale) * f64::from(world.voxel_scale.meters());
    crate::altitude::planet_world(world)
        && crate::altitude::altitude_m(eye_m, true) > STOWED_ABOVE_M
}

pub(crate) fn offhand_torch(world: &World) -> bool {
    world.inventory.offhand.is_some_and(|s| s.item.placeable_block().is_some_and(|b| b.is_torch()))
}

/// Whether either hand holds a torch, for the moving torch light.
pub(crate) fn main_hand_torch(world: &World, player: &crate::player::Player) -> bool {
    (world.adventure.wand == 0 || pinned())
        && held(world, player).is_some_and(|item| item.placeable_block().is_some_and(|b| b.is_torch()))
}

/// Viewmodel sway spring rate: low for heavy blocks, soft for torches, quick for legacy blades.
pub(crate) fn sway_rate(item: Option<Item>) -> f32 {
    let Some(item) = item else { return 16.0 };
    // Not `is_blade`: that reads the render-side tool bank, which the fixed-step tick must not depend on.
    if matches!(item, Item::Weapon(_)) { return 18.0 }
    match held_art::art(item) {
        Art::Block(_) => 10.0,
        Art::Icon(crate::egui_ui::icons::Icon::Torch) => 12.0,
        _ => 16.0,
    }
}

/// Torch tilt in the grip frame, shared by the drawn torch and its light.
fn torch_tilt() -> Quat {
    Quat::from_rotation_y(-0.55) * Quat::from_rotation_z(0.12)
}

fn torch_frame(v: &View) -> (Vec3, Quat) {
    let (centre, camera) = frame(v);
    let rotation = camera * torch_tilt();
    (centre + camera * TORCH_SEAT + rotation * Vec3::new(0.0, -0.085, 0.0), rotation)
}

/// Where the flame starts on a held torch, in the viewmodel's camera-relative space.
pub(super) fn torch_base(v: &View) -> Vec3 {
    let (centre, rotation) = torch_frame(v);
    centre + v.right * TORCH_GRIP_SIDE + rotation * Vec3::Y * TORCH_FLAME
}

/// Camera-space flame position of a held torch (right, up, forward), in metres.
pub(crate) fn torch_flame_offset() -> Vec3 {
    let local = torch_tilt() * Vec3::new(0.0, TORCH_FLAME - 0.085, 0.0);
    REST + TORCH_SEAT + Vec3::new(local.x + TORCH_GRIP_SIDE, local.y, -local.z)
}

pub(crate) fn torch_position(world: &World, player: &crate::player::Player, view: &View, left: bool) -> glam::DVec3 {
    let offset = torch_base(view) - view.eye;
    let offset = if left { offset - 2.0 * offset.dot(view.right) * view.right } else { offset };
    player.precise_eye(world.voxel_scale) * f64::from(world.voxel_scale.meters()) + offset.as_dvec3()
}

fn frame(v: &View) -> (Vec3, Quat) {
    let walk = (v.speed / 4.0).clamp(0.0, 1.0) * crate::comfort::camera_motion();
    // One smooth dip per step at mid-sway, in phase with the wand (`wand_pose::offset`).
    let bob = Vec3::new(v.gait.sin() * 0.007, -v.gait.cos().powi(2) * 0.011, 0.0) * walk;
    let local = REST + v.sway.extend(0.0) + bob + v.clip.offset - Vec3::Y * v.lowered.clamp(0.0, 1.0) * 0.35;
    let centre = v.eye + v.right * local.x + v.up * local.y + v.forward * local.z;
    let camera = Quat::from_mat3(&Mat3::from_cols(v.right, v.up, -v.forward));
    (centre, camera * v.clip.rotation)
}

/// A ready blade leans its tip forward, away from the eye.
fn blade_rest() -> Quat { Quat::from_rotation_x(-0.5) * Quat::from_rotation_y(-0.2) * Quat::from_rotation_z(0.25) }

struct SpriteGrip {
    /// Hand socket relative to sprite centre, in texels with positive Y up.
    socket: Vec3,
    /// Turns the painted shaft onto the hand's local Y axis.
    align: f32,
    pitch: f32,
    yaw: f32,
    roll: f32,
    half_depth: f32,
    shaft: bool,
    texel: f32,
}

fn long_tool_grip(icon: Icon) -> Option<SpriteGrip> {
    let (pitch, roll, half_depth, texel) = match icon {
        Icon::Pickaxe(6..=8) => (-0.34, -0.55, 0.58, 0.0185),
        Icon::Axe(_) => (-0.33, -0.53, 0.62, 0.018),
        Icon::Shovel(_) => (-0.39, -0.57, 0.48, 0.019),
        Icon::Hoe => (-0.39, -0.57, 0.48, 0.0185),
        Icon::BuiltTool(schema, ..) => {
            match engine_tools::PARTS.schemas.get(schema as usize)?.id.as_str() {
                "pickaxe" => (-0.34, -0.55, 0.58, 0.0185),
                "axe" => (-0.33, -0.53, 0.62, 0.018),
                "shovel" | "excavator" => (-0.39, -0.57, 0.56, 0.019),
                "hammer" => (-0.32, -0.49, 0.75, 0.0185),
                "scythe" => (-0.41, -0.53, 0.48, 0.0195),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(SpriteGrip {
        socket: Vec3::new(-3.5, -3.5, 0.0),
        align: std::f32::consts::FRAC_PI_4,
        pitch, yaw: -0.26, roll, half_depth,
        shaft: true, texel,
    })
}

fn sprite_grip(icon: Icon) -> SpriteGrip {
    use std::f32::consts::FRAC_PI_4;

    if let Some(grip) = long_tool_grip(icon) {
        return grip;
    }
    let (x, y, align, pitch, yaw, roll, half_depth) = match icon {
        Icon::BuiltTool(schema, ..) => {
            match engine_tools::PARTS.schemas.get(schema as usize).map(|def| def.id.as_str()) {
                Some("shortbow" | "longbow") => (5.5, 0.0, 0.0, -0.08, 0.48, 0.12, 0.65),
                Some("shield") => (0.0, 0.0, 0.0, 0.10, 0.28, 0.08, 0.9),
                _ => (-4.0, -4.0, FRAC_PI_4, -0.26, -0.38, -0.16, 0.7),
            }
        }
        Icon::Rod => (-4.0, -3.0, FRAC_PI_4, -0.62, -0.30, -0.09, 0.55),
        Icon::Bore => (-2.0, -3.5, 0.0, -0.05, 1.05, -0.06, 1.1),
        Icon::Shears => (-4.0, -4.5, 0.32, -0.34, -0.30, -0.35, 0.54),
        Icon::Sling => (0.0, -2.5, 0.0, -0.32, -0.25, 0.12, 0.8),
        Icon::Bucket(_) => (0.0, 5.5, 0.0, 0.03, -0.33, 0.04, 0.85),
        Icon::ClaimStake => (-3.5, -2.5, 0.61, -0.30, -0.35, -0.12, 0.75),
        Icon::Map | Icon::Clock => (2.5, -2.5, 0.0, -0.32, -0.22, 0.12, 0.4),
        _ => (0.0, -2.5, 0.0, -0.12, -0.38, -0.16, 0.5),
    };
    let texel = if matches!(icon, Icon::Shears) { 0.0135 } else { SPRITE_TEXEL };
    SpriteGrip { socket: Vec3::new(x, y, 0.0), align, pitch, yaw, roll, half_depth, shaft: false, texel }
}

fn sprite_wrist(camera: Quat, grip: &SpriteGrip) -> Quat {
    camera * Quat::from_rotation_x(grip.pitch)
        * Quat::from_rotation_y(grip.yaw)
        * Quat::from_rotation_z(grip.roll)
}

fn pan_placement(centre: Vec3, camera: Quat) -> (Vec3, Quat) {
    (centre, camera * Quat::from_rotation_x(0.28) * Quat::from_rotation_z(0.20))
}

/// What the hand shows on a melee swing: the held blade, else an owned legacy blade, else the held item.
pub(super) fn swung(world: &World, held: Option<Item>) -> Option<Item> {
    if held.is_some_and(is_blade) { return held; }
    let tier = world.adventure.weapon;
    (1..=5).contains(&tier).then_some(Item::Weapon(tier as u8))
        .or(held.filter(|&item| !matches!(held_art::art(item), Art::Wand(_))))
}

pub(super) fn bare_placement(v: &View) -> (Vec3, Quat) {
    let (centre, camera) = frame(v);
    (
        centre + camera * BARE_SEAT,
        camera * Quat::from_rotation_x(BARE_PITCH) * Quat::from_rotation_z(BARE_ROLL),
    )
}


/// Schema and part materials when the item is an assembled blade.
fn assembled_sword(item: Item) -> Option<super::sword_art::Parts> {
    let Item::Assembled { id, .. } = item else { return None };
    super::sword_art::assembled_parts(&held_art::tool(id)?)
}

/// Whether the item swings as a sword: a legacy blade or an assembled sword.
pub(super) fn is_blade(item: Item) -> bool { matches!(item, Item::Weapon(_)) || assembled_sword(item).is_some() }

/// Draws a blade item with its grip at `frame`'s origin.
pub(super) fn draw_blade(m: &mut Model, item: Item, frame: glam::Mat4) {
    match (item, assembled_sword(item)) {
        (_, Some(parts)) => super::sword_art::draw_assembled(m, frame, &parts),
        (Item::Weapon(tier), _) => super::sword_art::draw(m, frame, tier),
        _ => {}
    }
}

/// Appends the held item and returns the matching hand pose.
/// Wands draw through `wand::draw`.
pub(super) fn draw(m: &mut Model, item: Item, v: &View) -> (Vec3, Quat) {
    let (centre, camera) = frame(v);
    if is_blade(item) {
        let rotation = camera * blade_rest();
        draw_blade(m, item, glam::Mat4::from_rotation_translation(rotation, centre));
        return (centre, rotation);
    }
    match held_art::art(item) {
        Art::Wand(_) => {
            let rotation = camera * Quat::from_rotation_y(-0.55) * Quat::from_rotation_z(0.25);
            (centre + rotation * Vec3::new(0.0, -0.085, 0.0), rotation)
        }
        Art::Block(block) => {
            // Turned and tipped so the top and two sides read, as a held cube does.
            let wrist = camera * Quat::from_rotation_z(BLOCK_ROLL);
            let rotation = wrist * Quat::from_rotation_x(BLOCK_PITCH) * Quat::from_rotation_y(BLOCK_YAW);
            let centre = centre + camera * BLOCK_SEAT;
            cube(&mut Out { m, centre, rotation }, block);
            let contact = Vec3::new(CUBE_EDGE * 0.5 + 0.012, -CUBE_EDGE * 0.34, 0.0);
            (centre + rotation * contact, wrist)
        }
        Art::Icon(Icon::Torch) => {
            let (centre, rotation) = torch_frame(v);
            torch(&mut Out { m, centre: centre + v.right * TORCH_GRIP_SIDE, rotation }, v.torch_lit);
            (centre, rotation)
        }
        Art::Icon(Icon::Pan) => {
            let (centre, rotation) = pan_placement(centre, camera);
            pan(&mut Out { m, centre, rotation });
            (centre, rotation)
        }
        Art::Icon(icon) => {
            let grip = sprite_grip(icon);
            let rotation = sprite_wrist(camera, &grip);
            sprite(&mut Out { m, centre, rotation }, icon, &grip);
            (centre, rotation)
        }
    }
}
