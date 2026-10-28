//! Footsteps, ambience beds and music, driven from the listener's surroundings.
mod ambience;
pub mod footsteps;

use crate::audio::{Audio, Director, Mood, MusicTable};
use crate::player::Player;
use crate::world::{DayNightCycle, World};
use glam::Vec3;
use std::sync::LazyLock;

const MUSIC_TABLE: &str = include_str!("../../../assets/music.ron");
static MUSIC: LazyLock<Option<MusicTable>> = LazyLock::new(|| ron::from_str(MUSIC_TABLE).ok());
/// A living enemy this close (metres) counts as combat for music.
const COMBAT_RADIUS_M: f32 = 20.0;

#[derive(Default)]
pub struct Soundscape {
    steps: footsteps::Tracker,
    probe_clock: f32,
    mood: Option<Mood>,
    /// World seed the director was made for; a new world restarts the schedule.
    director: Option<(u32, Director)>,
}

/// Rain strength from storm clouds, matching the wet-surface response in `material_studio`.
fn rain(cumulonimbus: f32) -> f32 {
    ((cumulonimbus - 0.10) / 0.65).clamp(0.0, 1.0)
}

impl Soundscape {
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
        self.probe_clock -= dt;
        if self.probe_clock <= 0.0 {
            self.probe_clock = ambience::PROBE_SECONDS;
            let sea_m = world.sample_config().water_level() as f32 * meters;
            let around = ambience::probe(|p| world.get(p), eye.floor().as_ivec3(), meters, sea_m);
            let clouds = day_night.cloud_controls().cumulonimbus;
            let daylight = day_night.lighting().daylight;
            let weather = ambience::Weather { rain: rain(clouds), storm: clouds.clamp(0.0, 1.0), night: 1.0 - daylight };
            audio.set_ambience(ambience::levels(&around, &weather));
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
    fn rain_starts_with_storm_clouds() {
        assert_eq!(rain(0.0), 0.0);
        assert_eq!(rain(0.02), 0.0);
        assert!(rain(0.5) > 0.5);
        assert_eq!(rain(1.0), 1.0);
    }
}
