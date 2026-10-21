//! Lit, metre-scaled entity geometry, streamed separately from immutable terrain.

//! Moving parts advance from simulation state, never wall-clock render time.

use super::Vertex3;

use crate::world::{
    World,
    factory::{self, Kind},
};

use glam::{IVec3, Quat, Vec3};

mod wand;

pub struct Model {
    pub vertices: Vec<Vertex3>,

    scale: f32,

    surface_kind: f32,

    alpha: f32,

    /// 0 = intact, 1 = fully consumed. Written into the mesh specular channel
    /// for glow voxels; `world_shader.wgsl` reads it as a 3D pixel-dissolve
    /// threshold for micro-voxel particles.
    dissolve: f32,
}

impl Model {
    pub fn mark(&self) -> usize {
        self.vertices.len()
    }

    pub fn rotate_since(&mut self, mark: usize, pivot: Vec3, rotation: Quat) {
        for vertex in &mut self.vertices[mark..] {
            vertex.pos = ((pivot + rotation * (Vec3::from_array(vertex.pos) * self.scale - pivot))
                / self.scale)
                .to_array();

            vertex.normal = (rotation * Vec3::from_array(vertex.normal)).to_array();
        }
    }

    pub fn scale_since(&mut self, mark: usize, pivot: Vec3, stretch: Vec3) {
        for v in &mut self.vertices[mark..] {
            v.pos = ((pivot + (Vec3::from_array(v.pos) * self.scale - pivot) * stretch)
                / self.scale)
                .to_array();

            v.normal = (Vec3::from_array(v.normal) / stretch)
                .normalize_or_zero()
                .to_array();
        }
    }

    fn new(scale: f32) -> Self {
        Self {
            vertices: Vec::with_capacity(65536),

            scale,

            surface_kind: 41.0,

            alpha: 1.0,

            dissolve: 0.0,
        }
    }

    /// Select the shader material for non-glow model pieces. The wand hand uses
    /// the dedicated skin/leather branch instead of inheriting brushed machinery.
    pub fn set_surface_kind(&mut self, kind: f32) {
        self.surface_kind = kind;
    }

    fn triangle(&mut self, mut p: [Vec3; 3], normal: Vec3, color: [f32; 3], glow: f32) {
        if (p[1] - p[0]).cross(p[2] - p[0]).dot(normal) < 0.0 {
            p.swap(1, 2);
        }

        let roughness_or_alpha = if glow > 0.0 && self.alpha < 0.995 {
            -self.alpha.clamp(0.06, 0.98)
        } else {
            0.48
        };

        for p in p {
            self.vertices.push(Vertex3 {
                pos: (p / self.scale).to_array(),

                normal: normal.to_array(),

                color,

                surface: [
                    roughness_or_alpha,
                    // Negative is the dissolve tag. Normal glow meshes keep the
                    // old 0.42 specular value, so they are never mistaken for a
                    // particle half-way through its erosion.
                    if self.dissolve > 0.0 {
                        -self.dissolve.clamp(0.0, 1.0)
                    } else {
                        0.42
                    },
                    if glow > 0.0 { 40.0 } else { self.surface_kind },
                    glow,
                ],

                ao_strength: 1.0,
            });
        }
    }

    pub fn cuboid(&mut self, c: Vec3, size: Vec3, rot: Quat, color: [f32; 3], glow: f32) {
        for (n, u, v) in [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (-Vec3::X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (-Vec3::Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (-Vec3::Z, Vec3::Y, Vec3::X),
        ] {
            let face = n * size * 0.5;

            let u = u * size * 0.5;

            let v = v * size * 0.5;

            let p = [
                c + rot * (face - u - v),
                c + rot * (face + u - v),
                c + rot * (face + u + v),
                c + rot * (face - u + v),
            ];

            self.triangle([p[0], p[1], p[2]], rot * n, color, glow);

            self.triangle([p[0], p[2], p[3]], rot * n, color, glow);
        }
    }

    pub fn beam(&mut self, a: Vec3, b: Vec3, radius: f32, color: [f32; 3], glow: f32) {
        let axis = (b - a).normalize_or_zero();

        if axis.length_squared() < 0.5 {
            return;
        }

        let u = axis
            .cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X })
            .normalize();

        let v = axis.cross(u);

        for i in 0..8 {
            let t = i as f32 * std::f32::consts::TAU / 8.0;

            let t2 = (i + 1) as f32 * std::f32::consts::TAU / 8.0;

            let r = (u * t.cos() + v * t.sin()) * radius;

            let r2 = (u * t2.cos() + v * t2.sin()) * radius;

            let n = (r + r2).normalize();

            self.triangle([a + r, b + r, b + r2], n, color, glow);

            self.triangle([a + r, b + r2, a + r2], n, color, glow);

            self.triangle([a, a + r2, a + r], -axis, color, glow);

            self.triangle([b, b + r, b + r2], axis, color, glow);
        }
    }

