//! Authored crystal stag: longitudinal body, articulated hocks and a branching crown.
use super::*;

#[derive(Clone, Copy, Default)]
pub struct HartPose {
    pub gait: f32,
    pub moving: f32,
    pub bow: f32,
    pub rear: f32,
    pub turn: f32,
    pub recover: f32,
    pub breath: f32,
}

impl Model {
    // Bake coordinates before rigid-part animation and world rotation. The pixel
    // material therefore travels with the animal instead of swimming as it walks.
    fn hart_uv(&mut self, start: usize, origin: Vec3) {
        if self.surface_kind != 57.0 {
            return;
        }
        for v in &mut self.vertices[start..] {
            if v.surface[2] != 57.0 {
                continue;
            }
            let p = Vec3::from_array(v.pos) * self.scale - origin;
            let n = Vec3::from_array(v.normal).abs();
            let uv = if n.y >= n.x && n.y >= n.z {
                [p.x, p.z]
            } else if n.x >= n.z {
                [p.z, p.y]
            } else {
                [p.x, p.y]
            };
            v.surface[0] = uv[0];
            v.surface[1] = uv[1];
        }
    }
    /// Flat-shaded eight-sided loft. Rings are (z, y, half-width, half-height).
    fn hart_loft(&mut self, origin: Vec3, rings: &[[f32; 4]], color: [f32; 3], glow: f32) {
        let start = self.mark();
        let point = |r: [f32; 4], i: usize| {
            let a = (i % 8) as f32 * std::f32::consts::TAU / 8.0;
            origin + Vec3::new(a.cos() * r[2], r[1] + a.sin() * r[3], r[0])
        };
        for pair in rings.windows(2) {
            for i in 0..8 {
                let a = point(pair[0], i);
                let b = point(pair[0], i + 1);
                let c = point(pair[1], i + 1);
                let d = point(pair[1], i);
                let n = (d - a).cross(b - a).normalize_or_zero();
                let tone = [0.76, 0.94, 1.08, 1.0, 0.85, 0.64, 0.56, 0.68][i];
                let tint = color.map(|v| (v * tone).min(1.0));
                self.triangle([a, b, c], n, tint, glow);
                self.triangle([a, c, d], n, tint, glow);
            }
        }
        for (r, n) in [(rings[0], Vec3::Z), (*rings.last().unwrap(), Vec3::NEG_Z)] {
            let center = origin + Vec3::new(0.0, r[1], r[0]);
            for i in 0..8 {
                self.triangle([center, point(r, i), point(r, i + 1)], n, color, glow);
            }
        }
        for vertex in &mut self.vertices[start..] {
            vertex.surface[0] = 0.78;
            // A dielectric crystal, not the machinery mesh's very reflective fallback.
            vertex.surface[1] = 0.035;
        }
        // Bake each loft before its own ear/head rigid transform. Thin articulated
        // beams keep a uniform finish rather than sampling after their joints move.
        self.hart_uv(start, origin);
    }

