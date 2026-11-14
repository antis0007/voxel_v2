//! Windowless guest exercises against an ephemeral travel scene.
use super::*;
use world::{adventure::wildlife::vehicle, inventory::{Item, ItemStack}, travel};

pub(super) fn setup(world: &mut World, player: &Player) {
    let y = player.pos.y.floor() as i32;
    let mut cells = Vec::new();
    for x in 3..15 {
        for z in -7..8 {
            let edge = x == 3 || x == 14 || z == -7 || z == 7;
            cells.push((IVec3::new(x, y, z), if edge { BlockId::Dirt } else { BlockId::Water }));
        }
    }
    world.set_many(&cells);
    let scale = world.voxel_scale.meters();
    vehicle::review_scene(world,
        Vec3::new(0.5, y as f32, 3.5) * scale,
        Vec3::new(5.5, y as f32 + 0.9, 0.5) * scale);
}

pub(super) fn equip(world: &mut World) {
    world.inventory.hotbar[8] = Some(ItemStack::new(travel::ROD, 1));
    if let Some(row) = travel::trades::page(0).first() {
        for stack in &row.give {
            world.inventory.try_insert_pack_first(ItemStack::new(stack.item, stack.count));
        }
    }
    world.sync_from_inventory();
}

#[derive(Clone, Copy, Default)]
pub(super) enum Bot {
    #[default]
    Trade,
    Bought(Item, u32),
    Cast,
    Bite,
    Caught(Item, u32),
    Board(Option<Instant>),
    Steer(u32, Vec3),
    Dismount(Instant),
    Done,
}

fn aim(player: &mut Player, scale: VoxelScalePreset, target: Vec3) {
    let aim = (target / scale.meters() - player.eye(scale)).normalize_or_zero();
    player.yaw = aim.x.atan2(-aim.z);
    player.pitch = aim.y.asin().clamp(-1.5, 1.5);
}

impl Bot {
    pub(super) fn input(&mut self, session: &mut Session, world: &World, player: &mut Player) -> Input {
        let mut input = Input::default();
        if !session.ready() { return input; }
        let scale = world.voxel_scale;
        let metres = scale.meters();
        match *self {
            Self::Trade => {
                if vehicle::villager_centre(world, 0).is_some()
                    && let Some(row) = travel::trades::page(0).first()
                {
                    session.queue_resident(0, travel::ResidentAction::Trade(0));
                    *self = Self::Bought(row.get.item, world.inventory.count_item(row.get.item));
                }
            }
            Self::Bought(item, before) if world.inventory.count_item(item) > before => {
                eprintln!("[travel-review] trade confirmed");
                *self = Self::Cast;
            }
            Self::Bought(_, _) if matches!(world.expedition.message.as_str(),
                "Resident Out of Reach" | "Trade Unavailable" | "Out of Stock" | "Items Missing" | "Workshop Unavailable") =>
            {
                eprintln!("[travel-review] trade refused: {}", world.expedition.message);
                *self = Self::Done;
            }
            Self::Cast => {
                session.hand = 8;
                aim(player, scale, Vec3::new(7.5, player.pos.y.floor() + 0.5, -3.5) * metres);
                input.place_pressed = true;
                *self = Self::Bite;
            }
            Self::Bite => {
                if let Some(cast) = world.travel.cast && world.farming.clock >= cast.bite
                    && let Some(catch) = travel::fishing::catch(world.seed, cast.cell, cast.bite)
                {
                    input.place_pressed = true;
                    *self = Self::Caught(catch.item, world.inventory.count_item(catch.item));
                }
            }
            Self::Caught(item, before) if world.inventory.count_item(item) > before => {
                eprintln!("[travel-review] fishing catch confirmed");
                *self = Self::Board(None);
            }
            Self::Board(_) if world.travel.riding.is_some() => {
                if let Some(travel::Ride::Boat(id)) = world.travel.riding
                    && let Some(boat) = world.travel.boats.get(&id)
                {
                    eprintln!("[travel-review] mounted");
                    *self = Self::Steer(id, boat.pos);
                }
            }
            Self::Board(last) if last.is_none_or(|at| at.elapsed() > Duration::from_millis(500)) => {
                if let Some(boat) = world.travel.boats.values().next() {
                    aim(player, scale, boat.pos + Vec3::Y * 0.2);
                    input.interact = true;
                    *self = Self::Board(Some(Instant::now()));
                }
            }
            Self::Steer(id, start) => {
                player.yaw = std::f32::consts::FRAC_PI_2;
                if world.travel.boats.get(&id).is_some_and(|boat| start.distance(boat.pos) >= 0.7) {
                    eprintln!("[travel-review] steered");
                    *self = Self::Dismount(Instant::now());
                } else {
                    input.forward = true;
                }
            }
            Self::Dismount(started) => {
                input.sneak = true;
                if started.elapsed() > Duration::from_secs(1) && world.travel.riding.is_none() {
                    eprintln!("[travel-review] dismounted");
                    *self = Self::Done;
                }
            }
            _ => {}
        }
        input
    }
}
