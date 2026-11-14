//! Shell maze whose radius swells and narrows along an open axis.
use crate::{hash_u32, BlockId, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec3};

use super::CENTER;

pub const VERSION: u32 = 104;

/// Halls on a shell that fills the world height. Versions through 106 keep the small shell.
pub const VAST: u32 = 107;

/// Spiral drum with avenues, colonnades and a gallery. Versions through 108 keep the vast bays.
pub const GRAND: u32 = 109;

const MEAN: f32 = 24.0;
const SWING: f32 = 8.0;
/// Slow enough that the floor slope stays near nine degrees.
const WAVE: f32 = 0.02;
const CRUST: f32 = 2.0;
const WALL: f32 = 10.0;
const SECTORS: i32 = 16;
const ROW: f32 = 8.0;
const THICK: f32 = 0.85;
const GATE_SECTOR: i32 = 4;
const GATE_ROW: i32 = 1;

struct Shape {
    mean: f32,
    swing: f32,
    wave: f32,
    crust: f32,
    wall: f32,
    sectors: i32,
    row: f32,
    thick: f32,
    bulge: f32,
    bulge_k: f32,
    bulge_phase: f32,
    center: Vec3,
}

fn crust_thickness(version: u32) -> f32 {
    // Half a metre of rind falls between voxel centres and leaves holes.
    if version >= super::maze::BEND {
        4.0
    } else {
        CRUST
    }
}

fn shape(version: u32) -> Shape {
    if version >= GRAND {
        // Radius 75–275 m on y = 224. The waist stays narrow enough to see across.
        // Bays are 320 m along the axis. A second harmonic keeps the swell from a pure wave.
        Shape {
            mean: 175.0,
            swing: 90.0,
            wave: 0.0008,
            crust: crust_thickness(version),
            wall: 120.0,
            sectors: 6,
            row: 320.0,
            thick: 10.0,
            bulge: 10.0,
            bulge_k: 2.6,
            bulge_phase: 0.7,
            center: Vec3::new(0.0, 224.0, 0.0),
        }
    } else if version >= VAST {
        // Mean radius 200 m, swell 130–270 m. The crest stays inside y = -64..512
        // when the axis sits at y = 224. Bays are 96 m along the axis.
        Shape {
            mean: 200.0,
            swing: 70.0,
            wave: 0.002,
            crust: crust_thickness(version),
            wall: 64.0,
            sectors: 12,
            row: 96.0,
            thick: 5.0,
            bulge: 0.0,
            bulge_k: 1.0,
            bulge_phase: 0.0,
            center: Vec3::new(0.0, 224.0, 0.0),
        }
    } else {
        Shape {
            mean: MEAN,
            swing: SWING,
            wave: WAVE,
            crust: crust_thickness(version),
            wall: WALL,
            sectors: SECTORS,
            row: ROW,
            thick: THICK,
            bulge: 0.0,
            bulge_k: 1.0,
            bulge_phase: 0.0,
            center: CENTER,
        }
    }
}

fn radius_of(s: &Shape, z: f32) -> f32 {
    s.mean + s.swing * (z * s.wave).sin()
        + s.bulge * (z * s.wave * s.bulge_k + s.bulge_phase).sin()
}

#[cfg(test)]
fn slope_of(s: &Shape, z: f32) -> f32 {
    s.swing * s.wave * (z * s.wave).cos()
        + s.bulge * s.bulge_k * s.wave * (z * s.wave * s.bulge_k + s.bulge_phase).cos()
}

fn pitch_of(sectors: i32) -> f32 {
    std::f32::consts::TAU / sectors as f32
}

pub fn radius(z: f32) -> f32 {
    radius_of(&shape(VERSION), z)
}

pub(crate) fn pitch_for(version: u32) -> f32 {
    pitch_of(shape(version).sectors)
}

pub(crate) fn row_len(version: u32) -> f32 {
    shape(version).row
}

/// Ten bays. The small shell's districts stay 80 m long.
pub(super) fn district_span(version: u32) -> f32 {
    shape(version).row * 10.0
}

pub(crate) fn center_of(version: u32) -> Vec3 {
    shape(version).center
}

pub(crate) fn shell_radius(version: u32, z: f32) -> f32 {
    radius_of(&shape(version), z)
}

/// Centre, then mean, swing, wave, bulge, bulge frequency, bulge phase, crust.
pub(crate) fn wave_terms(version: u32) -> (Vec3, [f32; 7]) {
    let s = shape(version);
    (
        s.center,
        [s.mean, s.swing, s.wave, s.bulge, s.bulge_k, s.bulge_phase, s.crust],
    )
}

