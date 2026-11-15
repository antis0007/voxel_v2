//! Drifting air matter for the sunless realms: ash, embers and spores in Hell,
//! frost glitter on the cold Nether plain, slow dust in the Manyfold and Hall,
//! and sand, gilt glints, grotto mist and shaft dust in the Gilded Cave.
//! Each place also carries a rarer accent (tumbling gold leaf, ash flakes,
//! petals, prism glints), and seeded squalls (`portals::realm::squall`) blow a
//! second, faster layer through Hell's biomes and the Highfold.
use super::Model;
use crate::player::Player;
use crate::render::ChunkCache;
use crate::world::{BlockId, World, WorldType, chunk_coord_from_world_block};
use engine_core::hash::hash_u32;
use engine_worldgen::nether::Biome;
use glam::{DVec2, Vec3};

const TILE_M: f32 = 4.0;
const RADIUS_M: f32 = 18.0;
const COLUMN_M: f32 = 16.0;

/// How a mote's brightness moves over time.
#[derive(Clone, Copy)]
enum Twinkle {
    /// Holds steady: dust, ash, grit.
    Steady,
    /// Breathes in and out: foxfire, fireflies.
    Pulse,
    /// Dim most of the time, with a brief sharp flash: a gilt fleck or an ice
    /// crystal turning into the light.
    Glint,
    /// Kindles, cools from the mote colour toward `cool` and dies out; a new
    /// one kindles in its place. Alpha is zero at both ends of the life, so
    /// the restart never pops.
    Burn { cool: [f32; 3] },
    /// A flat flake flipping as it falls: the diamond narrows edge-on and
    /// flashes when it turns face-on. Gold leaf, petals, ash flakes.
    Tumble,
}

/// One family of particles. `rise` is metres per second, negative to fall.
#[derive(Clone, Copy)]
struct Mote {
    /// Share of the four slots per tile that carry this family.
    share: f32,
    rise: f32,
    sway: f32,
    radius: f32,
    color: [f32; 3],
    glow: f32,
    alpha: f32,
    twinkle: Twinkle,
}

const ASH: Mote = Mote { share: 0.55, rise: -0.42, sway: 0.34, radius: 0.020, color: [0.36, 0.33, 0.32], glow: 0.05, alpha: 0.70, twinkle: Twinkle::Steady };
const EMBER: Mote = Mote { share: 0.22, rise: 1.15, sway: 0.22, radius: 0.013, color: [1.0, 0.66, 0.28], glow: 1.6, alpha: 0.90, twinkle: Twinkle::Burn { cool: [0.62, 0.10, 0.04] } };
const SPORE: Mote = Mote { share: 0.60, rise: 0.10, sway: 0.50, radius: 0.016, color: [0.95, 0.74, 0.30], glow: 0.55, alpha: 0.55, twinkle: Twinkle::Steady };
const GRIT: Mote = Mote { share: 0.30, rise: -0.25, sway: 0.60, radius: 0.012, color: [0.20, 0.19, 0.20], glow: 0.03, alpha: 0.60, twinkle: Twinkle::Steady };
const FROST: Mote = Mote { share: 0.40, rise: -0.18, sway: 0.40, radius: 0.012, color: [0.70, 0.86, 0.95], glow: 0.35, alpha: 0.60, twinkle: Twinkle::Glint };
const FOXFIRE_SPORE: Mote = Mote { share: 0.05, rise: 0.06, sway: 0.45, radius: 0.010, color: [0.36, 0.95, 0.78], glow: 0.9, alpha: 0.6, twinkle: Twinkle::Pulse };
const DUST: Mote = Mote { share: 0.38, rise: 0.035, sway: 0.28, radius: 0.011, color: [0.78, 0.74, 0.66], glow: 0.22, alpha: 0.38, twinkle: Twinkle::Steady };
/// Gilded Cave: fine sand hanging in the lamplight.
const SAND: Mote = Mote { share: 0.30, color: [0.86, 0.70, 0.48], ..DUST };
/// Gilded Cave: flakes of gilt that flash as they turn.
const GILT: Mote = Mote { share: 0.06, rise: -0.02, sway: 0.24, radius: 0.009, color: [1.0, 0.84, 0.42], glow: 1.3, alpha: 0.80, twinkle: Twinkle::Glint };
/// Gilded grotto: cold damp haze over the pool.
const MIST: Mote = Mote { share: 0.24, rise: 0.02, sway: 0.36, radius: 0.012, color: [0.56, 0.80, 0.88], glow: 0.28, alpha: 0.30, twinkle: Twinkle::Steady };

