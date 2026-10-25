//! Creature families, shared stats and articulated presentation rigs.
//!
//! Gameplay identity stays on `Enemy.kind`; this module owns the one place that
//! answers what a family is called, how large it is, how it moves and what its
//! body looks like. Models are built from local metres and only rotated by the
//! outer `Enemy::model` pass, so animation never changes collision or save state.

use super::Enemy;
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Skirmisher,
    Defender,
    Flyer,
    Rooted,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Species {
    #[serde(deserialize_with = "leak")]
    pub name: &'static str,
    pub role: Role,
    pub health: f32,
    /// Physical body height in metres; `free()` reserves this volume.
    pub body_height: f32,
    pub body_width: f32,
    pub center_height: f32,
    pub tall: bool,
    pub flying: bool,
    pub brawler: bool,
    pub speed: f32,
    pub attack_range: f32,
    pub windup: f32,
    pub cooldown: f32,
}

/// The first proven families. Kinds 2, 3, 6–9 keep their existing definitions
/// and presentation until they receive the same authored treatment.
pub fn species(kind: u32) -> Option<&'static Species> {
    SPECIES.iter().find(|(k, _)| *k == kind).map(|(_, s)| s)
}

const SPECIES_RON: &str = include_str!("../../../../../assets/bestiary.ron");
/// `(enemy kind, species)` rows from `assets/bestiary.ron`. Kinds without a
/// row keep their older hard-coded presentation.
pub static SPECIES: std::sync::LazyLock<Vec<(u32, Species)>> = std::sync::LazyLock::new(|| {
    ron::from_str(SPECIES_RON).unwrap_or_else(|error| panic!("assets/bestiary.ron: {error}"))
});

fn leak<'de, D: serde::Deserializer<'de>>(d: D) -> Result<&'static str, D::Error> {
    <String as serde::Deserialize>::deserialize(d).map(|s| &*Box::leak(s.into_boxed_str()))
}

pub fn name(kind: u32) -> Option<&'static str> {
    species(kind).map(|s| s.name)
}

/// Animated, articulated body. The caller owns health-bar, affix motes,
/// crouch/flash transforms and yaw; this function only emits local geometry.
pub fn draw_body(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    match e.kind {
        0 => skitterer(e, m, time, wind, hit),
        1 => sentinel(e, m, time, wind, hit),
        4 => moth(e, m, time, wind, hit),
        5 => warden(e, m, time, wind, hit),
        10 => mimic(e, m, time, wind, hit),
        11 => lamprey(e, m, time, wind),
        12 => kiln(e, m, time, wind, hit),
        13 => porter(e, m, time, wind, hit),
        15 => matron(e, m, wind, hit),
        _ => {}
    }
}

fn matron(e: &Enemy, m: &mut Model, wind: f32, hit: [f32;3]) {
    let sway = (e.phase * 2.0).sin() * 0.04 * e.gait;
    m.cuboid(local(e,0.0,0.8,0.0),Vec3::new(1.5,1.4,1.3),Quat::from_rotation_z(sway),[0.3,0.4,0.2],0.0);
    m.cuboid(local(e,0.0,1.65-wind*0.2,0.0),Vec3::new(1.1,0.7,0.9),Quat::IDENTITY,hit,0.0);
    for side in [-1.0,1.0] {
        for i in 0..3 {
            let z = i as f32*0.45-0.45;
            limb(m,local(e,side*0.5,0.9,z),local(e,side*0.95,0.5,z+wind*0.3),local(e,side*1.1,0.1,z),0.1,[0.42,0.58,0.32]);
            // Large visible brood sacs contract with the committed cast.
            m.cuboid(local(e,side*0.63,1.25,z),Vec3::new(0.45,0.6-wind*0.15,0.35),Quat::IDENTITY,[0.52,0.36,0.52],0.0);
        }
        m.cuboid(local(e,side*0.25,1.8,-0.48),Vec3::splat(0.1),Quat::IDENTITY,[0.9,0.72,0.42],0.2);
    }
}