    pub fn orb(&mut self, c: Vec3, r: f32, color: [f32; 3], glow: f32) {
        for y in [-1.0, 1.0] {
            for x in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let p = [
                        c + Vec3::Y * r * y,
                        c + Vec3::X * r * x,
                        c + Vec3::Z * r * z,
                    ];

                    self.triangle(p, Vec3::new(x, y, z).normalize(), color, glow);
                }
            }
        }
    }

    /// Renders a glowing translucent 3D voxel cuboid (used for Noita-style 3D micro-voxel particles,
    /// crystalline shards, and floating temple pixel-glyphs).
    pub fn glow_voxel(
        &mut self,
        c: Vec3,
        size: Vec3,
        rot: Quat,
        color: [f32; 3],
        glow: f32,
        alpha: f32,
    ) {
        self.glow_voxel_with_dissolve(c, size, rot, color, glow, alpha, 0.0);
    }

    /// Glow voxel with a 0..1 pixel-dissolve threshold. The threshold is packed
    /// as a negative value in the specular channel (otherwise unused by glow
    /// particles) and read by the world shader, which erodes a 3D micro-voxel
    /// grid instead of shrinking a translucent billboard. A negative tag lets
    /// ordinary glow glyphs and beams keep their old 0.42 specular value.
    pub fn glow_voxel_with_dissolve(
        &mut self,
        c: Vec3,
        size: Vec3,
        rot: Quat,
        color: [f32; 3],
        glow: f32,
        alpha: f32,
        dissolve: f32,
    ) {
        let prev_alpha = self.alpha;
        let prev_dissolve = self.dissolve;
        self.alpha = alpha;
        self.dissolve = dissolve.clamp(0.0, 1.0);
        // The shader no longer erodes the face, so a dying piece shrinks here.
        // Shrinking the geometry is anchored to the piece's own centre, which a
        // world-grid discard could not be.
        let remaining = 1.0 - 0.8 * self.dissolve;
        self.cuboid(c, size * remaining, rot, color, glow.max(0.1));
        self.alpha = prev_alpha;
        self.dissolve = prev_dissolve;
    }

    /// Renders a glowing translucent 3D octahedron orb with Fresnel rim boost.
    pub fn glow_orb(&mut self, c: Vec3, r: f32, color: [f32; 3], glow: f32, alpha: f32) {
        let prev = self.alpha;
        self.alpha = alpha;
        self.orb(c, r, color, glow.max(0.1));
        self.alpha = prev;
    }

    /// Renders a glowing translucent 3D energy beam.
    pub fn glow_beam(
        &mut self,
        a: Vec3,
        b: Vec3,
        radius: f32,
        color: [f32; 3],
        glow: f32,
        alpha: f32,
    ) {
        let prev = self.alpha;
        self.alpha = alpha;
        self.beam(a, b, radius, color, glow.max(0.1));
        self.alpha = prev;
    }

    /// Renders a 5x5 3D voxel-pixel arcane temple glyph (inspired by Noita's Holy Mountain
    /// & Emerald Tablet runic holograms). Each active bit in the 25-bit pattern is a tiny
    /// translucent glowing 3D voxel cube floating in world space.
    pub fn temple_glyph(
        &mut self,
        c: Vec3,
        rot: Quat,
        glyph: u32,
        pixel: f32,
        color: [f32; 3],
        glow: f32,
        alpha: f32,
    ) {
        const GLYPHS: [u32; 8] = [
            // 0: Solar Eye / Spark Sigil
            0b01110_10101_11111_10101_01110,
            // 1: Sanctum Cross / Vigour Rune
            0b00100_01110_11011_01110_00100,
            // 2: Frost Crown / Crystalline Lattice
            0b10101_01110_11111_01110_10101,
            // 3: Void Spiral / Rift Sigil
            0b11110_10001_10111_10000_01111,
            // 4: Alchemical Chalice / Touch of Gold
            0b10001_11011_01110_00100_01110,
            // 5: Twin Fork / Bifurcate Glyph
            0b10001_11011_01110_00100_00100,
            // 6: Hourglass / Rapid Flux Glyph
            0b11111_01010_00100_01010_11111,
            // 7: Temple Pillar / Sovereign Crown
            0b10101_11111_01110_01010_11111,
        ];
        let bits = GLYPHS[(glyph as usize) % GLYPHS.len()];
        let step = pixel * 1.18;
        let cube = Vec3::new(pixel, pixel, pixel * 0.65);
        let prev = self.alpha;
        self.alpha = alpha;
        for row in 0..5 {
            for col in 0..5 {
                let bit = 24 - (row * 5 + col);
                if (bits >> bit) & 1 != 0 {
                    let offset =
                        Vec3::new((col as f32 - 2.0) * step, (2.0 - row as f32) * step, 0.0);
                    let is_center = row == 2 && col == 2;
                    let c_tint = if is_center {
                        [
                            (color[0] * 1.25).min(1.0),
                            (color[1] * 1.25).min(1.0),
                            (color[2] * 1.25).min(1.0),
                        ]
                    } else {
                        color
                    };
                    self.cuboid(
                        c + rot * offset,
                        cube,
                        rot,
                        c_tint,
                        glow * if is_center { 1.35 } else { 1.0 },
                    );
                }
            }
        }
        self.alpha = prev;
    }
}