/// Dust drifting through the beam from the grotto dome.
const SHAFT_MOTES: u32 = 144;
/// A shade warmer than `REALM_SHAFT_COLOR` (`realm_light.wgsl`), so lit dust
/// still reads against the beam itself.
const SHAFT_MOTE_COLOR: [f32; 3] = [1.0, 0.97, 0.88];
const SHAFT_RANGE_M: f32 = 56.0;
/// Half a pixel's view angle at 720p and a 70° field: a diamond of this radius
/// per metre of distance covers about one pixel.
const PIXEL_RADIUS_PER_M: f32 = 0.0011;

/// The two families a place carries; slots alternate between them. `pick` is
/// a per-tile 0..1 draw that dithers the boundary between moods.
fn families(world: &World, x_m: f32, z_m: f32, pick: f32) -> [Mote; 2] {
    let cfg = world.sample_config();
    match world.world_type {
        WorldType::Nether if cfg.is_hell() => match engine_worldgen::nether::biome(&cfg, x_m, z_m) {
            Some(Biome::FungalHollows) => [SPORE, Mote { share: 0.15, ..ASH }],
            Some(Biome::CinderRifts) => [Mote { share: 0.55, ..EMBER }, Mote { share: 0.35, ..ASH }],
            Some(Biome::BasaltScarps) => [GRIT, Mote { share: 0.08, ..EMBER }],
            Some(Biome::AshBarrens) => [Mote { share: 0.85, ..ASH }, Mote { share: 0.10, ..EMBER }],
            None => [ASH, EMBER],
        },
        WorldType::Nether => [FROST, Mote { share: 0.0, ..FROST }],
        // Lightless: fine grit sifting from the roof and rare foxfire spores, the only
        // motes that show without a lamp.
        WorldType::Realm(engine_voxel::RealmKind::Lightless) => [Mote { share: 0.22, alpha: 0.45, ..GRIT }, FOXFIRE_SPORE],
        // Highfold: pollen blown off the island tops and the odd pale seed tuft.
        WorldType::Realm(engine_voxel::RealmKind::Highfold) => [
            Mote { share: 0.18, rise: 0.12, sway: 0.75, color: [0.96, 0.92, 0.70], glow: 0.15, alpha: 0.40, ..DUST },
            Mote { share: 0.06, rise: 0.25, sway: 0.9, radius: 0.02, color: [1.0, 1.0, 0.96], glow: 0.1, alpha: 0.5, ..DUST },
        ],
        // Gloaming: fireflies drifting low between the trunks, and spores.
        WorldType::Realm(engine_voxel::RealmKind::Gloaming) => [
            Mote { share: 0.20, rise: 0.05, sway: 0.9, radius: 0.016, color: [1.0, 0.86, 0.38], glow: 1.6, alpha: 0.85, twinkle: Twinkle::Pulse },
            Mote { share: 0.12, rise: -0.04, sway: 0.4, radius: 0.012, color: [0.62, 0.52, 0.80], glow: 0.25, alpha: 0.45, twinkle: Twinkle::Steady },
        ],
        // Gilded Cave, warm to cold along the procession: sand and the odd gilt
        // fleck in the gallery; thick golden dust and many glints over the coin
        // dunes; cold mist and foxfire spores in the grotto.
        WorldType::Realm(engine_voxel::RealmKind::Gilded) => {
            let (hoard, grotto) = engine_worldgen::gilded::mood(world.seed, Vec3::new(x_m, 0.0, z_m));
            if grotto > pick {
                [MIST, Mote { share: 0.12, ..FOXFIRE_SPORE }]
            } else if hoard > pick {
                [Mote { share: 0.44, color: [0.94, 0.74, 0.44], glow: 0.30, ..SAND }, Mote { share: 0.18, ..GILT }]
            } else {
                [SAND, GILT]
            }
        }
        _ => [DUST, Mote { share: 0.0, ..DUST }],
    }
}

