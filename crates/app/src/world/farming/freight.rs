//! The Fernwake orchard sends part of its harvest across the editable bridge.
//! Source, in-transit and market stock share a save record.
use super::{World, known, resident};
use crate::world::BlockId;
use engine_worldgen::{WorldSampleConfig, WorldType, fernwake};
use glam::{IVec3, Vec3};

pub(crate) const SELLER: u32 = 7;
pub(super) const MARKET_CAPACITY: u32 = 12;
pub(super) const LOAD: u32 = 4;
pub(super) const TRIP_STEPS: u8 = 30;
pub(super) const LOCAL_RESERVE: u32 = 1;
const TICKS_PER_STEP: u64 = (1.0 / engine_core::time::PLAYER_STEP_SECONDS + 0.5) as u64;
const BRIDGE_START: f32 = 490.0;
const BRIDGE_END: f32 = 533.0;

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    pub(super) cargo: u32,
    pub(super) progress: u8,
    pub(crate) market: u32,
    pub(super) last_crossing: Option<Route>,
    pub(super) cargo_lane: Option<Route>,
    route: Option<Route>,
}

impl State {
    pub(crate) fn free_space(&self) -> u32 {
        MARKET_CAPACITY - self.market - self.cargo
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Route {
    North = 1,
    South = 2,
    Blocked = 3,
    Unobserved,
}

impl Route {
    fn lane(self) -> Option<f32> {
        match self {
            Self::North => Some(47.0),
            Self::South => Some(49.0),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::North => "North Lane",
            Self::South => "South Lane",
            Self::Blocked => "Bridge Blocked",
            Self::Unobserved => "Bridge Unobserved",
        }
    }
}

pub(crate) fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Hills && fernwake::freight::active(cfg)
}

pub(crate) fn bench_cell(cfg: &WorldSampleConfig) -> IVec3 {
    let p = fernwake::freight::point(cfg, fernwake::freight::BENCH);
    (p / cfg.voxel_meters()).floor().as_ivec3()
}

/// Inspect both walking lanes at the actual bridge deck. Edited chunks that
/// are not resident are unknown, so they can never clear a route for delivery.
fn deck_y(cfg: &WorldSampleConfig) -> i32 {
    let scale = cfg.voxel_meters();
    let datum = fernwake::freight::point(cfg, Vec3::ZERO).y;
    (datum / scale - 0.5).ceil() as i32 - 1
}

fn crossing(world: &World, cfg: &WorldSampleConfig, deck: i32) -> Route {
    let scale = cfg.voxel_meters();
    let headroom = (1.8 / scale).ceil() as i32;
    let mut unknown = false;
    let mut lanes = [Route::North, Route::South];
    if world.farming.freight.cargo_lane == Some(Route::South) {
        lanes.swap(0, 1);
    }
    for lane in lanes {
        let Some(z) = lane.lane() else { continue };
        let z = (z / scale).floor() as i32;
        let mut clear = true;
        let mut lane_unknown = false;
        'lane: for x in (BRIDGE_START / scale) as i32..(BRIDGE_END / scale) as i32 {
            for dy in 0..=headroom {
                match known(world, IVec3::new(x, deck + dy, z)) {
                    None => lane_unknown = true,
                    Some(block) if dy == 0 && block.is_solid() => {}
                    Some(block) if dy > 0 && block.is_empty() => {}
                    Some(_) => {
                        clear = false;
                        break 'lane;
                    }
                }
            }
        }
        if clear && !lane_unknown {
            return lane;
        }
        unknown |= clear && lane_unknown;
    }
    if unknown {
        Route::Unobserved
    } else {
        Route::Blocked
    }
}

pub(super) fn service(world: &mut World) {
    let f = &world.farming;
    if f.clock % TICKS_PER_STEP != 0 {
        return;
    }
    let cfg = world.sample_config();
    if !active(&cfg) {
        return;
    }
    let surplus = f.nursery_stock.saturating_sub(LOCAL_RESERVE);
    let can_dispatch = surplus > 0 && f.freight.free_space() != 0;
    let deck = deck_y(&cfg);
    let scale = cfg.voxel_meters();
    let middle = IVec3::new(
        ((BRIDGE_START + BRIDGE_END) * 0.5 / scale) as i32,
        deck,
        (48.0 / scale) as i32,
    );
    let busy = f.freight.cargo != 0 || can_dispatch;
    if !busy && !resident(world, middle) {
        world.farming.freight.route = Some(Route::Unobserved);
        return;
    }
    let route = crossing(world, &cfg, deck);
    let f = &mut world.farming;
    let shipment = &mut f.freight;
    shipment.route = Some(route);
    if route != Route::Unobserved {
        shipment.last_crossing = Some(route);
    }
    if route.lane().is_none() || !busy {
        return;
    }
    shipment.cargo_lane = Some(route);
    if shipment.cargo == 0 {
        shipment.cargo = LOAD.min(surplus).min(shipment.free_space());
        f.nursery_stock -= shipment.cargo;
        return;
    }
    shipment.progress += 1;
    if shipment.progress == TRIP_STEPS {
        shipment.market += shipment.cargo;
        shipment.cargo = 0;
        shipment.progress = 0;
        shipment.cargo_lane = None;
    }
}