pub fn build(world: &World, player: &crate::player::Player) -> Vec<Vertex3> {
    let scale = world.voxel_scale.meters();

    let viewer = player.pos * scale;

    let mut mesh = Model::new(scale);

    let mut nodes: Vec<_> = world
        .factory
        .nodes
        .iter()
        .filter(|(p, _)| (p.as_vec3() * scale - viewer).length_squared() < 128.0 * 128.0)
        .collect();

    nodes.sort_by(|(a, _), (b, _)| {
        (a.as_vec3() * scale - viewer)
            .length_squared()
            .total_cmp(&(b.as_vec3() * scale - viewer).length_squared())
    });

    let step = factory::step(world);

    // Items in transit, drawn at the position their real travel time puts them.
    {
        let mut moving: Vec<_> = world
            .factory
            .packets
            .iter()
            .map(|k| (k, k.position(scale)))
            .filter(|(_, at)| (*at - viewer).length_squared() < 48.0 * 48.0)
            .collect();
        moving.sort_by(|a, b| {
            (a.1 - viewer)
                .length_squared()
                .total_cmp(&(b.1 - viewer).length_squared())
        });
        for (packet, at) in moving.into_iter().take(220) {
            let color = factory::resource_color(packet.item);
            let spin = world.adventure.time * 3.0 + packet.progress;
            // A full stack reads as a fuller bead.
            let size = 0.15 + 0.07 * (packet.amount as f32 / 64.0).min(1.0);
            mesh.cuboid(
                at,
                Vec3::splat(size),
                Quat::from_rotation_y(spin) * Quat::from_rotation_x(spin * 0.6),
                color,
                0.55,
            );
        }
    }

    for (index, (&p, n)) in nodes.into_iter().enumerate() {
        let o = p.as_vec3() * scale;

        let c = o + Vec3::splat(0.5);

        let active = n.enabled && matches!(n.status, "Processing" | "Generating");

        let metal = [0.27, 0.33, 0.37];

        let dark = [0.085, 0.11, 0.13];

        let copper = [0.68, 0.38, 0.14];

        if matches!(n.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe) {
            let cable = n.kind == Kind::Cable;

            let r = if cable { 0.075 } else { 0.14 };

            let color = if cable {
                [0.10, 0.37, 0.41]
            } else if n.kind == Kind::FluidPipe {
                [0.18, 0.54, 0.69]
            } else {
                copper
            };

            let extractor = n.kind == Kind::Pipe && (n.mode == 1 || n.mode == 2);
            let extractor_color = if n.mode == 2 {
                [0.95, 0.72, 0.28]
            } else {
                [0.30, 0.88, 1.0]
            };
            let is_pipe = n.kind == Kind::Pipe;
            let joins = |d: IVec3| {
                world
                    .factory
                    .nodes
                    .get(&(p + d * step))
                    .is_some_and(|other| {
                        other.kind == n.kind
                            || !matches!(other.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe)
                    })
            };
            let arms: Vec<IVec3> = [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ]
            .into_iter()
            .filter(|d| joins(*d))
            .collect();
            let straight = arms.len() == 2 && arms[0] == -arms[1];
            if !is_pipe {
                mesh.cuboid(c, Vec3::splat(r * 2.6), Quat::IDENTITY, dark, 0.0);
            } else if !straight {
                // Item pipes are glass: only bends, tees and ends get a hub.
                mesh.cuboid(
                    c,
                    Vec3::splat(r * 2.3),
                    Quat::IDENTITY,
                    [0.34, 0.20, 0.10],
                    0.0,
                );
            } else {
                let axis = arms[0].as_vec3();
                mesh.beam(c - axis * 0.05, c + axis * 0.05, r * 1.22, metal, 0.0);
            }
            if extractor {
                // A bright, slightly larger hub marks a pipe that pulls.
                let pulse = 0.65 + 0.35 * (world.adventure.time * 4.0).sin().abs();
                mesh.cuboid(
                    c,
                    Vec3::splat(r * 2.9),
                    Quat::from_rotation_y(0.785),
                    extractor_color,
                    0.8 * pulse * if n.enabled { 1.0 } else { 0.1 },
                );
            }

            for d in [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                if !world
                    .factory
                    .nodes
                    .get(&(p + d * step))
                    .is_some_and(|other| {
                        other.kind == n.kind
                            || !matches!(other.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe)
                    })
                {
                    continue;
                }

                let dir = d.as_vec3();

                // Dense factories keep the same connections without spending

                // eight-sided collars and flow geometry on every distant tube.

                if index > 95 || (c - viewer).length_squared() > 38.0 * 38.0 {
                    let size = dir.abs() * 0.5 + (Vec3::ONE - dir.abs()) * (r * 2.0);

                    mesh.cuboid(c + dir * 0.25, size, Quat::IDENTITY, color, 0.0);

                    continue;
                }

                if is_pipe {
                    // Translucent tube so the items inside can be seen travelling.
                    mesh.glow_beam(c, c + dir * 0.5, r, [0.86, 0.52, 0.26], 0.12, 0.38);
                } else {
                    mesh.beam(c, c + dir * 0.5, r, color, 0.0);
                }

                if extractor {
                    // Double collar with a glowing band: reads as a mouth.
                    mesh.beam(c + dir * 0.36, c + dir * 0.40, r * 1.7, metal, 0.0);
                    mesh.beam(
                        c + dir * 0.40,
                        c + dir * 0.44,
                        r * 1.75,
                        extractor_color,
                        0.9,
                    );
                    mesh.beam(c + dir * 0.44, c + dir * 0.48, r * 1.7, metal, 0.0);
                } else {
                    mesh.beam(c + dir * 0.40, c + dir * 0.48, r * 1.35, metal, 0.0);
                }

                if n.activity > 0.0 && n.kind != Kind::Pipe {
                    let travel = (world.adventure.time * 1.8).fract() * 0.46;

                    mesh.beam(
                        c + dir * travel,
                        c + dir * (travel + 0.045),
                        r * 1.07,
                        if cable {
                            [0.2, 0.9, 1.0]
                        } else {
                            [1.0, 0.72, 0.22]
                        },
                        n.activity * 1.8,
                    );
                }
            }

            continue;
        }

        if index > 95 || (c - viewer).length() > 38.0 {
            mesh.cuboid(
                o + Vec3::new(0.5, 0.45, 0.5),
                Vec3::new(0.88, 0.84, 0.88),
                Quat::IDENTITY,
                n.kind.block().color(p),
                0.0,
            );

            continue;
        }

        // Plinth, four feet, and face sockets give every machine a shared scale.

        mesh.cuboid(
            o + Vec3::new(0.5, 0.12, 0.5),
            Vec3::new(0.92, 0.16, 0.92),
            Quat::IDENTITY,
            dark,
            0.0,
        );

        for x in [0.14, 0.86] {
            for z in [0.14, 0.86] {
                mesh.cuboid(
                    o + Vec3::new(x, 0.04, z),
                    Vec3::new(0.18, 0.08, 0.18),
                    Quat::IDENTITY,
                    metal,
                    0.0,
                );
            }
        }

        let lamp = if !n.enabled {
            [0.42, 0.12, 0.10]
        } else if active {
            [0.16, 0.93, 0.56]
        } else {
            [0.73, 0.50, 0.17]
        };

        match n.kind {
            Kind::Generator => {
                if n.mode == 1 {
                    // Arcane pile: the core runs cyan when cool and white-orange
                    // as heat climbs toward meltdown, and its pulse quickens.
                    let heat = (n.progress as f32 / 1000.0).clamp(0.0, 1.0);
                    let core = [0.35 + 0.65 * heat, 0.95 - 0.45 * heat, 1.0 - 0.85 * heat];
                    let pulse = 0.8 + 0.2 * (world.adventure.time * (2.0 + 7.0 * heat)).sin();
                    mesh.glow_orb(
                        o + Vec3::new(0.52, 0.94, 0.5),
                        0.12 + 0.06 * heat,
                        core,
                        (1.6 + 3.2 * heat) * pulse,
                        0.85,
                    );
                    for k in 0..4 {
                        let a = world.adventure.time * 1.4 + k as f32 * std::f32::consts::FRAC_PI_2;
                        mesh.glow_orb(
                            o + Vec3::new(0.52 + a.cos() * 0.2, 0.94, 0.5 + a.sin() * 0.2),
                            0.03,
                            core,
                            2.4,
                            0.8,
                        );
                    }
                }
                mesh.cuboid(
                    o + Vec3::new(0.52, 0.48, 0.5),
                    Vec3::new(0.57, 0.56, 0.68),
                    Quat::IDENTITY,
                    metal,
                    0.0,
                );

                mesh.beam(
                    o + Vec3::new(0.18, 0.54, 0.25),
                    o + Vec3::new(0.18, 0.54, 0.75),
                    0.21,
                    dark,
                    0.0,
                );

                let spin = Quat::from_rotation_z(n.phase * 5.0);

                for angle in [0.0, std::f32::consts::FRAC_PI_2] {
                    mesh.cuboid(
                        o + Vec3::new(0.18, 0.54, 0.22),
                        Vec3::new(0.37, 0.055, 0.055),
                        spin * Quat::from_rotation_z(angle),
                        copper,
                        0.0,
                    );
                }

                mesh.beam(
                    o + Vec3::new(0.72, 0.65, 0.69),
                    o + Vec3::new(0.72, 0.99, 0.69),
                    0.09,
                    dark,
                    0.0,
                );

                mesh.cuboid(
                    o + Vec3::new(0.52, 0.42, 0.15),
                    Vec3::new(0.28, 0.18, 0.035),
                    Quat::IDENTITY,
                    if active { [1.0, 0.32, 0.06] } else { dark },
                    if active { 2.0 } else { 0.0 },
                );
            }

            Kind::Crusher => {
                for x in [0.23, 0.77] {
                    mesh.cuboid(
                        o + Vec3::new(x, 0.50, 0.5),
                        Vec3::new(0.16, 0.65, 0.76),
                        Quat::IDENTITY,
                        metal,
                        0.0,
                    );
                }

                for x in [0.39, 0.61] {
                    mesh.beam(
                        o + Vec3::new(x, 0.68, 0.22),
                        o + Vec3::new(x, 0.68, 0.78),
                        0.14,
                        dark,
                        0.0,
                    );

                    for angle in [0.0, 1.57, 3.14, 4.71] {
                        let a = angle + n.phase * 4.5 * if x < 0.5 { 1.0 } else { -1.0 };

                        mesh.cuboid(
                            o + Vec3::new(x + a.cos() * 0.135, 0.68 + a.sin() * 0.135, 0.5),
                            Vec3::new(0.055, 0.055, 0.52),
                            Quat::from_rotation_z(a),
                            copper,
                            0.0,
                        );
                    }
                }
            }

            Kind::Furnace | Kind::AlloyKiln => {
                mesh.cuboid(
                    o + Vec3::new(0.5, 0.53, 0.52),
                    Vec3::new(0.82, 0.70, 0.78),
                    Quat::IDENTITY,
                    if n.kind == Kind::AlloyKiln {
                        [0.46, 0.25, 0.17]
                    } else {
                        metal
                    },
                    0.0,
                );

                mesh.cuboid(
                    o + Vec3::new(0.5, 0.49, 0.12),
                    Vec3::new(0.48, 0.34, 0.025),
                    Quat::IDENTITY,
                    dark,
                    0.0,
                );

                for x in [0.34, 0.5, 0.66] {
                    mesh.cuboid(
                        o + Vec3::new(x, 0.44, 0.10),
                        Vec3::new(0.07, 0.18, 0.025),
                        Quat::IDENTITY,
                        if active {
                            [1.0, 0.36, 0.08]
                        } else {
                            [0.26, 0.16, 0.10]
                        },
                        if active {
                            1.5 + (n.phase * 5.0).sin() * 0.3
                        } else {
                            0.0
                        },
                    );
                }

                mesh.beam(
                    o + Vec3::new(0.72, 0.85, 0.72),
                    o + Vec3::new(0.72, 0.99, 0.72),
                    0.11,
                    dark,
                    0.0,
                );
            }

            Kind::Feeder | Kind::Storage => {
                let height = if n.kind == Kind::Feeder { 0.58 } else { 0.64 };
                let body = match (n.kind, n.mode) {
                    (Kind::Storage, 1) => [0.42, 0.28, 0.16],
                    (Kind::Storage, 2) => [0.34, 0.26, 0.14],
                    (Kind::Storage, _) => copper,
                    _ => metal,
                };

                for (c, s) in [
                    (Vec3::new(0.14, 0.52, 0.5), Vec3::new(0.10, height, 0.78)),
                    (Vec3::new(0.86, 0.52, 0.5), Vec3::new(0.10, height, 0.78)),
                    (Vec3::new(0.5, 0.52, 0.14), Vec3::new(0.62, height, 0.10)),
                    (Vec3::new(0.5, 0.52, 0.86), Vec3::new(0.62, height, 0.10)),
                ] {
                    mesh.cuboid(
                        o + c,
                        s,
                        Quat::IDENTITY,
                        body,
                        0.0,
                    );
                }

                let amount = n.input.iter().chain(n.output.iter()).copied().sum::<u32>() as f32;

                if amount > 0.0 {
                    mesh.cuboid(
                        o + Vec3::new(0.5, 0.26 + 0.40 * (amount / 4096.0).min(1.0), 0.5),
                        Vec3::new(0.60, 0.08, 0.60),
                        Quat::IDENTITY,
                        [0.48, 0.51, 0.39],
                        0.0,
                    );
                }

                if n.kind == Kind::Storage {
                    mesh.cuboid(
                        o + Vec3::new(0.5, 0.88, 0.5),
                        Vec3::new(0.87, 0.08, 0.85),
                        Quat::IDENTITY,
                        dark,
                        0.0,
                    );
                }
            }

            Kind::Bioreactor | Kind::Cultivator => {
                for x in [0.21, 0.79] {
                    for z in [0.21, 0.79] {
                        mesh.beam(
                            o + Vec3::new(x, 0.2, z),
                            o + Vec3::new(x, 0.88, z),
                            0.045,
                            metal,
                            0.0,
                        );
                    }
                }

                mesh.beam(
                    o + Vec3::new(0.5, 0.21, 0.5),
                    o + Vec3::new(0.5, 0.29, 0.5),
                    0.32,
                    dark,
                    0.0,
                );

                mesh.beam(
                    o + Vec3::new(0.5, 0.86, 0.5),
                    o + Vec3::new(0.5, 0.94, 0.5),
                    0.32,
                    metal,
                    0.0,
                );

                mesh.orb(
                    o + Vec3::new(0.5, 0.54, 0.5),
                    0.24,
                    [0.16, 0.65, 0.39],
                    if active { 1.5 } else { 0.15 },
                );

                for i in 0..3 {
                    let a = n.phase * 2.0 + i as f32 * 2.094;

                    mesh.orb(
                        o + Vec3::new(
                            0.5 + a.cos() * 0.24,
                            0.43 + i as f32 * 0.11,
                            0.5 + a.sin() * 0.24,
                        ),
                        0.055,
                        [0.55, 0.96, 0.39],
                        if active { 2.0 } else { 0.0 },
                    );
                }
            }

            Kind::Fabricator => {
                mesh.cuboid(
                    o + Vec3::new(0.5, 0.36, 0.5),
                    Vec3::new(0.78, 0.24, 0.78),
                    Quat::IDENTITY,
                    metal,
                    0.0,
                );

                for x in [0.20, 0.8] {
                    mesh.beam(
                        o + Vec3::new(x, 0.35, 0.72),
                        o + Vec3::new(x, 0.94, 0.72),
                        0.055,
                        copper,
                        0.0,
                    );
                }

                let y = 0.69 + (n.phase * 6.0).sin() * 0.13;

                mesh.cuboid(
                    o + Vec3::new(0.5, y, 0.50),
                    Vec3::new(0.55, 0.12, 0.45),
                    Quat::IDENTITY,
                    dark,
                    0.0,
                );

                mesh.orb(
                    o + Vec3::new(0.5, 0.51, 0.5),
                    0.10,
                    [0.26, 0.65, 0.92],
                    if active { 1.4 } else { 0.0 },
                );
            }

            Kind::MiningDrill => {
                for x in [0.15, 0.85] {
                    for z in [0.15, 0.85] {
                        mesh.beam(
                            o + Vec3::new(x, 0.15, z),
                            o + Vec3::new(x, 0.94, z),
                            0.045,
                            metal,
                            0.0,
                        );
                    }
                }

                mesh.cuboid(
                    o + Vec3::new(0.5, 0.85, 0.5),
                    Vec3::new(0.84, 0.20, 0.84),
                    Quat::IDENTITY,
                    copper,
                    0.0,
                );

                mesh.beam(
                    o + Vec3::new(0.5, 0.08, 0.5),
                    o + Vec3::new(0.5, 0.8, 0.5),
                    0.09,
                    dark,
                    0.0,
                );

                for i in 0..5 {
                    mesh.cuboid(
                        o + Vec3::new(0.5, 0.16 + i as f32 * 0.11, 0.5),
                        Vec3::new(0.48 - i as f32 * 0.045, 0.08, 0.18),
                        Quat::from_rotation_y(n.phase * 8.0 + i as f32 * 0.8),
                        metal,
                        0.0,
                    );
                }
            }

            Kind::Pump | Kind::Tank | Kind::Washer => {
                for x in [0.16, 0.84] {
                    for z in [0.16, 0.84] {
                        mesh.beam(
                            o + Vec3::new(x, 0.16, z),
                            o + Vec3::new(x, 0.92, z),
                            0.045,
                            metal,
                            0.0,
                        );
                    }
                }

                mesh.cuboid(
                    o + Vec3::new(0.5, 0.92, 0.5),
                    Vec3::new(0.82, 0.08, 0.82),
                    Quat::IDENTITY,
                    metal,
                    0.0,
                );

                let fill = n.water as f32 / 8192.0;

                if fill > 0.0 {
                    mesh.cuboid(
                        o + Vec3::new(0.5, 0.22 + fill * 0.29, 0.5),
                        Vec3::new(0.61, fill * 0.58, 0.61),
                        Quat::IDENTITY,
                        [0.10, 0.44, 0.61],
                        0.10,
                    );
                }

                for i in 0..5 {
                    mesh.cuboid(
                        o + Vec3::new(0.82, 0.27 + i as f32 * 0.12, 0.12),
                        Vec3::new(0.08, 0.025, 0.025),
                        Quat::IDENTITY,
                        [0.81, 0.78, 0.60],
                        0.0,
                    );
                }

                if n.kind != Kind::Tank {
                    mesh.beam(
                        o + Vec3::new(0.25, 0.65, 0.5),
                        o + Vec3::new(0.75, 0.65, 0.5),
                        0.10,
                        copper,
                        0.0,
                    );

                    for i in 0..4 {
                        let a = n.phase * 6.0 + i as f32 * 1.57;

                        mesh.beam(
                            o + Vec3::new(0.5, 0.65, 0.5),
                            o + Vec3::new(0.5, 0.65 + a.sin() * 0.24, 0.5 + a.cos() * 0.24),
                            0.045,
                            metal,
                            0.0,
                        );
                    }

                    if n.kind == Kind::Pump {
                        mesh.beam(
                            o + Vec3::new(0.5, 0.05, 0.5),
                            o + Vec3::new(0.5, 0.48, 0.5),
                            0.13,
                            copper,
                            0.0,
                        );
                    }
                }
            }

            _ => {}
        }

        // Rivets, inset front panel and enamel warning stripes read at human scale.

        for x in [0.15, 0.85] {
            mesh.cuboid(
                o + Vec3::new(x, 0.16, 0.025),
                Vec3::splat(0.038),
                Quat::IDENTITY,
                [0.62, 0.65, 0.63],
                0.0,
            );
        }

        for i in 0..5 {
            mesh.cuboid(
                o + Vec3::new(0.24 + i as f32 * 0.10, 0.12, 0.018),
                Vec3::new(0.055, 0.08, 0.02),
                Quat::from_rotation_z(-0.4),
                [0.83, 0.57, 0.14],
                0.0,
            );
        }

        mesh.cuboid(
            o + Vec3::new(0.81, 0.26, 0.035),
            Vec3::new(0.07, 0.07, 0.035),
            Quat::IDENTITY,
            lamp,
            if active { 1.1 } else { 0.0 },
        );
    }

    mesh.surface_kind = 0.0;

    for enemy in &world.adventure.enemies {
        enemy.model(&mut mesh, world.adventure.time);
    }
    if let Some(companion) = &world.adventure.companion {
        companion.model(&mut mesh, world.adventure.time);
    }
    for shot in &world.adventure.projectiles {
        let color = shot.color();
        let dir = shot.velocity.normalize_or_zero();
        if let Some(payload) = shot.payload_color() {
            let back = shot.pos - dir * 0.18;
            mesh.glow_orb(back, 0.05, payload, 1.2, 0.45);
        }
        if !shot.hostile && (shot.spell == 10 || shot.spell == 6) {
            mesh.surface_kind = 46.0;
            let spin = world.adventure.time * if shot.spell == 10 { -0.55 } else { 0.4 };
            let rings = if shot.spell == 10 { 4 } else { 3 };
            for i in 0..rings {
                let a = spin + i as f32 * 1.4;
                let r = if shot.spell == 10 {
                    0.22 - i as f32 * 0.03
                } else {
                    0.16
                };
                let wobble = (world.adventure.time * 0.8 + i as f32).sin() * 0.015;
                mesh.orb(
                    shot.pos + Vec3::new(a.cos() * r, wobble, a.sin() * r),
                    0.04,
                    color,
                    0.0,
                );
            }
            mesh.surface_kind = 0.0;
        }
        if !shot.hostile && shot.spell == 10 {
            mesh.orb(shot.pos, 0.1, [0.08, 0.05, 0.12], 0.3);
            mesh.glow_orb(shot.pos, 0.13, color, 1.4, 0.4);
            for i in 0..4 {
                let a = world.adventure.time * 0.7 + i as f32 * 1.57;
                let r = 0.18;
                let p = shot.pos + Vec3::new(a.cos() * r, (a * 0.5).sin() * 0.02, a.sin() * r);
                mesh.glow_voxel(
                    p,
                    Vec3::splat(0.02),
                    Quat::IDENTITY,
                    crate::world::adventure::PALETTE_PLUM,
                    1.15,
                    0.45,
                );
            }
        } else {
            let body = (if shot.hostile { 0.13 } else { 0.09 }) * shot.mods.radius.max(0.4);
            let paper = crate::world::adventure::PALETTE_PAPER;
            let hot = [
                (color[0] * 0.62 + paper[0] * 0.38).min(1.0),
                (color[1] * 0.62 + paper[1] * 0.38).min(1.0),
                (color[2] * 0.62 + paper[2] * 0.38).min(1.0),
            ];
            mesh.orb(shot.pos, body * 0.38, hot, 1.5);
            mesh.glow_orb(shot.pos, body * 0.95, color, 1.45, 0.48);
            mesh.glow_beam(shot.pos - dir * 0.28, shot.pos, 0.016, color, 1.05, 0.38);
            if !shot.hostile {
                let side = dir.any_orthonormal_vector();
                let up = dir.cross(side).normalize_or_zero();
                let beads: usize = match shot.spell {
                    2 | 13 => 3,
                    3 | 14 => 6,
                    12 => 4,
                    _ => 4,
                };
                for i in 0..beads {
                    let u = i as f32 / beads as f32;
                    let phase = world.adventure.time * 1.1 + shot.pos.x * 0.2 + i as f32 * 1.4;
                    let (offset, scale, tint) = match shot.spell {
                        2 | 13 => (
                            dir * (0.16 - u * 0.10) + up * (u - 0.5) * 0.04,
                            0.028,
                            color,
                        ),
                        3 | 14 => {
                            let a = phase + i as f32 * 1.05;
                            ((side * a.cos() + up * a.sin()) * 0.16, 0.022, color)
                        }
                        12 => (
                            dir * -u * 0.22 + Vec3::Y * -u * 0.06,
                            0.034 - u * 0.01,
                            color,
                        ),
                        _ => {
                            let spiral =
                                (phase.cos() * side + phase.sin() * up) * (0.04 + u * 0.04);
                            (
                                spiral - dir * (0.12 + u * 0.40),
                                0.024 * (1.0 - u * 0.4),
                                color,
                            )
                        }
                    };
                    mesh.glow_voxel(
                        shot.pos + offset,
                        Vec3::splat(scale.max(0.012)),
                        Quat::from_rotation_y(phase),
                        tint,
                        1.15 * (1.0 - u * 0.25),
                        0.42,
                    );
                }
                if let Some(element) = shot.mods.element {
                    let tint = element.color();
                    let lift = match element {
                        crate::world::adventure::status::Element::Fire => Vec3::Y * 0.12,
                        crate::world::adventure::status::Element::Frost => Vec3::Y * -0.04,
                        _ => Vec3::ZERO,
                    };
                    for i in 0..2 {
                        let a = world.adventure.time * 0.8 + i as f32 * 3.14;
                        let p = shot.pos
                            + (side * a.cos() + up * a.sin()) * (0.1 + i as f32 * 0.02)
                            + lift;
                        mesh.glow_voxel(p, Vec3::splat(0.014), Quat::IDENTITY, tint, 1.15, 0.4);
                    }
                }
            }
        }
        if !shot.hostile && shot.mods.orbit {
            for i in 0..2 {
                let a = world.adventure.time * 0.7 + i as f32 * std::f32::consts::PI;
                let side = dir.any_orthonormal_vector();
                let up_v = dir.cross(side).normalize_or_zero();
                let sat = shot.pos + (side * a.cos() + up_v * a.sin()) * 0.19;
                mesh.glow_voxel(
                    sat,
                    Vec3::splat(0.032),
                    Quat::from_rotation_y(a),
                    crate::world::adventure::PALETTE_AMBER,
                    1.2,
                    0.42,
                );
            }
        }
        if !shot.hostile && shot.mods.homing {
            let a = world.adventure.time * 0.6;
            let side = dir.any_orthonormal_vector();
            let up_v = dir.cross(side).normalize_or_zero();
            for i in 0..3 {
                let ang = a + i as f32 * 2.094;
                let p = shot.pos - dir * 0.08 + (side * ang.cos() + up_v * ang.sin()) * 0.11;
                mesh.glow_voxel(
                    p,
                    Vec3::splat(0.020),
                    Quat::IDENTITY,
                    crate::world::adventure::PALETTE_MANA,
                    1.1,
                    0.4,
                );
            }
        }
    }
    let time = world.adventure.time;
    for (idx, drop) in world.inventory.dropped.iter().enumerate() {
        use crate::world::inventory::{ArmorTier, Item};
        let phase = time * 2.4 + idx as f32 * 1.13 + drop.pos.x * 0.7;
        let bob = (phase.sin() * 0.055) + 0.06;
        let rot = Quat::from_rotation_y(time * 1.65 + idx as f32 * 0.8)
            * Quat::from_rotation_z((phase * 0.7).cos() * 0.14);
        let (color, emissive, beam_col) = match drop.stack.item {
            Item::Block(b) => {
                let c = b.color((drop.pos / world.voxel_scale.meters()).as_ivec3());
                let em = if matches!(
                    b,
                    crate::material::BlockId::Crystal
                        | crate::material::BlockId::GlowCap
                        | crate::material::BlockId::Lava
                ) {
                    1.2
                } else {
                    0.0
                };
                (c, em, if em > 0.2 { Some(c) } else { None })
            }
            Item::Weapon(tier) => (
                [0.86, 0.74, 0.36],
                1.4,
                Some(if tier >= 3 {
                    [0.98, 0.78, 0.32]
                } else {
                    [0.82, 0.68, 0.38]
                }),
            ),
            Item::Armor(_, tier) => {
                let col = match tier {
                    ArmorTier::Copper => [0.84, 0.50, 0.28],
                    ArmorTier::Steel => [0.54, 0.68, 0.90],
                    ArmorTier::Lumen => [0.44, 0.92, 0.98],
                };
                (col, 1.1, Some(col))
            }
            Item::Resource(r) => {
                let col = match r {
                    0 | 1 => [0.88, 0.52, 0.28],
                    2 | 3 => [0.76, 0.80, 0.86],
                    4 => [0.86, 0.68, 0.32],
                    5 => [0.58, 0.72, 0.94],
                    6 | 7 => [0.44, 0.86, 0.98],
                    _ => [0.75, 0.58, 0.28],
                };
                (col, 0.65, Some(col))
            }
            Item::Food(_) => ([0.84, 0.52, 0.30], 0.35, None),
            Item::Tool(_) => ([0.68, 0.82, 0.92], 0.9, Some([0.68, 0.82, 0.92])),
        };

        let center = drop.pos + Vec3::Y * bob;
        let copies = if drop.stack.count >= 16 {
            3
        } else if drop.stack.count > 1 {
            2
        } else {
            1
        };
        for c_i in 0..copies {
            let offset = if c_i == 0 {
                Vec3::ZERO
            } else {
                let a = c_i as f32 * 2.1 + time * 0.8;
                Vec3::new(a.cos() * 0.045, c_i as f32 * 0.028, a.sin() * 0.045)
            };
            mesh.cuboid(center + offset, Vec3::splat(0.20), rot, color, emissive);
        }
        // Subtle ground shadow ring + vertical loot beacon for notable gear/resources
        if let Some(bc) = beam_col {
            mesh.glow_voxel(
                center + Vec3::Y * 0.24,
                Vec3::new(0.025, 0.38, 0.025),
                Quat::from_rotation_y(time * 0.9),
                bc,
                2.6,
                0.38,
            );
            let mote_t = (time * 1.4 + idx as f32 * 0.6).fract();
            mesh.glow_voxel(
                center
                    + Vec3::new(
                        (phase * 1.5).cos() * 0.08,
                        mote_t * 0.48,
                        (phase * 1.5).sin() * 0.08,
                    ),
                Vec3::splat(0.024 * (1.0 - mote_t * 0.5)),
                rot,
                bc,
                3.2,
                0.72 * (1.0 - mote_t),
            );
        }
    }

    crate::tool_effects::draw(&mut mesh, &world.adventure.tool_debris);
    crate::tool_effects::draw_blasts(&mut mesh, &world.adventure.blasts);
    crate::tool_effects::draw_splashes(&mut mesh, &world.adventure.splashes);

    for effect in &world.adventure.effects {
        let t = 1.0 - effect.life / effect.total;

        for i in 0..10 {
            let a = i as f32 * 2.39996 + effect.pos.x * 0.37;

            let travel = t * effect.radius;

            mesh.orb(
                effect.pos
                    + Vec3::new(a.cos(), 0.35 + (i as f32 * 1.7).sin() * 0.65, a.sin())
                        * travel
                        * (0.65 + (i % 3) as f32 * 0.22)
                    - Vec3::Y * t * t * effect.radius * 0.55,
                (0.035 + (i % 3) as f32 * 0.014) * (1.0 - t),
                effect.color,
                2.0 * (1.0 - t),
            );
        }
    }

    let eye = player.eye(world.voxel_scale) * scale;

    let forward = player.forward_dir();

    let right = forward.cross(Vec3::Y).normalize_or_zero();

    let up = right.cross(forward).normalize_or_zero();

    world.adventure.magic_models(&mut mesh, eye, forward);
    world
        .adventure
        .quest_beacon_models(&world.sample_config(), viewer, &mut mesh);

    for rem in &world.adventure.remains {
        let t = 0.65 - rem.life;

        for i in 0..7 {
            let a = i as f32 * 2.399;

            let drift = Vec3::new(a.cos() * 1.2, 1.5 + (i % 3) as f32 * 0.3, a.sin() * 1.2) * t
                - Vec3::Y * t * t * 4.0;

            mesh.cuboid(
                rem.pos + drift,
                Vec3::splat(0.11 * rem.life / 0.65),
                Quat::from_euler(glam::EulerRot::XYZ, a + t * 5.0, t * 3.0, a),
                if rem.kind == 0 || rem.kind == 3 {
                    [0.43, 0.32, 0.24]
                } else {
                    [0.34, 0.50, 0.53]
                },
                0.25,
            );
        }
    }

    if world.adventure.swing() > 0.0 {
        let heavy = world.adventure.finisher();

        let duration = world.adventure.swing_duration();

        let elapsed = (1.0 - world.adventure.swing()) * duration;

        let ease = |v: f32| {
            let t = v.clamp(0.0, 1.0);

            t * t * (3.0 - 2.0 * t)
        };

        let impact = world.adventure.swing_impact_time();

        let windup = impact * 0.55;

        let load = ease(elapsed / windup);

        let strike = ease((elapsed - windup) / (impact - windup));

        let recover = ease((elapsed - impact) / (duration - impact));

        let thrust = -0.10 * load + 0.48 * strike - 0.38 * recover;

        let direction = if world.adventure.backswing() {
            -1.0
        } else {
            1.0
        };

        let arc = direction * (0.35 * load - 1.05 * strike + 0.70 * recover);

        let grip = eye
            + forward * (0.38 + thrust)
            + right * (0.30 + arc * if heavy { 0.08 } else { 0.30 })
            + up * (-0.28 + load * 0.16 - strike * 0.23 + recover * 0.07);

        // Camera-local basis keeps the wrist and striking face aligned when

        // looking up/down. Rotate the whole tool, not just its position.

        let pitch = -0.65 * load + 1.65 * strike - recover;

        let shaft = (up * pitch.cos() + forward * pitch.sin() + right * arc * 0.55).normalize();

        // One camera-relative frame for the entire rigid tool. Generic beam()

        // changes its reference axis near vertical, visibly rolling the grip.

        let tool_right = (right - shaft * right.dot(shaft)).normalize();

        let tool_back = tool_right.cross(shaft);

        let tool_rotation = Quat::from_mat3(&glam::Mat3::from_cols(tool_right, shaft, tool_back));

        let tip = grip + shaft * 0.34;

        let wrist = grip - forward * 0.12 - up * 0.035;

        mesh.beam(
            eye + right * 0.43 - up * 0.48 + forward * 0.12,
            wrist,
            0.065,
            [0.22, 0.27, 0.25],
            0.0,
        );

        mesh.beam(wrist, grip + shaft * 0.035, 0.055, [0.57, 0.39, 0.27], 0.0);

        mesh.cuboid(
            (grip + tip) * 0.5,
            Vec3::new(0.055, 0.34, 0.055),
            tool_rotation,
            [0.30, 0.19, 0.105],
            0.0,
        );

        for i in 0..4 {
            let c = grip.lerp(tip, i as f32 * 0.12);

            mesh.cuboid(
                c,
                Vec3::new(0.064, 0.014, 0.064),
                tool_rotation,
                [0.16, 0.11, 0.08],
                0.0,
            );
        }

        let weapon = world.adventure.weapon;

        if weapon == 5 {
            // A long haft with a heavy, softly glowing head reads as a maul
            // rather than recolouring the sword silhouette.
            let haft = [0.30, 0.23, 0.16];
            let metal = [0.43, 0.50, 0.52];
            let glow = [0.48, 0.90, 0.82];
            mesh.cuboid(
                tip + shaft * 0.42,
                Vec3::new(0.075, 0.86, 0.075),
                tool_rotation,
                haft,
                0.0,
            );
            mesh.cuboid(
                tip + shaft * 0.83,
                Vec3::new(0.46, 0.22, 0.24),
                tool_rotation,
                metal,
                0.25,
            );
            mesh.cuboid(
                tip + shaft * 0.83 + right * 0.02,
                Vec3::new(0.30, 0.075, 0.15),
                tool_rotation,
                glow,
                1.1,
            );
            mesh.orb(tip + shaft * 0.94, 0.055, glow, 1.5);
        } else if weapon > 0 {
            let blade = match weapon {
                3 => [0.94, 0.36, 0.10],

                4 => [0.35, 0.79, 0.93],

                2 => [0.66, 0.73, 0.75],

                _ => [0.67, 0.37, 0.18],
            };

            let length = if weapon == 4 { 0.80 } else { 0.62 };

            let width = if weapon == 3 { 0.15 } else { 0.075 };

            mesh.cuboid(
                tip + shaft * (length * 0.5),
                Vec3::new(width, length, 0.035),
                tool_rotation,
                blade,
                if weapon >= 3 { 0.4 } else { 0.0 },
            );

            mesh.cuboid(
                tip,
                Vec3::new(0.28, 0.05, 0.08),
                tool_rotation,
                [0.53, 0.35, 0.14],
                0.0,
            );

            mesh.cuboid(
                tip + shaft * (length + 0.045),
                Vec3::new(width * 0.55, 0.10, 0.03),
                tool_rotation,
                blade,
                if weapon >= 3 { 0.6 } else { 0.0 },
            );

            if weapon >= 3 {
                mesh.orb(tip, 0.045, blade, 1.2);
            }
        } else {
            let metal = if world.expedition.tool_tier == 0 {
                [0.45, 0.47, 0.44]
            } else {
                [0.64, 0.39, 0.16]
            };

            mesh.cuboid(tip, Vec3::new(0.15, 0.15, 0.28), tool_rotation, metal, 0.0);

            if world.expedition.tool_tier == 2 {
                mesh.orb(tip, 0.05, [0.38, 0.76, 1.0], 1.3);
            }
        }
    }

    // The held wand: styled from the player's atelier choices, idle in the hand
    // and drawn forward when a cast is under way. The tool takes the hand on a swing.
    if world.adventure.wand > 0 && world.adventure.swing() <= 0.0 {
        let adv = &world.adventure;
        let spell = adv.loadout.slot(adv.loadout.cursor);
        wand::draw(
            &mut mesh,
            crate::wand_style::current(),
            &wand::WandView {
                eye,
                forward,
                right,
                up,
                yaw: player.yaw,
                pitch: player.pitch,
                time: adv.time,
                cast: if adv.casting() {
                    adv.cast_motion().clamp(0.0, 1.0)
                } else {
                    0.0
                },
                spell: crate::world::adventure::spell_color(spell),
                last_cast: adv.last_cast,
                tier: adv.wand,
                mana: (adv.mana / adv.max_mana().max(1.0)).clamp(0.0, 1.0),
            },
        );
    }
    mesh.vertices
}