const NONE: Mote = Mote { share: 0.0, ..DUST };
/// Gold leaf: a flake of gilt big enough to see flip.
const LEAF: Mote = Mote { share: 0.07, rise: -0.16, sway: 0.55, radius: 0.017, color: [1.0, 0.80, 0.38], glow: 0.9, alpha: 0.85, twinkle: Twinkle::Tumble };
/// Hell ash flake: a scrap of burnt ash, broad and dark.
const FLAKE: Mote = Mote { share: 0.14, rise: -0.30, sway: 0.70, radius: 0.026, color: [0.30, 0.27, 0.26], glow: 0.04, alpha: 0.80, twinkle: Twinkle::Tumble };
/// Squall ash: driven, pale with the sky's glow.
const STORM_ASH: Mote = Mote { share: 1.0, rise: -0.25, sway: 0.5, radius: 0.034, color: [0.62, 0.55, 0.50], glow: 0.30, alpha: 0.90, twinkle: Twinkle::Steady };
/// Fungal Hollows: a soft amber puff that breathes.
const PUFF: Mote = Mote { share: 0.09, rise: 0.05, sway: 0.40, radius: 0.034, color: [1.0, 0.70, 0.30], glow: 0.85, alpha: 0.30, twinkle: Twinkle::Pulse };

/// The accent family for the two extra slots per tile. Accents are larger and
/// rarer than the base motes: flipping flakes, puffs, prism glints.
fn accent(world: &World, x_m: f32, z_m: f32, pick: f32) -> Mote {
    let cfg = world.sample_config();
    match world.world_type {
        WorldType::Nether if cfg.is_hell() => match engine_worldgen::nether::biome(&cfg, x_m, z_m) {
            Some(Biome::FungalHollows) => PUFF,
            Some(Biome::CinderRifts) => Mote { share: 0.10, rise: 0.55, color: [0.42, 0.20, 0.12], glow: 0.35, ..FLAKE },
            Some(Biome::AshBarrens) | None => FLAKE,
            Some(Biome::BasaltScarps) => Mote { share: 0.05, ..FLAKE },
        },
        // Highfold: a petal now and then, blown off the island meadows.
        WorldType::Realm(engine_voxel::RealmKind::Highfold) => Mote { share: 0.05, rise: -0.10, sway: 1.1, radius: 0.020, color: [1.0, 0.82, 0.88], glow: 0.12, alpha: 0.75, twinkle: Twinkle::Tumble },
        // Gloaming: slow violet wisps between the trunks.
        WorldType::Realm(engine_voxel::RealmKind::Gloaming) => Mote { share: 0.04, rise: 0.03, sway: 0.6, radius: 0.040, color: [0.66, 0.50, 1.0], glow: 0.9, alpha: 0.22, twinkle: Twinkle::Pulse },
        WorldType::Realm(engine_voxel::RealmKind::Gilded) => {
            let (hoard, grotto) = engine_worldgen::gilded::mood(world.seed, Vec3::new(x_m, 0.0, z_m));
            if grotto > pick { NONE } else if hoard > pick { Mote { share: 0.14, ..LEAF } } else { LEAF }
        }
        // The folded realms: prism glints, cyan to violet.
        WorldType::Manyfold | WorldType::Hall => Mote {
            share: 0.06, rise: 0.0, sway: 0.2, radius: 0.008,
            color: if pick < 0.5 { [0.55, 0.92, 1.0] } else { [0.82, 0.62, 1.0] },
            glow: 1.4, alpha: 0.75, twinkle: Twinkle::Glint,
        },
        _ => NONE,
    }
}

