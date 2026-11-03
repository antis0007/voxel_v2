use super::*;
use crate::world::CloudWeatherField;

/// Bilinear read of a baked face at a world point, as the shader's sampler does.
fn sample_face(baked: &[u8], res: u32, face: u8, world: Vec2) -> f32 {
    let texel = 2.0 * HALF_M / f64::from(res);
    let fx = (f64::from(world.x) - f64::from(face) * PITCH_M + HALF_M) / texel - 0.5;
    let fz = (f64::from(world.y) + HALF_M) / texel - 0.5;
    let (i, j) = (fx.floor() as u32, fz.floor() as u32);
    let (tx, tz) = ((fx - fx.floor()) as f32, (fz - fz.floor()) as f32);
    let at = |i: u32, j: u32| f32::from(baked[(j.min(res - 1) * res + i.min(res - 1)) as usize]);
    let top = at(i, j) * (1.0 - tx) + at(i + 1, j) * tx;
    let bottom = at(i, j + 1) * (1.0 - tx) + at(i + 1, j + 1) * tx;
    top * (1.0 - tz) + bottom * tz
}

#[test]
fn shell_coverage_matches_the_weather_field_at_face_centres() {
    let controls = CloudControls { coverage: 0.7, ..CloudControls::default() };
    let res = 512;
    let cfg = ShellConfig { resolution: res, controls, weather_seed: 74 };
    // Off-centre points; the last lies on face 1.
    let points = [(0u8, Vec2::new(5_300.0, -2_100.0)), (0, Vec2::new(-11_000.0, 9_700.0)), (1, Vec2::new(PITCH_M as f32 + 3_000.0, 4_400.0))];
    for (face, point) in points {
        let mut field = CloudWeatherField::new(74);
        field.update(point, 0.0, controls, 74);
        // The stored level-0 texel nearest the clipmap centre, read back from the clipmap itself.
        let (texel_world, stored) = field.stored_coverage(0, 64, 64);
        assert!((texel_world - point).length() < 16.0);
        let baked = bake_shell(&cfg, 0.0, face);
        let got = sample_face(&baked, res, face, texel_world);
        let expected = stored * 255.0;
        assert!((got - expected).abs() <= 8.0, "face {face} at {texel_world}: shell {got} clipmap {expected}");
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
    let cfg = ShellConfig { resolution: 8, controls, weather_seed: 74 };
    let x = texel_chart_m(4, 8) as f32;
    let expected = coverage_at(Vec2::new(x, x) + march, controls, 74) * 255.0;
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
fn cloud_march_is_off_above_the_top_slab() {
    assert!(!volumetric_clouds_enabled(5_000.0, 4_000.0));
    assert!(volumetric_clouds_enabled(4_300.0, 4_000.0));
    let top = cloud_top_slab_m(&CloudControls::default());
    assert!(top > 7_000.0 && top < 8_000.0, "{top}");
}
