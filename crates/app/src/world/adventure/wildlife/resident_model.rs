//! Player-proportioned resident bodies with stable faces and practical workwear.
use super::Enemy;
use crate::mesh::models::{AUTHORED_SURFACE, Model};
use crate::world::adventure::settlement_guard::STRIKE_INTERVAL;
use crate::world::farming::jobs::WorkPose;
use engine_core::hash::hash_u32;
use glam::{Quat, Vec3};

#[derive(Clone, Copy)]
enum Workwear {
    Apron,
    Vest,
    Shawl,
    Sash,
}

pub(super) fn draw(
    e: &Enemy,
    mesh: &mut Model,
    time: f32,
    work: Option<WorkPose>,
    cargo: bool,
    stationed: bool,
) {
    let id = e.site.x as u32;
    let key = hash_u32(id ^ 0x8d12_c65b) as usize;
    let outfit = match id {
        3 | 5 => Workwear::Shawl,
        4 => Workwear::Apron,
        crate::world::farming::freight::SELLER => Workwear::Sash,
        _ if engine_worldgen::households::role(id) == Some(1) => Workwear::Apron,
        _ => [
            Workwear::Apron,
            Workwear::Vest,
            Workwear::Shawl,
            Workwear::Sash,
        ][key & 3],
    };
    let coat = [
        [0.41, 0.27, 0.18],
        [0.30, 0.36, 0.24],
        [0.23, 0.33, 0.39],
        [0.42, 0.26, 0.25],
        [0.38, 0.35, 0.27],
        [0.31, 0.27, 0.36],
    ][(key >> 2) % 6];
    let skin = [
        [0.73, 0.52, 0.38],
        [0.56, 0.35, 0.24],
        [0.39, 0.24, 0.16],
        [0.65, 0.42, 0.29],
    ][(key >> 6) & 3];
    let hair = [
        [0.13, 0.085, 0.06],
        [0.27, 0.16, 0.095],
        [0.43, 0.28, 0.14],
        [0.44, 0.43, 0.39],
    ][(key >> 8) & 3];
    let trim = coat.map(|c| c * 1.23);
    let trousers = coat.map(|c| c * 0.57);
    let leather = [0.20, 0.13, 0.09];
    let linen = [0.57, 0.48, 0.35];
    let width = [0.94, 1.0, 1.06][(key >> 10) % 3];
    let height = [0.94, 0.97, 1.0][(key >> 12) % 3];
    let gait = e.gait.clamp(0.0, 1.0);
    let work = work.filter(|_| !super::seeking_safety(e) && gait < 0.25);
    let guard_swing = if stationed && e.tell > 0.0 {
        0.6 * (e.tell / 0.34).clamp(0.0, 1.0)
    } else if stationed {
        (1.0 - (STRIKE_INTERVAL - e.cooldown) / 0.28).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let phase = e.phase * 5.0;
    let breath = (time * 1.4 + (key & 255) as f32).sin() * 0.007 * (1.0 - gait);

    mesh.with_surface_kind(AUTHORED_SURFACE, |mesh| {
        let neutral = Quat::IDENTITY;
        let proportions = Vec3::new(width, height, 1.0);
        let mut part = |pivot: Vec3, rotation: Quat, offset: Vec3, size: Vec3, color| {
            let center = (pivot + rotation * offset) * proportions;
            mesh.cuboid(e.pos + center, size * proportions, rotation, color, 0.0);
        };
        part(
            Vec3::ZERO,
            neutral,
            Vec3::new(0.0, 1.13 + breath, 0.0),
            Vec3::new(0.48, 0.57, 0.29),
            coat,
        );
        part(
            Vec3::ZERO,
            neutral,
            Vec3::new(0.0, 0.88, 0.0),
            Vec3::new(0.49, 0.07, 0.30),
            leather,
        );
        part(
            Vec3::ZERO,
            neutral,
            Vec3::new(0.0, 0.88, -0.153),
            Vec3::new(0.075, 0.05, 0.018),
            [0.54, 0.38, 0.20],
        );

        for side in [-1.0, 1.0] {
            let cycle = phase
                + if side > 0.0 {
                    std::f32::consts::PI
                } else {
                    0.0
                };
            let stride = cycle.sin() * gait;
            let lift = cycle.cos().max(0.0) * 0.09 * gait;
            let hip = Vec3::new(side * 0.122, 0.82, 0.0);
            let ankle = Vec3::new(hip.x, 0.12 + lift, -stride * 0.23);
            let knee = (hip + ankle) * 0.5 - Vec3::Z * (0.025 + lift * 0.6);
            for (top, bottom, width, depth, color) in [
                (hip, knee, 0.20, 0.22, trousers),
                (knee, ankle, 0.17, 0.19, trousers.map(|c| c * 0.9)),
            ] {
                let segment = top - bottom;
                part(
                    (top + bottom) * 0.5,
                    Quat::from_rotation_arc(Vec3::Y, segment.normalize()),
                    Vec3::ZERO,
                    Vec3::new(width, segment.length() + 0.035, depth),
                    color,
                );
            }
            part(
                ankle,
                neutral,
                Vec3::new(0.0, -0.052, -0.04),
                Vec3::new(0.22, 0.136, 0.30),
                leather,
            );

            let shoulder = Vec3::new(side * 0.307, 1.37 + breath, 0.0);
            let (reach, spread) = match work {
                Some(WorkPose::Mine { phase, .. }) if side > 0.0 => {
                    let swing = (1.0 - phase).powi(2);
                    (0.9 + 1.6 * swing, 0.03)
                }
                Some(WorkPose::Place { elapsed, .. }) => (1.4 * (1.0 - elapsed), -0.24),
                _ if cargo => (0.86 + stride * 0.07, -0.28),
                _ if stationed && side > 0.0 => (0.30 + guard_swing * 1.25, 0.08),
                _ => (-stride * 0.42, 0.08),
            };
            let arm = Quat::from_rotation_x(reach) * Quat::from_rotation_z(side * spread);
            part(
                shoulder,
                arm,
                Vec3::new(0.0, -0.23, 0.0),
                Vec3::new(0.16, 0.48, 0.21),
                coat,
            );
            part(
                shoulder,
                arm,
                Vec3::new(0.0, -0.43, 0.0),
                Vec3::new(0.17, 0.07, 0.22),
                trim,
            );
            part(
                shoulder,
                arm,
                Vec3::new(0.0, -0.50, 0.0),
                Vec3::new(0.14, 0.14, 0.16),
                skin,
            );
            if let Some(WorkPose::Mine { timber, .. }) = work
                && side > 0.0
            {
                part(
                    shoulder,
                    arm,
                    Vec3::new(0.0, -0.60, 0.025),
                    Vec3::new(0.044, 0.33, 0.045),
                    leather,
                );
                let shape = if timber {
                    Vec3::new(0.08, 0.15, 0.18)
                } else {
                    Vec3::new(0.06, 0.075, 0.24)
                };
                part(
                    shoulder,
                    arm,
                    Vec3::new(0.0, -0.74, 0.02),
                    shape,
                    [0.47, 0.50, 0.53],
                );
            }
        }

        if cargo && !matches!(work, Some(WorkPose::Mine { .. })) {
            let lift = gait * (phase * 2.0).sin() * 0.018;
            part(
                Vec3::ZERO,
                neutral,
                Vec3::new(0.0, 1.02 + lift, -0.45),
                Vec3::new(0.42, 0.25, 0.25),
                [0.40, 0.28, 0.17],
            );
            for x in [-0.14, 0.14] {
                part(
                    Vec3::ZERO,
                    neutral,
                    Vec3::new(x, 1.02 + lift, -0.578),
                    Vec3::new(0.045, 0.26, 0.018),
                    linen,
                );
            }
        }

        let mut detail = |x, y, z, w, h, d, color| {
            part(
                Vec3::ZERO,
                neutral,
                Vec3::new(x, y + breath, z),
                Vec3::new(w, h, d),
                color,
            );
        };
        detail(-0.18, 0.91, 0.13, 0.13, 0.15, 0.10, leather);
        detail(-0.18, 0.95, 0.183, 0.12, 0.052, 0.016, [0.31, 0.21, 0.13]);
        if stationed {
            detail(0.19, 1.33, -0.157, 0.092, 0.105, 0.026, [0.36, 0.47, 0.37]);
            detail(0.19, 1.34, -0.175, 0.035, 0.045, 0.012, [0.66, 0.52, 0.28]);
        } else {
            detail(0.19, 1.31, -0.153, 0.064, 0.048, 0.02, linen);
        }
        match outfit {
            Workwear::Apron => {
                detail(0.0, 1.11, -0.155, 0.33, 0.39, 0.032, leather);
                detail(0.0, 0.83, -0.165, 0.35, 0.23, 0.045, leather);
                detail(0.076, 1.01, -0.177, 0.14, 0.12, 0.016, [0.32, 0.22, 0.15]);
            }
            Workwear::Vest => {
                for side in [-1.0, 1.0] {
                    detail(side * 0.155, 1.14, 0.0, 0.18, 0.47, 0.32, trim);
                }
            }
            Workwear::Shawl => {
                detail(0.0, 1.36, 0.0, 0.55, 0.13, 0.34, trim);
                detail(-0.07, 1.21, -0.156, 0.13, 0.22, 0.04, trim);
                detail(0.05, 1.29, -0.179, 0.045, 0.05, 0.019, linen);
            }
            Workwear::Sash => {
                detail(-0.12, 1.14, -0.157, 0.10, 0.47, 0.028, linen);
                detail(0.16, 0.87, 0.15, 0.15, 0.20, 0.11, leather);
            }
        }
        detail(0.0, 1.47, 0.0, 0.21, 0.14, 0.22, skin);
        detail(0.0, 1.70, 0.0, 0.36, 0.30, 0.34, skin);
        detail(0.0, 1.56, -0.012, 0.29, 0.12, 0.31, skin);
        detail(0.0, 1.82, 0.0, 0.38, 0.09, 0.36, hair);
        detail(0.0, 1.70, 0.17, 0.36, 0.23, 0.04, hair);
        for side in [-1.0, 1.0] {
            detail(
                side * 0.18,
                1.665,
                0.009,
                0.052,
                0.09,
                0.075,
                skin.map(|c| c * 0.93),
            );
            detail(side * 0.09, 1.727, -0.177, 0.075, 0.017, 0.013, hair);
            detail(
                side * 0.091,
                1.69,
                -0.174,
                0.07,
                0.037,
                0.013,
                [0.73, 0.67, 0.56],
            );
            detail(
                side * 0.084,
                1.69,
                -0.183,
                0.027,
                0.036,
                0.010,
                [0.095, 0.105, 0.10],
            );
        }
        detail(0.0, 1.63, -0.177, 0.052, 0.06, 0.04, skin.map(|c| c * 0.88));
        detail(
            0.0,
            1.565,
            -0.171,
            0.069,
            0.015,
            0.013,
            skin.map(|c| c * 0.57),
        );
        match (key >> 15) & 3 {
            0 => detail(-0.10, 1.77, -0.175, 0.15, 0.065, 0.024, hair),
            1 => detail(0.0, 1.57, -0.172, 0.16, 0.085, 0.04, hair),
            2 => {
                detail(0.0, 1.82, 0.0, 0.40, 0.095, 0.38, trim);
                detail(0.0, 1.79, -0.183, 0.38, 0.045, 0.09, trim);
            }
            _ => detail(0.0, 1.77, 0.0, 0.38, 0.065, 0.365, linen),
        }
    });
}