/// What a squall carries over a place, if anything: Hell biomes and the
/// Highfold's gales. Shares are scaled by the squall's strength.
fn squall_families(world: &World, x_m: f32, z_m: f32) -> Option<[Mote; 2]> {
    let cfg = world.sample_config();
    match world.world_type {
        WorldType::Nether if cfg.is_hell() => Some(match engine_worldgen::nether::biome(&cfg, x_m, z_m) {
            Some(Biome::FungalHollows) => [Mote { share: 0.95, ..SPORE }, Mote { share: 0.25, ..PUFF }],
            Some(Biome::CinderRifts) => [Mote { share: 0.90, rise: 0.6, radius: 0.018, ..EMBER }, Mote { share: 0.45, ..STORM_ASH }],
            Some(Biome::BasaltScarps) => [Mote { share: 0.95, radius: 0.020, color: [0.34, 0.31, 0.31], glow: 0.10, alpha: 0.8, ..GRIT }, Mote { share: 0.20, ..FLAKE }],
            // Wind-borne ash catches the sky glow: paler and larger than the calm fall.
            Some(Biome::AshBarrens) | None => [STORM_ASH, Mote { share: 0.45, ..FLAKE }],
        }),
        WorldType::Realm(engine_voxel::RealmKind::Highfold) => Some([
            Mote { share: 0.55, rise: 0.20, sway: 0.9, color: [0.96, 0.92, 0.70], glow: 0.15, alpha: 0.40, ..DUST },
            Mote { share: 0.25, rise: -0.05, sway: 1.2, radius: 0.020, color: [1.0, 0.82, 0.88], glow: 0.12, alpha: 0.75, twinkle: Twinkle::Tumble },
        ]),
        _ => None,
    }
}

pub(super) fn draw(mesh: &mut Model, world: &World, player: &Player, chunks: &ChunkCache) {
    if world.ca_view.is_some()
        || !matches!(world.world_type, WorldType::Nether | WorldType::Manyfold | WorldType::Hall | WorldType::Realm(_))
    {
        return;
    }
    let scale = world.voxel_scale.meters();
    let eye_m = player.precise_eye(world.voxel_scale) * f64::from(scale);
    let eye = eye_m.as_vec3();
    let time = world.adventure.time;
    // One steady seeded wind per realm; the shell realms have still air.
    let angle = unit(world.seed ^ 0x4149_5257) * std::f32::consts::TAU;
    let breeze = match world.world_type {
        WorldType::Nether => 0.55,
        WorldType::Realm(engine_voxel::RealmKind::Highfold) => 1.4,
        _ => 0.0,
    };
    let wind = Vec3::new(angle.cos(), 0.0, angle.sin()) * breeze;
    let drift = wind.as_dvec3() * f64::from(time);
    let previous_alpha = mesh.alpha;
    let air = Air { world, chunks, eye_m, time };
    air.scatter(mesh, drift, Vec3::ZERO, 0x5245_4c4d, &|x, z, pick| {
        let [a, b] = families(world, x, z, pick);
        [a, b, accent(world, x, z, pick)]
    });
    // A squall blows a second layer through on its own, stronger wind, a
    // quarter turn off the steady breeze. Its travel is the integral of its
    // strength, so the layer speeds up and settles without a jump.
    let (strength, travel) = crate::world::portals::realm::squall(world.seed, time);
    if strength > 0.01 && squall_families(world, eye.x, eye.z).is_some() {
        let turn = angle + 0.45;
        let heading = Vec3::new(turn.cos(), 0.0, turn.sin());
        let gale = heading.as_dvec3() * f64::from(SQUALL_WIND_MPS * travel);
        // Each mote is drawn as the path it covers in a short exposure, so the
        // gale reads as driven streaks even in a still frame.
        let streak = heading * (SQUALL_WIND_MPS * strength * SQUALL_EXPOSURE_S);
        // Two independent sheets, the second a little faster, so the squall is
        // dense and does not move as one rigid lattice.
        for (salt, pace) in [(0x5351_4c4c_u32, 1.0_f64), (0x4741_4c45, 1.18)] {
            air.scatter(mesh, gale * pace, streak * pace as f32, salt, &|x, z, _| {
                let [a, b] = squall_families(world, x, z).unwrap_or([NONE, NONE]);
                [Mote { share: a.share * strength, ..a }, Mote { share: b.share * strength, ..b }, NONE]
            });
        }
    }
    if matches!(world.world_type, WorldType::Realm(engine_voxel::RealmKind::Gilded)) {
        draw_shaft(mesh, world, eye, time, chunks);
    }
    mesh.alpha = previous_alpha;
}

