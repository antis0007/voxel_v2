use super::*;
use crate::world::{CloudWeatherField, synoptic, wind_offset};

/// Bilinear read of the shell at a ground point, as the shader's sampler does,
/// from the four texels `bake_shell` would write there.
fn sample_shell(cfg: &ShellConfig, clock: WeatherClock, face: u8, ground: Vec2) -> f32 {
    let res = cfg.resolution;
    let texel = 2.0 * HALF_M / f64::from(res);
    let fx = (f64::from(ground.x) - f64::from(face) * PITCH_M + HALF_M) / texel - 0.5;
    let fz = (f64::from(ground.y) + HALF_M) / texel - 0.5;
    let (i, j) = (fx.floor() as u32, fz.floor() as u32);
    let (tx, tz) = ((fx - fx.floor()) as f32, (fz - fz.floor()) as f32);
    let at = |i: u32, j: u32| f32::from(shell_texel(cfg, clock, face, i.min(res - 1), j.min(res - 1)));
    let top = at(i, j) * (1.0 - tx) + at(i + 1, j) * tx;
    let bottom = at(i, j + 1) * (1.0 - tx) + at(i + 1, j + 1) * tx;
    top * (1.0 - tz) + bottom * tz
}

/// A clipmap centred over `ground` at `clock`, as the window keeps it.
fn clipmap_over(ground: Vec2, controls: CloudControls, clock: WeatherClock) -> CloudWeatherField {
    let mut field = CloudWeatherField::new(74);
    field.update_at(ground + clock.drift_m, 0.0, controls, 74, clock);
    field
}

/// Chart point of a body direction, with its face.
fn chart_of(dir: DVec3) -> (u8, Vec2) {
    let p = chart::from_body(dir * chart::RADIUS_M);
    let (face, _) = chart::split(p);
    (face as u8, Vec2::new(p.x as f32, p.z as f32))
}

/// Shell and clipmap level-0 coverage at the clipmap texel nearest `ground`, 0..255.
fn shell_and_clipmap(face: u8, ground: Vec2, controls: CloudControls, clock: WeatherClock) -> (f32, f32, Vec2) {
    let cfg = ShellConfig { resolution: 512, controls, weather_seed: 74, days: clock.days };
    let field = clipmap_over(ground, controls, clock);
    let (q, stored) = field.stored_coverage(0, 64, 64);
    assert!((q - (ground + clock.drift_m)).length() < 16.0);
    let texel_ground = q - clock.drift_m;
    (sample_shell(&cfg, clock, face, texel_ground), stored * 255.0, texel_ground)
}

#[test]
fn shell_coverage_matches_the_weather_field_at_face_centres() {
    let controls = CloudControls { coverage: 0.7, ..CloudControls::default() };
    // Off-centre points; the last lies on face 1.
    let points = [(0u8, Vec2::new(5_300.0, -2_100.0)), (0, Vec2::new(-11_000.0, 9_700.0)), (1, Vec2::new(PITCH_M as f32 + 3_000.0, 4_400.0))];
    for (face, point) in points {
        let (got, expected, at) = shell_and_clipmap(face, point, controls, WeatherClock::default());
        assert!((got - expected).abs() <= 8.0, "face {face} at {at}: shell {got} clipmap {expected}");
    }
}

#[test]
fn shell_matches_the_clipmap_on_every_face_with_drift_and_moving_weather() {
    let controls = CloudControls { coverage: 0.55, wind_dir_angle: 70.0, ..CloudControls::default() };
    for (time, days) in [(900.0, 17.3), (14_400.0, 230.85)] {
        let clock = WeatherClock::new(&controls, time, days);
        for face in 0..6u8 {
            for local in [Vec2::new(5_300.0, -2_100.0), Vec2::new(-19_000.0, 15_500.0)] {
                let ground = Vec2::new(f32::from(face) * PITCH_M as f32, 0.0) + local;
                let (got, expected, at) = shell_and_clipmap(face, ground, controls, clock);
                assert!((got - expected).abs() <= 8.0, "t {time} face {face} at {at}: shell {got} clipmap {expected}");
            }
        }
    }
}

