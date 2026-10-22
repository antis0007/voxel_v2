//! One physical receiver and one finite commissioning shipment per world.
//! Paid cargo is held here until an atomic inventory claim; neither a missed
//! window nor dismantling/replacing the console can reset the contract.
use super::{Channel, DayNightCycle, Phase, Site, calendar, service, site};
use crate::{
    player::Player,
    world::{
        BlockId, World,
        inventory::{Item, ItemStack},
    },
};
use glam::{IVec2, IVec3, Vec3};

const BUILD_COST: &[(u8, u32)] = &[(8, 8), (6, 4)];
const ORDER_COST: &[(u8, u32)] = &[(6, 4), (5, 1)];
const CARGO: &[(u8, u32)] = &[(15, 8), (12, 4)];
const FLIGHT_DAYS: f64 = 0.012;

#[derive(Clone, Copy)]
pub enum Action {
    Deploy,
    Order,
    Collect,
    Restore,
}

#[derive(Default)]
pub struct State {
    pub anchor: Option<IVec3>,
    /// 0: uncommissioned; 1: paid shipment; 2: claimed permanently.
    pub stage: u32,
    pub due_day: u32,
    pub due_time: f32,
}

impl State {
    pub fn due(&self) -> f64 {
        self.due_day as f64 + self.due_time as f64
    }
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let mut push = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        push(u32::from(self.anchor.is_some()));
        if let Some(p) = self.anchor {
            for n in p.to_array() {
                push(n as u32);
            }
            push(self.stage);
            push(self.due_day);
            push(self.due_time.to_bits());
        }
    }
    pub fn decode(read: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Self> {
        let present = read()?;
        anyhow::ensure!(present <= 1, "Invalid receiver flag");
        if present == 0 {
            return Ok(Self::default());
        }
        let p = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
        let stage = read()?;
        let due_day = read()?;
        let due_time = f32::from_bits(read()?);
        anyhow::ensure!(
            p.x.unsigned_abs() <= 1_024_000
                && p.z.unsigned_abs() <= 1_024_000
                && p.y.unsigned_abs() <= 65536
                && stage <= 2
                && due_time.is_finite()
                && (0.0..1.0).contains(&due_time),
            "Invalid receiver ledger"
        );
        Ok(Self {
            anchor: Some(p),
            stage,
            due_day,
            due_time,
        })
    }
}

#[derive(Clone, Default)]
pub struct View {
    pub position: Option<Vec3>,
    pub status: &'static str,
    pub minutes: u32,
    pub quote_minutes: u32,
    pub can_deploy: bool,
    pub can_order: bool,
    pub can_collect: bool,
    pub can_restore: bool,
}

fn affordable(world: &World, cost: &[(u8, u32)]) -> bool {
    cost.iter()
        .all(|&(i, n)| world.inventory.count_item(Item::Resource(i)) >= n)
}
fn pay(world: &mut World, cost: &[(u8, u32)]) {
    for &(i, n) in cost {
        let removed = world.inventory.remove_item(Item::Resource(i), n);
        debug_assert!(removed);
    }
    world.sync_from_inventory();
}
pub fn position(world: &World) -> Option<Vec3> {
    world
        .orbital
        .receiver
        .anchor
        .map(|a| (a.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters())
}
pub fn intact(world: &World) -> bool {
    world
        .orbital
        .receiver
        .anchor
        .is_some_and(|a| world.get(a) == BlockId::Workbench && world.get(a - IVec3::Y).is_solid())
}
fn near(world: &World, player: &Player) -> bool {
    position(world).is_some_and(|p| {
        (player.eye(world.voxel_scale) * world.voxel_scale.meters() - p).length() <= 6.0
    })
}
pub fn focused(world: &World, player: &Player) -> bool {
    if !near(world, player) {
        return false;
    }
    let Some(a) = world.orbital.receiver.anchor else {
        return false;
    };
    intact(world)
        && crate::world::raycast_block(
            world,
            player.eye(world.voxel_scale),
            player.forward_dir(),
            6.0 / world.voxel_scale.meters(),
        )
        .is_some_and(|(p, _)| {
            // The one-block-high footing shields the inset console from a
            // grounded player's ray. Treat its actual registered stone surface
            // as the same station; do not interact through replacement blocks.
            let r = world.sample_config().blocks_for_height(1.0);
            p == a || (p.y == a.y && (p.x - a.x).abs() <= r && (p.z - a.z).abs() <= r
                && world.get(p) == BlockId::StoneBricks)
        })
}

/// Deployment never flattens terrain or overwrites another owner's blocks.
/// Player chooses a clear, dry, supported patch within the discovery corridor.
fn deployment(world: &World, player: &Player) -> Result<IVec3, &'static str> {
    if world.orbital.receiver.anchor.is_some() {
        return Err("A receiver is already registered. Its contract ledger survives dismantling.");
    }
    let s = site(world.seed, IVec2::ZERO).unwrap();
    let scale = world.voxel_scale.meters();
    let origin = player.pos * scale;
    if Vec3::new(origin.x - s.x, 0.0, origin.z - s.z).length() > 96.0 {
        return Err(
            "Receiver alignment requires a site within 96 m of corridor (1100, -900). Prepare a dry, flat patch there.",
        );
    }
    let forward = player.flat_forward();
    let probe = player.pos + forward * (4.0 / scale) + Vec3::Y * (3.0 / scale);
    let Some((ground, _)) = crate::world::raycast_block(world, probe, Vec3::NEG_Y, 8.0 / scale)
    else {
        return Err("No supported receiver site four metres ahead.");
    };
    let a = ground + IVec3::Y;
    let r = world.sample_config().blocks_for_height(1.0);
    let h = world.sample_config().blocks_for_height(3.0);
    if a.y + h >= world.sample_config().max_world_y() {
        return Err("Receiver exceeds the world height limit.");
    }
    for z in -r..=r {
        for x in -r..=r {
            let p = a + IVec3::new(x, 0, z);
            if world
                .factory
                .nodes
                .contains_key(&crate::world::factory::anchor(
                    p,
                    crate::world::factory::step(world),
                ))
            {
                return Err("Receiver footprint overlaps protected factory machinery.");
            }
            if !world.get(p - IVec3::Y).is_solid() {
                return Err("Prepare a flat, solid receiver foundation four metres ahead.");
            }
            for y in 0..h {
                if !world.get(p + IVec3::Y * y).is_empty() {
                    return Err(
                        "Receiver footprint must be dry and clear, with three metres of headroom.",
                    );
                }
            }
        }
    }
    Ok(a)
}