/// What every mote layer shares for one frame.
struct Air<'a> {
    world: &'a World,
    chunks: &'a ChunkCache,
    eye_m: glam::DVec3,
    time: f32,
}

impl Air<'_> {
    /// Scatter one layer of motes over the 11 x 11 tiles round the eye. `drift`
    /// moves the whole layer; `salt` keeps layers independent. Slots 0-3
    /// alternate the first two families, slots 4-5 carry the third (accent).
    fn scatter(&self, mesh: &mut Model, drift: glam::DVec3, streak: Vec3, salt: u32, kinds_at: &dyn Fn(f32, f32, f32) -> [Mote; 3]) {
        let (world, time) = (self.world, self.time);
        let inv_scale = world.voxel_scale.meters().recip();
        let eye = self.eye_m.as_vec3();
        let grid_eye = self.eye_m - drift;
        let tile = (DVec2::new(grid_eye.x, grid_eye.z) / f64::from(TILE_M)).floor().as_ivec2();
        for z in tile.y - 5..=tile.y + 5 {
            for x in tile.x - 5..=tile.x + 5 {
                let tile_seed = hash_u32(world.seed ^ hash_u32(x as u32) ^ (z as u32).rotate_left(16) ^ salt);
                let kinds = kinds_at((x as f32 + 0.5) * TILE_M, (z as f32 + 0.5) * TILE_M, unit(tile_seed ^ 11));
                for i in 0..6_u32 {
                    let seed = hash_u32(tile_seed ^ i);
                    let mote = kinds[if i < 4 { (i & 1) as usize } else { 2 }];
                    if unit(seed ^ 7) >= mote.share {
                        continue;
                    }
                    let speed = mote.rise * (0.7 + unit(seed ^ 1) * 0.6);
                    let travel = f64::from(unit(seed ^ 2) * COLUMN_M) + f64::from(time) * f64::from(speed);
                    let y = eye.y
                        + (travel - f64::from(eye.y) + f64::from(COLUMN_M * 0.5)).rem_euclid(f64::from(COLUMN_M)) as f32
                        - COLUMN_M * 0.5;
                    let vertical_fade = ((COLUMN_M * 0.5 - (y - eye.y).abs()) / 2.5).clamp(0.0, 1.0);
                    if vertical_fade <= 0.01 {
                        continue;
                    }
                    let phase = time * (0.5 + unit(seed ^ 8)) + unit(seed ^ 3) * std::f32::consts::TAU;
                    let sway = Vec3::new(phase.sin(), (phase * 0.6).sin() * 0.35, (phase * 0.75).cos()) * mote.sway;
                    let pos = Vec3::new(
                        ((f64::from(x) + f64::from(unit(seed ^ 4))) * f64::from(TILE_M) + drift.x) as f32,
                        y,
                        ((f64::from(z) + f64::from(unit(seed ^ 5))) * f64::from(TILE_M) + drift.z) as f32,
                    ) + sway;
                    let to_eye = eye - pos;
                    let distance_squared = to_eye.length_squared();
                    if !(0.36..RADIUS_M * RADIUS_M).contains(&distance_squared) {
                        continue;
                    }
                    let distance = distance_squared.sqrt();
                    let (pulse, glow_gain, color) = twinkle(mote, seed, time);
                    let fade = ((RADIUS_M - distance) / 5.0).clamp(0.0, 1.0)
                        * vertical_fade
                        * ((distance - 0.6) / 1.4).clamp(0.0, 1.0)
                        * pulse;
                    if fade <= 0.02 || !open(self.chunks, (pos * inv_scale).floor().as_ivec3()) {
                        continue;
                    }
                    let (radius, coverage) = pixel_floor(mote.radius * (0.7 + unit(seed ^ 6) * 0.7), distance);
                    mesh.alpha = mote.alpha * fade * coverage;
                    let facing = to_eye / distance;
                    let glow = mote.glow * glow_gain;
                    if matches!(mote.twinkle, Twinkle::Tumble) {
                        flake(mesh, pos, facing, radius, color, glow, tumble_spin(seed, time));
                    } else if streak != Vec3::ZERO {
                        smear(mesh, pos, facing, radius, color, glow, streak);
                    } else {
                        billboard(mesh, pos, facing, radius, color, glow);
                    }
                }
            }
        }
    }
}

