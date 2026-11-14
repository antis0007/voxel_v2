//! Picks ambience bed levels from a bounded probe of the voxels around the listener.
use crate::audio::{Bed, BED_COUNT};
use engine_content::BlockId;
use glam::IVec3;

/// Seconds between probes; beds cross-fade over a few seconds anyway.
pub const PROBE_SECONDS: f32 = 0.5;
/// How far up a column is searched for a roof, metres.
const SKY_REACH_M: f32 = 24.0;
/// Horizontal offset of the four outer sky columns, metres.
const SKY_SPREAD_M: f32 = 3.0;
const WALL_REACH_M: f32 = 12.0;
/// Rings searched for water, metres from the listener.
const WATER_RINGS_M: [f32; 3] = [4.0, 12.0, 32.0];
/// Weight of a water hit in each ring; the outer ring is a distant shore or surf.
const WATER_WEIGHTS: [f32; 3] = [0.5, 0.25, 0.1];
/// Depth below the eye searched for water in each ring column, metres.
const WATER_DEPTH_M: f32 = 8.0;
/// Rock overhead (metres) between a roofed room and a cave.
const ROCK_ROOF_M: (f32, f32) = (3.0, 10.0);
/// Distance out to the edge columns and how far below the eye they look for ground, metres.
const EDGE_OUT_M: f32 = 4.0;
const DROP_M: f32 = 16.0;
/// Rings searched for canopy, metres from the listener, and the weight of a hit in each.
/// Spaced under a crown's width so a lone tree between rings is not missed.
const CANOPY_RINGS_M: [f32; 4] = [5.0, 9.0, 14.0, 20.0];
const CANOPY_WEIGHTS: [f32; 4] = [1.0, 0.8, 0.55, 0.35];
/// Canopy column scan: samples every `CANOPY_STEP_M` from 2 m below to 12 m above the eye.
const CANOPY_STEP_M: f32 = 2.0;
const CANOPY_STEPS: std::ops::RangeInclusive<i32> = -1..=6;
/// Water moving less than this share of full motion counts as still.
const STILL_MOTION: f32 = 0.2;

const DIRECTIONS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

