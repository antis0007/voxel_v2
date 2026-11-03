//! The Meridian Lift: fixed atmospheric service corridors of an old orbital
//! commons. Traffic is reconstructed from world seed and saved calendar, even
//! where no chunks are loaded. Bookings and the finite receiver ledger are saved.
use super::{DayNightCycle, WorldSampleConfig, hash_u32, terrain_height};
use glam::{IVec2, Vec3};
pub mod receiver;
pub mod radio;
pub mod megastructures;
pub mod skyhook;
pub mod meteors;
pub mod passes;

pub const CELL_M: f32 = 4096.0;
pub const SIGHT_RANGE_M: f32 = 6000.0;
/// Magically supported atmospheric infrastructure, not a ballistic orbit.
pub const HOLD_CLEARANCE_M: f32 = 360.0;
pub const UPPER_ANCHORAGE_OFFSET_M: f32 = 18_000.0;
pub const PARKING_FRACTION: f32 = 0.38;
const SERVICE_DAYS: f64 = 0.22;
const NOTICE_DAYS: f64 = 0.025;

#[derive(Clone, Copy, Debug)]
pub enum Class {
    Courier,
    Freight,
    Conservatory,
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Self::Courier => "Needle courier",
            Self::Freight => "Talon freight lift",
            Self::Conservatory => "Harbour conservatory",
        }
    }
    pub fn width(self) -> f32 {
        match self {
            Self::Courier => 12.0,
            Self::Freight => 32.0,
            Self::Conservatory => 80.0,
        }
    }
    pub fn upper_ring_radius_m(self) -> f32 {
        match self {Self::Courier=>38.0,Self::Freight=>300.0,Self::Conservatory=>1200.0}
    }
    pub fn manifest(self) -> &'static str {
        match self {
            Self::Courier => {
                "Sealed correspondence; recipient keys required. Contents are never broadcast."
            }
            Self::Freight => {
                "Ceramic feedstock and replacement tension braid. Return mass reserved for worn components."
            }
            Self::Conservatory => {
                "Living seed archive remains quarantined. Maintenance parts use separate sealed service capsules."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Approach,
    Descending,
    Holding,
    Ascending,
    Departed,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Approach => "Alignment notice",
            Self::Descending => "Descending",
            Self::Holding => "Atmospheric hold",
            Self::Ascending => "Ascending",
            Self::Departed => "Next window",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub cell: IVec2,
    pub key: u32,
    pub class: Class,
    pub x: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Service {
    pub site: Site,
    pub phase: Phase,
    /// 0 at the upper endpoint, 1 at atmospheric hold. Continuous at phase joins.
    pub lowering: f32,
    pub minutes_to_change: u32,
    pub requested: bool,
    pub handoff_day: f64,
}

impl Service {
    /// Active atmospheric suspension follows a broad approach arc. This is a
    /// magitech winch trajectory, not a ballistic orbit. Both endpoints have
    /// zero lateral velocity; the receiver's fixed hold position is unchanged.
    pub fn carriage_position(self, foot: Vec3, top: Vec3) -> Vec3 {
        let t = self.lowering.clamp(0.0, 1.0);
        let envelope = (std::f32::consts::PI * t).sin().powi(2);
        let heading = (self.site.key >> 8) as f32 / 16777216.0 * std::f32::consts::TAU;
        let sense = if self.site.key & 1 == 0 { 1.0 } else { -1.0 };
        let azimuth = heading + sense * (t - 0.5) * 1.8;
        foot.lerp(top, (1.0 - t) * PARKING_FRACTION)
            + Vec3::new(azimuth.cos(), 0.0, azimuth.sin()) * (1800.0 * envelope)
    }

    /// Bounded analytic suspension response, not a collision simulation. The
    /// pendulum frequency follows suspension length; larger decks swing slower.
    /// A smooth driving envelope closes both ends of each leg exactly, so
    /// skipped time and reloads cannot accumulate unstable spring energy.
    pub fn suspension_angle(self, day: &DayNightCycle) -> f32 {
        let start = match self.phase {
            Phase::Descending => self.handoff_day - SERVICE_DAYS * 0.5,
            Phase::Ascending => self.handoff_day + SERVICE_DAYS * 0.1,
            _ => return 0.0,
        };
        let duration = SERVICE_DAYS * 0.4;
        let u = ((calendar(day) - start) / duration).clamp(0.0, 1.0) as f32;
        let seconds = u * duration as f32 * day.day_length_seconds();
        let omega = (9.81 / (self.site.class.width() * 0.71)).sqrt();
        let envelope = (std::f32::consts::PI * u).sin().powi(2);
        let direction = if self.phase == Phase::Ascending { -1.0 } else { 1.0 };
        direction * 0.045 * envelope * (-0.08 * seconds).exp() * (omega * seconds).sin()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Channel {
    Radio,
    Resonance,
}

#[derive(Clone, Copy, Debug)]
pub struct Booking {
    cell: IVec2,
    day: u32,
    time: f32,
    channel: Channel,
}

#[derive(Default)]
pub struct State {
    pass_cache: std::cell::RefCell<Option<passes::Cache>>,
    booking: Option<Booking>,
    pub receiver: receiver::State,
    pub radio: radio::State,
    // Small presentation cache only. Traversing the world never accumulates
    // terrain probes, and no cached values enter the save format.
    heights: std::cell::RefCell<Vec<(IVec2, f32)>>,
}

#[derive(Clone, Debug)]
pub struct Contact {
    pub service: Service,
    pub distance_m: f32,
    pub bearing: u32,
    pub can_request: bool,
}

#[derive(Clone, Default)]
pub struct View {
    pub pass_forecasts: Vec<passes::Forecast>,
    pub pass_forecasts_pending: bool,
    pub calendar: f64,
    pub contacts: Vec<Contact>,
    pub megastructures: Vec<megastructures::Contact>,
    pub skyhooks: Vec<skyhook::Contact>,
    pub receiver: receiver::View,
    pub radio: radio::View,
}

pub fn view(world: &super::World, day: &DayNightCycle, player: &crate::player::Player) -> View {
    View {
        calendar: calendar(day),
        pass_forecasts: if world.orbital.radio.level>0 {passes::forecast(world,day,player.pos*world.voxel_scale.meters())} else {Vec::new()},
        pass_forecasts_pending: world.orbital.radio.level>0 && passes::pending(world),
        megastructures: megastructures::contacts(world.seed, day, player.pos * world.voxel_scale.meters()),
        skyhooks: skyhook::contacts(world.seed, day, player.pos * world.voxel_scale.meters()),
        contacts: contacts(
            world.seed,
            day,
            &world.orbital,
            player.pos * world.voxel_scale.meters(),
        ),
        receiver: receiver::view(world, day, player),
        radio: radio::view(world, day, player),
    }
}

fn unit(key: u32) -> f32 {
    (hash_u32(key) >> 8) as f32 / 16777216.0
}

fn calendar(day: &DayNightCycle) -> f64 {
    day.day_index as f64 + day.time_of_day as f64
}

/// Corridors never stand closer than this. Cells are jittered inside 4 km, so
/// neighbouring cells could otherwise land a kilometre and a half apart.
pub const MIN_SPACING_M: f32 = 3200.0;

/// A corridor survives unless a higher-priority neighbour stands within
/// `MIN_SPACING_M`. The starting corridor always wins; otherwise the larger key
/// does, so every pair of survivors is at least that far apart and no survivor
/// moves.
pub fn site(seed: u32, cell: IVec2) -> Option<Site> {
    let me = raw_site(seed, cell)?;
    if cell == IVec2::ZERO {
        return Some(me);
    }
    for dz in -1..=1 {
        for dx in -1..=1 {
            let other_cell = cell + IVec2::new(dx, dz);
            if other_cell == cell {
                continue;
            }
            let Some(other) = raw_site(seed, other_cell) else { continue };
            let close = Vec3::new(other.x - me.x, 0.0, other.z - me.z).length() < MIN_SPACING_M;
            if close && (other_cell == IVec2::ZERO || other.key > me.key) {
                return None;
            }
        }
    }
    Some(me)
}

fn raw_site(seed: u32, cell: IVec2) -> Option<Site> {
    let key = hash_u32(
        seed ^ (cell.x as u32).wrapping_mul(0x9e3779b9)
            ^ (cell.y as u32).wrapping_mul(0x85ebca6b)
            ^ 0x4c494654,
    );
    if cell != IVec2::ZERO && unit(key) > 0.46 {
        return None;
    }
    // One reliable discovery corridor near the starting region. Other corridors
    // are sparse, world-anchored, and never follow or spawn around the camera.
    let (x, z) = if cell == IVec2::ZERO {
        (1100.0, -900.0)
    } else {
        (
            (cell.x as f32 + 0.18 + unit(key ^ 1) * 0.64) * CELL_M,
            (cell.y as f32 + 0.18 + unit(key ^ 2) * 0.64) * CELL_M,
        )
    };
    let class = match hash_u32(key ^ 3) % 10 {
        0 => Class::Conservatory,
        1..=4 => Class::Freight,
        _ => Class::Courier,
    };
    Some(Site {
        cell,
        key,
        class,
        x,
        z,
    })
}

/// Always available to observation equipment, including between visits.
fn service(site: Site, day: &DayNightCycle, state: &State) -> Service {
    let now = calendar(day);
    let booked = state.booking.filter(|b| b.cell == site.cell).filter(|b| {
        let start = b.day as f64 + b.time as f64;
        now >= start - NOTICE_DAYS && now < start + SERVICE_DAYS
    });
    let (start, requested) = if let Some(b) = booked {
        (b.day as f64 + b.time as f64, true)
    } else {
        // A corridor has a regular three-day cadence and a seeded phase. This
        // creates globally varied traffic without rerolling on load/approach.
        let offset = unit(site.key ^ 4) as f64 * 3.0;
        let cycle = ((now - offset + NOTICE_DAYS) / 3.0).floor();
        let mut start = cycle * 3.0 + offset;
        if now >= start + SERVICE_DAYS {
            start += 3.0;
        }
        (start, false)
    };
    let p = ((now - start) / SERVICE_DAYS) as f32;
    let ease = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let (phase, lowering, change) = if p < 0.0 {
        (
            if now >= start - NOTICE_DAYS {
                Phase::Approach
            } else {
                Phase::Departed
            },
            0.0,
            start,
        )
    } else if p < 0.4 {
        (Phase::Descending, ease(p / 0.4), start + SERVICE_DAYS * 0.4)
    } else if p < 0.6 {
        (Phase::Holding, 1.0, start + SERVICE_DAYS * 0.6)
    } else {
        (
            Phase::Ascending,
            1.0 - ease((p - 0.6) / 0.4),
            start + SERVICE_DAYS,
        )
    };
    Service {
        site,
        phase,
        lowering,
        requested,
        handoff_day: start + SERVICE_DAYS * 0.5,
        minutes_to_change: ((change - now).max(0.0) * 1440.0).ceil() as u32,
    }
}

pub fn contacts(seed: u32, day: &DayNightCycle, state: &State, viewer: Vec3) -> Vec<Contact> {
    let centre = IVec2::new(
        (viewer.x / CELL_M).floor() as i32,
        (viewer.z / CELL_M).floor() as i32,
    );
    let now = calendar(day);
    let busy = state
        .booking
        .is_some_and(|b| now < b.day as f64 + b.time as f64 + SERVICE_DAYS);
    let mut result = Vec::new();
    for z in -2..=2 {
        for x in -2..=2 {
            let Some(site) = site(seed, centre + IVec2::new(x, z)) else {
                continue;
            };
            let delta = Vec3::new(site.x - viewer.x, 0.0, site.z - viewer.z);
            let distance_m = delta.length();
            if distance_m > SIGHT_RANGE_M {
                continue;
            }
            let service = service(site, day, state);
            result.push(Contact {
                service,
                distance_m,
                bearing: (delta
                    .x
                    .atan2(-delta.z)
                    .to_degrees()
                    .rem_euclid(360.0)
                    .round() as u32)
                    % 360,
                // A booking must fit wholly before the next scheduled approach;
                // otherwise returning to the regular timetable would teleport an
                // already descending carriage into the middle of another visit.
                can_request: !busy
                    && service.phase == Phase::Departed
                    && service.minutes_to_change
                        > ((SERVICE_DAYS + NOTICE_DAYS * 2.0) * 1440.0).ceil() as u32,
            });
        }
    }
    result.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m));
    result
}

/// Corridors remain visible beyond radio/service range: large crowns with their
/// sealed upper structures, the rest as a braid and carriage. This does not
/// extend request permissions.
pub fn visual_contacts(seed: u32, day: &DayNightCycle, state: &State, viewer: Vec3) -> Vec<Contact> {
    let mut result=contacts(seed,day,state,viewer);
    let centre=IVec2::new((viewer.x/CELL_M).floor() as i32,(viewer.z/CELL_M).floor() as i32);
    let mut far=Vec::new();
    for z in -6..=6 {for x in -6..=6 {
        let Some(site)=site(seed,centre+IVec2::new(x,z)) else {continue;};
        let d=Vec3::new(site.x-viewer.x,0.0,site.z-viewer.z).length();
        if d>SIGHT_RANGE_M && d<=megastructures::RANGE_M+4000.0 {
            far.push(Contact {service:service(site,day,state),distance_m:d,bearing:0,can_request:false});
        }
    }}
    // Dozens of braids converging on the horizon looked wrong; keep the nearest few.
    far.sort_by(|a,b|a.distance_m.total_cmp(&b.distance_m));
    far.truncate(MAX_FAR_CORRIDORS);
    result.extend(far);
    result
}

/// Corridors drawn beyond sight range.
const MAX_FAR_CORRIDORS: usize = 3;

impl State {
    pub fn request(
        &mut self,
        seed: u32,
        day: &DayNightCycle,
        viewer: Vec3,
        cell: IVec2,
        channel: Channel,
    ) -> &'static str {
        let eligible = contacts(seed, day, self, viewer)
            .iter()
            .any(|c| c.service.site.cell == cell && c.can_request);
        if !eligible {
            return "Meridian: alignment unavailable. Read the current service window.";
        }
        let start = calendar(day) + NOTICE_DAYS;
        self.booking = Some(Booking {
            cell,
            day: start.floor() as u32,
            time: start.fract() as f32,
            channel,
        });
        match channel {
            Channel::Radio => {
                "Return requested. Carriage descending after alignment. Cargo locked."
            }
            Channel::Resonance => {
                "Return requested. Carriage descending after alignment. Cargo locked."
            }
        }
    }

    pub fn encode(&self, bytes: &mut Vec<u8>, version: u8) {
        let mut push = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        push(u32::from(self.booking.is_some()));
        if let Some(b) = self.booking {
            push(b.cell.x as u32);
            push(b.cell.y as u32);
            push(b.day);
            push(b.time.to_bits());
            push(match b.channel {
                Channel::Radio => 0,
                Channel::Resonance => 1,
            });
        }
        if version >= 25 {
            self.receiver.encode(bytes);
        }
        if version >= 28 { self.radio.encode(bytes); }
    }

    pub fn decode(
        read: &mut impl FnMut() -> anyhow::Result<u32>,
        version: u8,
    ) -> anyhow::Result<Self> {
        let present = read()?;
        anyhow::ensure!(present <= 1, "Invalid orbital booking flag");
        let mut state = Self::default();
        if present != 0 {
            let cell = IVec2::new(read()? as i32, read()? as i32);
            let day = read()?;
            let time = f32::from_bits(read()?);
            let channel = read()?;
            anyhow::ensure!(
                cell.x.unsigned_abs() <= 1_000_000
                    && cell.y.unsigned_abs() <= 1_000_000
                    && time.is_finite()
                    && (0.0..1.0).contains(&time)
                    && channel <= 1,
                "Invalid orbital booking"
            );
            state.booking = Some(Booking {
                cell,
                day,
                time,
                channel: if channel == 0 {
                    Channel::Radio
                } else {
                    Channel::Resonance
                },
            });
        }
        if version >= 25 {
            state.receiver = receiver::State::decode(read,version)?;
        }
        if version >= 28 { state.radio = radio::State::decode(read)?; }
        Ok(state)
    }
}

/// A high atmospheric hold, deliberately clear of the terrain. This first
/// platform has no landing, collision or passenger transfer. Receiver shipments
/// use their own small service capsule and persistent cargo ledger.
pub fn hold_height(cfg: &WorldSampleConfig, state: &State, site: Site) -> f32 {
    if let Some((_, height)) = state
        .heights
        .borrow()
        .iter()
        .find(|(cell, _)| *cell == site.cell)
    {
        return *height;
    }
    let mut ground = cfg.world_y(cfg.water_level());
    for dz in [-1.0, 0.0, 1.0] {
        for dx in [-1.0, 0.0, 1.0] {
            let x = ((site.x + dx * 120.0) * cfg.inv_voxel_meters()).floor() as i32;
            let z = ((site.z + dz * 120.0) * cfg.inv_voxel_meters()).floor() as i32;
            ground = ground.max(cfg.world_y(terrain_height(cfg, x, z)));
        }
    }
    let height = ground + HOLD_CLEARANCE_M;
    let mut cache = state.heights.borrow_mut();
    if cache.len() >= 64 {
        cache.clear();
    }
    cache.push((site.cell, height));
    height
}

#[cfg(test)]
mod far_corridor_tests {
    use super::*;

    #[test]
    fn distant_corridors_are_capped() {
        let day = DayNightCycle::default();
        let state = State::default();
        for seed in [1_u32, 74, 9001, 0xdead_beef] {
            for viewer in [Vec3::ZERO, Vec3::new(9000.0, 100.0, -4000.0), Vec3::new(-30000.0, 100.0, 22000.0)] {
                let far = visual_contacts(seed, &day, &state, viewer)
                    .iter()
                    .filter(|c| c.distance_m > SIGHT_RANGE_M)
                    .count();
                assert!(far <= MAX_FAR_CORRIDORS, "seed {seed} viewer {viewer}: {far} far corridors");
            }
        }
    }
    #[test]
    fn corridors_keep_their_distance() {
        for seed in [1_u32, 74, 9001, 0xdead_beef] {
            let sites: Vec<Site> = (-30..30)
                .flat_map(|z| (-30..30).map(move |x| IVec2::new(x, z)))
                .filter_map(|cell| site(seed, cell))
                .collect();
            assert!(sites.len() > 300, "seed {seed}: only {} corridors", sites.len());
            for (i, a) in sites.iter().enumerate() {
                for b in &sites[i + 1..] {
                    let d = Vec3::new(a.x - b.x, 0.0, a.z - b.z).length();
                    assert!(d >= MIN_SPACING_M, "seed {seed}: {:?} and {:?} are {d:.0} m apart", a.cell, b.cell);
                }
            }
            assert!(site(seed, IVec2::ZERO).is_some(), "the starting corridor must stay");
        }
    }
}