/// Squall wind at full strength, metres per second.
const SQUALL_WIND_MPS: f32 = 7.5;
/// Exposure a squall streak covers: its length is wind speed times this.
const SQUALL_EXPOSURE_S: f32 = 0.045;

/// A mote stretched along `streak` (its travel over the exposure), as seen
/// from the eye: a long diamond, never shorter than the round mote.
fn smear(mesh: &mut Model, pos: Vec3, facing: Vec3, radius: f32, color: [f32; 3], glow: f32, streak: Vec3) {
    let along = streak - facing * streak.dot(facing);
    let length = along.length() * 0.5;
    if length <= radius {
        billboard(mesh, pos, facing, radius, color, glow);
        return;
    }
    let axis = along / (length * 2.0);
    let side = facing.cross(axis) * radius;
    let a = axis * length;
    mesh.triangle([pos + a, pos - side, pos + side], facing, color, glow);
    mesh.triangle([pos - a, pos + side, pos - side], facing, color, glow);
}

/// Spin angle of a tumbling flake; `twinkle` and `flake` read the same one.
fn tumble_spin(seed: u32, time: f32) -> f32 {
    time * (1.6 + unit(seed ^ 12) * 2.4) + unit(seed ^ 13) * std::f32::consts::TAU
}

/// A flipping flake: a diamond whose width follows the cosine of its spin
/// about a seeded in-plane axis, so it narrows to a sliver edge-on.
fn flake(mesh: &mut Model, pos: Vec3, facing: Vec3, radius: f32, color: [f32; 3], glow: f32, spin: f32) {
    let width = spin.cos().abs().max(0.12);
    // Long axis leans by the spin's slow drift, so flakes do not all align.
    let tilt = spin * 0.13;
    let right = Vec3::Y.cross(facing).try_normalize().unwrap_or(Vec3::X);
    let up = facing.cross(right);
    let axis = right * tilt.cos() + up * tilt.sin();
    let across = up * tilt.cos() - right * tilt.sin();
    let a = axis * radius * 1.3;
    let b = across * radius * width;
    mesh.triangle([pos + a, pos - b, pos + b], facing, color, glow);
    mesh.triangle([pos - a, pos + b, pos - b], facing, color, glow);
}

