//! The held wand. Built every frame from a `WandStyle` out of the same glowing
//! micro-voxel primitives as the rest of the entity geometry, so it picks up the
//! engine's emissive pass, bloom and night grade instead of being a sprite on top.
//! Everything animates from the adventure clock, never from wall time.
use super::Model;
use crate::wand_style::{GRIPS, SHAFTS, WandStyle};
use glam::{Mat3, Quat, Vec3};

pub struct WandView {
    pub eye: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub time: f32,
    /// 0 at rest, 1 at the peak of a cast.
    pub cast: f32,
    /// Colour of the spell under the wand's cursor.
    pub spell: [f32; 3],
    pub last_cast: u32,
    pub tier: u32,
    /// Mana as a 0..1 fraction.
    pub mana: f32,
    /// One entry per deck slot the wand has: the spell's colour, or `None` when
    /// the slot is empty. Slots beyond the wand's capacity are absent.
    pub deck: Vec<Option<[f32; 3]>>,
    pub cursor: usize,
    /// Chassis seed of a found wand (0 for crafted): proportions vary with it.
    pub seed: u32,
}

/// Found wands differ in length and girth, so two wands never share a silhouette.
fn proportions(seed: u32) -> (f32, f32) {
    if seed == 0 {
        (1.0, 1.0)
    } else {
        (0.86 + 0.3 * hash01(seed ^ 0x51), 0.8 + 0.45 * hash01(seed ^ 0xA7))
    }
}

/// First-person viewmodel scale. The arm and wand are authored in world metres,
/// but a metre-scale wand at 0.6 m from the eye fills far too much of a 70°
/// view. Shrinking the whole rig around the grip keeps the hand attached while
/// restoring a readable silhouette and keeping the tip light aligned.
pub const VIEWMODEL_SCALE: f32 = 0.84;