/// What the probe measured, each 0..1 except the altitude.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surroundings {
    /// Share of the five sky columns with no roof.
    pub sky: f32,
    /// Thick rock overhead and walls around.
    pub underground: f32,
    /// Water close by, nearer rings weigh more.
    pub water: f32,
    /// Measured horizontal motion or a visible falling water column.
    pub water_motion: f32,
    /// Eye height above sea level, metres.
    pub altitude_m: f32,
    /// Share of directions that fall away by more than `DROP_M`: cliff edges, ridges, towers.
    pub exposure: f32,
    /// Solid terrain within the bounded downward probe; excludes high flight.
    pub grounded: bool,
    /// Ears under water, set by the caller from the player rather than probed.
    pub submerged: f32,
    /// Leaf and bush cells per compass sector (`DIRECTIONS` order), nearer rings weigh more.
    pub canopy: [f32; 8],
    /// Still water per compass sector, from the same rings as `water`.
    pub still_water: [f32; 8],
    /// Nearby sector coverage: flowers, fungi, crystals, lava. Counts are
    /// bounded per direction; stacking blocks never multiplies the bonus.
    pub flora: [f32; 4],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Weather {
    pub rain: f32,
    pub storm: f32,
    pub night: f32,
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Probes the listener's surroundings at roughly one-metre spacing.
#[cfg(test)]
pub fn probe(get: impl Fn(IVec3) -> BlockId, eye: IVec3, meters: f32, sea_level_m: f32) -> Surroundings {
    sample(get, eye, meters, sea_level_m).with_flow(|_| 0.0)
}

pub(super) struct Sample {
    surroundings: Surroundings,
    sources: Vec<WaterSource>,
}

struct WaterSource {
    position: IVec3,
    weight: f32,
    sector: usize,
    falling: bool,
}

impl Sample {
    /// Keep flow live even when the geometry was sampled on a worker.
    pub(super) fn with_flow(mut self, speed: impl Fn(IVec3) -> f32) -> Surroundings {
        let mut water = 0.0;
        let mut motion = 0.0;
        for source in self.sources {
            let moving = if source.falling { 1.0 } else { (speed(source.position).max(0.0) / 1.5).min(1.0) };
            water += source.weight;
            motion += source.weight * moving;
            if moving < STILL_MOTION {
                let still = &mut self.surroundings.still_water[source.sector];
                *still = (*still + source.weight * (1.0 - moving)).min(1.0);
            }
        }
        self.surroundings.water = f32::min(water / 2.0, 1.0);
        self.surroundings.water_motion = if water > 0.0 { (motion / water).clamp(0.0, 1.0) } else { 0.0 };
        self.surroundings
    }
}

pub(super) fn sample(get: impl Fn(IVec3) -> BlockId, eye: IVec3, meters: f32, sea_level_m: f32) -> Sample {
    let meters = meters.max(1.0e-3);
    let stride = (1.0 / meters).round().max(1.0) as i32;
    let cells = |m: f32| ((m / meters) as i32 / stride).max(1);
    let spread = (SKY_SPREAD_M / meters).round() as i32;
    let mut rock = 0u32;
    let rock_metres = |count: u32| count as f32 * stride as f32 * meters;
    for k in 1..=cells(SKY_REACH_M) {
        if get(eye + IVec3::Y * (k * stride)).is_opaque() {
            rock += 1;
            if rock_metres(rock) >= ROCK_ROOF_M.1 { break; }
        }
    }
    let mut open = u32::from(rock == 0);
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let column = eye + IVec3::new(dx * spread, 0, dz * spread);
        open += u32::from(!(1..=cells(SKY_REACH_M))
            .any(|k| get(column + IVec3::Y * (k * stride)).is_opaque()));
    }
    let roof = smoothstep(ROCK_ROOF_M.0, ROCK_ROOF_M.1, rock_metres(rock));
    // Wall samples only scale underground ambience, which is zero without enough roof.
    let underground = if roof > 0.0 {
        let walls = DIRECTIONS.iter().filter(|&&(dx, dz)| {
            (1..=cells(WALL_REACH_M)).any(|k| get(eye + IVec3::new(dx, 0, dz) * (k * stride)).is_opaque())
        }).count();
        roof * (0.4 + 0.6 * walls as f32 / 8.0)
    } else {
        0.0
    };
    let mut sources = Vec::new();
    for (ring, weight) in WATER_RINGS_M.iter().zip(WATER_WEIGHTS) {
        let r = (ring / meters).round() as i32;
        for (sector, (dx, dz)) in DIRECTIONS.into_iter().enumerate() {
            let column = eye + IVec3::new(dx * r, 0, dz * r);
            if let Some(p) = (-cells(WATER_DEPTH_M)..=1).rev()
                .map(|k| column + IVec3::Y * (k * stride)).find(|&p| get(p) == BlockId::Water) {
                // Solid walls hide a source; no room-wide roar from sealed pipes.
                let delta = p - eye;
                let steps = (delta.as_vec3().length() * meters).ceil().max(1.0) as i32;
                let blocked = (1..steps).any(|k| {
                    let q = (eye.as_vec3() + delta.as_vec3() * (k as f32 / steps as f32)).round().as_ivec3();
                    get(q).is_opaque()
                });
                if blocked { continue; }
                // A one-metre probe can jump through a thin pool floor at fine
                // voxel scales. Only an unsupported water cell is a fall.
                sources.push(WaterSource { position: p, weight, sector, falling: get(p - IVec3::Y) == BlockId::Air });
            }
        }
    }
    let mut canopy = [0.0f32; 8];
    let canopy_step = cells(CANOPY_STEP_M) * stride;
    for (ring, weight) in CANOPY_RINGS_M.iter().zip(CANOPY_WEIGHTS) {
        let r = (ring / meters).round() as i32;
        for (sector, (dx, dz)) in DIRECTIONS.into_iter().enumerate() {
            if canopy[sector] >= 2.0 { continue; }
            let column = eye + IVec3::new(dx * r, 0, dz * r);
            let mut hits = 0;
            for k in CANOPY_STEPS {
                if matches!(get(column + IVec3::Y * (k * canopy_step)), BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush) {
                    hits += 1;
                    if canopy[sector] + weight * hits as f32 >= 2.0 { break; }
                }
            }
            canopy[sector] += weight * hits as f32;
        }
    }
    for c in &mut canopy {
        *c = (*c / 2.0).min(1.0);
    }
    let out = (EDGE_OUT_M / meters).round() as i32;
    // Exposure is a cliff edge, not open air: without ground under the eye there is no edge.
    let grounded = (0..=cells(DROP_M)).any(|k| get(eye - IVec3::Y * (k * stride)).is_opaque());
    let drops = if grounded { DIRECTIONS
        .iter()
        .filter(|&&(dx, dz)| {
            let column = eye + IVec3::new(dx * out, 0, dz * out);
            !(0..=cells(DROP_M)).any(|k| get(column - IVec3::Y * (k * stride)).is_opaque())
        })
        .count() } else { 0 };
    let mut flora = [0.0; 4];
    let flora_stride = (0.25 / meters).round().max(1.0) as i32;
    let flora_down = (3.0 / (flora_stride as f32 * meters)).ceil() as i32;
    let flora_up = (4.0 / (flora_stride as f32 * meters)).ceil() as i32;
    for (dx, dz) in std::iter::once((0, 0)).chain(DIRECTIONS) {
        let mut seen = [false; 4];
        for radius in [2.0, 5.0, 9.0] {
            let r = (radius / meters).round() as i32;
            for height in -flora_down..=flora_up {
                let p = eye + IVec3::new(dx * r, height * flora_stride, dz * r);
                let family = match get(p) {
                    BlockId::Lavender | BlockId::Orchid | BlockId::ManaFlower | BlockId::CollectorFlower => Some(0),
                    BlockId::FungalSoil | BlockId::GroveMoss | BlockId::AmberCap | BlockId::UmberCap
                        | BlockId::SporeBlossom | BlockId::GlowBerries | BlockId::CaveVine => Some(1),
                    BlockId::Crystal => Some(2),
                    BlockId::Lava => Some(3),
                    _ => None,
                };
                if let Some(i) = family && !seen[i] {
                    let delta = p - eye;
                    let steps = (delta.as_vec3().length() * meters * 2.0).ceil().max(1.0) as i32;
                    let blocked = (1..steps).any(|k| {
                        let q = (eye.as_vec3() + delta.as_vec3() * (k as f32 / steps as f32)).round().as_ivec3();
                        q != p && get(q).is_opaque()
                    });
                    seen[i] = !blocked;
                }
            }
        }
        for i in 0..4 { flora[i] += f32::from(u8::from(seen[i])) / 9.0; }
    }
    let surroundings = Surroundings {
        exposure: drops as f32 / 8.0,
        grounded,
        sky: open as f32 / 5.0,
        underground,
        altitude_m: eye.y as f32 * meters - sea_level_m,
        canopy,
        flora,
        ..Default::default()
    };
    Sample { surroundings, sources }
}

