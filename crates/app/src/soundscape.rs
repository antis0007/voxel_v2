//! Footsteps, ambience beds and music, driven from the listener's surroundings.
mod ambience;
mod fauna;
pub mod footsteps;

use crate::audio::{Audio, Contact, Director, Mood, MusicTable};
use crate::player::Player;
use crate::world::{self, DayNightCycle, World, mining::STRIKE_PHASE};
use engine_content::BlockId;
use glam::{IVec3, Vec3};
use std::sync::LazyLock;

const MUSIC_TABLE: &str = include_str!("../../../assets/music.ron");
static MUSIC: LazyLock<Option<MusicTable>> = LazyLock::new(|| ron::from_str(MUSIC_TABLE).ok());
/// A living enemy this close (metres) counts as combat for music.
const COMBAT_RADIUS_M: f32 = 20.0;
/// Seconds between strikes while a block is being mined.

#[derive(Default)]
pub struct Soundscape {
    steps: footsteps::Tracker,
    probe_clock: f32,
    mood: Option<Mood>,
    /// World seed the director was made for; a new world restarts the schedule.
    director: Option<(u32, Director)>,
    dig_clock: f32,
    strikes: u32,
    /// Frame counter that varies each animal's call roll.
    calls: u32,
}

/// Grazers past this distance (metres) are not heard calling.
const CALL_RADIUS_M: f32 = 32.0;
/// Mean seconds between one animal's calls.
const CALL_EVERY_S: f32 = 11.0;

/// Listener yaw in turns clockwise from -z, the frame placed animal calls pan in.
fn listener_yaw(forward: Vec3) -> f32 {
    (forward.x.atan2(-forward.z) / std::f32::consts::TAU).rem_euclid(1.0)
}

/// Rain strength from storm clouds, matching the wet-surface response in `material_studio`.
fn rain(cumulonimbus: f32) -> f32 {
    ((cumulonimbus - 0.10) / 0.65).clamp(0.0, 1.0)
}