/// Metres from the axis to the outside of the crust, plus a margin for the lip.
pub(crate) fn outer_reach(version: u32) -> f32 {
    let s = shape(version);
    s.mean + s.swing + s.bulge + s.crust + 4.0
}

pub fn acceleration(point_m: Vec3) -> Vec3 {
    field(VERSION, point_m)
}

pub(super) fn field(version: u32, point_m: Vec3) -> Vec3 {
    let s = shape(version);
    let p = point_m - s.center;
    let radial = Vec3::new(p.x, p.y, 0.0);
    let dist = radial.length();
    let radius = radius_of(&s, p.z);
    if dist < 0.05 || dist > radius + s.crust + 4.0 {
        return Vec3::ZERO;
    }
    radial * (28.0 / radius)
}

pub fn socket(version: u32) -> Vec3 {
    let s = shape(version);
    let z = GATE_ROW as f32 * s.row + s.row * 0.5 - 1.8;
    place(s.center, z, (GATE_SECTOR as f32 + 0.5) * pitch_of(s.sectors), radius_of(&s, z) - 0.8)
}

pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    let s = shape(cfg.generator_version);
    let z = s.row * 0.5;
    place(s.center, z, 0.5 * pitch_of(s.sectors), radius_of(&s, z) - 1.6) / cfg.voxel_meters()
}

pub fn column(cfg: &WorldSampleConfig, _x: i32, _z: i32) -> ColumnSample {
    let s = shape(cfg.generator_version);
    let height = cfg.blocks_for_height(s.center.y + s.mean + s.swing + s.bulge + s.crust);
    let surface = if cfg.generator_version >= super::dream::VERSION {
        BlockId::Limestone
    } else {
        BlockId::WhitePlaster
    };
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone,
        height,
        surface,
        filler: surface,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

pub(crate) struct Sample {
    pub above: f32,
    pub sector: i32,
    pub row: i32,
    pub local_a: f32,
    pub local_z: f32,
    pub z: f32,
    pub class: Class,
}

pub(crate) enum Class {
    Hollow,
    Crust,
    Bedrock,
    Wall,
}

pub(crate) fn sample(cfg: &WorldSampleConfig, cell: IVec3) -> Sample {
    let s = shape(cfg.generator_version);
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters() - s.center;
    let radial = Vec3::new(p.x, p.y, 0.0);
    let dist = radial.length();
    let radius = radius_of(&s, p.z);
    let above = dist - radius;
    let grand = cfg.generator_version >= GRAND;
    let raw = if dist < 0.05 {
        0.0
    } else {
        radial.x.atan2(-radial.y).rem_euclid(std::f32::consts::TAU)
    };
    let angle = if grand { (raw + super::maze::twist(cfg.generator_version, p.z)).rem_euclid(std::f32::consts::TAU) } else { raw };
    let sector_pitch = pitch_of(s.sectors);
    let sector = (angle / sector_pitch).floor() as i32 % s.sectors;
    let row = (p.z / s.row).floor() as i32;
    let local_a = angle - sector as f32 * sector_pitch;
    let local_z = p.z - row as f32 * s.row;
    let wall_h = if grand { super::grand::clearance(radius) } else { s.wall };
    // The field maze owns the interior. Versions through 111 keep the colonnade.
    let field = cfg.generator_version >= super::maze::FIELD;
    let rind = if cfg.generator_version >= super::maze::BEND { 2.0 } else { 0.5 };
    let class = if dist < 0.05 {
        Class::Hollow
    } else if (s.crust - rind..s.crust).contains(&above) {
        Class::Bedrock
    } else if (0.0..s.crust).contains(&above) {
        Class::Crust
    } else if grand && !field && super::grand::relief(above, sector, row, local_a, local_z, sector_pitch, s.row).is_some() {
        Class::Crust
    } else if !(-wall_h..0.0).contains(&above) {
        Class::Hollow
    } else {
        let a_thick = s.thick / radius.max(12.0);
        let around = local_a < a_thick && around_closed(cfg.seed, sector, row, s.sectors);
        let along = local_z < s.thick && along_closed(cfg.seed, sector, row, s.sectors);
        // A plug across one corridor. Docking clears it; the tree around it stays open.
        let plug = sector == GATE_SECTOR
            && row == GATE_ROW
            && (local_z - s.row * 0.5).abs() < 0.55
            && local_a > a_thick
            && local_a < sector_pitch - a_thick;
        let maze = if field {
            plug
        } else if grand && !plug {
            super::grand::keeps_divider(
                around, along, sector, s.sectors, row, local_a, local_z, above, wall_h, radius,
            )
        } else {
            around || along || plug
        };
        if maze { Class::Wall } else { Class::Hollow }
    };
    Sample { above, sector, row, local_a, local_z, z: p.z, class }
}

pub fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    let sample = sample(cfg, cell);
    if let Some(block) = super::maze::dress(cfg.generator_version, cfg.seed, &sample) {
        return block;
    }
    if cfg.generator_version >= super::maze::FIELD {
        return match sample.class {
            Class::Bedrock => BlockId::Bedrock,
            Class::Crust | Class::Wall => BlockId::WhitePlaster,
            Class::Hollow => BlockId::Air,
        };
    }
    if cfg.generator_version >= super::expedition::VERSION {
        return super::expedition::dress(cfg, &sample);
    }
    if cfg.generator_version >= super::dream::VERSION {
        return super::dream::dress(cfg, &sample);
    }
    match sample.class {
        Class::Bedrock => BlockId::Bedrock,
        Class::Crust | Class::Wall => BlockId::WhitePlaster,
        Class::Hollow => BlockId::Air,
    }
}

