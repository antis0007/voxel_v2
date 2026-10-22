//! Short-lived material chips and settling motes from successful edits only.
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

pub struct Burst {
    pub pos: Vec3,
    pub color: [f32; 3],
    pub age: f32,
    pub placing: bool,
    pub scale: f32,
}

/// World-space blast feedback: readable at every voxel scale, with a bounded
/// fragment count regardless of how many terrain cells the crater contains.
pub struct Blast {
    pub pos: Vec3,
    pub normal: Vec3,
    pub radius: f32,
    pub color: [f32; 3],
    pub age: f32,
}

/// A short-lived surface contact ring. The physical solver is still the voxel
/// volume field; this is the visible pressure disturbance at the free surface
/// when a player, projectile or rigid body crosses it.
pub struct Splash {
    pub pos: Vec3,
    pub radius: f32,
    pub age: f32,
    pub life: f32,
    pub strength: f32,
}

pub fn draw_splashes(mesh: &mut Model, splashes: &[Splash]) {
    const SEGMENTS: usize = 24;
    for splash in splashes {
        let t = (splash.age / splash.life.max(0.001)).clamp(0.0, 1.0);
        let fade = (1.0 - t).powi(2) * splash.strength.clamp(0.0, 1.0);
        if fade <= 0.002 {
            continue;
        }
        let radius = splash.radius * (0.22 + 0.92 * t.sqrt());
        let inner = radius * (0.58 + t * 0.18);
        let width = (0.010 + 0.027 * (1.0 - t)) * splash.radius.sqrt();
        let origin = splash.pos + Vec3::Y * 0.012;
        let foam = [0.74, 0.92, 0.93];
        let water = [0.30, 0.68, 0.72];
        for i in 0..SEGMENTS {
            let a = i as f32 * std::f32::consts::TAU / SEGMENTS as f32;
            let b = (i + 1) as f32 * std::f32::consts::TAU / SEGMENTS as f32;
            let ca = Vec3::new(a.cos(), 0.0, a.sin());
            let cb = Vec3::new(b.cos(), 0.0, b.sin());
            mesh.beam(
                origin + ca * radius,
                origin + cb * radius,
                width,
                foam,
                3.0 * fade,
            );
            // An inner, slower ring reads as the trough behind the crest.
            mesh.beam(
                origin + ca * inner,
                origin + cb * inner,
                width * 0.55,
                water,
                1.35 * fade,
            );
        }
        // A few short crown jets make the contact legible when the camera is
        // low and the ring alone is foreshortened.
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0 + t * 0.7;
            let r = radius * 0.78;
            let base = origin + Vec3::new(a.cos(), 0.0, a.sin()) * r;
            let lift = (0.16 + 0.42 * splash.strength) * (1.0 - t) * fade;
            mesh.beam(base, base + Vec3::Y * lift, width * 0.72, foam, 2.4 * fade);
        }
    }
}

pub fn draw_blasts(mesh: &mut Model, blasts: &[Blast]) {
    for b in blasts {
        let t = b.age;
        let fade = (1.0 - t / 1.2).max(0.0);
        let n = b.normal.normalize_or_zero();
        let tangent = n
            .cross(if n.y.abs() > 0.9 { Vec3::X } else { Vec3::Y })
            .normalize_or_zero();
        let bitangent = n.cross(tangent);
        let origin = b.pos + n * 0.15;
        // A hot pressure ring travels across the impact plane, followed by
        // larger stone fragments and cooler, slower dust.
        if t < 0.38 {
            let r = b.radius * (t / 0.38).sqrt();
            let glow = 1.0 - t / 0.38;
            for i in 0..32 {
                let a = i as f32 * std::f32::consts::TAU / 32.0;
                let z = (i + 1) as f32 * std::f32::consts::TAU / 32.0;
                mesh.beam(
                    origin + (tangent * a.cos() + bitangent * a.sin()) * r,
                    origin + (tangent * z.cos() + bitangent * z.sin()) * r,
                    (0.028 + b.radius * 0.009) * glow,
                    [1.0, 0.64, 0.19],
                    5.0 * glow,
                );
            }
            // Keep the pressure ring, but leave its centre open after a brief
            // pixel flash so an enemy's next tell stays readable through a hit.
            if t < 0.10 {
                mesh.glow_voxel(
                    origin + n * t * 2.0,
                    Vec3::splat((b.radius * 0.08).min(0.15) * (1.0-t/0.10)),
                    Quat::IDENTITY,
                    [1.0,0.83,0.42],
                    1.8*glow,
                    0.8,
                );
            }
        }
        for i in 0..24 {
            let a = i as f32 * 2.39996;
            let ring = tangent * a.cos() + bitangent * a.sin();
            let speed = b.radius * (1.2 + (i % 5) as f32 * 0.22);
            let velocity =
                (ring * 0.7 + n * (0.45 + (i % 3) as f32 * 0.18) + Vec3::Y * 0.3) * speed;
            let p = origin + ring * b.radius * 0.12 + velocity * t - Vec3::Y * 4.9 * t * t;
            let size = ((0.035 + (i % 4) as f32 * 0.015) * b.radius.sqrt()).min(0.12) * fade;
            mesh.cuboid(
                p,
                Vec3::splat(size),
                Quat::from_rotation_y(a + t * 5.0) * Quat::from_rotation_x(a + t * 3.0),
                b.color,
                0.0,
            );
            if i % 4 == 0 {
                let dust = origin
                    + ring * b.radius * t * 0.65
                    + n * t * b.radius * 0.6
                    + Vec3::Y * t * 0.8;
                let c = b.color.map(|v| v * 0.65 + 0.16);
                mesh.glow_voxel(dust,Vec3::splat(0.05*fade),Quat::IDENTITY,c,0.0,0.45*fade);
            }
        }
    }
}

pub fn draw(mesh: &mut Model, bursts: &[Burst]) {
    for burst in bursts {
        let t = burst.age;
        let fade = (1.0 - t / 0.55).clamp(0.0, 1.0);
        for i in 0..7 {
            let a = i as f32 * 2.39996 + burst.pos.x * 0.31;
            let radial = Vec3::new(a.cos(), 0.5 + (i % 3) as f32 * 0.24, a.sin());
            let offset = if burst.placing {
                radial * (0.28 * fade * burst.scale)
            } else {
                radial * t * (0.7 + (i % 3) as f32 * 0.25) * burst.scale.sqrt()
                    - Vec3::Y * 2.4 * t * t
            };
            mesh.cuboid(
                burst.pos + offset,
                Vec3::splat((0.018 + (i % 3) as f32 * 0.008) * fade * burst.scale.sqrt()),
                Quat::from_rotation_y(a + t * 3.0),
                burst.color,
                0.0,
            );
        }
    }
}