/// The strongest cyclone eye on the planet at `days`, as a body direction.
fn strongest_eye(days: f64) -> DVec3 {
    (0..20_000u32)
        .map(|k| {
            let z = 1.0 - 2.0 * (f64::from(k) + 0.5) / 20_000.0;
            let lon = f64::from(k) * 2.399_963_229_728_653;
            DVec3::new(lon.cos() * (1.0 - z * z).sqrt(), lon.sin() * (1.0 - z * z).sqrt(), z)
        })
        .max_by(|a, b| synoptic::regional(*a, days, 74).storm.total_cmp(&synoptic::regional(*b, days, 74).storm))
        .unwrap_or(DVec3::Y)
}

#[test]
fn a_storm_seen_from_orbit_is_the_storm_on_the_ground() {
    use crate::simulation::storms::precipitation;
    // A fair day: any storm here comes from the planet's weather, not the day's dial.
    let controls = CloudControls { coverage: 0.3, cumulonimbus: 0.02, ..CloudControls::default() };
    let days = 9.4;
    let clock = WeatherClock::new(&controls, 600.0, days);
    let eye = strongest_eye(days);
    assert!(synoptic::regional(eye, days, 74).storm > 0.5, "no mature cyclone at day {days}");
    let (face, ground) = chart_of(eye);
    // Orbit: the shell is overcast over the eye.
    let cfg = ShellConfig { resolution: 512, controls, weather_seed: 74, days };
    let shell = sample_shell(&cfg, clock, face, ground) / 255.0;
    assert!(shell > 0.6, "shell {shell}");
    // Descent: every clipmap level holds the same deck around the player.
    let field = clipmap_over(ground, controls, clock);
    for level in 0..3 {
        let (_, c) = field.stored_coverage(level, 64, 64);
        assert!(c > 0.6 && (c - shell).abs() < 0.12, "level {level}: {c} vs shell {shell}");
    }
    // Ground: it rains under it, and the march's rain channel agrees.
    let here = synoptic::controls_at(controls, None, ground.as_dvec2(), days, 74);
    assert!(precipitation(here.cumulonimbus) > 0.3, "gameplay rain {}", precipitation(here.cumulonimbus));
    let wet = (0..16).map(|k| crate::world::precipitation_at(ground + clock.drift_m + Vec2::new(k as f32 * 60.0, 0.0), clock, controls, 74)).sum::<f32>() / 16.0;
    assert!(wet > 0.05, "march rain {wet}");
    // Far from any cyclone the fair day stays dry in play and in the march.
    let away = (0..64)
        .map(|k| chart_of(DVec3::new((f64::from(k) * 0.7).cos(), 0.4, (f64::from(k) * 0.7).sin()).normalize()).1)
        .find(|p| synoptic::regional(synoptic::body_dir(p.as_dvec2()), days, 74).storm < 0.02)
        .unwrap_or(Vec2::ZERO);
    let there = synoptic::controls_at(controls, None, away.as_dvec2(), days, 74);
    assert_eq!(precipitation(there.cumulonimbus), 0.0);
    assert!(crate::world::precipitation_at(away + clock.drift_m, clock, controls, 74) < 0.02);
}

#[test]
fn coverage_agrees_through_the_handoff_band() {
    // Across the 2.5-6 km band the shell and the march overlap: their mean cover
    // over a 2 km patch must match so the crossfade neither thickens nor thins the deck.
    let controls = CloudControls { coverage: 0.5, ..CloudControls::default() };
    let clock = WeatherClock::new(&controls, 1_200.0, 3.7);
    let cfg = ShellConfig { resolution: 512, controls, weather_seed: 74, days: clock.days };
    for ground in [Vec2::new(4_000.0, -6_000.0), Vec2::new(PITCH_M as f32 * 2.0 - 9_000.0, 12_000.0)] {
        let face = chart::split(DVec3::new(f64::from(ground.x), 0.0, 0.0)).0 as u8;
        let field = clipmap_over(ground, controls, clock);
        let (mut march, mut shell) = (0.0, 0.0);
        // Level 1 has 32 m texels; 64 x 64 of them span 2 km.
        for y in 32..96 {
            for x in 32..96 {
                let (q, c) = field.stored_coverage(1, x, y);
                march += c;
                shell += sample_shell(&cfg, clock, face, q - clock.drift_m) / 255.0;
            }
        }
        let (march, shell) = (march / 4096.0, shell / 4096.0);
        assert!((march - shell).abs() < 0.03, "at {ground}: march {march} shell {shell}");
    }
}