    pub fn crystal_hart(
        &mut self,
        feet: Vec3,
        yaw: f32,
        pose: HartPose,
        jewel: [f32; 3],
        flash: bool,
    ) {
        let mark = self.mark();
        let previous_material = self.surface_kind;
        // The rime custodian is dark hide beneath aged ceramic, with narrow warm vents.
        // Keep named rigid parts and deliberate silhouette clumps; no micro-voxel cloud.
        let rime = jewel[2] > jewel[0] * 3.0;
        self.surface_kind = if rime { 57.0 } else { 56.0 };
        let body = if flash {
            [0.88, 0.95, 1.0]
        } else if rime {
            [0.115, 0.091, 0.078]
        } else {
            jewel.map(|c| c * 0.50 + 0.025)
        };
        let edge = if rime {
            [0.56, 0.53, 0.45]
        } else {
            jewel.map(|c| c * 0.52 + 0.38)
        };
        let ivory = if rime {
            [0.31, 0.28, 0.24]
        } else {
            [0.66, 0.76, 0.78]
        };
        let hoof = [0.10, 0.14, 0.19];
        let bob = pose.breath * 0.022 + pose.moving * (pose.gait * 2.0).cos() * 0.025;
        let body_origin = feet + Vec3::Y * (bob - pose.bow * 0.13 + pose.rear * 0.08);
        self.hart_loft(
            body_origin,
            &[
                [1.02, 1.62, 0.22, 0.30],
                [0.65, 1.66, 0.49, 0.53],
                [0.10, 1.73, 0.39, 0.39],
                [-0.35, 1.78, 0.59, 0.73],
                [-0.72, 1.85, 0.48, 0.68],
                [-0.94, 1.90, 0.32, 0.48],
            ],
            body,
            0.0,
        );
        // Brisket and rising neck join the shoulder instead of floating above a box.
        self.hart_loft(
            body_origin,
            &[
                [-0.38, 1.63, 0.39, 0.61],
                [-0.73, 1.90, 0.44, 0.69],
                [-1.00, 2.27, 0.32, 0.51],
                [-1.20, 2.56, 0.17, 0.26],
            ],
            body.map(|c| c * 1.10),
            0.0,
        );
        self.beam(
            body_origin + Vec3::new(0.0, 1.26, -0.66),
            body_origin + Vec3::new(0.0, 2.41, -1.18),
            0.13,
            ivory,
            0.0,
        );
        if rime {
            for side in [-1.0_f32, 1.0] {
                // Three overlapping mantle plates and recessed heat-rejection slots.
                for i in 0..3 {
                    let p = body_origin
                        + Vec3::new(
                            side * (0.48 + i as f32 * 0.025),
                            2.12 - i as f32 * 0.17,
                            -0.62 + i as f32 * 0.20,
                        );
                    self.cuboid(
                        p,
                        Vec3::new(0.17, 0.66, 0.31),
                        Quat::from_rotation_z(side * (0.16 + pose.recover * 0.22)),
                        edge,
                        0.0,
                    );
                    self.cuboid(
                        p + Vec3::new(side * 0.091, 0.05, -0.03),
                        Vec3::new(0.018, 0.30, 0.055),
                        Quat::IDENTITY,
                        [0.74, 0.30, 0.065],
                        0.35 + pose.recover * 0.7,
                    );
                }
                // A small number of tapered fur locks carries the shaggy silhouette.
                for i in 0..7 {
                    let z = -0.94 + i as f32 * 0.19;
                    self.beam(
                        body_origin + Vec3::new(side * 0.34, 1.67, z),
                        body_origin
                            + Vec3::new(
                                side * (0.38 + (i % 2) as f32 * 0.04),
                                0.92 + (i % 3) as f32 * 0.13,
                                z + 0.10,
                            ),
                        0.085,
                        body.map(|c| c * (0.72 + (i % 3) as f32 * 0.12)),
                        0.0,
                    );
                }
            }
        }
        // Four two-joint legs. Opposing diagonal pairs share a distance-driven gait;
        // feet stay on the floor through stance and lift only during the swing.
        for side in [-1.0_f32, 1.0] {
            for front in [false, true] {
                let z = if front { -0.60 } else { 0.69 };
                let phase = pose.gait
                    + if (side > 0.0) == front {
                        0.0
                    } else {
                        std::f32::consts::PI
                    };
                let swing = phase.sin();
                let foot = feet
                    + Vec3::new(
                        side * 0.33,
                        0.10 + swing.max(0.0) * 0.22 * pose.moving
                            + if front { pose.rear * 0.3 } else { 0.0 },
                        z + phase.cos() * 0.34 * pose.moving
                            + if front { pose.rear * 0.23 } else { 0.0 },
                    );
                let hip = body_origin + Vec3::new(side * 0.31, if front { 1.54 } else { 1.50 }, z);
                let knee = feet
                    + Vec3::new(
                        side * 0.38,
                        0.79 + bob * 0.3 + if front { pose.rear * 0.4 } else { 0.0 },
                        z + if front { -0.13 } else { 0.27 } + swing * 0.09 * pose.moving,
                    );
                self.beam(hip, knee, if front { 0.12 } else { 0.15 }, body, 0.0);
                self.beam(knee, foot + Vec3::Y * 0.12, 0.063, ivory, 0.0);
                for toe in [-1.0_f32, 1.0] {
                    self.cuboid(
                        foot + Vec3::X * (toe * 0.049),
                        Vec3::new(0.075, 0.20, 0.25),
                        Quat::IDENTITY,
                        hoof,
                        0.0,
                    );
                }
                // The split hoof is a structural silver seam, not noise over the whole leg.
                self.cuboid(
                    foot + Vec3::new(0.0, 0.01, -0.129),
                    Vec3::new(0.025, 0.15, 0.015),
                    Quat::IDENTITY,
                    edge,
                    0.0,
                );
            }
        }
        self.beam(
            body_origin + Vec3::new(0.0, 1.95, 0.90),
            body_origin + Vec3::new(0.0, 1.72, 1.23),
            0.09,
            ivory,
            0.0,
        );
        let head = self.mark();
        let pivot = body_origin + Vec3::new(0.0, 2.40, -1.10);
        self.hart_loft(
            body_origin,
            &[
                [-1.03, 2.67, 0.15, 0.25],
                [-1.31, 2.68, 0.25, 0.30],
                [-1.58, 2.56, 0.17, 0.21],
                [-1.97, 2.42, 0.085, 0.095],
                [-2.03, 2.41, 0.09, 0.09],
            ],
            edge,
            0.0,
        );
        self.cuboid(
            body_origin + Vec3::new(0.0, 2.41, -2.055),
            Vec3::new(0.11, 0.09, 0.075),
            Quat::IDENTITY,
            hoof,
            0.0,
        );
        self.hart_loft(
            body_origin,
            &[
                [-1.25, 2.43, 0.17, 0.12],
                [-1.65, 2.33, 0.14, 0.09],
                [-1.96, 2.31, 0.085, 0.06],
            ],
            edge.map(|c| c * 0.72),
            0.0,
        );
        for side in [-1.0_f32, 1.0] {
            // Leaf-shaped ears use tapered cut facets, set apart from the antler roots.
            let ear = self.mark();
            self.hart_loft(
                body_origin + Vec3::new(side * 0.31, 2.91, -1.12),
                &[
                    [0.13, 0.0, 0.025, 0.04],
                    [0.0, 0.17, 0.105, 0.22],
                    [-0.09, 0.38, 0.015, 0.02],
                ],
                body,
                0.0,
            );
            self.rotate_since(
                ear,
                body_origin + Vec3::new(side * 0.31, 2.91, -1.12),
                Quat::from_rotation_z(-side * 0.60),
            );
            self.cuboid(
                body_origin + Vec3::new(side * 0.217, 2.70, -1.43),
                Vec3::new(0.06, 0.14, 0.20),
                Quat::from_rotation_y(side * 0.25),
                hoof,
                0.0,
            );
            self.beam(
                body_origin + Vec3::new(side * 0.20, 2.81, -1.30),
                body_origin + Vec3::new(side * 0.27, 2.78, -1.53),
                0.045,
                edge,
                0.0,
            );
            self.cuboid(
                body_origin + Vec3::new(side * 0.252, 2.70, -1.43),
                Vec3::new(0.035, 0.065, 0.11),
                Quat::from_rotation_y(side * 0.25),
                jewel,
                1.25,
            );
            let root = body_origin + Vec3::new(side * 0.16, 2.90, -1.17);
            let a = root + Vec3::new(side * 0.43, 0.32, 0.11);
            let b = root + Vec3::new(side * 1.05, 0.50, 0.31);
            let c = root + Vec3::new(side * 1.65, 0.73, 0.49);
            for (start, end, radius) in [(root, a, 0.14), (a, b, 0.12), (b, c, 0.08)] {
                self.beam(start, end, radius, edge, 0.08);
            }
            if rime {
                for i in 0..6 {
                    let t = i as f32 / 5.0;
                    let p = a.lerp(c, t);
                    let tip = p + Vec3::new(side * 0.08, 0.48 + t * 0.42, 0.04);
                    self.beam(p, tip, 0.07, edge.map(|v| v * 0.72), 0.0);
                    self.beam(
                        tip - Vec3::Y * 0.07,
                        tip + Vec3::Y * 0.035,
                        0.075,
                        [0.74, 0.80, 0.81],
                        0.0,
                    );
                    if i % 2 == 0 {
                        self.beam(
                            p + Vec3::Y * 0.12,
                            p + Vec3::Y * 0.32,
                            0.022,
                            [0.69, 0.28, 0.06],
                            0.35,
                        );
                    }
                }
            }
            for (start, delta, r) in [
                (a, Vec3::new(side * 0.03, 0.39, -0.26), 0.048),
                (b, Vec3::new(side * 0.06, 0.49, -0.18), 0.040),
                (c, Vec3::new(side * 0.22, 0.22, -0.19), 0.027),
            ] {
                let tip = start + delta;
                self.beam(start, tip, r, edge, 0.08);
                self.beam(
                    tip,
                    tip + Vec3::new(side * 0.02, 0.15, -0.025),
                    r * 0.35,
                    jewel,
                    0.25,
                );
            }
        }
        self.rotate_since(
            head,
            pivot,
            Quat::from_rotation_y(pose.turn)
                * Quat::from_rotation_x(pose.bow * -0.72 + pose.recover * 0.17 + pose.rear * 0.30),
        );
        // A small sternum gem is the visual focal point, framed by three cut plates.
        self.orb(
            body_origin + Vec3::new(0.0, 1.88, -0.96),
            0.115,
            if rime { [0.65, 0.27, 0.06] } else { jewel },
            0.35 + pose.recover * 0.45,
        );
        for side in [-1.0_f32, 1.0] {
            self.beam(
                body_origin + Vec3::new(side * 0.13, 1.64, -0.89),
                body_origin + Vec3::new(side * 0.25, 2.05, -0.93),
                0.028,
                edge,
                0.0,
            );
        }
        self.rotate_since(mark, feet, Quat::from_rotation_y(yaw));
        self.surface_kind = previous_material;
    }
}