fn local(e: &Enemy, x: f32, y: f32, z: f32) -> Vec3 {
    e.pos + Vec3::new(x, y, z)
}

/// A two-segment limb. Joints are separate pixels so a leg bends instead of
/// stretching as one beam.
fn limb(m: &mut Model, hip: Vec3, knee: Vec3, foot: Vec3, thickness: f32, color: [f32; 3]) {
    m.beam(hip, knee, thickness, color, 0.0);
    m.beam(knee, foot, thickness * 0.72, color, 0.0);
    m.cuboid(
        knee,
        Vec3::splat(thickness * 2.0),
        Quat::IDENTITY,
        color,
        0.0,
    );
}

fn skitterer(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let bob = (e.phase * 7.0).sin().abs() * 0.035 * e.gait;
    let crouch = wind * 0.10;
    let shell = [0.30, 0.34, 0.22];
    let belly = [0.15, 0.19, 0.13];
    m.cuboid(
        local(e, 0.0, 0.34 + bob - crouch, 0.04),
        Vec3::new(0.66, 0.24, 0.58),
        Quat::from_rotation_x(wind * 0.12),
        shell,
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 0.28 + bob - crouch, 0.16),
        Vec3::new(0.50, 0.12, 0.42),
        Quat::IDENTITY,
        belly,
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 0.36 + bob - crouch, -0.34),
        Vec3::new(0.40, 0.18, 0.26),
        Quat::IDENTITY,
        hit,
        0.06,
    );
    for i in 0..3 {
        m.cuboid(
            local(e, 0.0, 0.47 + bob - crouch, 0.16 - i as f32 * 0.16),
            Vec3::new(0.54 - i as f32 * 0.06, 0.055, 0.12),
            Quat::IDENTITY,
            if i % 2 == 0 {
                [0.38, 0.39, 0.24]
            } else {
                shell
            },
            0.0,
        );
    }
    // Eight splayed legs with a two-joint walk cycle driven by distance.
    for side in [-1.0_f32, 1.0] {
        for i in 0..4 {
            let ph = e.phase * 8.0 + i as f32 * 1.55 + side * 0.7;
            let (stride, lift) = super::enemy_presentation::step(ph, 0.13, 0.09, e.gait * (1.0 - wind));
            let z = 0.26 - i as f32 * 0.17;
            let hip = local(e, side * 0.27, 0.32 + bob - crouch, z);
            let knee = local(e, side * 0.50, 0.30 + lift + bob - crouch, z + stride);
            let foot = local(e, side * (0.63 + wind * 0.10), 0.04 + lift, z + stride);
            limb(m, hip, knee, foot, 0.035, [0.27, 0.23, 0.14]);
        }
    }
    for x in [-0.13, 0.13] {
        m.orb(
            local(e, x, 0.40 + bob, -0.47),
            0.045,
            [0.55, 1.0, 0.38],
            2.0,
        );
        let a = time * 3.1 + x * 10.0;
        m.beam(
            local(e, x, 0.42 + bob, -0.46),
            local(e, x * 2.2 + a.sin() * 0.05, 0.53 + a.cos() * 0.04, -0.74),
            0.016,
            [0.36, 0.38, 0.22],
            0.0,
        );
    }
    for x in [-0.08, 0.08] {
        m.cuboid(
            local(e, x, 0.26 + bob, -0.58 - wind * 0.05),
            Vec3::new(0.045, 0.055, 0.12 + wind * 0.08),
            Quat::from_rotation_z(x.signum() * (0.25 + wind * 0.35)),
            [0.45, 0.33, 0.16],
            0.0,
        );
    }
}

