//! Openings into another realm, for the renderer. Reads the hell gate layout
//! (`engine_worldgen::hell_gate`); travel stays in `world::hell_gate`.
use super::Frame;
use crate::world::{World, WorldType};
use glam::{DQuat, DVec2, DVec3};

/// The arch opening, 4.8 by 4.9 m, centred 2.45 m above the floor.
const HALF: DVec2 = DVec2::new(2.4, 2.45);
/// Each face sits 1 cm off the arch plane so the two never coincide.
const PROUD: f64 = 0.01;

/// One face of an opening into another realm.
pub struct Gate {
    pub frame: Frame,
    /// True draws the underworld vista; false a mist membrane, used on the
    /// underworld side whose far side (the surface) has no stand-in.
    pub vista: bool,
    pub hell: bool,
}

/// Both faces of the nearest intact gate.
pub fn gates(world: &World, feet_m: glam::Vec3) -> Vec<Gate> {
    let mut result = Vec::new();
    for gate in &world.portals.hell {
        if gate.local.frame(f64::from(world.voxel_scale.meters()), true).center.distance(feet_m.as_dvec3()) < 96.0
            && gate.intact(world) {
            for frame in gate.frames(f64::from(world.voxel_scale.meters())) {
                if frame.center.distance(feet_m.as_dvec3()) < 96.0 {
                    result.push(Gate { frame, vista: false, hell: true });
                }
            }
        }
    }
    let cfg = world.sample_config();
    if !engine_worldgen::hell_gate::enabled(&cfg) {
        return result;
    }
    let c = engine_worldgen::hell_gate::center(&cfg, feet_m);
    let base = engine_worldgen::hell_gate::floor(&cfg, c);
    if (c - feet_m).length() > 96.0 || !crate::world::hell_gate::intact(world, c, base) {
        return result;
    }
    let vista = world.world_type != WorldType::Nether;
    let center = DVec3::new(f64::from(c.x), f64::from(base) + HALF.y, f64::from(c.z));
    result.extend([0.0, std::f64::consts::PI]
        .map(|a| {
            let rotation = DQuat::from_rotation_y(a);
            Gate { frame: Frame { center: center + rotation * DVec3::Z * PROUD, rotation, half_size: HALF }, vista, hell: world.sample_config().is_hell() }
        })
        .into_iter()
    );
    result
}

/// The Lightless has no sky. Sun and moon are off and the ambient floor is
/// near black, so block light (torches, foxfire, lava) is the only light.
pub fn lightless_lighting(state: &mut crate::world::LightingState) {
    state.sun_dir = glam::Vec3::new(0.0, -1.0, 0.0);
    state.moon_dir = glam::Vec3::new(0.0, -1.0, 0.0);
    state.sun_color = [0.0; 3];
    state.moon_color = [0.0; 3];
    state.sun_intensity = 0.0;
    state.moon_intensity = 0.0;
    state.ambient_color = [0.004, 0.005, 0.007];
    state.sky_top = [0.0; 3];
    state.sky_horizon = [0.0; 3];
    state.fog_color = [0.003, 0.004, 0.005];
    state.fog_start = 6.0;
    state.fog_end = 90.0;
    state.daylight = 0.0;
    state.twilight = 0.0;
    state.mood = [0.0; 4];
    state.mood_tint = [1.0, 1.0, 1.0, 0.0];
}

/// The Gilded Cave has no sky either, but a low warm ambient keeps gold and
/// sandstone readable between lamps; warm haze softens the far cavern walls.
pub fn gilded_lighting(state: &mut crate::world::LightingState) {
    lightless_lighting(state);
    state.ambient_color = [0.030, 0.020, 0.012];
    state.fog_color = [0.022, 0.014, 0.007];
    state.fog_start = 24.0;
    state.fog_end = 170.0;
}

/// Pick the realm's light state.
pub fn realm_lighting(kind: engine_voxel::RealmKind, state: &mut crate::world::LightingState) {
    match kind {
        engine_voxel::RealmKind::Lightless => lightless_lighting(state),
        engine_voxel::RealmKind::Gilded => gilded_lighting(state),
        // Open sky: the day cycle and the sky shader light it as they are.
        engine_voxel::RealmKind::Highfold => {}
    }
}

/// Cold light for the Nether's frozen plain: no day cycle, a dim high source,
/// teal shade. The sky shader draws the matching dome.
pub fn lighting(state: &mut crate::world::LightingState, hell: bool) {
    if hell {
        state.hell = true;
        state.moon_dir = -state.sun_dir;
        state.sun_color = [1.0, 0.57, 0.39];
        state.moon_color = [0.0; 3];
        state.ambient_color = [0.17, 0.115, 0.15];
        state.sky_top = [0.021, 0.004, 0.012];
        state.sky_horizon = [0.30, 0.049, 0.026];
        state.fog_color = [0.19, 0.035, 0.029];
        state.fog_start = 80.0;
        state.fog_end = 1900.0;
        state.fog_height = 100.0;
        state.fog_falloff = 0.008;
        let day = ((state.sun_dir.y + 0.08) / 0.20).clamp(0.0, 1.0);
        state.sun_intensity = 0.92 * day;
        state.moon_intensity = 0.0;
        state.daylight = day;
        state.twilight = 0.25;
        state.exposure = 1.0;
        state.saturation = 1.02;
        state.mood = [0.0; 4];
        state.mood_tint = [1.0, 1.0, 1.0, 0.0];
        return;
    }
    state.sun_dir = glam::Vec3::new(0.35, 0.80, 0.25).normalize();
    state.moon_dir = -state.sun_dir;
    state.sun_color = [0.62, 0.70, 1.0];
    state.moon_color = [0.0; 3];
    state.ambient_color = [0.06, 0.09, 0.14];
    state.sky_top = [0.010, 0.015, 0.030];
    state.sky_horizon = [0.040, 0.080, 0.100];
    state.fog_color = [0.030, 0.060, 0.080];
    state.sun_intensity = 0.30;
    state.moon_intensity = 0.0;
    state.daylight = 0.15;
    state.twilight = 0.0;
    state.mood = [0.0; 4];
}