#[test]
fn shell_drifts_with_the_march_wind() {
    // The march samples `weather_at_world(p.xz + wind * time * wind_spd * wx_mult)` with
    // `time = sky_time * advection` and the low-deck mult 8.0 * 1.65; a change there fails here.
    let shader = include_str!("../../shaders/cloud_shader.wgsl");
    for needle in [
        "let wx_offset_march = wind * time * wind_spd * wx_mult;",
        "weather_at_world(p.xz + wx_offset_march)",
        "const CLOUD_ADVECTION_SPEED_SCALE : f32 = 1.65;",
        "select(select(8.0, 6.65, layer_id == 1), 12.0, layer_id == 2)",
        "let wind     = safe_normalize2(light.weather_globals[0].zw",
    ] {
        assert!(shader.contains(needle), "cloud_shader.wgsl changed: {needle}");
    }
    let render = include_str!("../../render.rs");
    for needle in [
        "sky_time * cloud_controls.advection,\n                cloud_controls.wind_speed,\n                cloud_controls.wind_dir_angle.to_radians().cos(),\n                cloud_controls.wind_dir_angle.to_radians().sin(),",
    ] {
        assert!(render.replace("\r\n", "\n").contains(needle), "weather_globals[0] changed");
    }
    let controls = CloudControls { coverage: 0.7, wind_dir_angle: 30.0, ..CloudControls::default() };
    let (time, angle) = (60.0_f32, 30.0_f32.to_radians());
    let march = Vec2::new(angle.cos(), angle.sin()) * (time * controls.advection) * controls.wind_speed * (8.0 * 1.65);
    assert!((wind_offset(&controls, time) - march).length() < 1e-3);
    let cfg = ShellConfig { resolution: 8, controls, weather_seed: 74, days: 2.5 };
    let x = texel_chart_m(4, 8) as f32;
    let clock = WeatherClock { drift_m: march, days: 2.5, ..WeatherClock::default() };
    let expected = coverage_at(Vec2::new(x, x) + march, clock, controls, 74) * 255.0;
    let got = f32::from(bake_shell(&cfg, time, 0)[4 * 8 + 4]);
    assert!((got - expected).abs() <= 1.0);
}

#[test]
fn night_lights_cap_at_256_strongest_first() {
    let sites: Vec<(DVec3, IVec2)> = (0..300).map(|i| (DVec3::new(f64::from(i), 0.0, 0.0), IVec2::new(i, -i))).collect();
    let lights = night_lights_from(74, &sites, &[], DVec3::ZERO);
    assert_eq!(lights.len(), 256);
    assert!(lights.windows(2).all(|w| w[0].intensity >= w[1].intensity));
    let strongest = sites.iter().map(|s| site_light(74, s.1).0).fold(0.0, f32::max);
    assert_eq!(lights[0].intensity, strongest);
}

#[test]
fn equal_lights_keep_the_256_nearest() {
    let sites: Vec<(DVec3, IVec2)> = (0..300).map(|i| (DVec3::new(f64::from(299 - i) * 100.0, 0.0, 0.0), IVec2::new(7, 7))).collect();
    let lights = night_lights_from(74, &sites, &[], DVec3::ZERO);
    assert_eq!(lights.len(), 256);
    assert!(lights.iter().all(|l| l.body[0] < 256.0 * 100.0));
    assert_eq!(lights[0].body[0], 0.0);
}

#[test]
fn site_lights_vary_in_intensity_and_radius_within_their_bounds() {
    let lights: Vec<(f32, f32)> = (0..64).map(|i| site_light(74, IVec2::new(i % 8, i / 8))).collect();
    assert!(lights.iter().all(|&(k, r)| (1.5..=3.0).contains(&k) && (900.0..=1_500.0).contains(&r)));
    let spread = |f: fn(&(f32, f32)) -> f32| lights.iter().map(f).fold(f32::MIN, f32::max) - lights.iter().map(f).fold(f32::MAX, f32::min);
    assert!(spread(|l| l.0) > 0.5 && spread(|l| l.1) > 200.0);
}

