use super::{World, freight, known, resident};
use crate::world::{BlockId, inventory::ItemStack};
use engine_worldgen::planet_freight::{BRIDGE, DETOUR_Z, HOLDINGS, LANES, Site, site};
use glam::{IVec3, Vec3};

pub(crate) mod review;

const STOCK_CAPACITY: u32 = 12;
const MARKET_CAPACITY: u32 = 24;
const LOAD: u32 = 4;
const SECOND: u64 = (1.0 / engine_core::time::PLAYER_STEP_SECONDS + 0.5) as u64;
const SHIP_EVERY: u64 = 8;
const HARVEST_EVERY: u64 = 20;
const DETOUR: u8 = 2;
const DRIVERS: [u32; 2] = [0x4652_0001, 0x4652_0002];
const DRIVER_NAMES: [&str; 2] = ["Nell", "Tobin"];
pub(crate) const DRIVER_HEALTH: f32 = 20.0;
pub(super) const DRIVER_PAUSE: u64 = 5 * SECOND;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct State {
    pub(super) stock: [u32; 2],
    pub(super) market: u32,
    pub(super) cargo: u32,
    pub(super) progress: u8,
    pub(super) holding: u8,
    pub(super) lane: u8,
    pub(super) wounds: [f32; 2],
    pub(super) pause_until: u64,
    crossing: Crossing,
    lanes: [Clearance; 2],
    detour: Clearance,
    wait: Wait,
    moving: bool,
    retreating: bool,
    market_open: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Crossing {
    Open(u8),
    Blocked,
    #[default]
    Unobserved,
}

#[derive(Clone, Copy, Debug, Default)]
enum Wait {
    #[default]
    None,
    Crossing,
    Road(Clearance),
    Counter,
    Driver,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Clearance {
    Open,
    Blocked,
    #[default]
    Unobserved,
}

fn fork(holding: u8) -> u8 {
    if holding == 0 { 6 } else { 12 }
}

fn duration(holding: u8, lane: u8) -> u8 {
    fork(holding) + if lane == DETOUR { 22 } else { 14 }
}

pub(super) fn valid(state: &State) -> bool {
    state.stock.iter().all(|&n| n <= STOCK_CAPACITY)
        && state.cargo <= LOAD
        && state.market <= MARKET_CAPACITY - state.cargo
        && state.holding < 2
        && state.lane <= DETOUR
        && state
            .wounds
            .iter()
            .all(|&damage| (0.0..=DRIVER_HEALTH).contains(&damage))
        && state.progress < duration(state.holding, state.lane)
        && (state.cargo != 0 || state.progress == 0)
}

/// Read the authoritative saved edit without materializing a terrain chunk.
/// An unreadable record remains unobserved and cannot admit a shipment.
fn route_cell(world: &World, p: IVec3) -> Option<BlockId> {
    if let Some(block) = known(world, p) {
        return Some(block);
    }
    let coord = crate::world::chunk_coord_from_world_block(p);
    let disk = world.regions.view.load(coord).ok().flatten()?;
    Some(disk.chunk.blocks[engine_voxel::ChunkData::local_index(p.x, p.y, p.z)])
}

/// A straight leg swept at cart width, with a continuous floor and headroom.
fn clearance(world: &World, from: Vec3, to: Vec3) -> Clearance {
    let v = world.voxel_scale.meters();
    let headroom = (1.8 / v).ceil() as i32;
    let delta = to - from;
    let steps = (delta.length() / (v * 0.5)).ceil().max(1.0) as u32;
    let side = Vec3::new(-delta.z, 0.0, delta.x).normalize_or_zero() * 0.45;
    let mut unknown = false;
    let mut last = None;
    for i in 0..=steps {
        let p = from + delta * (i as f32 / steps as f32);
        let lo = ((p - side.abs()) / v).floor().as_ivec3();
        let hi = ((p + side.abs()) / v).floor().as_ivec3();
        if last == Some((lo, hi)) {
            continue;
        }
        last = Some((lo, hi));
        for x in lo.x..=hi.x {
            for z in lo.z..=hi.z {
                for dy in -1..headroom {
                    match route_cell(world, IVec3::new(x, lo.y + dy, z)) {
                        None => unknown = true,
                        Some(b) if if dy < 0 { b.is_solid() } else { b.is_empty() } => {}
                        Some(_) => return Clearance::Blocked,
                    }
                }
            }
        }
    }
    if unknown {
        Clearance::Unobserved
    } else {
        Clearance::Open
    }
}

fn bridge_lane(world: &World, site: Site, lane: u8) -> Clearance {
    let z = LANES[lane as usize];
    let start = site.origin + Vec3::new(BRIDGE.start as f32, 0.0, z);
    let end = site.origin + Vec3::new(BRIDGE.end as f32 - world.voxel_scale.meters() * 0.5, 0.0, z);
    clearance(world, start, end)
}

fn crossing(lanes: [Clearance; 3], preferred: u8) -> Crossing {
    for lane in [preferred, 0, 1, DETOUR] {
        if lanes[lane as usize] == Clearance::Open {
            return Crossing::Open(lane);
        }
    }
    if lanes.contains(&Clearance::Unobserved) {
        Crossing::Unobserved
    } else {
        Crossing::Blocked
    }
}

/// Both lanes share the two road approaches and diverge at the abutment.
fn route_pose(site: Site, holding: u8, lane: u8, progress: f32) -> (Vec3, f32) {
    let (x, z) = HOLDINGS[holding as usize];
    let (x, z) = (x as f32, z as f32);
    let fork = f32::from(fork(holding));
    let east = BRIDGE.end as f32;
    let west = BRIDGE.start as f32;
    let normal = || {
        let lane = LANES[lane as usize];
        [
            (0.0, Vec3::new(x, 0.0, z - 3.5 * z.signum())),
            (2.0, Vec3::new(x, 0.0, 0.0)),
            (fork, Vec3::new(east + 2.0, 0.0, 0.0)),
            (fork + 1.0, Vec3::new(east + 1.0, 0.0, lane)),
            (fork + 6.0, Vec3::new(west - 1.0, 0.0, lane)),
            (fork + 7.0, Vec3::new(west - 2.0, 0.0, 0.0)),
            (fork + 12.0, Vec3::new(0.5, 0.0, 0.0)),
            (fork + 14.0, Vec3::new(0.5, 0.0, 4.0)),
        ]
    };
    let detour = [
        (0.0, Vec3::new(x, 0.0, z - 3.5 * z.signum())),
        (2.0, Vec3::new(x, 0.0, 0.0)),
        (fork, Vec3::new(east + 2.0, 0.0, 0.0)),
        (fork + 4.0, Vec3::new(east + 2.0, 0.0, DETOUR_Z)),
        (fork + 10.0, Vec3::new(west - 2.0, 0.0, DETOUR_Z)),
        (fork + 15.0, Vec3::new(west - 2.0, 0.0, 0.0)),
        (fork + 20.0, Vec3::new(0.5, 0.0, 0.0)),
        (fork + 22.0, Vec3::new(0.5, 0.0, 4.0)),
    ];
    let points = if lane == DETOUR { detour } else { normal() };
    let [(t0, a), (t1, b)] = points
        .windows(2)
        .find(|leg| progress < leg[1].0)
        .unwrap_or(&points[points.len() - 2..])
    else {
        unreachable!("route legs have two points")
    };
    let delta = *b - *a;
    let t = ((progress - t0) / (t1 - t0)).clamp(0.0, 1.0);
    (site.origin + a.lerp(*b, t), (-delta.z).atan2(delta.x))
}

fn detour_clearance(world: &World, site: Site) -> Clearance {
    let offset = f32::from(fork(0));
    let points = [0.0, 4.0, 10.0, 15.0].map(|step| route_pose(site, 0, DETOUR, offset + step).0);
    let mut unknown = false;
    for edge in points.windows(2) {
        match clearance(world, edge[0], edge[1]) {
            Clearance::Blocked => return Clearance::Blocked,
            Clearance::Unobserved => unknown = true,
            Clearance::Open => {}
        }
    }
    if unknown {
        Clearance::Unobserved
    } else {
        Clearance::Open
    }
}

fn counter_open(world: &World, site: Site) -> bool {
    let v = world.voxel_scale.meters();
    let base = ((site.market() - Vec3::Y * 0.5) / v).floor().as_ivec3();
    route_cell(world, base - IVec3::Y).is_some_and(BlockId::is_solid)
        && (0..(1.0 / v).round() as i32)
            .all(|dy| route_cell(world, base + IVec3::Y * dy) == Some(BlockId::Planks))
}

fn harvest(world: &World, site: Site, holding: usize) -> u32 {
    let v = world.voxel_scale.meters();
    let centre = site.field(holding);
    let mut planted = 0;
    let mut total = 0;
    let lo = ((centre - Vec3::new(1.5, 0.0, 2.5)) / v).floor().as_ivec3();
    let hi = ((centre + Vec3::new(1.5, 0.0, 1.5) - Vec3::splat(v * 0.5)) / v)
        .floor()
        .as_ivec3();
    for x in lo.x..=hi.x {
        for z in lo.z..=hi.z {
            let p = IVec3::new(x, lo.y, z);
            total += 1;
            if route_cell(world, p) == Some(BlockId::WheatCrop)
                && route_cell(world, p - IVec3::Y) == Some(BlockId::FungalSoil)
            {
                planted += 1;
            }
        }
    }
    planted * LOAD / total
}

pub(super) fn service(world: &mut World) {
    let clock = world.farming.clock;
    if clock % SECOND != 0 {
        return;
    }
    let Some(site) = site(&world.sample_config()) else {
        return;
    };
    let state = world.farming.planet_freight.unwrap_or_default();
    let lanes = [0, 1].map(|lane| bridge_lane(world, site, lane));
    let detour = detour_clearance(world, site);
    let preferred = if state.cargo == 0 { 0 } else { state.lane };
    let route = crossing([lanes[0], lanes[1], detour], preferred);
    let now = clock / SECOND;
    let harvested = [0, 1].map(|i| {
        if now % HARVEST_EVERY == 1 + i as u64 * 6 {
            harvest(world, site, i)
        } else {
            0
        }
    });
    let market_open = counter_open(world, site);
    let mut next = state;
    next.crossing = route;
    next.lanes = lanes;
    next.detour = detour;
    next.market_open = market_open;
    next.moving = false;
    next.retreating = false;
    next.wait = Wait::None;
    for (stock, harvested) in next.stock.iter_mut().zip(harvested) {
        *stock = (*stock + harvested).min(STOCK_CAPACITY);
    }
    if next.cargo == 0 {
        if !market_open {
            next.wait = Wait::Counter;
        } else if let Crossing::Open(lane) = route {
            if now % SHIP_EVERY == 0 {
                dispatch(world, site, lane, &mut next);
            } else if matches!(state.wait, Wait::Road(_))
                && next.market < MARKET_CAPACITY
                && next.stock != [0; 2]
            {
                next.wait = state.wait;
            }
        } else {
            next.wait = Wait::Crossing;
        }
    } else {
        advance(world, site, &mut next);
    }
    world.farming.planet_freight = Some(next);
}

fn dispatch(world: &World, site: Site, lane: u8, state: &mut State) {
    let room = MARKET_CAPACITY - state.market;
    for holding in [1 - state.holding, state.holding] {
        if state.wounds[holding as usize] >= DRIVER_HEALTH {
            continue;
        }
        let stock = &mut state.stock[holding as usize];
        let load = LOAD.min(*stock).min(room);
        if load == 0 {
            continue;
        }
        let from = route_pose(site, holding, lane, 0.0).0;
        let to = route_pose(site, holding, lane, 1.0).0;
        let road = clearance(world, from, to);
        if road != Clearance::Open {
            state.wait = Wait::Road(road);
            continue;
        }
        *stock -= load;
        state.cargo = load;
        state.holding = holding;
        state.lane = lane;
        state.pause_until = 0;
        state.wait = Wait::None;
        break;
    }
}

fn advance(world: &World, site: Site, state: &mut State) {
    if state.wounds[state.holding as usize] >= DRIVER_HEALTH {
        state.wait = Wait::Driver;
        return;
    }
    if world.farming.clock < state.pause_until {
        state.wait = Wait::Driver;
        if state.progress > 0 {
            let at = |p| route_pose(site, state.holding, state.lane, f32::from(p)).0;
            if clearance(world, at(state.progress), at(state.progress - 1)) == Clearance::Open {
                state.progress -= 1;
                state.retreating = true;
            }
        }
        return;
    }
    let junction = fork(state.holding);
    let exit = junction + if state.lane == DETOUR { 10 } else { 6 };
    let clearance_now = if state.lane == DETOUR {
        state.detour
    } else {
        state.lanes[state.lane as usize]
    };
    if state.progress > junction && state.progress < exit && clearance_now == Clearance::Blocked {
        let from = route_pose(site, state.holding, state.lane, f32::from(state.progress)).0;
        let to = route_pose(
            site,
            state.holding,
            state.lane,
            f32::from(state.progress - 1),
        )
        .0;
        if clearance(world, from, to) == Clearance::Open {
            state.progress -= 1;
            state.retreating = true;
            return;
        }
        // The damage can be behind the cart. Let the forward sweep decide
        // whether it can still reach the far bank.
    }
    if state.progress == junction {
        let Crossing::Open(lane) = state.crossing else {
            state.wait = Wait::Crossing;
            return;
        };
        state.lane = lane;
    }
    let arrival = state.progress + 1 == duration(state.holding, state.lane);
    if arrival && !state.market_open {
        state.wait = Wait::Counter;
        return;
    }
    let from = route_pose(site, state.holding, state.lane, f32::from(state.progress)).0;
    let to = route_pose(
        site,
        state.holding,
        state.lane,
        f32::from(state.progress + 1),
    )
    .0;
    let road = clearance(world, from, to);
    if road != Clearance::Open {
        state.wait = Wait::Road(road);
        return;
    }
    if arrival {
        state.market += state.cargo;
        state.cargo = 0;
        state.progress = 0;
    } else {
        state.progress += 1;
        state.moving = true;
    }
}

pub(crate) fn view(world: &World, viewer: Vec3) -> freight::View {
    let Some(state) = world.farming.planet_freight else {
        return freight::View::default();
    };
    let Some(site) = site(&world.sample_config()) else {
        return freight::View::default();
    };
    if viewer.distance_squared(site.origin) > 110.0 * 110.0 {
        return freight::View::default();
    }
    let v = world.voxel_scale.meters();
    let cart = (state.cargo != 0)
        .then(|| {
            let progress = visual_progress(world, state);
            let pose = |at| route_pose(site, state.holding, state.lane, at);
            let supported = |p: Vec3| {
                let floor = ((p - Vec3::Y * (v * 0.5)) / v).floor().as_ivec3();
                resident(world, floor) && world.get(floor).is_solid()
            };
            let (point, yaw) = pose(progress);
            if supported(point) {
                return Some((point, yaw));
            }
            (0..state.progress)
                .rev()
                .map(|i| pose(f32::from(i)))
                .find(|&(point, _)| supported(point))
        })
        .flatten();
    let counter = ((site.market() + Vec3::Y * (0.5 - v * 0.5)) / v)
        .floor()
        .as_ivec3();
    let crates = (state.market != 0 && resident(world, counter) && counter_open(world, site))
        .then(|| (site.market() + Vec3::Y * 0.5, state.market.div_ceil(LOAD)));
    freight::View {
        cart: cart.map(|(point, _)| point),
        cart_yaw: cart.map_or(0.0, |(_, yaw)| yaw),
        crates,
    }
}

pub(crate) fn driver_name(id: u32) -> Option<&'static str> {
    DRIVERS
        .iter()
        .position(|&driver| driver == id)
        .map(|index| DRIVER_NAMES[index])
}

pub(crate) fn driver(world: &World, viewer: Vec3) -> Option<(u32, Vec3, f32, f32, f32, f32)> {
    let state = world.farming.planet_freight?;
    let health = DRIVER_HEALTH - state.wounds[state.holding as usize];
    if state.cargo == 0 || health <= 0.0 {
        return None;
    }
    let view = view(world, viewer);
    let cart = view.cart?;
    let yaw = view.cart_yaw;
    let gait = if state.moving || state.retreating {
        1.0
    } else {
        0.0
    };
    Some((
        DRIVERS[state.holding as usize],
        cart + glam::Quat::from_rotation_y(yaw) * Vec3::X * 0.85,
        yaw - std::f32::consts::FRAC_PI_2,
        visual_progress(world, state),
        gait,
        health,
    ))
}

pub(crate) fn remember_driver(world: &mut World, id: u32, health: f32) {
    let Some(state) = world.farming.planet_freight.as_mut() else {
        return;
    };
    if state.cargo == 0 || DRIVERS[state.holding as usize] != id {
        return;
    }
    let wounds = (DRIVER_HEALTH - health).clamp(0.0, DRIVER_HEALTH);
    let previous = &mut state.wounds[state.holding as usize];
    if wounds > *previous {
        *previous = wounds;
        state.pause_until = world.farming.clock.saturating_add(DRIVER_PAUSE);
        state.wait = Wait::Driver;
    }
}

pub(crate) fn talk(world: &mut World, id: u32) {
    let Some(state) = world.farming.planet_freight else {
        return;
    };
    let Some(name) = driver_name(id) else { return };
    if DRIVERS[state.holding as usize] != id || state.cargo == 0 {
        return;
    }
    let status = match state.wait {
        Wait::Driver => "Taking cover",
        Wait::Crossing => "Crossing blocked",
        Wait::Road(_) => "Road obstructed",
        Wait::Counter => "Receiving counter closed",
        Wait::None => "Delivering to Westbank Market",
    };
    world
        .expedition
        .say(format!("{name} · {status} · {} biomass", state.cargo));
}

pub(crate) fn dev_status(world: &World) -> String {
    let Some(site) = site(&world.sample_config()) else {
        return "No Planet freight site".to_owned();
    };
    let state = world.farming.planet_freight.unwrap_or_default();
    let index = state.holding as usize;
    let id = DRIVERS[index];
    let bodies = world
        .adventure
        .enemies
        .iter()
        .filter(|actor| crate::world::adventure::wildlife::vehicle::driver_id(actor) == Some(id))
        .count();
    let (at, _) = route_pose(site, state.holding, state.lane, f32::from(state.progress));
    format!(
        "{} ({id}) · {} health · {} active bodies · stocks {:?} · cargo {} · market {} · lane {} · step {} · {:.1}, {:.1}, {:.1} · {:?}",
        DRIVER_NAMES[index],
        DRIVER_HEALTH - state.wounds[index],
        bodies,
        state.stock,
        state.cargo,
        state.market,
        state.lane,
        state.progress,
        at.x,
        at.y,
        at.z,
        state.wait
    )
}

pub(crate) fn stranded_distance(
    world: &World,
    eye: Vec3,
    forward: Vec3,
    reach: f32,
) -> Option<f32> {
    let state = world.farming.planet_freight?;
    if state.cargo == 0 || state.wounds[state.holding as usize] < DRIVER_HEALTH {
        return None;
    }
    let center = view(world, eye).cart? + Vec3::Y * 0.35;
    let distance = (center - eye).dot(forward);
    ((0.0..=reach).contains(&distance)
        && (eye + forward * distance).distance_squared(center) <= 0.45 * 0.45
        && crate::world::adventure::line(world, eye, center))
    .then_some(distance)
}

pub(crate) fn recover(world: &mut World) {
    #[cfg(feature = "multiplayer")]
    if world
        .multiplayer
        .as_ref()
        .is_some_and(|session| !session.shared_tick)
    {
        return;
    }
    let Some(state) = world.farming.planet_freight.as_mut() else {
        return;
    };
    if state.cargo == 0 || state.wounds[state.holding as usize] < DRIVER_HEALTH {
        return;
    }
    let (inserted, _) = world
        .inventory
        .try_insert(ItemStack::new(super::pantry::BIOMASS, state.cargo));
    state.cargo -= inserted;
    if state.cargo == 0 {
        state.progress = 0;
        state.pause_until = 0;
    }
    if inserted == 0 {
        world.expedition.say("Inventory full");
    } else {
        world
            .expedition
            .say(format!("Recovered {inserted} biomass"));
    }
}

fn visual_progress(world: &World, state: State) -> f32 {
    let fraction = (world.farming.clock % SECOND) as f32 / SECOND as f32;
    let progress = f32::from(state.progress);
    if state.moving {
        progress - 1.0 + fraction
    } else if state.retreating {
        progress + 1.0 - fraction
    } else {
        progress
    }
}

pub(crate) struct Report {
    pub site: Site,
    pub lanes: [Clearance; 2],
    pub detour: Option<Clearance>,
    pub cart: Option<Vec3>,
    pub text: String,
}

pub(crate) fn report(world: &World, viewer: Vec3) -> Option<Report> {
    let site = site(&world.sample_config())?;
    if viewer.distance_squared(site.origin) > 450.0 * 450.0 {
        return None;
    }
    let state = world.farming.planet_freight.unwrap_or_default();
    let bridge = match state.crossing {
        Crossing::Open(0) => "North lane open",
        Crossing::Open(DETOUR) => "Service crossing open",
        Crossing::Open(_) => "South lane open",
        Crossing::Blocked => "Bridge blocked",
        Crossing::Unobserved => "Crossing unobserved",
    };
    let shipment = match state.cargo {
        0 if state.market == MARKET_CAPACITY => "Market full".to_owned(),
        0 => "Awaiting dispatch".to_owned(),
        n => {
            let source = if state.holding == 0 {
                "North holding"
            } else {
                "South holding"
            };
            let time = duration(state.holding, state.lane) - state.progress;
            let driver = DRIVER_NAMES[state.holding as usize];
            format!("{driver} · {source} · {n} cargo · {time}s remaining")
        }
    };
    let wait = match state.wait {
        Wait::None => "",
        Wait::Crossing => "\nWaiting for bridge",
        Wait::Road(Clearance::Blocked) => "\nRoad blocked",
        Wait::Road(_) => "\nRoad unobserved",
        Wait::Counter => "\nReceiving counter damaged or unobserved",
        Wait::Driver if state.wounds[state.holding as usize] >= DRIVER_HEALTH => {
            "\nDriver lost · cargo stranded"
        }
        Wait::Driver => "\nDriver taking cover",
    };
    Some(Report {
        site,
        lanes: state.lanes,
        detour: (world.generator_version >= engine_voxel::PLANET_FREIGHT_DETOUR_GENERATOR
            || state.detour == Clearance::Open)
            .then_some(state.detour),
        cart: (state.cargo != 0)
            .then(|| route_pose(site, state.holding, state.lane, f32::from(state.progress)).0),
        text: format!(
            "{bridge}\nHolding stocks {} / {}\n{shipment}{wait}\nMarket stock {}",
            state.stock[0], state.stock[1], state.market
        ),
    })
}

pub(crate) fn collect(world: &mut World, cell: IVec3) -> bool {
    let Some(site) = site(&world.sample_config()) else {
        return false;
    };
    let q = (cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters() - site.origin;
    if !(0.0..1.0).contains(&q.x)
        || !(0.0..1.0).contains(&q.y)
        || !(5.0..6.0).contains(&q.z)
        || world.get(cell) != BlockId::Planks
    {
        return false;
    }
    #[cfg(feature = "multiplayer")]
    if world
        .multiplayer
        .as_ref()
        .is_some_and(|session| !session.shared_tick)
    {
        world.expedition.say("The host manages this market");
        return true;
    }
    if !counter_open(world, site) {
        world.expedition.say("Repair the receiving counter");
        return true;
    }
    let state = world
        .farming
        .planet_freight
        .get_or_insert_with(State::default);
    if state.market == 0 {
        world.expedition.say("Market stock empty");
        return true;
    }
    let (inserted, _) = world.inventory.try_insert(ItemStack {
        item: super::pantry::BIOMASS,
        count: state.market.min(LOAD),
    });
    state.market -= inserted;
    if inserted == 0 {
        world.expedition.say("Inventory full");
    } else {
        world
            .expedition
            .say(format!("Collected {inserted} biomass"));
    }
    true
}