/// How enclosed the listener is, 0 open air to 1 a cave; drives the room reverb.
/// A roofed room counts a little, so a house is not as dry as a field.
pub fn space(s: &Surroundings) -> f32 {
    (s.underground + (1.0 - s.sky) * 0.25).clamp(0.0, 1.0)
}

/// Scale height and top of the air, metres above sea level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air {
    /// Density stays at sea level up to here; the planet sky fade starts above its ground.
    pub floor_m: f32,
    pub scale_height_m: f32,
    pub top_m: f32,
}

/// Earthlike Rayleigh falloff from `assets/atmosphere.ron`, cut where chunk loading stops and
/// the globe view takes over, so the view from above never carries wind.
pub const EARTH_AIR: Air = Air { floor_m: 0.0, scale_height_m: 8_000.0, top_m: 6_000.0 };
/// Matches the planet branch of `sky_shader.wgsl` (exp(-(alt - 150) / 1400)), so the beds go
/// silent where the sky turns black; the top is the altitude where chunk loading stops.
pub const PLANET_AIR: Air = Air { floor_m: 150.0, scale_height_m: 1_400.0, top_m: 6_000.0 };
/// Realms under a star void: the Nether, the Hall and the Manyfold carry no outdoor sound.
pub const VOID_AIR: Air = Air { floor_m: 0.0, scale_height_m: 1.0, top_m: 0.0 };

/// Air density relative to sea level, 0 in vacuum.
pub fn air(profile: Air, altitude_m: f32) -> f32 {
    if profile.top_m <= 0.0 {
        return 0.0;
    }
    let above = (altitude_m - profile.floor_m).max(0.0);
    (-above / profile.scale_height_m).exp() * (1.0 - smoothstep(0.5 * profile.top_m, profile.top_m, altitude_m))
}

