//! Re-entering debris drawn as a hot streak with a bright head. Geometry only;
//! the schedule is `world::orbital::meteors`, a pure function of the sky clock.
use super::{architecture::part, megastructures::long_range_since, tether, Model, Quat, Vec3};
use crate::world::{orbital::meteors, DayNightCycle, World};

const HOT: [f32; 3] = [1.0, 0.82, 0.55];
const HEAD: [f32; 3] = [1.0, 0.96, 0.86];

pub(super) fn build(mesh: &mut Model, world: &World, day: &DayNightCycle, viewer: Vec3, pixel: f32) {
    for s in meteors::streaks(world.seed, day, viewer) {
        let mark = mesh.mark();
        let reach = (s.head - viewer).length().max(1.0);
        // About 3 px wide and a 7 px head so it reads at 55 km; both swell and
        // shrink with the streak's own brightness.
        let width = (reach * pixel * 3.0 * s.fade).max(1.0);
        tether(mesh, s.tail, s.head, viewer, pixel, HOT, width);
        part(mesh, s.head, Vec3::splat((reach * pixel * 7.0 * s.fade).max(1.0)), Quat::IDENTITY, HEAD, 1.0);
        long_range_since(mesh, mark);
        for v in &mut mesh.vertices[mark..] {
            // Celestial lighting tag with full sunlight: it is its own light.
            v.ao_strength = 5.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldType;

    fn world() -> World {
        World::new(WorldType::Superflat, crate::fluid::VoxelScalePreset::OneMeter)
    }

    fn night(real: f32) -> DayNightCycle {
        let mut day = DayNightCycle::default();
        day.set_time_of_day(0.0);
        day.advance(real);
        day
    }

    #[test]
    fn a_live_streak_emits_geometry_and_noon_emits_none() {
        let world = world();
        let viewer = Vec3::new(0.0, 60.0, 0.0);
        let pixel = 2.0 * (0.5_f32).tan() / 1080.0;
        let mut drawn = 0;
        for i in 0..1_800 {
            let day = night(i as f32 * 0.05);
            let mut mesh = Model::new(1.0);
            build(&mut mesh, &world, &day, viewer, pixel);
            let alive = !meteors::streaks(world.seed, &day, viewer).is_empty();
            assert_eq!(!mesh.vertices.is_empty(), alive, "frame {i}");
            if alive {
                drawn += 1;
                // The streak's own lighting tag, so it is not darkened by the shadow path.
                assert!(mesh.vertices.iter().all(|v| v.ao_strength == 5.0));
            }
        }
        assert!(drawn > 0, "no streak was drawn in 90 s of night");
        let mut noon = DayNightCycle::default();
        noon.set_time_of_day(0.5);
        let mut mesh = Model::new(1.0);
        build(&mut mesh, &world, &noon, viewer, pixel);
        assert!(mesh.vertices.is_empty());
    }
}