/// Brightness of a mote at `time`: (alpha gain, glow gain, colour).
fn twinkle(mote: Mote, seed: u32, time: f32) -> (f32, f32, [f32; 3]) {
    match mote.twinkle {
        Twinkle::Steady => (1.0, 1.0, mote.color),
        Twinkle::Pulse => {
            let beat = (time * (1.3 + unit(seed ^ 9) * 2.2) + unit(seed ^ 10) * 9.0).sin();
            ((0.35 + 0.65 * beat).clamp(0.0, 1.0), 1.0, mote.color)
        }
        Twinkle::Glint => {
            // A flash of about a sixth of a second every two to five seconds.
            let period = 2.0 + unit(seed ^ 9) * 3.0;
            let beat = (time / period + unit(seed ^ 10)).fract();
            let flash = (1.0 - ((beat - 0.5) * period / 0.08).abs()).max(0.0);
            let flash = flash * flash * (3.0 - 2.0 * flash);
            (0.30 + 0.70 * flash, 0.35 + 2.4 * flash, mote.color)
        }
        Twinkle::Burn { cool } => {
            // A life of one to two and a half seconds.
            let life = (time / (1.0 + unit(seed ^ 9) * 1.5) + unit(seed ^ 10)).fract();
            let kindle = (life / 0.08).min(1.0);
            let heat = 1.0 - life;
            let color = std::array::from_fn(|i| cool[i] + (mote.color[i] - cool[i]) * heat * heat);
            (kindle * heat.sqrt(), 0.25 + 0.75 * heat * heat, color)
        }
        Twinkle::Tumble => {
            // A flash as the face turns to the eye.
            let face = tumble_spin(seed, time).cos().abs();
            (0.55 + 0.45 * face, 0.4 + 2.2 * face.powi(12), mote.color)
        }
    }
}

/// A two-triangle diamond facing the eye.
fn billboard(mesh: &mut Model, pos: Vec3, facing: Vec3, radius: f32, color: [f32; 3], glow: f32) {
    let right = Vec3::Y.cross(facing).try_normalize().unwrap_or(Vec3::X);
    let up = facing.cross(right) * radius;
    let side = right * radius;
    mesh.triangle([pos + up, pos - side, pos + side], facing, color, glow);
    mesh.triangle([pos - up, pos + side, pos - side], facing, color, glow);
}

/// Dust turning slowly in the shaft of light from the grotto dome, lit by the
/// beam itself. Each mote is a pure function of its index and the clock.
fn draw_shaft(mesh: &mut Model, world: &World, eye: Vec3, time: f32, chunks: &ChunkCache) {
    let shaft = engine_worldgen::gilded::light_shaft(world.seed);
    let centre = glam::Vec2::from_array(shaft.centre);
    if glam::Vec2::new(eye.x, eye.z).distance(centre) > SHAFT_RANGE_M {
        return;
    }
    let inv_scale = world.voxel_scale.meters().recip();
    let height = (shaft.top_y - shaft.bottom_y).max(1.0);
    for i in 0..SHAFT_MOTES {
        let seed = hash_u32(world.seed ^ 0x5348_4654 ^ i.wrapping_mul(0x9e37_79b9));
        // Spread a little past the beam edge; the edge fade hides the overspill.
        let r = unit(seed).sqrt() * shaft.radius * 1.15;
        let spin = if seed & 1 == 0 { 1.0 } else { -1.0 };
        let angle = unit(seed ^ 1) * std::f32::consts::TAU + time * spin * (0.03 + unit(seed ^ 2) * 0.05);
        let fall = 0.03 + unit(seed ^ 3) * 0.06;
        let y = shaft.bottom_y + (unit(seed ^ 4) * height - time * fall).rem_euclid(height);
        let phase = time * (0.4 + unit(seed ^ 5) * 0.5) + unit(seed ^ 6) * std::f32::consts::TAU;
        let pos = Vec3::new(centre.x + angle.cos() * r, y, centre.y + angle.sin() * r)
            + Vec3::new(phase.sin(), (phase * 0.7).sin() * 0.5, (phase * 0.8).cos()) * 0.06;
        let to_eye = eye - pos;
        let distance = to_eye.length();
        if distance < 0.6 {
            continue;
        }
        let edge = 1.0 - smoothstep(0.65, 1.15, r / shaft.radius);
        let ends = ((y - shaft.bottom_y) / 1.5).min((shaft.top_y - y) / 4.0).clamp(0.0, 1.0);
        let near = ((distance - 0.6) / 1.4).clamp(0.0, 1.0);
        let far = ((SHAFT_RANGE_M - distance) / 10.0).clamp(0.0, 1.0);
        let fade = edge * ends * near * far;
        if fade <= 0.02 || !open(chunks, (pos * inv_scale).floor().as_ivec3()) {
            continue;
        }
        // Each mote catches the beam as it turns.
        let catch = 0.55 + 0.45 * (time * (0.6 + unit(seed ^ 7)) + unit(seed ^ 8) * 9.0).sin();
        let (radius, coverage) = pixel_floor(0.010 + unit(seed ^ 9) * 0.008, distance);
        mesh.alpha = 0.9 * fade * coverage.sqrt();
        billboard(mesh, pos, to_eye / distance, radius, SHAFT_MOTE_COLOR, 2.4 * catch);
    }
}