fn sentinel(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let sway = (time * 0.9 + e.slot as f32).sin() * 0.035;
    let pulse = 0.5 + 0.5 * (time * 2.2).sin();
    let stem = [0.30, 0.28, 0.18];
    let cap = [0.34, 0.48, 0.22];
    let gill = [0.52, 0.40, 0.24];
    for i in 0..5 {
        let a = i as f32 * std::f32::consts::TAU / 5.0 + 0.4;
        m.beam(
            local(e, 0.0, 0.18, 0.0),
            local(e, a.cos() * 0.34, 0.02, a.sin() * 0.30),
            0.045,
            [0.20, 0.16, 0.11],
            0.0,
        );
    }
    for i in 0..4 {
        let y = 0.18 + i as f32 * 0.24;
        m.cuboid(
            local(e, sway * i as f32 * 0.4, y, 0.0),
            Vec3::new(0.20 - i as f32 * 0.02, 0.26, 0.20 - i as f32 * 0.02),
            Quat::from_rotation_z(sway * 0.5),
            if i % 2 == 0 { stem } else { hit },
            0.0,
        );
    }
    m.cuboid(
        local(e, sway, 1.19, 0.0),
        Vec3::new(0.92, 0.20, 0.86),
        Quat::from_rotation_z(sway * 0.4),
        cap,
        0.16,
    );
    m.cuboid(
        local(e, sway, 1.30, 0.0),
        Vec3::new(0.64, 0.12, 0.60),
        Quat::from_rotation_z(sway * 0.4),
        [0.42, 0.55, 0.26],
        0.12,
    );
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::TAU / 6.0;
        m.cuboid(
            local(e, sway + a.cos() * 0.40, 1.09, a.sin() * 0.37),
            Vec3::new(0.16, 0.045, 0.10),
            Quat::from_rotation_y(a),
            gill,
            0.0,
        );
    }
    let swell = 1.0 + pulse * 0.08 + wind * 0.30;
    for (i, (x, z)) in [(-0.26_f32, 0.18_f32), (0.27, 0.20), (0.0, -0.30)]
        .into_iter()
        .enumerate()
    {
        let y = 0.78 + i as f32 * 0.10 + (time * 1.7 + i as f32).sin() * 0.025;
        m.glow_orb(
            local(e, sway + x, y, z),
            0.105 * swell,
            [0.58, 0.90, 0.33],
            1.8 + wind * 2.2,
            0.50 + pulse * 0.14,
        );
    }
    m.orb(local(e, sway, 1.02, -0.19), 0.075, [0.74, 1.0, 0.42], 2.0);
    for i in 0..4 {
        let a = time * 1.3 + i as f32 * 1.7;
        m.glow_voxel(
            local(
                e,
                sway + a.cos() * 0.54,
                1.26 + (a * 1.4).sin() * 0.08,
                a.sin() * 0.5,
            ),
            Vec3::splat(0.020),
            Quat::IDENTITY,
            [0.62, 0.94, 0.36],
            1.4,
            0.8,
        );
    }
}