impl Soundscape {
    /// Plays a block breaking or being set down, in its material's voice.
    pub fn block(&mut self, audio: &mut Audio, contact: Contact, block: BlockId, cell: IVec3, world: &World, eye_m: Vec3, forward: Vec3) {
        let at = (cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
        self.strikes = self.strikes.wrapping_add(1);
        let seed = crate::audio::hash(world.seed ^ self.strikes, cell.x as u32 ^ (cell.z as u32).rotate_left(16));
        audio.contact(contact, footsteps::surface(block), Some(at), 1.0, seed, eye_m, forward);
    }

    /// Grazers near the listener call at random, each on its own roll.
    fn animal_calls(&mut self, audio: &mut Audio, world: &World, eye_m: Vec3, forward: Vec3, dt: f32) {
        self.calls = self.calls.wrapping_add(1);
        let chance = dt / CALL_EVERY_S;
        for (i, e) in world.adventure.enemies.iter().enumerate() {
            if e.health <= 0.0 || e.pos.distance_squared(eye_m) > CALL_RADIUS_M * CALL_RADIUS_M {
                continue;
            }
            let Some(pitch) = e.call_pitch() else { continue };
            let roll = crate::audio::hash(world.seed ^ self.calls, i as u32) as f32 / u32::MAX as f32;
            if roll < chance {
                let event = crate::audio::Event { cue: crate::audio::Cue::Bleat, pos: Some(e.pos), gain: 0.55, pitch };
                audio.play(event, eye_m, forward);
                crate::egui_ui::subtitles::note(&event, eye_m, forward);
            }
        }
    }

    /// Strikes the mined block when the viewmodel stroke lands, so the last hit is the break.
    /// `timing` is (worked, needed) seconds; `dig_clock` keeps the previous stroke phase.
    pub fn dig(&mut self, audio: &mut Audio, target: Option<(IVec3, BlockId)>, timing: Option<(f32, f32)>, world: &World, eye_m: Vec3, forward: Vec3) {
        let (Some((cell, block)), Some((elapsed, needed))) = (target, timing) else {
            self.dig_clock = 0.0;
            return;
        };
        let phase = crate::world::mining::stroke_phase(elapsed, needed);
        let landed = if phase >= self.dig_clock { self.dig_clock < STRIKE_PHASE && phase >= STRIKE_PHASE }
            else { self.dig_clock < STRIKE_PHASE || phase >= STRIKE_PHASE };
        // A finished break sits exactly on a strike, which the wrap test can miss by rounding.
        if landed || elapsed + 1e-6 >= needed && self.dig_clock < STRIKE_PHASE {
            self.block(audio, Contact::Hit, block, cell, world, eye_m, forward);
        }
        self.dig_clock = if elapsed + 1e-6 >= needed { 1.0 } else { phase };
    }

    pub fn tick(&mut self, audio: &mut Audio, world: &World, player: &Player, day_night: &DayNightCycle, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let meters = world.voxel_scale.meters();
        let eye = player.eye(world.voxel_scale);
        let (eye_m, forward) = (eye * meters, player.forward_dir());
        let gait = footsteps::Gait {
            phase: player.walk_phase(),
            on_ground: player.on_ground,
            flying: player.flying,
            swimming: player.swimming,
            crouch: player.crouch_t,
        };
        if let Some((gain, stride)) = self.steps.advance(gait) {
            let feet = (player.pos - Vec3::Y * 0.05).floor().as_ivec3();
            let wading = world.get(feet + glam::IVec3::Y) == engine_content::BlockId::Water;
            let surface = if wading { crate::audio::Surface::Water } else { footsteps::surface(world.get(feet)) };
            audio.footstep(surface, None, gain, crate::audio::hash(world.seed, stride), eye_m, forward);
        }
        self.animal_calls(audio, world, eye_m, forward, dt);
        audio.set_listener_yaw(listener_yaw(forward));
        self.probe_clock -= dt;
        if self.probe_clock <= 0.0 {
            self.probe_clock = ambience::PROBE_SECONDS;
            let sea_m = world.sample_config().water_level() as f32 * meters;
            let mut around = ambience::probe_flow(|p| world.get(p), |p| world.water_velocity(p).length(), eye.floor().as_ivec3(), meters, sea_m);
            around.submerged = f32::from(u8::from(player.underwater));
            let clouds = day_night.cloud_controls().cumulonimbus;
            let daylight = day_night.lighting().daylight;
            let weather = ambience::Weather { rain: rain(clouds), storm: clouds.clamp(0.0, 1.0), night: 1.0 - daylight };
            let profile = match world.world_type {
                engine_voxel::WorldType::Planet => ambience::PLANET_AIR,
                engine_voxel::WorldType::Nether | engine_voxel::WorldType::Hall | engine_voxel::WorldType::Manyfold => ambience::VOID_AIR,
                _ => ambience::EARTH_AIR,
            };
            let mut levels = ambience::levels(&around, &weather, profile);
            if world.orbital.express.ride.is_some() {
                // Sealed cabin: the drive carries, outside beds fall to a trace.
                for level in &mut levels { *level *= 0.15; }
                levels[crate::audio::Bed::Drive as usize] = 1.0;
            }
            audio.set_ambience(levels);
            audio.set_space(ambience::space(&around));
            audio.set_air(ambience::air(profile, around.altitude_m));
            audio.set_submerged(around.submerged);
            let cfg = world.sample_config();
            let reach = (fauna::BLEND_REACH_M / meters).round() as i32;
            let (ex, ez) = (eye.x.floor() as i32, eye.z.floor() as i32);
            let names = [(0, 0), (reach, 0), (-reach, 0), (0, reach), (0, -reach)]
                .map(|(dx, dz)| format!("{:?}", world::landscape::sample(&cfg, ex + dx, ez + dz).biome));
            let (voice, placement) = fauna::voice(
                fauna::blended([&names[0], &names[1], &names[2], &names[3], &names[4]]),
                &fauna::Listening {
                    daylight,
                    underground: around.underground.max(around.submerged),
                    rain: weather.rain,
                    air: ambience::air(profile, around.altitude_m),
                },
                &around,
                listener_yaw(forward),
            );
            audio.set_fauna(voice, placement);
            let vitals = &world.adventure;
            audio.set_strain(1.0 - (vitals.health / vitals.max_health().max(1.0)).clamp(0.0, 1.0));
            let combat = world.adventure.enemies.iter().any(|e| {
                e.health > 0.0 && e.pos.distance_squared(eye_m) < COMBAT_RADIUS_M * COMBAT_RADIUS_M
            });
            self.mood = Some(if combat {
                Mood::Combat
            } else if around.underground > 0.5 {
                Mood::Underground
            } else if daylight < 0.35 {
                Mood::Night
            } else {
                Mood::Day
            });
        }
        let (Some(table), Some(mood)) = (MUSIC.as_ref(), self.mood) else { return };
        if self.director.as_ref().is_none_or(|(seed, _)| *seed != world.seed) {
            self.director = Some((world.seed, Director::new(table, world.seed)));
        }
        if let Some((_, director)) = self.director.as_mut()
            && let Some(piece) = director.update(table, dt, mood)
        {
            audio.start_music(piece);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_ron_parses_with_a_piece_for_every_mood() {
        let table: MusicTable = ron::from_str(MUSIC_TABLE).expect("assets/music.ron");
        for mood in [Mood::Day, Mood::Night, Mood::Underground, Mood::Combat] {
            assert!(table.pieces.iter().any(|p| p.moods.contains(&mood)), "{mood:?}");
        }
        assert!(table.gap_s.0 >= 60.0, "pieces are separated by long silences");
        for def in &table.pieces {
            let piece = crate::audio::compose(def, 1);
            assert!(!piece.notes.is_empty() && piece.length_s < 240.0, "{}", def.name);
            assert!(def.scale.windows(2).all(|w| w[0] < w[1]) && def.scale.iter().all(|s| *s < 12), "{}", def.name);
        }
    }

    #[test]
    fn listener_yaw_turns_clockwise_from_north() {
        assert!((listener_yaw(Vec3::new(0.0, 0.0, -1.0))).abs() < 1e-6);
        assert!((listener_yaw(Vec3::new(1.0, 0.0, 0.0)) - 0.25).abs() < 1e-6);
        assert!((listener_yaw(Vec3::new(0.0, 0.0, 1.0)) - 0.5).abs() < 1e-6);
        assert!((listener_yaw(Vec3::new(-1.0, 0.0, 0.0)) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn rain_starts_with_storm_clouds() {
        assert_eq!(rain(0.0), 0.0);
        assert_eq!(rain(0.02), 0.0);
        assert!(rain(0.5) > 0.5);
        assert_eq!(rain(1.0), 1.0);
    }
}