pub(crate) fn status(world: &World) -> String {
    let f = &world.farming.freight;
    match f.route {
        Some(Route::Blocked) => "Bridge Blocked".into(),
        Some(route @ (Route::North | Route::South)) if f.cargo > 0 => {
            format!(
                "{} · {} in Transit · {} s",
                route.label(),
                f.cargo,
                TRIP_STEPS - f.progress
            )
        }
        None | Some(Route::Unobserved) => f.last_crossing.map_or_else(
            || "Bridge Unobserved".into(),
            |route| format!("Last Report · {}", route.label()),
        ),
        _ if f.market == MARKET_CAPACITY => "Receiving Stock Full".into(),
        _ => "Awaiting Surplus".into(),
    }
}

pub(super) fn supply_issue(world: &World) -> &'static str {
    let farming = &world.farming;
    let freight = &farming.freight;
    match freight.route.or(freight.last_crossing) {
        Some(Route::Blocked) => "Bridge blocked · restore one lane's deck and clear its headroom",
        Some(Route::Unobserved) if freight.cargo > 0 || farming.nursery_stock > LOCAL_RESERVE => {
            "Bridge out of view · inspect the crossing"
        }
        _ if freight.cargo > 0 => "Glowcaps are on the freight cart",
        _ if farming.nursery_stock <= LOCAL_RESERVE => "Nursery has no surplus · check the glowcap beds and intake",
        _ => "Waiting for the next cart",
    }
}

/// Temporary work stops for the people responsible for the active shipment.
/// Recruitment and player-assigned homes take priority in the resident mover.
pub(crate) fn work_goal(world: &World, resident_id: u32) -> Option<Vec3> {
    if !matches!(resident_id, super::watershed::SELLER | SELLER) {
        return None;
    }
    let f = &world.farming.freight;
    if f.cargo == 0 || f.route.and_then(Route::lane).is_none() {
        return None;
    }
    let cfg = world.sample_config();
    if !active(&cfg) {
        return None;
    }
    match resident_id {
        super::watershed::SELLER if f.progress < 6 => {
            fernwake::nursery(&cfg).map(|p| p - Vec3::new(2.0, 0.0, 1.0))
        }
        SELLER if f.progress >= TRIP_STEPS - 8 => Some(fernwake::freight::point(
            &cfg,
            fernwake::freight::CLERK - Vec3::Z * 2.0,
        )),
        _ => None,
    }
}

#[derive(Default)]
pub(crate) struct View {
    pub cart: Option<Vec3>,
    pub cart_yaw: f32,
    pub crates: Option<(Vec3, u32)>,
}

pub(crate) fn view(world: &World, viewer: Vec3) -> View {
    let state = &world.farming.freight;
    if (state.cargo == 0 && state.market == 0)
        || !(450.0..630.0).contains(&viewer.x)
        || !(10.0..88.0).contains(&viewer.z)
    {
        return View::default();
    }
    let cfg = world.sample_config();
    if !active(&cfg) {
        return View::default();
    }
    let scale = cfg.voxel_meters();
    let bench = fernwake::freight::point(&cfg, fernwake::freight::BENCH);
    let cell = bench_cell(&cfg);
    let crates = (state.market != 0
        && bench.distance_squared(viewer) < 35.0 * 35.0
        && resident(world, cell)
        && world.get(cell) == BlockId::Planks)
        .then(|| {
            (
                Vec3::new(bench.x, (cell.y + 1) as f32 * scale, bench.z),
                state.market.div_ceil(LOAD),
            )
        });
    View {
        cart: cart_position(world, &cfg),
        cart_yaw: 0.0,
        crates,
    }
}

fn cart_position(world: &World, cfg: &WorldSampleConfig) -> Option<Vec3> {
    let state = &world.farming.freight;
    let z = state
        .cargo_lane
        .and_then(Route::lane)
        .filter(|_| state.cargo != 0)?;
    let scale = cfg.voxel_meters();
    let deck = deck_y(cfg);
    let elapsed = if state.route.and_then(Route::lane).is_some() {
        (world.farming.clock % TICKS_PER_STEP) as f32 / TICKS_PER_STEP as f32
    } else {
        0.0
    };
    let t = (f32::from(state.progress) + elapsed) / f32::from(TRIP_STEPS);
    let x = BRIDGE_START + 0.6 + (BRIDGE_END - BRIDGE_START - 1.2) * t;
    let z = ((z / scale).floor() + 0.5) * scale;
    let cell = IVec3::new((x / scale).floor() as i32, deck, (z / scale).floor() as i32);
    let headroom = (0.6 / scale).ceil() as i32;
    (resident(world, cell)
        && world.get(cell).is_solid()
        && (1..=headroom)
            .all(|dy| known(world, cell + IVec3::Y * dy).is_some_and(BlockId::is_empty)))
    .then_some(Vec3::new(x, (deck + 1) as f32 * scale, z))
}