/// Light thrown by the held wand's head, in metres, for the torch-light list.
pub fn wand_tip_light(
    world: &World,
    player: &crate::player::Player,
    scale: f32,
) -> Option<(Vec3, [f32; 3], f32)> {
    let adv = &world.adventure;
    if adv.wand == 0 || adv.swing() > 0.0 {
        return None;
    }
    let eye = player.eye(world.voxel_scale) * scale;
    let forward = player.forward_dir();
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let spell = adv.loadout.slot(adv.loadout.cursor);
    Some(wand::tip_light(
        crate::wand_style::current(),
        &wand::WandView {
            eye,
            forward,
            right,
            up,
            yaw: player.yaw,
            pitch: player.pitch,
            time: adv.time,
            cast: if adv.casting() {
                adv.cast_motion().clamp(0.0, 1.0)
            } else {
                0.0
            },
            spell: crate::world::adventure::spell_color(spell),
            last_cast: adv.last_cast,
            tier: adv.wand,
            mana: (adv.mana / adv.max_mana().max(1.0)).clamp(0.0, 1.0),
        },
    ))
}

#[cfg(test)]
mod dissolve_tests {
    use super::*;

    #[test]
    fn glow_voxel_dissolve_uses_the_negative_specular_tag() {
        let mut model = Model::new(1.0);
        model.glow_voxel(
            Vec3::ZERO,
            Vec3::splat(0.1),
            Quat::IDENTITY,
            [1.0, 0.5, 0.2],
            2.0,
            0.7,
        );
        assert_eq!(model.vertices.len(), 36);
        assert!(model.vertices.iter().all(|v| v.surface[1] == 0.42));

        model.glow_voxel_with_dissolve(
            Vec3::ZERO,
            Vec3::splat(0.1),
            Quat::IDENTITY,
            [1.0, 0.5, 0.2],
            2.0,
            0.7,
            0.6,
        );
        assert!(model.vertices[36..].iter().all(|v| v.surface[1] == -0.6));
        // The tag is temporary; the next ordinary glow voxel is intact again.
        model.glow_voxel(
            Vec3::ZERO,
            Vec3::splat(0.1),
            Quat::IDENTITY,
            [1.0, 0.5, 0.2],
            2.0,
            0.7,
        );
        assert!(model.vertices[72..].iter().all(|v| v.surface[1] == 0.42));
    }

    #[test]
    fn hand_geometry_can_request_the_skin_material() {
        let mut model = Model::new(1.0);
        model.set_surface_kind(54.0);
        model.cuboid(
            Vec3::ZERO,
            Vec3::splat(0.1),
            Quat::IDENTITY,
            [0.7, 0.5, 0.35],
            0.0,
        );
        assert!(model.vertices.iter().all(|v| v.surface[2] == 54.0));
        assert!(model.vertices.iter().all(|v| v.surface[1] == 0.42));
    }
}