#[test]
fn factory_chunks_merge_into_one_light_and_small_ones_stay_dark() {
    let clusters = [(DVec3::X, 640u32), (DVec3::Y, 7), (DVec3::Z, 8)];
    let lights = night_lights_from(74, &[], &clusters, DVec3::ZERO);
    assert_eq!(lights.len(), 2);
    assert!((lights[0].intensity - 6.0).abs() < 1e-5);
    assert_eq!(lights[1].intensity, FACTORY_INTENSITY);
}

#[test]
fn shader_shell_height_matches_the_host() {
    let source = crate::shader_sources::globe_shader_source();
    assert!(source.contains(&format!("const GLOBE_SHELL_M: f32 = {SHELL_HEIGHT_M:.1};")));
}

#[test]
fn shell_and_march_hand_over_on_the_same_band() {
    let (low, high) = HANDOFF_M;
    let globe = crate::shader_sources::globe_shader_source();
    assert!(globe.contains(&format!("const GLOBE_HANDOFF_LOW_M: f32 = {low:.1};")));
    assert!(globe.contains(&format!("const GLOBE_HANDOFF_HIGH_M: f32 = {high:.1};")));
    let composite = include_str!("../../shaders/cloud_composite.wgsl");
    assert!(composite.contains(&format!("smoothstep({low:.1}, {high:.1}, cam.planet[1].w)")));
}

#[test]
fn cloud_march_is_off_above_the_top_slab() {
    assert!(!volumetric_clouds_enabled(5_000.0, 4_000.0));
    assert!(volumetric_clouds_enabled(4_300.0, 4_000.0));
    let top = cloud_top_slab_m(&CloudControls::default());
    assert!(top > 7_000.0 && top < 8_000.0, "{top}");
}

#[test]
fn cloud_elements_share_the_surface_size_dial() {
    let common = include_str!("../../shaders/cloud_flow_shared.wgsl");
    assert!(common.contains("return mix(1120.0, 640.0, clamp(noise_scale, 0.0, 1.0)) / 880.0;"));
    let low = include_str!("../../shaders/cloud_morphology.wgsl");
    assert!(low.contains("mix(1120.0, 640.0, saturate1(light.weather_globals[2].x)) * world_scale"));
    let march = include_str!("../../shaders/cloud_shader.wgsl");
    assert_eq!(march.matches("cloud_horizontal_scale() * cloud_size_ratio(light.weather_globals[2].x)").count(), 2);
    let orbit = include_str!("../../shaders/globe_shell.wgsl");
    assert!(orbit.contains("world_scale * cloud_size_ratio(shell_blend.appearance.w)"));
    assert!(orbit.contains("select(1.0, 0.58, (globe.flags & 64u) != 0u)"));
    let driver = include_str!("../globe_driver.rs");
    assert!(driver.contains("| if cfg.is_planet() { 64 } else { 0 }"));
    assert!(!orbit.contains("mix(0.65, 1.55"));
    let shared = include_str!("../../shaders/cloud_shared.wgsl");
    assert!(shared.contains("select(1.0, 0.58, cam.planet[0].w"));
}

#[test]
fn cloud_element_filtering_follows_the_scaled_metres() {
    let march = include_str!("../../shaders/cloud_shader.wgsl");
    assert_eq!(march.matches("let sample_footprint_m = world_footprint_m / horizontal_scale;").count(), 2);
    let orbit = include_str!("../../shaders/globe_shell.wgsl");
    assert!(orbit.contains("let footprint = world_footprint / globe_cloud_element_scale();"));
    assert!(orbit.contains("let element_footprint = footprint / globe_cloud_element_scale();"));
    assert!(orbit.contains("smoothstep(120.0, 400.0, element_footprint)"));
    assert!(orbit.contains("let filtered_footprint = footprint / scale * fibre_scale;"));
}