fn moth(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let c = e.center();
    let hover = (time * 2.6 + e.slot as f32).sin() * 0.06;
    let body = [0.42, 0.34, 0.48];
    let wing = [0.54, 0.66, 0.36];
    let wing_dark = [0.30, 0.40, 0.24];
    m.cuboid(
        c + Vec3::new(0.0, hover, 0.0),
        Vec3::new(0.22, 0.18, 0.36),
        Quat::IDENTITY,
        body,
        0.0,
    );
    for i in 0..3 {
        m.cuboid(
            c + Vec3::new(0.0, hover - 0.015 * i as f32, 0.23 + i as f32 * 0.12),
            Vec3::new(0.16 - i as f32 * 0.025, 0.13, 0.12),
            Quat::IDENTITY,
            [0.34, 0.26, 0.40],
            0.0,
        );
    }
    m.cuboid(
        c + Vec3::new(0.0, hover + 0.03, -0.25),
        Vec3::new(0.17, 0.15, 0.15),
        Quat::IDENTITY,
        [0.50, 0.42, 0.54],
        0.0,
    );
    for x in [-0.07, 0.07] {
        m.orb(
            c + Vec3::new(x, hover + 0.06, -0.33),
            0.035,
            [0.84, 1.0, 0.52],
            2.4,
        );
        m.beam(
            c + Vec3::new(x, hover + 0.10, -0.33),
            c + Vec3::new(x * 2.5, hover + 0.26, -0.50),
            0.010,
            [0.72, 0.82, 0.52],
            0.4,
        );
    }
    // Four wings. The tell folds them back before the dive; otherwise they beat.
    let beat = (time * 13.0 + e.phase * 2.0).sin();
    for side in [-1.0_f32, 1.0] {
        for layer in 0..2 {
            let fold = 1.0 - wind * 0.85;
            let spread = (if layer == 0 { 0.58 } else { 0.44 }) * fold;
            let flap = beat * (0.20 + layer as f32 * 0.06) * (1.0 - wind * 0.7);
            let center = c + Vec3::new(
                side * (0.13 + spread * 0.55),
                hover + 0.03 + layer as f32 * 0.04,
                0.03 - layer as f32 * 0.13,
            );
            let rot = Quat::from_rotation_z(side * (0.22 + flap))
                * Quat::from_rotation_y(side * 0.20 * fold);
            m.cuboid(
                center,
                Vec3::new(spread, 0.022, 0.46 - layer as f32 * 0.12),
                rot,
                if layer == 0 { wing } else { wing_dark },
                0.16,
            );
            m.orb(
                center + Vec3::new(side * spread * 0.30, 0.02, -0.08),
                0.035,
                hit,
                1.4,
            );
        }
    }
    for x in [-0.06, 0.06] {
        m.beam(
            c + Vec3::new(x, -0.08, -0.06),
            c + Vec3::new(x * 2.2, -0.18, 0.06),
            0.012,
            [0.28, 0.24, 0.20],
            0.0,
        );
    }
}

fn warden(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    super::enemy_presentation::warden(e, m, time, wind, hit);
}

// ---- Deep-encounter families -------------------------------------------------

