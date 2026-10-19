//! Lit, metre-scaled entity geometry, streamed separately from immutable terrain.

//! Moving parts advance from simulation state, never wall-clock render time.

use super::Vertex3;

use crate::world::{

    World,

    factory::{self, Kind},

};

use glam::{IVec3, Quat, Vec3};



pub struct Model {

    pub vertices: Vec<Vertex3>,

    scale: f32,

    surface_kind: f32,

    alpha: f32,

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

        }

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

                    0.42,

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
        let prev = self.alpha;
        self.alpha = alpha;
        self.cuboid(c, size, rot, color, glow.max(0.1));
        self.alpha = prev;
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
                    let offset = Vec3::new((col as f32 - 2.0) * step, (2.0 - row as f32) * step, 0.0);
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
                    self.cuboid(c + rot * offset, cube, rot, c_tint, glow * if is_center { 1.35 } else { 1.0 });
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

            mesh.cuboid(c, Vec3::splat(r * 2.6), Quat::IDENTITY, dark, 0.0);

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

                mesh.beam(c, c + dir * 0.5, r, color, 0.0);

                mesh.beam(c + dir * 0.40, c + dir * 0.48, r * 1.35, metal, 0.0);

                if n.activity > 0.0 {

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

                        if n.kind == Kind::Storage {

                            copper

                        } else {

                            metal

                        },

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
    for shot in &world.adventure.projectiles {
        let color = shot.color();
        let dir = shot.velocity.normalize_or_zero();
        if let Some(payload) = shot.payload_color() {
            let back = shot.pos - dir * 0.18;
            mesh.glow_orb(back, 0.065, payload, 2.8, 0.78);
        }
        if !shot.hostile && (shot.spell == 10 || shot.spell == 6) {
            mesh.surface_kind = 46.0;
            let spin = world.adventure.time * if shot.spell == 10 { -2.6 } else { 1.4 };
            let rings = if shot.spell == 10 { 7 } else { 5 };
            for i in 0..rings {
                let a = spin + i as f32 * 0.9;
                let r = if shot.spell == 10 { 0.26 - i as f32 * 0.028 } else { 0.18 };
                let wobble = (world.adventure.time * 3.2 + i as f32).sin() * 0.05;
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
            mesh.orb(shot.pos, 0.12, [0.05, 0.02, 0.10], 0.0);
            mesh.glow_orb(shot.pos, 0.155, color, 4.2, 0.56);
            for i in 0..8 {
                let a = world.adventure.time * 5.8 + i as f32 * 0.785;
                let r = 0.19 + (i as f32 * 0.015);
                let p = shot.pos + Vec3::new(a.cos() * r, (a * 1.5).sin() * 0.04, a.sin() * r);
                mesh.glow_voxel(
                    p,
                    Vec3::splat(0.028),
                    Quat::from_rotation_y(a) * Quat::from_rotation_x(a * 0.7),
                    if i & 1 == 0 { [0.95, 0.72, 1.0] } else { color },
                    4.4,
                    0.74,
                );
            }
        } else {
            mesh.orb(
                shot.pos,
                if shot.hostile { 0.13 } else { 0.088 },
                [
                    (color[0] * 0.48 + 0.52).min(1.0),
                    (color[1] * 0.48 + 0.52).min(1.0),
                    (color[2] * 0.48 + 0.52).min(1.0),
                ],
                3.6,
            );
            mesh.glow_orb(
                shot.pos,
                if shot.hostile { 0.165 } else { 0.125 },
                color,
                2.8,
                0.58,
            );
            mesh.glow_beam(shot.pos - dir * 0.36, shot.pos, 0.026, color, 2.6, 0.72);
        }
        if !shot.hostile && shot.mods.orbit {
            for i in 0..2 {
                let a = world.adventure.time * 8.4 + i as f32 * std::f32::consts::PI;
                let side = dir.any_orthonormal_vector();
                let up_v = dir.cross(side).normalize_or_zero();
                let sat = shot.pos + (side * a.cos() + up_v * a.sin()) * 0.19;
                mesh.glow_voxel(
                    sat,
                    Vec3::splat(0.032),
                    Quat::from_rotation_y(a),
                    [1.0, 0.86, 0.38],
                    3.8,
                    0.82,
                );
            }
        }
        if !shot.hostile && shot.mods.homing {
            let a = world.adventure.time * 6.5;
            let side = dir.any_orthonormal_vector();
            let up_v = dir.cross(side).normalize_or_zero();
            for i in 0..3 {
                let ang = a + i as f32 * 2.094;
                let p = shot.pos - dir * 0.08 + (side * ang.cos() + up_v * ang.sin()) * 0.11;
                mesh.glow_voxel(p, Vec3::splat(0.020), Quat::IDENTITY, [0.46, 1.0, 0.82], 3.4, 0.76);
            }
        }
    }
    for drop in &world.inventory.dropped {

        let color = match drop.stack.item {

            crate::world::inventory::Item::Block(b) => b.color((drop.pos / world.voxel_scale.meters()).as_ivec3()),

            _ => [0.75, 0.58, 0.28],

        };

        mesh.cuboid(drop.pos, Vec3::splat(0.22), Quat::from_rotation_y(0.35), color, 0.0);

    }

    crate::tool_effects::draw(&mut mesh, &world.adventure.tool_debris);
    crate::tool_effects::draw_blasts(&mut mesh, &world.adventure.blasts);

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

        if weapon > 0 {

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

    if world.adventure.casting() && world.adventure.wand > 0 {
        let t = world.adventure.cast_motion();
        let recoil = t * t * 0.13;
        let grip =
            eye + forward * (0.48 - recoil) + right * (0.25 + t * 0.08) - up * (0.27 + t * 0.04);
        let tip =
            eye + forward * (0.76 - recoil) + right * (0.13 + t * 0.12) - up * (0.07 + t * 0.12);
        let color = crate::world::adventure::spell_color(world.adventure.last_cast);
        let shaft_color = match world.adventure.wand {
            1 => [0.29, 0.18, 0.10],
            2 => [0.22, 0.28, 0.36],
            _ => [0.28, 0.16, 0.34],
        };
        let ferrule_color = match world.adventure.loadout.mode {
            1 => [0.36, 0.82, 0.96],
            2 => [0.96, 0.62, 0.24],
            3 => [0.88, 0.38, 0.94],
            _ => [0.76, 0.52, 0.22],
        };
        mesh.beam(grip, tip, 0.026, shaft_color, 0.0);
        mesh.beam(tip - forward * 0.09, tip, 0.038, ferrule_color, 0.35);
        mesh.orb(tip, 0.048 + t * 0.022, color, 2.8 + t * 1.4);
        mesh.glow_orb(tip, 0.078 + t * 0.030, color, 3.4, 0.56);
        let glyph_rot = Quat::from_rotation_y(player.yaw)
            * Quat::from_rotation_x(-player.pitch)
            * Quat::from_rotation_z(world.adventure.time * 3.8);
        mesh.temple_glyph(
            tip + forward * 0.065,
            glyph_rot,
            world.adventure.last_cast,
            0.014 + t * 0.004,
            color,
            4.2,
            0.74,
        );
        for i in 0..4 {
            let a = i as f32 * 1.571 + world.adventure.time * 7.2;
            let spark_pos = tip + (right * a.cos() + up * a.sin()) * (0.075 + t * 0.055);
            mesh.glow_voxel(
                spark_pos,
                Vec3::splat(0.018),
                Quat::from_rotation_z(a),
                color,
                3.6,
                0.80,
            );
        }
    }
    mesh.vertices
}