pub fn view(world: &World, day: &DayNightCycle, player: &Player) -> View {
    let state = &world.orbital.receiver;
    let local = near(world, player);
    let live = local && intact(world);
    View {
        position: position(world),
        status: if state.anchor.is_none() {
            "No ground receiver registered"
        } else if state.stage == 2 {
            "Commissioning delivery completed · ledger closed"
        } else if local && !live {
            "Console unavailable · cargo entitlement retained"
        } else if state.stage == 1 && calendar(day) >= state.due() {
            "Cargo secured · ready for collection"
        } else if state.stage == 1 {
            "Shipment paid · awaiting orbital handoff"
        } else {
            "Receiver ready · commissioning contract available"
        },
        minutes: if state.stage == 1 {
            ((state.due() - calendar(day)).max(0.0) * 1440.0).ceil() as u32
        } else {
            0
        },
        quote_minutes: if live && state.stage == 0 {
            ((forecast(world, day, player).0 - calendar(day)).max(0.0) * 1440.0).ceil() as u32
        } else {
            0
        },
        can_deploy: state.anchor.is_none()
            && affordable(world, BUILD_COST)
            && deployment(world, player).is_ok(),
        can_order: live && state.stage == 0 && affordable(world, ORDER_COST),
        can_collect: live && state.stage == 1 && calendar(day) >= state.due(),
        can_restore: local
            && state
                .anchor
                .is_some_and(|a| world.get(a).is_empty() && world.get(a - IVec3::Y).is_solid())
            && affordable(world, &[(8, 2)]),
    }
}

/// Choose an existing hold or reserve a free descent. An occupied booking at
/// another corridor is preserved; in that case use this corridor's next service.
fn forecast(world: &World, day: &DayNightCycle, player: &Player) -> (f64, bool) {
    let site = site(world.seed, IVec2::ZERO).unwrap();
    let s = service(site, day, &world.orbital);
    if s.phase == Phase::Departed
        && super::contacts(
            world.seed,
            day,
            &world.orbital,
            player.pos * world.voxel_scale.meters(),
        )
        .iter()
        .any(|c| c.service.site.cell == IVec2::ZERO && c.can_request)
    {
        return (
            calendar(day) + super::NOTICE_DAYS + super::SERVICE_DAYS * 0.5,
            true,
        );
    }
    if s.handoff_day >= calendar(day) + FLIGHT_DAYS {
        return (s.handoff_day, false);
    }
    // The current midpoint has passed. The next regular pass is independent of
    // player occupancy and remains reconstructible while the region is unloaded.
    let offset = super::unit(site.key ^ 4) as f64 * 3.0;
    let start = ((calendar(day) - offset) / 3.0).floor() * 3.0 + offset + 3.0;
    (start + super::SERVICE_DAYS * 0.5, false)
}