fn mimic(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let open = if e.alert > 0.0 {
        (1.0 - e.alert / 4.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let jaw = (open * 0.75 + wind * 0.35).clamp(0.0, 1.0);
    let wood = [0.30, 0.20, 0.12];
    let trim = [0.55, 0.42, 0.19];
    let dark = [0.10, 0.07, 0.05];
    let tongue = [0.62, 0.16, 0.16];
    m.cuboid(
        local(e, 0.0, 0.34, 0.0),
        Vec3::new(0.80, 0.42, 0.58),
        Quat::IDENTITY,
        wood,
        0.0,
    );
    for x in [-0.30, 0.30] {
        for z in [-0.19, 0.19] {
            m.cuboid(
                local(e, x, 0.15, z),
                Vec3::new(0.13, 0.20, 0.14),
                Quat::IDENTITY,
                dark,
                0.0,
            );
        }
    }
    m.cuboid(
        local(e, 0.0, 0.58 + jaw * 0.18, 0.10 - jaw * 0.22),
        Vec3::new(0.84, 0.12, 0.62),
        Quat::from_rotation_x(-jaw * 1.05),
        if jaw > 0.08 { hit } else { wood },
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 0.62 + jaw * 0.18, 0.10 - jaw * 0.22),
        Vec3::new(0.68, 0.035, 0.08),
        Quat::from_rotation_x(-jaw * 1.05),
        trim,
        0.0,
    );
    for x in [-0.36, 0.36] {
        m.cuboid(
            local(e, x, 0.35, 0.0),
            Vec3::new(0.045, 0.40, 0.60),
            Quat::IDENTITY,
            trim,
            0.0,
        );
    }
    m.cuboid(
        local(e, 0.0, 0.36, -0.31),
        Vec3::new(0.13, 0.13, 0.06),
        Quat::IDENTITY,
        trim,
        0.0,
    );
    if jaw > 0.10 {
        for i in 0..5 {
            let x = -0.28 + i as f32 * 0.14;
            m.cuboid(
                local(e, x, 0.46, -0.25),
                Vec3::new(0.05, 0.08 + jaw * 0.04, 0.05),
                Quat::IDENTITY,
                [0.86, 0.83, 0.68],
                0.0,
            );
        }
        let wiggle = (time * 9.0).sin() * 0.04;
        m.beam(
            local(e, 0.0, 0.43, -0.20),
            local(e, wiggle, 0.34, -0.72),
            0.045,
            tongue,
            0.25,
        );
        for x in [-0.12, 0.12] {
            m.orb(local(e, x, 0.50, -0.30), 0.035, [1.0, 0.62, 0.18], 2.2);
        }
    }
}

fn lamprey(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let body = [0.20, 0.32, 0.34];
    let dark = [0.10, 0.17, 0.20];
    let coil = wind * 0.22;
    for i in 0..7 {
        let t = i as f32 / 6.0;
        let z = -0.24 + t * 1.05;
        let wave = (time * 3.1 + e.phase * 5.0 - t * 2.6).sin() * (0.06 + t * 0.16);
        let y = 0.25 + (1.0 - t) * 0.08 + (time * 1.8 + t * 2.2).cos() * 0.035;
        m.cuboid(
            local(e, wave, y + coil * t * 0.25, z),
            Vec3::new(0.15 - t * 0.07, 0.15 - t * 0.06, 0.18),
            Quat::from_rotation_y(wave * 1.4),
            if i % 2 == 0 { body } else { dark },
            0.0,
        );
        for side in [-1.0_f32, 1.0] {
            m.cuboid(
                local(e, wave + side * (0.10 - t * 0.04), y + 0.03, z + 0.02),
                Vec3::new(0.07 - t * 0.03, 0.025, 0.12),
                Quat::from_rotation_z(side * 0.35),
                [0.28, 0.52, 0.52],
                0.0,
            );
        }
        if i % 2 == 0 {
            m.glow_voxel(
                local(e, wave + 0.09 - t * 0.03, y + 0.01, z),
                Vec3::splat(0.014),
                Quat::IDENTITY,
                [0.42, 0.94, 1.0],
                1.5,
                0.8,
            );
        }
    }
    let head_wave = (time * 3.1 + e.phase * 5.0).sin() * 0.07;
    m.cuboid(
        local(e, head_wave, 0.30, -0.36),
        Vec3::new(0.22, 0.20, 0.22),
        Quat::IDENTITY,
        body,
        0.0,
    );
    m.orb(
        local(e, head_wave, 0.31, -0.48),
        0.10,
        [1.0, 0.34, 0.28],
        2.4,
    );
    m.orb(
        local(e, head_wave, 0.31, -0.49),
        0.055,
        [0.08, 0.02, 0.03],
        0.4,
    );
    for side in [-1.0_f32, 1.0] {
        m.beam(
            local(e, head_wave + side * 0.12, 0.34, -0.38),
            local(e, head_wave + side * 0.24, 0.30, -0.62 + wind * 0.08),
            0.018,
            [0.42, 0.66, 0.66],
            0.0,
        );
    }
}

fn kiln(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let iron = [0.30, 0.25, 0.20];
    let soot = [0.12, 0.10, 0.09];
    let brass = [0.58, 0.41, 0.16];
    let heat = 0.55 + 0.45 * (time * 3.2).sin().abs();
    let vent = wind.clamp(0.0, 1.0);
    m.cuboid(
        local(e, 0.0, 0.56, 0.0),
        Vec3::new(0.72, 0.58, 0.72),
        Quat::IDENTITY,
        iron,
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 0.38, -0.34),
        Vec3::new(0.54, 0.30, 0.18),
        Quat::IDENTITY,
        soot,
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 0.28, -0.45),
        Vec3::new(0.26, 0.10 + vent * 0.22, 0.025),
        Quat::from_rotation_x(-vent * 0.35),
        [0.94, 0.36, 0.08],
        2.0 + heat * 2.0 + vent * 2.5,
    );
    m.cuboid(
        local(e, 0.20, 1.02, 0.12),
        Vec3::new(0.18, 0.52, 0.18),
        Quat::IDENTITY,
        soot,
        0.0,
    );
    m.cuboid(
        local(e, 0.20, 1.30, 0.12),
        Vec3::new(0.26, 0.08, 0.26),
        Quat::IDENTITY,
        brass,
        0.0,
    );
    for i in 0..4 {
        let t = (time * 2.0 + i as f32 * 0.25).fract();
        m.glow_voxel(
            local(
                e,
                0.20 + (i as f32 * 1.7).sin() * 0.12,
                1.36 + t * 0.28,
                0.12 + (i as f32 * 2.3).cos() * 0.10,
            ),
            Vec3::splat(0.018 + vent * 0.008),
            Quat::IDENTITY,
            [0.96, 0.42, 0.10],
            1.6 * (1.0 - t),
            0.68,
        );
    }
    for side in [-1.0_f32, 1.0] {
        for i in 0..3 {
            let z = -0.22 + i as f32 * 0.22;
            let (swing, lift) = super::enemy_presentation::step(
                e.phase * 6.0 + i as f32 * 2.1 + side, 0.12, 0.075, e.gait * (1.0 - wind));
            limb(
                m,
                local(e, side * 0.30, 0.34, z),
                local(e, side * 0.52, 0.18, z + swing),
                local(e, side * 0.58, 0.04 + lift, z + swing),
                0.045,
                soot,
            );
        }
        m.cuboid(
            local(e, side * 0.42, 0.68, -0.22),
            Vec3::new(0.10, 0.10, 0.34 + vent * 0.12),
            Quat::from_rotation_x(vent * 0.30),
            brass,
            0.0,
        );
    }
    m.orb(
        local(e, -0.24, 0.80, -0.30),
        0.055,
        [1.0, 0.58, 0.18],
        1.8 + heat,
    );
    m.orb(local(e, 0.24, 0.80, -0.30), 0.055, hit, 1.6);
}