/// Deterministic 0..1 value from an integer, so particle seeds never change
/// between frames or machines.
fn hash01(n: u32) -> f32 {
    let mut h = n.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

/// Smooth lifetime envelope: exactly 0 at birth and death, so a particle that
/// loops its phase fades in and out instead of popping at the wrap.
fn life(ph: f32) -> f32 {
    (std::f32::consts::PI * ph.clamp(0.0, 1.0)).sin().powf(1.4)
}

/// Edge of one wand "pixel", in metres. Every orb on the wand is built from
/// cubes of exactly this size on this grid, so the head is made of the same
/// pixels as the rest of the rig rather than a smooth or faceted blob.
const WAND_PIXEL: f32 = 0.012;

/// A sphere of `WAND_PIXEL` cubes. Small spheres are solid; larger ones are a
/// shell one pixel thick (`hollow`), which keeps the vertex count bounded.
fn voxel_sphere(
    m: &mut Model,
    rot: Quat,
    centre: Vec3,
    radius: f32,
    colour: [f32; 3],
    glow: f32,
    alpha: f32,
) {
    let radius = radius.clamp(WAND_PIXEL * 0.6, 0.07);
    let n = (radius / WAND_PIXEL).ceil() as i32;
    let hollow = radius > WAND_PIXEL * 2.6;
    let inner = if hollow {
        radius - WAND_PIXEL * 1.1
    } else {
        -1.0
    };
    for x in -n..=n {
        for y in -n..=n {
            for z in -n..=n {
                let p = Vec3::new(x as f32, y as f32, z as f32) * WAND_PIXEL;
                let d = p.length();
                if d > radius || d < inner {
                    continue;
                }
                m.glow_voxel(
                    centre + rot * p,
                    Vec3::splat(WAND_PIXEL * 0.94),
                    rot,
                    colour,
                    glow,
                    alpha,
                );
            }
        }
    }
}

fn voxel_orb(m: &mut Model, rot: Quat, centre: Vec3, radius: f32, colour: [f32; 3], glow: f32) {
    voxel_sphere(m, rot, centre, radius, colour, glow, 0.96);
}

/// A solid cut-gem bead: an ellipsoid stretched 1.25x along the wand axis, every
/// `WAND_PIXEL` cube shaded by its surface normal against a fixed key light. The
/// shading is continuous (no bands), so the gem reads as a volume rather than the
/// flat disc a single colour gives. A small specular cap sits on the upper facet.
fn voxel_gem(m: &mut Model, rot: Quat, centre: Vec3, radius: f32, colour: [f32; 3]) {
    // Lit enamel kind: solid, shaded by normal, no wand-wood grain or bands.
    let prev_kind = m.surface_kind;
    m.set_surface_kind(41.0);
    let radius = radius.clamp(WAND_PIXEL * 1.2, 0.05);
    let stretch = 1.25;
    let n = (radius * stretch / WAND_PIXEL).ceil() as i32;
    let light = Vec3::new(-0.45, 0.70, 0.55).normalize();
    let half = (light + Vec3::Y).normalize();
    // Dark, desaturated body so the lit facets have contrast; the spell colour
    // lives in the rim and the diffuse top, not in a flat fill.
    let deep = scaled(mix(colour, [0.10, 0.09, 0.13], 0.6), 0.30);
    let lit = mix(colour, [1.0, 1.0, 1.0], 0.2);
    let rim_colour = mix(colour, [1.0, 1.0, 1.0], 0.5);
    for x in -n..=n {
        for y in -n..=n {
            for z in -n..=n {
                let p = Vec3::new(x as f32, y as f32, z as f32) * WAND_PIXEL;
                // Ellipsoid radii: wide across, stretched along +Y (the wand axis).
                let q = Vec3::new(p.x, p.y / stretch, p.z);
                if q.length() > radius {
                    continue;
                }
                // Gradient of the ellipsoid: y is divided by stretch squared.
                let normal = Vec3::new(p.x, p.y / (stretch * stretch), p.z).normalize_or_zero();
                let normal = if normal == Vec3::ZERO {
                    Vec3::Y
                } else {
                    normal
                };
                let diffuse = normal.dot(light).max(0.0);
                let spec = normal.dot(half).max(0.0).powf(40.0);
                // Fresnel-style rim: surfaces turning away from the lens catch the colour.
                let rim = (1.0 - normal.z.abs()).powf(3.0);
                let shade = mix(deep, lit, diffuse);
                let col = mix(mix(shade, rim_colour, rim * 0.55), [1.0, 1.0, 1.0], spec * 0.85);
                // glow must stay 0: any glow > 0 makes `Model::triangle` pick the
                // emissive particle kind 40, which ignores this per-cube shading.
                m.cuboid(
                    centre + rot * p,
                    Vec3::splat(WAND_PIXEL * 0.94),
                    rot,
                    col,
                    0.0,
                );
            }
        }
    }
    m.set_surface_kind(prev_kind);
}

fn voxel_glow_orb(
    m: &mut Model,
    rot: Quat,
    centre: Vec3,
    radius: f32,
    colour: [f32; 3],
    glow: f32,
    alpha: f32,
) {
    voxel_sphere(m, rot, centre, radius, colour, glow, alpha.max(0.5));
}

/// Rotation taking +Y to `dir`; identity for a degenerate direction.
fn aim(dir: Vec3) -> Quat {
    let d = dir.normalize_or_zero();
    if d == Vec3::ZERO {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_arc(Vec3::Y, d)
    }
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn scaled(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Where the wand sits this frame: grip, unit axis, and length in metres.
fn pose(v: &WandView) -> (Vec3, Vec3, f32) {
    let cast = ease(v.cast);
    let recoil = v.cast * v.cast * 0.13;
    // Resting wand: held low and to the right, tip up and slightly inward.
    let idle_grip = v.eye + v.forward * 0.68 + v.right * 0.31 - v.up * 0.30;
    let idle_dir = (v.forward * 0.84 + v.up * 0.52 - v.right * 0.18).normalize();
    // Casting wand: the existing stance, thrust forward with recoil.
    let cast_grip = v.eye + v.forward * (0.54 - recoil) + v.right * (0.29 + v.cast * 0.07)
        - v.up * (0.31 + v.cast * 0.04);
    let cast_tip = v.eye + v.forward * (0.80 - recoil) + v.right * (0.17 + v.cast * 0.10)
        - v.up * (0.11 + v.cast * 0.10);
    // Two incommensurate sines make a slow figure-eight. The old single sine
    // read as a mechanical bob; this keeps the wand alive without drifting.
    let breath = (v.time * 1.35).sin();
    let figure = (v.time * 0.71).sin() * (v.time * 1.13).cos();
    let sway = v.up * (breath * 0.010 + figure * 0.004) * (1.0 - cast)
        + v.right * (figure * 0.007 + breath * 0.002) * (1.0 - cast)
        + v.forward * (breath * 0.004) * (1.0 - cast);
    let grip = idle_grip.lerp(cast_grip, cast) + sway;
    let dir = idle_dir
        .lerp((cast_tip - cast_grip).normalize(), cast)
        .normalize();
    let length = [0.34, 0.34, 0.39, 0.44][(v.tier as usize).min(3)] * proportions(v.seed).0;
    (grip, dir, length)
}

/// The light the wand throws on the world: position, colour, strength. Cave walls
/// pick up the spell's colour as you walk past, and flare when you cast.
pub fn tip_light(style: WandStyle, v: &WandView) -> (Vec3, [f32; 3], f32) {
    let (grip, dir, length) = pose(v);
    let pulse = 0.5 + 0.5 * (v.time * 0.7).sin();
    let strength = 2.1 + pulse * 0.3 + ease(v.cast) * 3.2;
    (
        grip + dir * (length * VIEWMODEL_SCALE + 0.045),
        style.glow_colour(v.spell),
        strength,
    )
}

pub fn draw(m: &mut Model, style: WandStyle, v: &WandView) {
    let cast = ease(v.cast);
    let (grip, dir, length) = pose(v);
    let side = (v.right - dir * v.right.dot(dir)).normalize_or_zero();
    let back = side.cross(dir);
    let rot = Quat::from_mat3(&Mat3::from_cols(side, dir, back));
    let at = |s: f32| grip + dir * s;
    let tip = at(length);
    let glow = style.glow_colour(v.spell);
    let shaft = &SHAFTS[style.shaft as usize];
    let radius = (0.016 + v.tier.min(3) as f32 * 0.0015) * proportions(v.seed).1;
    let pulse = 0.5 + 0.5 * (v.time * 0.7).sin();
    let power = 2.3 + pulse * 0.3 + cast * 2.4;
    let ring = |a: f32| side * a.cos() + back * a.sin();
    let rig_mark = m.mark();

    // Sleeve, wrist and a closed fist around the grip. The old hand was one
    // back-of-hand box with four bars; this version has a palm heel, articulated
    // proximal/distal finger segments, knuckle highlights, a two-part thumb,
    // a layered sleeve and stitch pixels. Casting curls the fingers and rolls
    // the wrist without moving the grip away from the shaft.
    let skin = [0.62, 0.45, 0.31];
    let skin_lit = [0.76, 0.57, 0.40];
    let skin_dark = [0.42, 0.29, 0.20];
    let glove = [0.40, 0.29, 0.19];
    let glove_dark = [0.27, 0.19, 0.13];
    let seam = [0.65, 0.48, 0.29];
    let cloth = [0.36, 0.30, 0.24];
    let cloth_dark = [0.25, 0.21, 0.17];
    // Skin and cloth are lit geometry, not glow particles. Kind 54 is the
    // dedicated pixel skin/leather shader branch.
    m.set_surface_kind(54.0);
    let hand_twist = cast * 0.18 + (v.time * 0.8).sin() * 0.025;
    let hand_rot = rot * Quat::from_rotation_y(hand_twist);
    let wrist = grip - dir * 0.048 + side * 0.012 - back * 0.004;
    // Keep the sleeve origin low and off the right edge. The old point sat close
    // enough to the centre that a 5 cm-radius connector read as a giant glossy
    // tube across the lower-right of the screen.
    let sleeve_end = v.eye - v.forward * 0.18 + v.right * 0.52 - v.up * 0.55
        + v.up * (v.time * 1.2).sin() * 0.004;

    // Cuff band and stitch pixels at the wrist.
    m.cuboid(
        wrist - dir * 0.018,
        Vec3::new(0.098, 0.052, 0.086),
        hand_rot,
        glove_dark,
        0.0,
    );
    for i in 0..4 {
        let across = (i as f32 - 1.5) * 0.019;
        m.cuboid(
            wrist - dir * 0.018 + side * across + back * 0.044,
            Vec3::new(0.008, 0.010, 0.008),
            hand_rot,
            seam,
            0.0,
        );
    }
    // Palm heel and the back of the hand, split so the knuckles catch light.
    m.cuboid(
        at(0.030) + side * 0.028,
        Vec3::new(0.062, 0.086, 0.072),
        hand_rot,
        glove,
        0.0,
    );
    m.cuboid(
        at(0.064) + side * 0.030 + back * 0.006,
        Vec3::new(0.040, 0.078, 0.062),
        hand_rot,
        skin_dark,
        0.0,
    );
    for i in 0..4 {
        let s = 0.012 + i as f32 * 0.021;
        // Proximal segment lies along the shaft.
        m.cuboid(
            at(s) - side * 0.008 - back * 0.004,
            Vec3::new(0.070, 0.018, 0.058),
            hand_rot,
            if i % 2 == 0 { glove } else { glove_dark },
            0.0,
        );
        // Distal segment curls toward the palm with the cast, wrapping the shaft.
        let curl = 0.009 + cast * 0.011;
        m.cuboid(
            at(s + 0.012) - side * (0.024 + curl) - back * 0.008,
            Vec3::new(0.052, 0.016, 0.050),
            hand_rot,
            skin,
            0.0,
        );
        // A one-pixel knuckle highlight keeps the hand readable at a distance.
        m.cuboid(
            at(s) + side * 0.025 + back * 0.020,
            Vec3::new(0.018, 0.010, 0.012),
            hand_rot,
            skin_lit,
            0.0,
        );
    }
    // Thumb: base pad plus a distal pixel pressing along the shaft.
    m.cuboid(
        at(0.086) + back * 0.024 - side * 0.004,
        Vec3::new(0.034, 0.044, 0.034),
        hand_rot,
        skin,
        0.0,
    );
    m.cuboid(
        at(0.116) + back * 0.032 - side * (0.010 + cast * 0.006),
        Vec3::new(0.022, 0.034, 0.022),
        hand_rot,
        skin_lit,
        0.0,
    );
    // The solid wand body uses its own wood/leather material. Treating every
    // shaft/grip piece with glow > 0 put it through the glow-particle shader,
    // which flattened the texture and produced the broken colour bands.
    m.set_surface_kind(55.0);

    // Shaft by material.
    match style.shaft {
        2 => {
            m.glow_beam(at(0.0), tip, radius * 1.3, shaft.base, 0.9, 0.62);
            m.glow_beam(
                at(0.02),
                tip,
                radius * 0.45,
                shaft.accent,
                2.0 + pulse,
                0.92,
            );
        }
        _ => m.beam(at(0.0), tip, radius, shaft.base, 0.0),
    }
    match style.shaft {
        0 => {
            for i in 0..4 {
                m.cuboid(
                    at(length * (0.26 + 0.18 * i as f32)),
                    Vec3::new(0.042, 0.010, 0.042),
                    rot,
                    shaft.dark,
                    0.0,
                );
            }
        }
        1 => {
            for i in 0..3 {
                m.cuboid(
                    at(length * (0.30 + 0.22 * i as f32)),
                    Vec3::new(0.044, 0.040, 0.044),
                    rot,
                    shaft.accent,
                    0.0,
                );
            }
        }
        3 => {
            for i in 0..4 {
                let s = length * (0.25 + 0.2 * i as f32);
                let breath = 0.85 + 0.15 * (v.time * 1.1 + i as f32).sin();
                m.glow_voxel(
                    at(s) + side * (radius + 0.003),
                    Vec3::splat(0.010),
                    rot,
                    shaft.accent,
                    1.1 * breath,
                    0.72,
                );
            }
        }
        4 => {
            for i in 0..3 {
                m.cuboid(
                    at(length * (0.30 + 0.22 * i as f32)),
                    Vec3::new(0.044, 0.012, 0.044),
                    rot,
                    shaft.accent,
                    0.0,
                );
            }
        }
        5 => {
            for k in 0..9 {
                let a = k as f32 * 2.4 + v.time * 0.4;
                let breath = 0.88 + 0.12 * (v.time * 0.8 + k as f32 * 0.4).sin();
                m.glow_voxel(
                    at(length * (0.10 + 0.10 * k as f32)) + ring(a) * (radius + 0.002),
                    Vec3::splat(0.006),
                    rot,
                    shaft.accent,
                    1.15 * breath,
                    0.7,
                );
            }
        }
        _ => {}
    }

    // Grip wrap.
    let wrap = GRIPS[style.grip as usize].1;
    for i in 0..7 {
        let band = if i % 2 == 0 { wrap } else { scaled(wrap, 0.6) };
        m.cuboid(
            at(0.01 + i as f32 * 0.026),
            Vec3::new(0.040, 0.022, 0.040),
            rot,
            band,
            0.0,
        );
    }

    // Rune beads and the mana vein.
    if style.runes > 0 {
        let lit_total = v.mana.clamp(0.0, 1.0) * 4.0;
        for i in 0..4 {
            let lit = (lit_total - i as f32).clamp(0.0, 1.0);
            let s = length * (0.36 + 0.14 * i as f32);
            let colour = mix(shaft.dark, glow, lit);
            m.glow_voxel(
                at(s) + side * (radius + 0.005),
                Vec3::new(0.016, 0.020, 0.010),
                rot,
                colour,
                0.25 + 3.2 * lit,
                0.92,
            );
        }
        if style.runes == 2 {
            let vein = length * v.mana.clamp(0.0, 1.0);
            if vein > 0.02 {
                m.glow_beam(at(0.02), at(0.02 + vein), 0.0045, glow, 2.8 + pulse, 0.85);
            }
        }
    }

    // Spell sockets: the deck is set into the shaft, one stone per slot, in the
    // colour of the spell mounted there. The next spell to fire pulses.
    let sockets = v.deck.len().max(1) as f32;
    for (i, slot) in v.deck.iter().enumerate() {
        let s = length * (0.3 + 0.5 * (i as f32 + 0.5) / sockets);
        let at_slot = at(s) - side * (radius + 0.004);
        m.cuboid(at_slot, Vec3::new(0.016, 0.018, 0.016), rot, scaled(shaft.dark, 0.7), 0.0);
        if let Some(colour) = slot {
            let next = i == v.cursor % v.deck.len().max(1);
            let flare = if next { 1.6 + pulse * 1.4 + cast * 3.0 } else { 0.9 };
            m.glow_voxel(at_slot - side * 0.004, Vec3::new(0.011, 0.013, 0.008), rot, *colour, flare, 0.95);
        }
    }

    // Pommel and two collars. Three stepped sizes make a readable pixel
    // silhouette instead of one uniform cylinder ending in a flat cap.
    m.cuboid(
        at(-0.012),
        Vec3::new(0.052, 0.026, 0.052),
        rot,
        scaled(shaft.dark, 0.82),
        0.0,
    );
    m.cuboid(
        at(0.002),
        Vec3::new(0.046, 0.012, 0.046),
        rot,
        shaft.accent,
        0.0,
    );
    for i in 0..3 {
        let s = length - 0.042 + i as f32 * 0.012;
        m.cuboid(
            at(s),
            Vec3::new(0.040 - i as f32 * 0.003, 0.010, 0.040 - i as f32 * 0.003),
            rot,
            if i % 2 == 0 { shaft.accent } else { shaft.dark },
            0.0,
        );
    }

    // Head.
    let crown = tip + dir * 0.04;
    match style.head {
        0 => {
            // A cut gem held in three prongs. The prongs stop at the gem's
            // equator so nothing crosses its face.
            for k in 0..3 {
                let o = ring(0.3 + k as f32 * 2.094);
                m.beam(
                    tip + o * 0.020,
                    tip + dir * 0.034 + o * 0.030,
                    0.0060,
                    shaft.accent,
                    0.2,
                );
            }
            voxel_gem(m, rot, crown, 0.030 + cast * 0.006, glow);
        }
        1 => {
            for k in 0..2 {
                let r = Quat::from_axis_angle(dir, k as f32 * 0.785) * rot;
                m.glow_voxel(
                    crown + dir * 0.02,
                    Vec3::new(0.034, 0.120 + cast * 0.02, 0.034),
                    r,
                    scaled(glow, 0.85),
                    power * 0.9,
                    0.78,
                );
            }
            voxel_orb(
                m,
                rot,
                crown + dir * 0.02,
                0.020,
                [1.0, 1.0, 1.0],
                power + 1.0,
            );
            voxel_glow_orb(m, rot, crown + dir * 0.02, 0.070, glow, power, 0.34);
        }
        2 => {
            for k in 0..3 {
                let a = v.time * 1.4 + k as f32 * 2.094;
                voxel_orb(
                    m,
                    rot,
                    crown + dir * 0.012 + ring(a) * 0.046,
                    0.021,
                    glow,
                    power + 0.6,
                );
            }
            voxel_orb(m, rot, crown, 0.016, [1.0, 1.0, 1.0], power + 1.2);
            voxel_glow_orb(m, rot, crown, 0.060, glow, power * 0.8, 0.30);
        }
        3 => {
            voxel_orb(m, rot, crown, 0.032 + cast * 0.008, glow, power + 0.8);
            for k in 0..5 {
                let a = v.time * 0.55 + k as f32 * 1.256;
                m.glow_voxel(
                    crown + ring(a) * 0.066,
                    Vec3::splat(0.014),
                    rot,
                    glow,
                    power,
                    0.88,
                );
                let b = -v.time * 0.4 + k as f32 * 1.256;
                let p = side * (b.cos() * 0.054) + dir * (b.sin() * 0.054);
                m.glow_voxel(
                    crown + p,
                    Vec3::splat(0.010),
                    rot,
                    scaled(glow, 0.8),
                    power * 0.9,
                    0.82,
                );
            }
        }
        4 => {
            m.cuboid(
                tip + dir * 0.012,
                Vec3::new(0.038, 0.020, 0.038),
                rot,
                [0.16, 0.08, 0.05],
                0.4,
            );
            for i in 0..7 {
                let h = i as f32 * 0.024;
                let wob = (side * (v.time * 1.4 + i as f32 * 0.7).sin()
                    + back * (v.time * 1.1 + i as f32).cos())
                    * 0.004
                    * i as f32;
                let size = 0.062 * (1.0 - i as f32 * 0.11) + cast * 0.010;
                let c = mix(glow, [1.0, 0.78, 0.40], (i as f32 / 9.0).min(0.6));
                m.glow_voxel(
                    tip + dir * (0.03 + h) + wob,
                    Vec3::splat(size.max(0.012)),
                    rot,
                    c,
                    (power * 0.7).min(2.6) - i as f32 * 0.12,
                    0.80,
                );
            }
        }
        _ => {
            let c = crown + dir * 0.005;
            let blink = (v.time / 4.3).fract() < 0.03;
            voxel_orb(m, rot, c, 0.040, [0.92, 0.95, 1.0], 0.9);
            if blink {
                m.cuboid(
                    c + v.forward * 0.01,
                    Vec3::new(0.090, 0.030, 0.060),
                    Quat::IDENTITY,
                    shaft.dark,
                    0.0,
                );
            } else {
                voxel_glow_orb(
                    m,
                    rot,
                    c + v.forward * 0.022,
                    0.025,
                    glow,
                    power + 0.6,
                    0.92,
                );
                m.cuboid(
                    c + v.forward * 0.040,
                    Vec3::new(0.008, 0.032, 0.008),
                    Quat::IDENTITY,
                    [0.02, 0.02, 0.04],
                    0.0,
                );
            }
        }
    }

    // ---- Magic ------------------------------------------------------------
    // Layered like a modern action-RPG spell: a hot core inside two coloured
    // halos, runes orbiting the head, wisps that spiral up and cool from white
    // to the spell's deep hue, and a rain of embers. Every particle is a pure
    // function of the adventure clock and a fixed seed, and every looping one
    // fades through `life()`, so nothing pops when it respawns.
    let vital = 0.4 + 0.6 * v.mana.clamp(0.0, 1.0).sqrt();
    let hot = mix(glow, [1.0, 1.0, 1.0], 0.62);
    let deep = scaled(glow, 0.75);
    let tau = std::f32::consts::TAU;

    // Runes circling the head. The last spell cast changes how many there are.
    let rune_count = 5 + (v.last_cast % 4);
    for k in 0..rune_count {
        let a = v.time * 0.9 + k as f32 * tau / rune_count as f32;
        let twinkle = 0.65 + 0.35 * (v.time * 2.3 + k as f32 * 1.9).sin();
        let pos = crown + ring(a) * 0.072 + dir * (0.018 * (a * 2.0 + k as f32).sin());
        let tangent = side * -a.sin() + back * a.cos();
        m.glow_voxel(
            pos,
            Vec3::new(0.005, 0.024 + 0.010 * cast, 0.005),
            aim(tangent),
            if k % 2 == 0 { hot } else { glow },
            power * 1.15 * twinkle * vital,
            0.9,
        );
    }

    // Wisps spiral up from the head, cooling from white to the deep hue.
    for k in 0..14u32 {
        let seed = hash01(k * 7 + 3);
        let ph = (v.time * (0.28 + 0.12 * seed) + k as f32 / 14.0).fract();
        let a = seed * tau + ph * (2.6 + seed * 2.0);
        let r = 0.018 + 0.07 * ph * (0.5 + seed);
        let p = crown
            + dir * (0.05 + ph * (0.20 + 0.10 * seed))
            + ring(a) * r
            + Vec3::Y * (ph * ph * 0.05);
        let e = life(ph);
        m.glow_voxel(
            p,
            Vec3::splat(0.004 + (0.009 * (0.6 + seed * 0.6) + cast * 0.004) * e),
            rot,
            mix(hot, deep, ph),
            (0.9 + cast * 2.4) * e * vital,
            0.85,
        );
    }

    // Embers fall away from the head under gravity.
    for k in 0..8u32 {
        let seed = hash01(k * 13 + 5);
        let ph = (v.time * (0.42 + 0.2 * seed) + k as f32 * 0.125 + seed).fract();
        let p = crown + dir * 0.06 + ring(seed * tau * 3.0) * (0.04 + 0.05 * ph)
            - Vec3::Y * (ph * ph * 0.28);
        let e = life(ph);
        m.glow_voxel(
            p,
            Vec3::splat(0.003 + 0.005 * e),
            rot,
            mix(glow, [1.0, 0.85, 0.5], 0.5),
            (1.0 + cast * 1.5) * e * vital,
            0.9,
        );
    }

    // Energy climbs the shaft toward the head in pulses.
    for j in 0..3u32 {
        let ph = (v.time * 0.55 + j as f32 / 3.0).fract();
        m.glow_voxel(
            at(length * (0.12 + 0.80 * ph)),
            Vec3::new(radius * 2.6, 0.014, radius * 2.6),
            rot,
            hot,
            (0.6 + 1.8 * cast + 0.8 * vital) * life(ph),
            0.55,
        );
    }

    // The cast itself: a star flare, an expanding shockwave and flung shards.
    if cast > 0.015 {
        let burst = cast;
        let centre = crown + v.forward * 0.05;
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4 + 0.3 + v.time * 0.2;
            let axis = v.right * a.cos() + v.up * a.sin();
            m.glow_voxel(
                centre,
                Vec3::new(0.004, 0.10 + 0.26 * burst, 0.004),
                aim(axis),
                if k % 2 == 0 { [1.0, 1.0, 1.0] } else { hot },
                3.2 * burst,
                0.9,
            );
        }
        voxel_glow_orb(
            m,
            rot,
            centre,
            0.024 + 0.03 * burst,
            [1.0, 1.0, 1.0],
            3.5 * burst,
            0.9,
        );
        let ring_r = 0.05 + 0.32 * (1.0 - (1.0 - burst) * (1.0 - burst));
        for k in 0..20 {
            let a = k as f32 * tau / 20.0 + 0.2;
            m.glow_voxel(
                centre + (v.right * a.cos() + v.up * a.sin()) * ring_r,
                Vec3::splat(0.007 + 0.004 * burst),
                rot,
                glow,
                2.4 * burst,
                0.8,
            );
        }
        for k in 0..10u32 {
            let seed = hash01(k * 11 + 1);
            let spread =
                (v.right * (seed * 2.0 - 1.0) + v.up * (hash01(k * 17 + 9) * 2.0 - 1.0)) * 0.35;
            let d = (v.forward + spread).normalize();
            m.glow_voxel(
                crown + d * (0.06 + 0.55 * burst * (0.5 + seed)),
                Vec3::new(0.005, 0.030 + 0.02 * burst, 0.005),
                aim(d),
                mix(glow, hot, seed),
                3.0 * burst,
                0.9,
            );
        }
    }

    // What the wand sheds. Particles fade in over the first sixth of their
    // life (`born`) so a respawn never appears at full size.
    let strength = 0.45 + 0.55 * cast;
    let born = |ph: f32| (ph * 6.0).min(1.0);
    match style.trail {
        1 => {
            for k in 0..7 {
                let ph = (v.time * 0.8 + k as f32 / 7.0).fract();
                let a = k as f32 * 2.4;
                let p = crown + ring(a) * (ph * 0.12) + dir * (ph * 0.04) - v.up * (ph * ph * 0.10);
                m.glow_voxel(
                    p,
                    Vec3::splat((0.013 * (1.0 - ph) * strength + 0.002) * born(ph)),
                    rot,
                    glow,
                    (1.4 + 2.2 * cast) * (1.0 - ph),
                    0.8,
                );
            }
        }
        2 => {
            for k in 0..3 {
                let a = v.time * 1.8 + k as f32 * 2.094;
                let c = crown + dir * 0.02 + ring(a) * 0.10;
                let r = Quat::from_rotation_z(a);
                m.glow_voxel(
                    c,
                    Vec3::new(0.034, 0.007, 0.007),
                    r,
                    [1.0, 0.97, 0.85],
                    2.6 * strength,
                    0.8,
                );
                m.glow_voxel(
                    c,
                    Vec3::new(0.007, 0.034, 0.007),
                    r,
                    [0.86, 0.78, 0.58],
                    2.6 * strength,
                    0.8,
                );
                m.glow_voxel(c, Vec3::splat(0.008), r, glow, 3.0 * strength, 0.8);
            }
        }
        3 => {
            for k in 0..8 {
                let ph = (v.time * 0.5 + k as f32 / 8.0).fract();
                let drift = (side * (k as f32 * 3.1 + v.time).sin()
                    + back * (k as f32 * 2.3).cos())
                    * 0.03
                    * ph;
                m.glow_voxel(
                    crown + dir * 0.03 + drift + Vec3::Y * (ph * 0.22),
                    Vec3::splat((0.011 * (1.0 - ph) * strength + 0.002) * born(ph)),
                    rot,
                    mix(glow, [1.0, 0.5, 0.2], 0.5),
                    (1.4 + 2.0 * cast) * (1.0 - ph),
                    0.8,
                );
            }
        }
        4 if style.head != 4 => {
            for i in 1..=12 {
                let f = i as f32 / 12.0;
                let wave = (side * (v.time * 1.2 - f * 2.0).sin()
                    + v.up * (v.time * 0.9 - f * 1.6).cos())
                    * 0.01
                    * f;
                m.glow_voxel(
                    crown + dir * (0.04 + f * 0.22) + wave,
                    Vec3::splat(0.020 * (1.0 - f * 0.8) * strength + 0.003),
                    rot,
                    mix(glow, [1.0, 1.0, 1.0], 0.3 * (1.0 - f)),
                    (1.6 + 2.4 * cast) * (1.0 - f),
                    0.7 * (1.0 - f * 0.4),
                );
            }
        }
        _ => {}
    }

    // The glyph of the spell just cast blooms ahead of the head.
    if v.cast > 0.02 {
        let glyph_rot = Quat::from_rotation_y(v.yaw)
            * Quat::from_rotation_x(-v.pitch)
            * Quat::from_rotation_z(v.time * 0.6);
        m.temple_glyph(
            crown + v.forward * 0.065,
            glyph_rot,
            v.last_cast,
            0.014 + v.cast * 0.004,
            glow,
            3.0,
            0.6,
        );
    }

    // Pull the authored rig into first-person scale around the grip. The
    // original sleeve is scaled with it; one unscaled connective beam reaches
    // back toward the camera so the arm never appears to float.
    let scaled_wrist = grip + (wrist - grip) * VIEWMODEL_SCALE;
    m.scale_since(rig_mark, grip, Vec3::splat(VIEWMODEL_SCALE));

    // Forearm, built after scaling so its thickness is not shrunk with the hand:
    // a tapered run of overlapping cloth blocks from the screen edge to the
    // cuff, with a fold highlight on the lit side, a leather bracer, and a rune
    // gem and mana studs that answer the spell and the cast.
    m.set_surface_kind(54.0);
    let arm = scaled_wrist - sleeve_end;
    let arm_len = arm.length().max(1e-3);
    let arm_dir = arm / arm_len;
    let arm_side = (v.right - arm_dir * v.right.dot(arm_dir)).normalize_or_zero();
    let arm_back = arm_side.cross(arm_dir);
    let arm_rot = Quat::from_mat3(&Mat3::from_cols(arm_side, arm_dir, arm_back));
    let cloth_lit = [0.50, 0.42, 0.33];
    let segments = 7;
    for i in 0..segments {
        let t = (i as f32 + 0.5) / segments as f32;
        let w = 0.150 + (0.098 - 0.150) * t;
        let d = w * 0.88;
        let centre = sleeve_end + arm_dir * (arm_len * t);
        let seg_len = arm_len / segments as f32 * 1.14;
        m.cuboid(
            centre,
            Vec3::new(w, seg_len, d),
            arm_rot,
            if i % 2 == 0 { cloth } else { cloth_dark },
            0.0,
        );
        // Fold highlight on the upper face, one pixel proud of the cloth.
        m.cuboid(
            centre + arm_back * (d * 0.5),
            Vec3::new(w * 0.46, seg_len * 0.9, 0.012),
            arm_rot,
            cloth_lit,
            0.0,
        );
    }
    // Leather bracer near the wrist, with a gold buckle.
    let bracer = sleeve_end + arm_dir * (arm_len * 0.80);
    m.cuboid(
        bracer,
        Vec3::new(0.118, 0.060, 0.104),
        arm_rot,
        glove_dark,
        0.0,
    );
    m.cuboid(
        bracer + arm_back * 0.052,
        Vec3::new(0.050, 0.030, 0.012),
        arm_rot,
        [0.85, 0.65, 0.30],
        0.0,
    );
    // Rune gem and mana studs. They glow in the spell's colour, flare on a
    // cast, and the studs fill with the mana the player has left.
    m.glow_voxel(
        bracer + arm_back * 0.060,
        Vec3::new(0.024, 0.024, 0.016),
        arm_rot,
        hot,
        (1.4 + 3.0 * cast) * vital,
        0.9,
    );
    for i in 0..4 {
        let lit = (v.mana.clamp(0.0, 1.0) * 4.0 - i as f32).clamp(0.0, 1.0);
        m.glow_voxel(
            sleeve_end + arm_dir * (arm_len * (0.50 + 0.07 * i as f32)) + arm_back * 0.082,
            Vec3::new(0.014, 0.012, 0.010),
            arm_rot,
            mix(shaft.dark, glow, lit),
            0.2 + 2.2 * lit,
            0.92,
        );
    }
    // Kind 55 has fixed matte roughness/reflectance in the shader, leaving
    // surface.x/y/w available for its enchantment's sRGB colour. Carry the
    // selected spell separately from the authored shaft/grip base colour and
    // cosmetic glow override. Only this wand's solid pieces receive metadata.
    for vertex in &mut m.vertices[rig_mark..] {
        if vertex.surface[2] == 55.0 {
            vertex.surface[0] = v.spell[0].clamp(0.0, 1.0);
            vertex.surface[1] = v.spell[1].clamp(0.0, 1.0);
            vertex.surface[3] = v.spell[2].clamp(0.0, 1.0);
        }
    }
    m.set_surface_kind(41.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewmodel_stays_inside_a_readable_first_person_bounds() {
        let mut mesh = Model::new(1.0);
        draw(
            &mut mesh,
            WandStyle::default(),
            &WandView {
                eye: Vec3::ZERO,
                forward: Vec3::NEG_Z,
                right: Vec3::X,
                up: Vec3::Y,
                yaw: 0.0,
                pitch: 0.0,
                time: 0.0,
                cast: 0.0,
                spell: [0.3, 0.9, 0.5],
                last_cast: 0,
                tier: 1,
                mana: 0.8,
                deck: vec![Some([1.0, 0.4, 0.1]), None, Some([0.3, 0.6, 1.0]), None],
                cursor: 2,
                seed: 41,
            },
        );
        assert!(!mesh.vertices.is_empty());
        assert!(
            mesh.vertices.iter().any(|v| v.surface[2] == 55.0),
            "wand body must use the solid wand material"
        );
        assert!(
            mesh.vertices.iter().any(|v| v.surface[2] == 54.0),
            "hand must use the skin/leather material"
        );
        let farthest = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.pos).length())
            .fold(0.0f32, f32::max);
        assert!(
            farthest < 1.35,
            "first-person wand reached {farthest:.2} m from the eye"
        );
    }
}