/// Grow a mote to about one screen pixel at range, so far motes neither drop
/// below the raster nor shimmer in and out. Returns the drawn radius and an
/// alpha gain that keeps a grown mote dimmer than a near one.
fn pixel_floor(radius: f32, distance: f32) -> (f32, f32) {
    let floor = distance * PIXEL_RADIUS_PER_M;
    if radius >= floor {
        (radius, 1.0)
    } else {
        (floor, (radius / floor).sqrt().max(0.4))
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn unit(seed: u32) -> f32 {
    (hash_u32(seed) >> 8) as f32 / 16_777_216.0
}

/// Motes never show inside rock, lava or water; unloaded space stays empty.
fn open(chunks: &ChunkCache, p: glam::IVec3) -> bool {
    chunks
        .cpu_chunks
        .get(&chunk_coord_from_world_block(p))
        .is_some_and(|chunk| {
            let block = chunk.get_world(p);
            !block.is_solid() && !matches!(block, BlockId::Water | BlockId::Lava)
        })
}

/// Hush eyes: a pale pair for each colony member within sight. They hold at
/// the edge of the light, so the pair is what shows; a blink now and then.
pub(super) fn draw_hush(mesh: &mut Model, world: &World, player: &Player) {
    if world.world_type.realm().is_none() || world.ca_view.is_some() {
        return;
    }
    let scale = world.voxel_scale.meters();
    let eye = (player.precise_eye(world.voxel_scale) * f64::from(scale)).as_vec3();
    let time = world.adventure.time;
    let previous_alpha = mesh.alpha;
    for (i, h) in world.lightless.hush.iter().enumerate() {
        let head = Vec3::from_array(h.pos) + Vec3::Y * 0.9;
        let to_eye = eye - head;
        let distance = to_eye.length();
        if !(2.0..36.0).contains(&distance) {
            continue;
        }
        let seed = hash_u32(world.seed ^ (i as u32).wrapping_mul(0x9e37_79b9));
        // Closed for about a fifth of a second every few seconds.
        let beat = (time * (0.21 + unit(seed) * 0.12) + unit(seed ^ 1)).fract();
        if beat < 0.05 {
            continue;
        }
        let facing = to_eye / distance;
        let right = Vec3::Y.cross(facing).try_normalize().unwrap_or(Vec3::X);
        let up = facing.cross(right);
        let fade = ((36.0 - distance) / 8.0).clamp(0.0, 1.0);
        mesh.alpha = 0.9 * fade;
        for side in [-1.0f32, 1.0] {
            let c = head + right * (0.085 * side);
            let (r, u) = (right * 0.034, up * 0.022);
            mesh.triangle([c + u, c - r, c + r], facing, [0.88, 0.84, 1.0], 1.8);
            mesh.triangle([c - u, c + r, c - r], facing, [0.88, 0.84, 1.0], 1.8);
        }
    }
    mesh.alpha = previous_alpha;
}