fn porter(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let frame = [0.34, 0.29, 0.23];
    let brass = [0.56, 0.43, 0.20];
    let cloth = [0.16, 0.24, 0.24];
    let cargo = [0.34, 0.24, 0.14];
    for (x, phase) in [(-0.25, 0.0), (0.25, std::f32::consts::PI)] {
        let (swing, lift) = super::enemy_presentation::step(
            e.phase * 4.2 + phase, 0.34, 0.12, e.gait * (1.0 - wind));
        limb(
            m,
            local(e, x, 0.62, 0.0),
            local(e, x * 1.2, 0.32, swing * 0.6),
            local(e, x * 1.25, 0.04 + lift, swing),
            0.085,
            [0.20, 0.17, 0.14],
        );
    }
    m.cuboid(
        local(e, 0.0, 1.02 - wind * 0.05, 0.10),
        Vec3::new(0.62, 0.64, 0.44),
        Quat::from_rotation_x(0.18 + wind * 0.12),
        frame,
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 1.25, -0.08),
        Vec3::new(0.52, 0.18, 0.30),
        Quat::IDENTITY,
        brass,
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 0.76, -0.18),
        Vec3::new(0.12, 0.50, 0.12),
        Quat::IDENTITY,
        cloth,
        0.0,
    );
    m.cuboid(
        local(e, 0.0, 1.34, -0.30),
        Vec3::new(0.28, 0.24, 0.26),
        Quat::from_rotation_x(-0.22),
        [0.38, 0.31, 0.24],
        0.0,
    );
    m.orb(local(e, 0.0, 1.36, -0.45), 0.040, hit, 1.8);
    for side in [-1.0_f32, 1.0] {
        let hand = local(e, side * 0.52, 0.72 + wind * 0.32, -0.28 - wind * 0.22);
        m.beam(local(e, side * 0.30, 1.18, 0.02), hand, 0.070, frame, 0.0);
        m.cuboid(
            hand,
            Vec3::new(0.15, 0.14, 0.15),
            Quat::IDENTITY,
            brass,
            0.0,
        );
        if side < 0.0 {
            let cargo_bob = (time * 1.4).sin() * 0.015;
            m.cuboid(
                hand + Vec3::new(0.0, 0.24 + cargo_bob, 0.0),
                Vec3::new(0.40, 0.34, 0.32),
                Quat::from_rotation_y((time * 0.5).sin() * 0.06),
                cargo,
                0.0,
            );
            m.cuboid(
                hand + Vec3::new(0.0, 0.41 + cargo_bob, 0.0),
                Vec3::new(0.42, 0.05, 0.34),
                Quat::IDENTITY,
                brass,
                0.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    #[test]
    fn proven_families_have_shared_definitions() {
        for kind in [0, 1, 4, 5, 10, 11, 12, 13] {
            let s = species(kind).expect("family must exist");
            assert!(s.health > 0.0 && s.body_height > 0.0 && s.body_width > 0.0);
            assert!(!s.name.is_empty());
        }
        assert_eq!(species(0).unwrap().role, Role::Skirmisher);
        assert_eq!(species(1).unwrap().role, Role::Rooted);
        assert!(species(4).unwrap().flying);
        assert!(!species(5).unwrap().flying);
        assert!(!species(5).unwrap().brawler);
        assert_eq!(species(10).unwrap().name, "Threshold Mimic");
        assert_eq!(species(11).unwrap().name, "Cistern Lamprey");
        assert_eq!(species(12).unwrap().name, "Kiln Crawler");
        assert_eq!(species(13).unwrap().name, "Vault Porter");
        assert!(
            species(3).is_none(),
            "unproven families use the old fallback"
        );
    }

    #[test]
    fn warden_guard_is_breakable_without_save_state() {
        let mut enemy = Enemy::new(IVec3::ZERO, 0, 5, Vec3::ZERO, 0);
        assert!(enemy.guard);
        enemy.health = enemy.max_health() * 0.49;
        assert!(enemy.check_guard_break());
        assert!(!enemy.guard);
        assert!(!enemy.check_guard_break());
    }
}

#[cfg(test)]
mod species_table {
    use super::*;

    /// Serialising the loaded table reproduces the file (minus its comment
    /// header), so the file is the single source of truth.
    fn assert_round_trip<T: serde::Serialize>(rows: &T, file: &str) {
        let text = ron::ser::to_string_pretty(rows, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = file.replace('\r', "");
        let body = match file.find("\n[\n") {
            Some(i) => &file[i + 1..],
            None => file.as_str(),
        };
        let (a, b) = (text.trim_end(), body.trim_end());
        if a != b {
            let i = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            let lo = i.saturating_sub(60);
            panic!(
                "table file differs from the loaded rows at byte {i}; rerun the ignored dump test to rewrite it.\n table: {:?}\n  file: {:?}",
                &a[lo..(i + 60).min(a.len())],
                &b[lo..(i + 60).min(b.len())]
            );
        }
    }

    #[test]
    fn bestiary_ron_round_trips() {
        assert_round_trip(&*SPECIES, SPECIES_RON);
    }

    /// Rewrites assets/bestiary.ron from the loaded table (keeps the header).
    #[test]
    #[ignore = "writes assets/bestiary.ron"]
    fn dump_bestiary_ron() {
        let header = SPECIES_RON.split("[").next().unwrap_or("");
        let text = ron::ser::to_string_pretty(&*SPECIES, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/bestiary.ron");
        std::fs::write(path, format!("{header}{text}")).unwrap();
    }
}