/// Target level of each bed for the surroundings and weather.
pub fn levels(s: &Surroundings, w: &Weather, profile: Air) -> [f32; BED_COUNT] {
    // Outdoor beds carry through air; a sealed cave keeps its own drone.
    let surface = (1.0 - s.underground) * air(profile, s.altitude_m) * (1.0 - s.submerged);
    let height = smoothstep(0.0, 90.0, s.altitude_m);
    let mut out = [0.0; BED_COUNT];
    let in_air = air(profile, s.altitude_m);
    // Void realms are not vacuum: their caves and water keep their own sound.
    let carried = if profile.top_m <= 0.0 { 1.0 } else { in_air };
    // A calm field is a quiet rush; height, storm and cliffs bring the wind up.
    out[Bed::Wind as usize] = (s.sky * (0.21 + 0.4 * height + 0.35 * w.storm + 0.3 * s.exposure)).min(1.0) * surface;
    // A cave drone is air resonating in rock; a sealed airless one is silent.
    out[Bed::Cave as usize] = s.underground * (1.0 - s.sky) * carried;
    out[Bed::Water as usize] = (s.water * (0.08 + 0.92 * s.water_motion) * (1.0 - 0.25 * s.underground) * carried)
        // Under water the bed becomes the low churn all around the head.
        .max(0.55 * s.submerged);
    out[Bed::Rain as usize] = w.rain * (0.3 + 0.7 * s.sky) * surface;
    // The suit fades in as the outside goes quiet, so there is never dead silence.
    let thin = 1.0 - air(profile, s.altitude_m);
    out[Bed::Suit as usize] = 0.7 * thin * thin;
    out[Bed::Insects as usize] = 0.8 * w.night * s.sky * (1.0 - w.rain) * (1.0 - height) * surface;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat ground at y = 0 and below, optional water surface rows.
    fn open_field(p: IVec3) -> BlockId {
        if p.y < 0 { BlockId::Stone } else { BlockId::Air }
    }

    fn cave(p: IVec3) -> BlockId {
        // A 9-wide pocket two cells tall, deep under rock.
        if p.y >= 0 && p.y < 3 && p.x.abs() < 5 && p.z.abs() < 5 { BlockId::Air } else { BlockId::Stone }
    }

    fn lakeshore(p: IVec3) -> BlockId {
        match p {
            p if p.y < -1 => BlockId::Stone,
            p if p.y == -1 && p.x > 2 => BlockId::Water,
            p if p.y == -1 => BlockId::Sand,
            _ => BlockId::Air,
        }
    }

    /// Open field with a leaf block cluster 10 m to +x, 3 to 8 m up.
    fn grove(p: IVec3) -> BlockId {
        if (8..=12).contains(&p.x) && (3..=8).contains(&p.y) && p.z.abs() <= 2 { BlockId::Leaves } else { open_field(p) }
    }

    const EYE: IVec3 = IVec3::new(0, 1, 0);
    const DAY: Weather = Weather { rain: 0.0, storm: 0.0, night: 0.0 };

    #[test]
    fn canopy_is_found_in_its_sector_only() {
        let s = probe(grove, EYE, 1.0, 0.0);
        assert!(s.canopy[0] > 0.3, "+x sector {}", s.canopy[0]);
        assert!(s.canopy[4] == 0.0 && s.canopy[2] == 0.0);
        assert_eq!(probe(open_field, EYE, 1.0, 0.0).canopy, [0.0; 8]);
    }

    #[test]
    fn canopy_survives_fine_voxels() {
        let quarter = probe(|p| grove(IVec3::new(p.x.div_euclid(4), p.y.div_euclid(4), p.z.div_euclid(4))), EYE * 4, 0.25, 0.0);
        let metre = probe(grove, EYE, 1.0, 0.0);
        assert!((quarter.canopy[0] - metre.canopy[0]).abs() < 0.3, "quarter {} metre {}", quarter.canopy[0], metre.canopy[0]);
    }

    #[test]
    fn still_water_is_per_sector_and_ignores_falls() {
        let s = probe(lakeshore, EYE, 1.0, 0.0);
        assert!(s.still_water[0] > 0.0 && s.still_water[4] == 0.0);
        let fall = |p: IVec3| if p.x == 6 && p.y == 2 { BlockId::Water } else { open_field(p) };
        assert_eq!(probe(fall, EYE, 1.0, 0.0).still_water, [0.0; 8]);
    }

    #[test]
    fn open_sky_plays_wind_and_no_cave() {
        let s = probe(open_field, EYE, 1.0, 0.0);
        assert_eq!((s.sky, s.underground, s.water), (1.0, 0.0, 0.0));
        let beds = levels(&s, &DAY, EARTH_AIR);
        assert!(beds[Bed::Wind as usize] > 0.2);
        assert_eq!(beds[Bed::Cave as usize], 0.0);
        let night = levels(&s, &Weather { night: 1.0, ..DAY }, EARTH_AIR);
        assert!(night[Bed::Insects as usize] > 0.5);
        let rain = levels(&s, &Weather { rain: 1.0, storm: 1.0, night: 1.0 }, EARTH_AIR);
        assert!(rain[Bed::Rain as usize] > 0.9 && rain[Bed::Insects as usize] == 0.0);
        assert!(rain[Bed::Wind as usize] > beds[Bed::Wind as usize]);
        assert_eq!(space(&s), 0.0);
    }

    #[test]
    fn a_cave_plays_the_drone_and_silences_the_surface_beds() {
        let s = probe(cave, EYE, 1.0, 0.0);
        assert_eq!(s.sky, 0.0);
        assert!(s.underground > 0.95, "underground {}", s.underground);
        let beds = levels(&s, &Weather { rain: 1.0, storm: 1.0, night: 1.0 }, EARTH_AIR);
        assert!(beds[Bed::Cave as usize] > 0.95);
        for bed in [Bed::Wind, Bed::Rain, Bed::Insects] {
            assert!(beds[bed as usize] < 0.05, "{bed:?} {}", beds[bed as usize]);
        }
        assert!(space(&s) > 0.95);
    }

    #[test]
    fn a_shore_plays_water_and_scale_keeps_the_answer() {
        let s = probe(lakeshore, EYE, 1.0, 0.0);
        assert!(s.water > 0.3, "water {}", s.water);
        assert!((0.0..=0.08).contains(&levels(&s, &DAY, EARTH_AIR)[Bed::Water as usize]));
        assert_eq!(probe(open_field, EYE, 1.0, 0.0).water, 0.0);
        // Quarter-metre voxels: same field, coordinates four times larger.
        let quarter = probe(|p| lakeshore(IVec3::new(p.x.div_euclid(4), p.y.div_euclid(4), p.z.div_euclid(4))), EYE * 4, 0.25, 0.0);
        assert!((quarter.water - s.water).abs() < 0.3 && quarter.sky == 1.0);
    }

    #[test]
    fn still_water_is_silent_in_vacuum() {
        let mut s = probe(lakeshore, EYE, 1.0, 0.0);
        s.altitude_m = 100_000.0;
        assert_eq!(levels(&s, &DAY, EARTH_AIR)[Bed::Water as usize], 0.0);
    }

    #[test]
    fn a_thin_roof_is_not_a_cave() {
        let hut = |p: IVec3| if p.y == 4 { BlockId::Planks } else { open_field(p) };
        let s = probe(hut, EYE, 1.0, 0.0);
        assert!(s.sky < 1.0 && s.underground == 0.0);
    }

    #[test]
    fn wind_rises_with_altitude() {
        let mut s = probe(open_field, EYE, 1.0, 0.0);
        let low = levels(&s, &DAY, EARTH_AIR)[Bed::Wind as usize];
        s.altitude_m = 120.0;
        assert!(levels(&s, &DAY, EARTH_AIR)[Bed::Wind as usize] > low * 2.0);
    }

    #[test]
    fn a_cliff_edge_is_exposed_and_windier() {
        let field = probe(open_field, EYE, 1.0, 0.0);
        assert_eq!(field.exposure, 0.0);
        // Ground ends at x = 2; the far side drops away.
        let cliff = |p: IVec3| if p.x > 2 && p.y > -40 { BlockId::Air } else { open_field(p) };
        let edge = probe(cliff, EYE, 1.0, 0.0);
        assert!(edge.exposure >= 0.3, "exposure {}", edge.exposure);
        assert!(levels(&edge, &DAY, EARTH_AIR)[Bed::Wind as usize] > levels(&field, &DAY, EARTH_AIR)[Bed::Wind as usize] + 0.1);
    }

    #[test]
    fn open_air_is_not_a_cliff_edge() {
        let flying = probe(open_field, IVec3::new(0, 40, 0), 1.0, 0.0);
        assert_eq!(flying.exposure, 0.0);
        assert!(flying.sky == 1.0 && flying.underground == 0.0);
    }

    #[test]
    fn the_planet_beds_fade_where_its_sky_turns_black() {
        let mut s = probe(open_field, EYE, 1.0, 0.0);
        assert_eq!(air(PLANET_AIR, 100.0), 1.0);
        // The sky shader's air at 5 km is exp(-4850 / 1400) = 0.031.
        s.altitude_m = 5_000.0;
        assert!(levels(&s, &DAY, PLANET_AIR)[Bed::Wind as usize] < 0.03);
        assert!(levels(&s, &DAY, PLANET_AIR)[Bed::Suit as usize] > 0.6);
        s.altitude_m = 6_000.0;
        assert_eq!(levels(&s, &DAY, PLANET_AIR)[Bed::Wind as usize], 0.0);
    }

    #[test]
    fn void_realms_carry_no_outdoor_sound() {
        let s = probe(open_field, EYE, 1.0, 0.0);
        let storm = Weather { rain: 1.0, storm: 1.0, night: 1.0 };
        let beds = levels(&s, &storm, VOID_AIR);
        for bed in [Bed::Wind, Bed::Rain, Bed::Insects] {
            assert_eq!(beds[bed as usize], 0.0, "{bed:?}");
        }
        assert!(beds[Bed::Suit as usize] > 0.6);
        assert_eq!(air(VOID_AIR, 0.0), 0.0);
    }

    #[test]
    fn diving_swaps_the_surface_for_the_water() {
        let mut s = probe(open_field, EYE, 1.0, 0.0);
        s.submerged = 1.0;
        let beds = levels(&s, &Weather { rain: 1.0, storm: 1.0, night: 1.0 }, EARTH_AIR);
        for bed in [Bed::Wind, Bed::Rain, Bed::Insects] {
            assert_eq!(beds[bed as usize], 0.0, "{bed:?}");
        }
        assert!(beds[Bed::Water as usize] > 0.5);
    }

    #[test]
    fn space_is_silent() {
        let mut s = probe(open_field, EYE, 1.0, 0.0);
        s.altitude_m = 80_000.0;
        let storm = Weather { rain: 1.0, storm: 1.0, night: 1.0 };
        for bed in [Bed::Wind, Bed::Rain, Bed::Insects] {
            assert_eq!(levels(&s, &storm, EARTH_AIR)[bed as usize], 0.0, "{bed:?}");
        }
        s.altitude_m = 80_000.0;
        assert!(levels(&s, &storm, EARTH_AIR)[Bed::Suit as usize] > 0.6);
        s.altitude_m = 4_500.0;
        assert!(levels(&s, &DAY, EARTH_AIR)[Bed::Wind as usize] < 0.4);
        // Terrain unloads by 6 km and the globe takes over; nothing of the surface reaches it.
        for altitude in [6_000.0, 20_000.0, 30_000.0] {
            s.altitude_m = altitude;
            let beds = levels(&s, &storm, EARTH_AIR);
            for bed in [Bed::Wind, Bed::Rain, Bed::Insects] {
                assert_eq!(beds[bed as usize], 0.0, "{bed:?} at {altitude} m");
            }
        }
        s.altitude_m = 0.0;
        assert_eq!(levels(&s, &DAY, EARTH_AIR)[Bed::Suit as usize], 0.0);
    }

    #[test]
    fn the_small_planet_goes_quiet_below_its_limb() {
        let mut s = probe(open_field, EYE, 1.0, 0.0);
        s.altitude_m = 3_000.0;
        assert!(levels(&s, &DAY, PLANET_AIR)[Bed::Wind as usize] < 0.12);
        s.altitude_m = 5_000.0;
        assert!(levels(&s, &DAY, PLANET_AIR)[Bed::Suit as usize] > 0.6);
        s.altitude_m = 6_000.0;
        assert_eq!(levels(&s, &DAY, PLANET_AIR)[Bed::Wind as usize], 0.0);
        s.altitude_m = 0.0;
        assert!(levels(&s, &DAY, PLANET_AIR)[Bed::Wind as usize] > 0.2);
    }
}