pub fn action(
    world: &mut World,
    day: &DayNightCycle,
    player: &Player,
    action: Action,
) -> Vec<IVec3> {
    let mut changed = Vec::new();
    let message = match action {
        Action::Deploy => {
            match deployment(world, player) {
                Err(reason) => reason,
                Ok(_) if !affordable(world, BUILD_COST) => {
                    "Receiver requires 8 iron ingots and 4 copper ingots."
                }
                Ok(a) => {
                    let r = world.sample_config().blocks_for_height(1.0);
                    let mut edits = Vec::new();
                    for z in -r..=r {
                        for x in -r..=r {
                            let p = a + IVec3::new(x, 0, z);
                            edits.push((
                                p,
                                if p == a {
                                    BlockId::Workbench
                                } else {
                                    BlockId::StoneBricks
                                },
                            ));
                        }
                    }
                    let chunks: std::collections::HashSet<_> = edits
                        .iter()
                        .map(|&(p, _)| crate::world::chunk_coord_from_world_block(p))
                        .collect();
                    if chunks.into_iter().any(|c| world.resident_edit(c).is_err()) {
                        world
                            .expedition
                            .say("Receiver terrain is unavailable. No materials were charged.");
                        return changed;
                    }
                    world.set_many(&edits);
                    // set_many can refuse protected machine cells. Only charge
                    // and register if every validated cell was actually placed.
                    if edits.iter().all(|&(p, id)| world.get(p) == id) {
                        pay(world, BUILD_COST);
                        world.orbital.receiver.anchor = Some(a);
                        changed.extend(edits.iter().map(|&(p, _)| p));
                        "Meridian receiver registered. Aim at its console and press F to open the service ledger."
                    } else {
                        "Receiver installation deferred by protected machinery. Clear the footprint first."
                    }
                }
            }
        }
        Action::Order
            if near(world, player)
                && intact(world)
                && world.orbital.receiver.stage == 0
                && affordable(world, ORDER_COST) =>
        {
            let (due, book) = forecast(world, day, player);
            if book {
                world.orbital.request(
                    world.seed,
                    day,
                    player.pos * world.voxel_scale.meters(),
                    IVec2::ZERO,
                    Channel::Radio,
                );
            }
            pay(world, ORDER_COST);
            let state = &mut world.orbital.receiver;
            state.stage = 1;
            state.due_day = due.floor() as u32;
            state.due_time = due.fract() as f32;
            "Contract ML-C01 accepted: 4 copper ingots and 1 crystal paid. A sealed capsule will deliver 8 steel ingots and 4 ancient circuits."
        }
        Action::Collect
            if near(world, player)
                && intact(world)
                && world.orbital.receiver.stage == 1
                && calendar(day) >= world.orbital.receiver.due() =>
        {
            let mut inventory = world.inventory.clone();
            let fits = CARGO
                .iter()
                .all(|&(i, n)| inventory.try_insert(ItemStack::new(Item::Resource(i), n)).1 == 0);
            if fits {
                world.inventory = inventory;
                world.orbital.receiver.stage = 2;
                world.sync_from_inventory();
                "ML-C01 completed: 8 steel ingots and 4 ancient circuits collected. This commissioning grant is now closed."
            } else {
                "Make room for the complete shipment. Your sealed cargo will remain in the receiver."
            }
        }
        Action::Restore if view(world, day, player).can_restore => {
            let a = world.orbital.receiver.anchor.unwrap();
            world.set(a, BlockId::Workbench);
            if world.get(a) == BlockId::Workbench {
                pay(world, &[(8, 2)]);
                changed.push(a);
                "Receiver console restored. Its original contract ledger is intact."
            } else {
                "Console placement blocked by protected machinery."
            }
        }
        _ => {
            "Use the intact receiver within six metres; check materials, shipment status and the closed contract ledger."
        }
    };
    world.expedition.say(message);
    changed
}

/// Presentation only: persistent entitlement is the receiver ledger, not the
/// temporary mesh. Missing a descent never loses already paid cargo.
pub fn capsule(world: &World, day: &DayNightCycle) -> Option<(Vec3, Vec3, f32)> {
    let state = &world.orbital.receiver;
    if state.stage != 1 {
        return None;
    }
    let pad = position(world)? + Vec3::Y * 1.1;
    let progress =
        ((calendar(day) - state.due() + FLIGHT_DAYS) / FLIGHT_DAYS).clamp(0.0, 1.0) as f32;
    if progress <= 0.0 {
        return None;
    }
    let s: Site = site(world.seed, IVec2::ZERO)?;
    let upper = Vec3::new(
        s.x,
        super::hold_height(&world.sample_config(), &world.orbital, s),
        s.z,
    );
    Some((upper, pad, progress))
}
