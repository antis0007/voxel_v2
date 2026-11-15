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

/// The Gilded Cave has no sky either. A low warm ambient keeps gold and
/// sandstone readable between lamps; it swells to a golden fill in the coin
/// cavern and turns cold teal in the grotto, following the eye.
pub fn gilded_lighting(state: &mut crate::world::LightingState, eye_m: glam::Vec3, seed: u32) {
    lightless_lighting(state);
    let (hoard, grotto) = engine_worldgen::gilded::mood(seed, eye_m);
    let mix3 = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    let ambient = mix3(mix3([0.045, 0.030, 0.016], [0.260, 0.160, 0.065], hoard), [0.070, 0.190, 0.230], grotto);
    let fog = mix3(mix3([0.022, 0.014, 0.007], [0.040, 0.024, 0.009], hoard), [0.010, 0.032, 0.044], grotto);
    state.ambient_color = ambient;
    state.fog_color = fog;
    state.fog_start = 24.0;
    state.fog_end = 170.0;
    state.realm = gilded_rig(hoard, grotto, seed);
}

/// A packed 0xRRGGBB sRGB layout colour as linear light at `strength`.
fn layout_light(packed: f32, strength: f32) -> [f32; 3] {
    let v = packed.max(0.0) as u32;
    let lin = |c: u32| {
        let s = c as f32 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    [lin((v >> 16) & 255) * strength, lin((v >> 8) & 255) * strength, lin(v & 255) * strength]
}

/// The Gilded Cave's light rig for the world shader, from the realm's layout
/// (editable in the dev tools). The sky-free fill is gold in the coin cavern
/// and cobalt in the grotto; the cavern air is deep red so the far heaps stand
/// in a red haze, the grotto air blue. The pools glow and throw caustics; a
/// shaft of light falls from the dome boss onto the summit.
fn gilded_rig(hoard: f32, grotto: f32, seed: u32) -> [[f32; 4]; 12] {
    let l = engine_worldgen::gilded::layout(seed);
    let mix3 = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    let fill = mix3(mix3(layout_light(l.gallery_fill, l.gallery_fill_i), layout_light(l.hoard_fill, l.hoard_fill_i), hoard),
        layout_light(l.grotto_fill, l.grotto_fill_i), grotto);
    let air = mix3(mix3([0.050, 0.026, 0.012], layout_light(l.hoard_air, l.hoard_air_i), hoard),
        layout_light(l.grotto_air, l.grotto_air_i), grotto);
    let range = 90.0 + (l.hoard_air_range - 90.0) * hoard + (l.grotto_air_range - 90.0) * grotto;
    let mut rig = [[0.0; 4]; 12];
    rig[0] = [1.0, hoard, grotto, 0.0];
    rig[1] = [fill[0], fill[1], fill[2], 0.0];
    rig[2] = [air[0], air[1], air[2], range];
    for (i, pool) in engine_worldgen::gilded::glow_pools(seed).iter().take(3).enumerate() {
        let (colour, strength) = if pool.palette == 0 { (l.grotto_water, l.grotto_water_i) } else { (l.hall_water, l.hall_water_i) };
        // w bit 0: round; bit 1: a coin bed (cavern pools sit their bed deeper in the blue).
        let flags = f32::from(u8::from(pool.round)) + 2.0 * f32::from(u8::from(pool.palette != 0));
        rig[3 + i * 2] = [pool.centre[0], pool.centre[1], pool.half[0], pool.half[1]];
        rig[4 + i * 2] = [pool.surface_y, strength, colour, flags];
    }
    let shaft = engine_worldgen::gilded::light_shaft(seed);
    rig[9] = [shaft.centre[0], shaft.centre[1], shaft.radius, l.shaft_i];
    rig[10] = [shaft.bottom_y, shaft.top_y, l.shaft_color, 0.0];
    rig
}

/// Pick the realm's light state.
pub fn realm_lighting(kind: engine_voxel::RealmKind, eye_m: glam::Vec3, seed: u32, state: &mut crate::world::LightingState) {
    match kind {
        engine_voxel::RealmKind::Lightless => lightless_lighting(state),
        engine_voxel::RealmKind::Gilded => gilded_lighting(state, eye_m, seed),
        engine_voxel::RealmKind::Highfold => {
            let day = state.daylight;
            let blend = |night: f32, noon: f32| night + (noon - night) * day;
            state.sky_top = [blend(0.025, 0.19), blend(0.031, 0.37), blend(0.085, 0.72)];
            state.fog_color = [blend(0.065, 0.41), blend(0.09, 0.63), blend(0.19, 0.78)];
            state.fog_start = 65.0;
            state.fog_end = 500.0;
            state.realm[11] = [1.0, 0.0, 0.0, 1.0];
        }
        engine_voxel::RealmKind::Gloaming => gloaming_lighting(state),
    }
}

/// The Gloaming is held at dusk: the sun sits 5° over the western rim at
/// every hour, so the atmosphere draws a sunset; violet ambient and fog fill
/// the forest under the canopies.
pub fn gloaming_lighting(state: &mut crate::world::LightingState) {
    let elevation = 5.0f32.to_radians();
    state.sun_dir = glam::Vec3::new(-elevation.cos(), elevation.sin(), 0.18).normalize();
    state.moon_dir = glam::Vec3::new(0.4, 0.55, -0.73).normalize();
    state.sun_color = [1.0, 0.56, 0.42];
    state.moon_color = [0.55, 0.62, 1.0];
    state.sun_intensity = 0.55;
    state.moon_intensity = 0.18;
    state.fog_height = engine_worldgen::gloaming::POOL_M;
    state.fog_falloff = 0.008;
    state.ambient_color = [0.115, 0.090, 0.185];
    state.sky_top = [0.075, 0.046, 0.19];
    state.fog_color = [0.21, 0.135, 0.32];
    state.fog_start = 18.0;
    state.fog_end = 165.0;
    state.realm[11] = [1.0, 0.0, 0.0, 1.0];
    state.daylight = 0.35;
    state.twilight = 1.0;
}

/// Cold light for the Nether's frozen plain: no day cycle, a dim high source,
/// teal shade. The sky shader draws the matching dome.
pub fn lighting(state: &mut crate::world::LightingState, hell: bool) {
    if hell {
        state.hell = true;
        // Hell flag for the world shader's Hell-only surfaces (`hell_world()`).
        state.realm[11][1] = 1.0;
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

// `shaders/hell_surfaces.wgsl` names these packed material ids (BlockId + 1).
const _: () = assert!(
    crate::world::BlockId::Ash as u32 + 1 == 62
        && crate::world::BlockId::FungalSoil as u32 + 1 == 29
        && crate::world::BlockId::UmberCap as u32 + 1 == 87
        && crate::world::BlockId::Hellstone as u32 + 1 == 191
);

/// Squall windows: one window in `SQUALL_PERIOD_S` seconds of world time.
pub const SQUALL_PERIOD_S: f32 = 150.0;

/// A seeded squall at `time` (world seconds): `(strength, travel)`. Some
/// windows carry one; its strength rises and falls as sin² across the window,
/// so it is zero at both ends. `travel` is the integral of strength over the
/// window so far, in seconds, so squall wind displaces motes smoothly however
/// the strength moves. Presentation only; a pure function of seed and clock.
pub fn squall(seed: u32, time: f32) -> (f32, f32) {
    // Review captures: `VOXEL_REVIEW_SQUALL=<0..1>` holds a steady squall.
    static REVIEW: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    if let Some(s) = *REVIEW.get_or_init(|| {
        std::env::var("VOXEL_REVIEW_SQUALL").ok().and_then(|v| v.parse::<f32>().ok()).map(|s| s.clamp(0.0, 1.0))
    }) {
        return (s, s * time);
    }
    let window = (time / SQUALL_PERIOD_S).floor();
    let x = time / SQUALL_PERIOD_S - window;
    let h = engine_core::hash::hash_u32(seed ^ 0x5351_4c4c ^ (window as i64 as u32).wrapping_mul(0x9e37_79b9));
    if (h & 0xffff) as f32 / 65_536.0 > 0.42 {
        return (0.0, 0.0);
    }
    let peak = 0.55 + 0.45 * (h >> 16) as f32 / 65_536.0;
    let s = (std::f32::consts::PI * x).sin();
    let travel = peak * SQUALL_PERIOD_S * (x * 0.5 - (std::f32::consts::TAU * x).sin() / (2.0 * std::f32::consts::TAU));
    (peak * s * s, travel)
}

/// Hell weather over the eye: a squall closes the fog in and tints it by the
/// biome it blows across. Ash storms grey out the barrens, ember gales glow in
/// the rifts, spore blooms turn the hollows amber, grit gales darken the scarps.
pub fn hell_weather(state: &mut crate::world::LightingState, cfg: engine_worldgen::WorldSampleConfig, time: f32, eye_m: glam::Vec3) {
    if !cfg.is_hell() {
        return;
    }
    let (strength, _) = squall(cfg.seed, time);
    if strength <= 0.0 {
        return;
    }
    use engine_worldgen::nether::Biome;
    let (tint, reach, glow) = match engine_worldgen::nether::biome(&cfg, eye_m.x, eye_m.z) {
        Some(Biome::AshBarrens) | None => ([0.13, 0.095, 0.085], 0.18, 0.0),
        Some(Biome::CinderRifts) => ([0.36, 0.085, 0.030], 0.30, 0.35),
        Some(Biome::FungalHollows) => ([0.30, 0.17, 0.060], 0.35, 0.20),
        Some(Biome::BasaltScarps) => ([0.075, 0.050, 0.050], 0.22, 0.0),
    };
    let s = strength;
    for i in 0..3 {
        state.fog_color[i] += (tint[i] - state.fog_color[i]) * s * 0.8;
        state.ambient_color[i] += tint[i] * glow * s;
    }
    state.fog_start += (4.0 - state.fog_start) * s;
    state.fog_end += (state.fog_end * reach - state.fog_end) * s;
    state.sun_intensity *= 1.0 - 0.55 * s;
    // For shaders that want it (motes read the same squall on the CPU).
    state.realm[11][2] = s;
}