pub fn passage(cfg: &WorldSampleConfig) -> Vec<(IVec3, BlockId)> {
    let s = shape(cfg.generator_version);
    let z = GATE_ROW as f32 * s.row + s.row * 0.5;
    let mid = place(s.center, z, (GATE_SECTOR as f32 + 0.5) * pitch_of(s.sectors), radius_of(&s, z) - 2.0);
    let lo = ((mid - Vec3::splat(4.0)) / cfg.voxel_meters()).floor().as_ivec3();
    let hi = ((mid + Vec3::splat(4.0)) / cfg.voxel_meters()).ceil().as_ivec3();
    let mut cells = Vec::new();
    for y in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            for cz in lo.z..=hi.z {
                let cell = IVec3::new(x, y, cz);
                let expected = block(cfg, cell);
                if expected.is_solid() && expected != BlockId::Bedrock {
                    cells.push((cell, expected));
                }
            }
        }
    }
    cells
}

fn place(center: Vec3, z: f32, angle: f32, dist: f32) -> Vec3 {
    center + Vec3::new(angle.sin() * dist, -angle.cos() * dist, z)
}

pub(super) fn pitch() -> f32 {
    pitch_of(SECTORS)
}

fn opens_around(seed: u32, sector: i32, row: i32) -> bool {
    hash_u32(
        seed ^ (sector as u32).wrapping_mul(0x9E37_79B9) ^ (row as u32).wrapping_mul(0x85EB_CA6B),
    ) & 1
        == 0
}

fn extra(seed: u32, sector: i32, row: i32) -> bool {
    hash_u32(
        seed ^ (sector as u32).wrapping_mul(0x27D4_EB2D) ^ (row as u32).wrapping_mul(0x1656_67B1),
    ) % 5
        == 0
}

fn around_closed(seed: u32, sector: i32, row: i32, sectors: i32) -> bool {
    let prev = (sector - 1).rem_euclid(sectors);
    !opens_around(seed, prev, row) && !extra(seed, prev, row)
}

