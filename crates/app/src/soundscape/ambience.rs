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
const WATER_RINGS_M: [f32; 2] = [4.0, 12.0];
/// Depth below the eye searched for water in each ring column, metres.
const WATER_DEPTH_M: f32 = 8.0;
/// Rock overhead (metres) between a roofed room and a cave.
const ROCK_ROOF_M: (f32, f32) = (3.0, 10.0);

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
    /// Eye height above sea level, metres.
    pub altitude_m: f32,
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

/// Probes about 360 cells at roughly one-metre spacing, whatever the voxel size.
pub fn probe(get: impl Fn(IVec3) -> BlockId, eye: IVec3, meters: f32, sea_level_m: f32) -> Surroundings {
    let meters = meters.max(1.0e-3);
    let stride = (1.0 / meters).round().max(1.0) as i32;
    let cells = |m: f32| ((m / meters) as i32 / stride).max(1);
    let spread = (SKY_SPREAD_M / meters).round() as i32;
    let roofed = |column: IVec3, rock: &mut u32| {
        let mut covered = false;
        for k in 1..=cells(SKY_REACH_M) {
            if get(column + IVec3::Y * (k * stride)).is_opaque() {
                covered = true;
                *rock += 1;
            }
        }
        covered
    };
    let mut rock = 0;
    let mut open = u32::from(!roofed(eye, &mut rock));
    let mut ignored = 0;
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        open += u32::from(!roofed(eye + IVec3::new(dx * spread, 0, dz * spread), &mut ignored));
    }
    let walls = DIRECTIONS
        .iter()
        .filter(|&&(dx, dz)| {
            (1..=cells(WALL_REACH_M)).any(|k| get(eye + IVec3::new(dx, 0, dz) * (k * stride)).is_opaque())
        })
        .count();
    let mut water = 0.0;
    for (ring, weight) in WATER_RINGS_M.iter().zip([0.5, 0.25]) {
        let r = (ring / meters).round() as i32;
        for (dx, dz) in DIRECTIONS {
            let column = eye + IVec3::new(dx * r, 0, dz * r);
            if (-cells(WATER_DEPTH_M)..=1).any(|k| get(column + IVec3::Y * (k * stride)) == BlockId::Water) {
                water += weight;
            }
        }
    }
    let rock_m = rock as f32 * stride as f32 * meters;
    Surroundings {
        sky: open as f32 / 5.0,
        underground: smoothstep(ROCK_ROOF_M.0, ROCK_ROOF_M.1, rock_m) * (0.4 + 0.6 * walls as f32 / 8.0),
        water: f32::min(water / 2.0, 1.0),
        altitude_m: eye.y as f32 * meters - sea_level_m,
    }
}

/// Target level of each bed for the surroundings and weather.
pub fn levels(s: &Surroundings, w: &Weather) -> [f32; BED_COUNT] {
    let surface = 1.0 - s.underground;
    let height = smoothstep(0.0, 90.0, s.altitude_m);
    let mut out = [0.0; BED_COUNT];
    out[Bed::Wind as usize] = (s.sky * (0.25 + 0.5 * height + 0.35 * w.storm)).min(1.0) * surface;
    out[Bed::Cave as usize] = s.underground * (1.0 - s.sky);
    out[Bed::Water as usize] = s.water * (1.0 - 0.5 * s.underground);
    out[Bed::Rain as usize] = w.rain * (0.3 + 0.7 * s.sky) * surface;
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

    const EYE: IVec3 = IVec3::new(0, 1, 0);
    const DAY: Weather = Weather { rain: 0.0, storm: 0.0, night: 0.0 };

    #[test]
    fn open_sky_plays_wind_and_no_cave() {
        let s = probe(open_field, EYE, 1.0, 0.0);
        assert_eq!((s.sky, s.underground, s.water), (1.0, 0.0, 0.0));
        let beds = levels(&s, &DAY);
        assert!(beds[Bed::Wind as usize] > 0.2);
        assert_eq!(beds[Bed::Cave as usize], 0.0);
        let night = levels(&s, &Weather { night: 1.0, ..DAY });
        assert!(night[Bed::Insects as usize] > 0.5);
        let rain = levels(&s, &Weather { rain: 1.0, storm: 1.0, night: 1.0 });
        assert!(rain[Bed::Rain as usize] > 0.9 && rain[Bed::Insects as usize] == 0.0);
        assert!(rain[Bed::Wind as usize] > beds[Bed::Wind as usize]);
    }

    #[test]
    fn a_cave_plays_the_drone_and_silences_the_surface_beds() {
        let s = probe(cave, EYE, 1.0, 0.0);
        assert_eq!(s.sky, 0.0);
        assert!(s.underground > 0.95, "underground {}", s.underground);
        let beds = levels(&s, &Weather { rain: 1.0, storm: 1.0, night: 1.0 });
        assert!(beds[Bed::Cave as usize] > 0.95);
        for bed in [Bed::Wind, Bed::Rain, Bed::Insects] {
            assert!(beds[bed as usize] < 0.05, "{bed:?} {}", beds[bed as usize]);
        }
    }

    #[test]
    fn a_shore_plays_water_and_scale_keeps_the_answer() {
        let s = probe(lakeshore, EYE, 1.0, 0.0);
        assert!(s.water > 0.3, "water {}", s.water);
        assert!(levels(&s, &DAY)[Bed::Water as usize] > 0.3);
        assert_eq!(probe(open_field, EYE, 1.0, 0.0).water, 0.0);
        // Quarter-metre voxels: same field, coordinates four times larger.
        let quarter = probe(|p| lakeshore(IVec3::new(p.x.div_euclid(4), p.y.div_euclid(4), p.z.div_euclid(4))), EYE * 4, 0.25, 0.0);
        assert!((quarter.water - s.water).abs() < 0.3 && quarter.sky == 1.0);
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
        let low = levels(&s, &DAY)[Bed::Wind as usize];
        s.altitude_m = 120.0;
        assert!(levels(&s, &DAY)[Bed::Wind as usize] > low * 2.0);
    }
}