fn along_closed(seed: u32, sector: i32, row: i32, sectors: i32) -> bool {
    let behind = row - 1;
    opens_around(seed, sector.rem_euclid(sectors), behind) && !extra(seed, sector, behind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldType;
    use engine_core::VoxelScalePreset;

    fn world(version: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version: version,
            seed: 424242,
            world_type: WorldType::Manyfold,
            voxel_scale: VoxelScalePreset::OneMeter,
        }
    }

    fn at(cfg: &WorldSampleConfig, p: Vec3) -> BlockId {
        let cell = (p / cfg.voxel_meters() - Vec3::splat(0.5)).floor().as_ivec3();
        super::super::block(cfg, cell)
    }

    #[test]
    fn the_radius_swells_and_narrows() {
        let crest = std::f32::consts::FRAC_PI_2 / WAVE;
        assert!((radius(0.0) - 24.0).abs() < 1e-3);
        assert!((radius(crest) - 32.0).abs() < 0.05, "crest {}", radius(crest));
        assert!((radius(-crest) - 16.0).abs() < 0.05, "throat {}", radius(-crest));
        let slope = SWING * WAVE;
        assert!(slope < 0.2, "floor slope {slope} is too steep to walk");
    }

    #[test]
    fn gravity_follows_the_local_radius() {
        let floor = CENTER + Vec3::new(0.0, -radius(0.0), 0.0);
        let g = acceleration(floor);
        assert!((g.length() - 28.0).abs() < 0.05, "|g| {}", g.length());
        assert!(g.y < -27.0, "down at the bottom is outward, got {g}");
        assert!(acceleration(CENTER).length() < 0.01);
        let narrow = -std::f32::consts::FRAC_PI_2 / WAVE;
        let side = CENTER + Vec3::new(radius(narrow), 0.0, narrow);
        let gs = acceleration(side);
        assert!((gs.length() - 28.0).abs() < 0.05, "|g| at the throat {}", gs.length());
        assert!(gs.x > 27.0, "down on the side follows the shell, got {gs}");
        let old = Vec3::new(0.0, 16.0, 0.0);
        assert_eq!(super::super::acceleration_for(101, old), super::super::acceleration(old));
    }

    #[test]
    fn the_shell_continues_and_older_gates_do_not() {
        let cfg = world(VERSION);
        let floor = CENTER + Vec3::new(0.0, -(radius(0.0) + 0.4), 0.0);
        assert_eq!(at(&cfg, floor), BlockId::WhitePlaster);
        let crest = std::f32::consts::FRAC_PI_2 / WAVE;
        let high = CENTER + Vec3::new(0.0, -(radius(crest) + 0.4), crest);
        assert_eq!(at(&cfg, high), BlockId::WhitePlaster, "no floor where the shell is widest");
        let old = world(super::super::INFINITE_MAZE);
        assert_eq!(at(&old, floor), BlockId::Air);
        let tube = world(super::super::TALL_WHITE_WALLS);
        assert_eq!(at(&tube, high), BlockId::Air);
    }

    #[test]
    fn spawn_stands_in_air_on_the_floor() {
        let cfg = world(VERSION);
        let feet = spawn(&cfg).floor().as_ivec3();
        assert!(!super::super::block(&cfg, feet).is_solid(), "spawn {feet} is inside a wall");
        let metres = (feet.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
        let radial = Vec3::new(metres.x - CENTER.x, metres.y - CENTER.y, 0.0).normalize_or_zero();
        assert_eq!(at(&cfg, metres + radial * 2.0), BlockId::WhitePlaster);
    }

    #[test]
    fn the_weight_gate_blocks_one_corridor() {
        let cfg = world(VERSION);
        let z = GATE_ROW as f32 * ROW + ROW * 0.5;
        let angle = (GATE_SECTOR as f32 + 0.5) * pitch();
        let plug = place(CENTER, z, angle, radius(z) - 2.0);
        assert_eq!(at(&cfg, plug), BlockId::WhitePlaster);
        let open = place(CENTER, z - 2.2, angle, radius(z - 2.2) - 2.0);
        assert_eq!(at(&cfg, open), BlockId::Air, "the socket stands in the corridor");
        assert!(!passage(&cfg).is_empty());
    }

    #[test]
    fn vast_halls_fill_the_world_and_older_shells_stay_small() {
        let s = shape(VAST);
        let crest = std::f32::consts::FRAC_PI_2 / s.wave;
        let outer = radius_of(&s, crest) + s.crust;
        assert!((radius_of(&s, 0.0) - 200.0).abs() < 1e-3);
        assert!((radius_of(&s, crest) - 270.0).abs() < 0.05, "crest {}", radius_of(&s, crest));
        assert!((radius_of(&s, -crest) - 130.0).abs() < 0.05, "throat {}", radius_of(&s, -crest));
        assert!(s.center.y + outer < 512.0, "crest clears the world top");
        assert!(s.center.y - outer > -64.0, "throat clears the world floor");
        assert!(s.swing * s.wave < 0.2, "floor slope is too steep to walk");
        let arc = pitch_of(s.sectors) * s.mean;
        assert!(arc > 90.0 && s.row >= 90.0 && s.wall >= 60.0, "bay {arc} x {} x {}", s.row, s.wall);

        let cfg = world(VAST);
        let z = GATE_ROW as f32 * s.row + s.row * 0.5 - 10.0;
        let angle = (GATE_SECTOR as f32 + 0.5) * pitch_of(s.sectors);
        let radius = radius_of(&s, z);
        let open = place(s.center, z, angle, radius - 30.0);
        assert_eq!(at(&cfg, open), BlockId::Air, "hall centre is walled in");
        for (dz, da) in [(0.0, 0.0), (15.0, 0.0), (-15.0, 0.0), (0.0, 15.0 / radius), (0.0, -15.0 / radius)] {
            let step = place(s.center, z + dz, angle + da, radius_of(&s, z + dz) - 10.0);
            assert_eq!(at(&cfg, step), BlockId::Air, "open volume at {dz},{da}");
        }
        let plug_z = z + 10.0;
        let plug = place(s.center, plug_z, angle, radius_of(&s, plug_z) - 40.0);
        assert!(at(&cfg, plug).is_solid(), "the gate wall is only a low partition");
        let over = place(s.center, plug_z, angle, radius_of(&s, plug_z) - 70.0);
        assert_eq!(at(&cfg, over), BlockId::Air, "the hollow above the wall is filled");
        let floor = s.center + Vec3::new(0.0, -(radius_of(&s, 0.0) + 0.4), 0.0);
        assert!(at(&cfg, floor).is_solid(), "no floor on the vast shell");
        let g = field(VAST, s.center + Vec3::new(0.0, -radius_of(&s, 0.0), 0.0));
        assert!((g.length() - 28.0).abs() < 0.05, "|g| {}", g.length());
        let feet = spawn(&cfg).floor().as_ivec3();
        assert!(!super::super::block(&cfg, feet).is_solid(), "spawn {feet} is inside a wall");
        assert_eq!(at(&world(super::super::dream::VERSION), floor), BlockId::Air);
        assert!((super::radius(0.0) - 24.0).abs() < 1e-3);
    }

    #[test]
    fn the_grand_drum_opens_a_vista_and_keeps_the_vast_shell() {
        use super::super::grand;
        let s = shape(GRAND);
        let waist_z = -std::f32::consts::FRAC_PI_2 / s.wave;
        let crest_z = std::f32::consts::FRAC_PI_2 / s.wave;
        let waist = radius_of(&s, waist_z);
        let crest = radius_of(&s, crest_z);
        let outer = crest + s.bulge + s.crust;
        assert!(waist < 110.0, "waist {waist} hides the far floor");
        assert!(crest > 250.0, "crest {crest}");
        assert!(s.center.y + outer < 512.0 && s.center.y - outer > -64.0);
        assert!(slope_of(&s, 0.0).abs() < 0.2 && slope_of(&s, waist_z).abs() < 0.2);
        assert!(s.row >= 300.0 && pitch_of(s.sectors) * s.mean > 160.0);

        let cfg = world(GRAND);
        let near = place(s.center, waist_z, -grand::twist(waist_z), waist + 0.4);
        let far = place(s.center, waist_z, std::f32::consts::PI - grand::twist(waist_z), waist + 0.4);
        assert!(at(&cfg, near).is_solid() && at(&cfg, far).is_solid(), "the drum has no far floor");
        let well = place(
            s.center,
            waist_z,
            -grand::twist(waist_z),
            waist - grand::clearance(waist) - 8.0,
        );
        assert_eq!(at(&cfg, well), BlockId::Air, "the waist is walled shut");

        let z = s.row + 2.0;
        let avenue = place(
            s.center,
            z,
            0.5 * pitch_of(s.sectors) - grand::twist(z),
            radius_of(&s, z) - 20.0,
        );
        assert_eq!(at(&cfg, avenue), BlockId::Air, "the avenue is crossed by a wall");

        let mut pier = false;
        let mut opening = false;
        for sector in 0..s.sectors {
            for k in 0..8 {
                let local_z = 16.0 + k as f32 * 36.0;
                let z = s.row * 5.0 + local_z;
                let angle = sector as f32 * pitch_of(s.sectors) + 0.01 - grand::twist(z);
                let p = place(s.center, z, angle, radius_of(&s, z) - 15.0);
                if at(&cfg, p).is_solid() { pier = true; } else { opening = true; }
            }
        }
        assert!(pier && opening, "colonnade pier={pier} opening={opening}");

        let deck_z = 24.0;
        let deck = place(
            s.center,
            deck_z,
            pitch_of(s.sectors) + 0.2 - grand::twist(deck_z),
            radius_of(&s, deck_z) - 36.8,
        );
        assert!(at(&cfg, deck).is_solid(), "no gallery");
        let eye = place(
            s.center,
            s.row * 0.5,
            1.5 * pitch_of(s.sectors) - grand::twist(s.row * 0.5),
            radius_of(&s, s.row * 0.5) - 36.8,
        );
        assert_eq!(at(&cfg, eye), BlockId::Air, "the gallery has no opening");

        let feet = spawn(&cfg).floor().as_ivec3();
        assert!(!super::super::block(&cfg, feet).is_solid(), "spawn {feet} is inside a wall");
        assert!((shell_radius(VAST, 0.0) - 200.0).abs() < 1e-3);
        let old = world(VAST);
        assert_eq!(at(&old, near), BlockId::Air);
    }
}
