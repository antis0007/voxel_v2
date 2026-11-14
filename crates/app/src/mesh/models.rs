//! Lit, metre-scaled entity geometry, streamed separately from immutable terrain.

//! Moving parts advance from simulation state, never wall-clock render time.

use super::Vertex3;

use crate::world::{
    World,
    factory::{self, Kind},
    inventory::ItemStack,
};

use glam::{IVec3, Quat, Vec3};

mod conduit;
pub(crate) mod chest;
mod freight;
mod particle;
mod spell_fx;
mod precipitation;
mod realm_air;
pub(crate) mod voxel_asset;
mod machine_casing;
mod wand;
pub(crate) mod held_item;
pub(crate) mod wand_kit;
pub(crate) mod reference_sprites;
pub(crate) mod sword_art;
pub(crate) mod wand_sprite;
mod network_player;
mod traversal;
mod wayfarer;
mod wand_asset;
mod wand_pose;
#[cfg(test)]
mod wand_checks;
#[cfg(test)]
mod cuboid_tests;
mod hart;
mod orbital;
pub mod posed;
pub use hart::HartPose;

/// Vertex-coloured moving geometry, never a terrain/biome material (family 0).
pub const AUTHORED_SURFACE: f32 = 39.0;

/// Metre-sized remote player geometry; all positions remain in block coordinates.
#[cfg(feature = "multiplayer")]
pub fn append_network_player(vertices: &mut Vec<Vertex3>, scale: f32, position: Vec3, yaw: f32,
    pitch: f32, phase: f32, stride: f32, flying: bool, attack: f32, id: u32, frame: Quat) {
    let mut mesh=Model::with_vertices(scale,std::mem::take(vertices));
    mesh.set_surface_kind(AUTHORED_SURFACE);
    let begin = mesh.vertices.len();
    network_player::draw(&mut mesh,position*scale,yaw,pitch,phase,stride,flying,attack,id);
    let origin = position * scale;
    for vertex in &mut mesh.vertices[begin..] {
        vertex.pos = (origin + frame * (Vec3::from_array(vertex.pos) - origin)).to_array();
        vertex.normal = (frame * Vec3::from_array(vertex.normal)).to_array();
    }
    *vertices=mesh.vertices;
}

pub struct Model {
    pub vertices: Vec<Vertex3>,

    scale: f32,

    surface_kind: f32,

    alpha: f32,

    /// Transient erosion-front marker in the unused glow specular channel.
    /// Ordinary glow keeps 0.42; negative values drive the shader's rim light.
    dissolve: f32,
}

impl Model {
    /// Author texels before the part rotates, so armour grain follows its joints.
    pub fn material_box(&mut self, c: Vec3, size: Vec3, rotation: Quat,
        color: [f32;3], kind: f32, energy: f32) {
        let mark = self.mark();
        let previous = self.surface_kind;
        // Creature finishes extend the model palette without aliasing packed
        // voxel keys. Neutral family 39 carries the creature finish in w.
        let creature=(69.0..=72.0).contains(&kind);
        self.surface_kind = if creature { AUTHORED_SURFACE } else { kind };
        self.cuboid(c, size, Quat::IDENTITY, color, 0.0);
        self.bake_model_uv_since(mark, c, self.surface_kind);
        for vertex in &mut self.vertices[mark..] { vertex.surface[3] = if creature {kind} else {energy.clamp(0.0,1.0)}; }
        self.rotate_since(mark, c, rotation);
        self.surface_kind = previous;
    }
    pub fn scale_meters(&self) -> f32 { self.scale }

    /// Dithered distance fade for everything drawn since `mark`, using the
    /// chunk-arrival screen dither: coverage 1 draws whole, lower drops an
    /// ordered share of pixels. Carried as the normal's length (2 - coverage);
    /// the vertex shader decodes it per vertex and re-normalises. Call last:
    /// `scale_since` re-normalises normals and would erase it.
    pub fn fade_since(&mut self, mark: usize, coverage: f32) {
        if coverage >= 0.999 { return; }
        let length = 2.0 - coverage.clamp(0.0, 1.0);
        for vertex in &mut self.vertices[mark..] {
            vertex.normal = (Vec3::from_array(vertex.normal).normalize_or(Vec3::Y) * length).to_array();
        }
    }

    /// Closed rectangular limb, with material coordinates aligned to the bone.
    pub fn material_beam(&mut self, a: Vec3, b: Vec3, radius: f32, color: [f32;3], kind: f32) {
        let delta=b-a;
        if delta.length_squared()<1e-8 { return; }
        self.material_box((a+b)*0.5,Vec3::new(radius*2.0,delta.length(),radius*2.0),
            Quat::from_rotation_arc(Vec3::Y,delta.normalize()),color,kind,0.0);
    }

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
        Self::with_vertices(scale, Vec::with_capacity(65536))
    }

    fn with_vertices(scale: f32, vertices: Vec<Vertex3>) -> Self {
        Self {
            vertices,

            scale,

            surface_kind: 41.0,

            alpha: 1.0,

            dissolve: 0.0,
        }
    }

    /// Select the shader material for non-glow model pieces. The wand hand uses
    /// the dedicated skin/leather branch instead of inheriting brushed machinery.
    pub fn set_surface_kind(&mut self, kind: f32) -> f32 {
        std::mem::replace(&mut self.surface_kind, kind)
    }

    /// A model's material changes must not recolour the next entity or tool.
    pub fn with_surface_kind(&mut self, kind: f32, draw: impl FnOnce(&mut Self)) {
        let previous = self.set_surface_kind(kind);
        draw(self);
        self.set_surface_kind(previous);
    }

    /// Bake planar UVs before rigid animation; authored body pixels travel with it.
    pub fn bake_model_uv_since(&mut self, start: usize, origin: Vec3, kind: f32) {
        for v in &mut self.vertices[start..] {
            if v.surface[2] != kind { continue; }
            let p=Vec3::from_array(v.pos)*self.scale-origin;
            let n=Vec3::from_array(v.normal).abs();
            let uv=if n.y>=n.x && n.y>=n.z {[p.x,p.z]}
                else if n.x>=n.z {[p.z,p.y]} else {[p.x,p.y]};
            v.surface[0]=uv[0];v.surface[1]=uv[1];
        }
    }

    fn triangle_surface(&self, glow: f32) -> [f32; 4] {
        let roughness_or_alpha = if glow > 0.0 && self.alpha < 0.995 {
            -self.alpha.clamp(0.06, 0.98)
        } else {
            0.48
        };
        [
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
        ]
    }

    fn triangle(&mut self, mut p: [Vec3; 3], normal: Vec3, color: [f32; 3], glow: f32) {
        if (p[1] - p[0]).cross(p[2] - p[0]).dot(normal) < 0.0 {
            p.swap(1, 2);
        }
        let surface = self.triangle_surface(glow);

        for p in p {
            self.vertices.push(Vertex3 {
                pos: (p / self.scale).to_array(),

                normal: normal.to_array(),

                color,

                surface,

                ao_strength: 1.0,
            });
        }
    }

    pub fn cuboid(&mut self, c: Vec3, size: Vec3, rot: Quat, color: [f32; 3], glow: f32) {
        // Adjacent faces share corners, including rotation, translation and scale.
        let half = size * 0.5;
        let corners = [
            Vec3::new(-half.x, -half.y, -half.z),
            Vec3::new(half.x, -half.y, -half.z),
            Vec3::new(-half.x, half.y, -half.z),
            Vec3::new(half.x, half.y, -half.z),
            Vec3::new(-half.x, -half.y, half.z),
            Vec3::new(half.x, -half.y, half.z),
            Vec3::new(-half.x, half.y, half.z),
            Vec3::new(half.x, half.y, half.z),
        ].map(|p| c + rot * p);
        let positions = corners.map(|p| (p / self.scale).to_array());
        let surface = self.triangle_surface(glow);
        for (normal, [a, b, c, d]) in [
            (Vec3::X, [1, 3, 7, 5]),
            (-Vec3::X, [0, 4, 6, 2]),
            (Vec3::Y, [2, 6, 7, 3]),
            (-Vec3::Y, [0, 1, 5, 4]),
            (Vec3::Z, [4, 5, 7, 6]),
            (-Vec3::Z, [0, 2, 3, 1]),
        ] {
            let normal = rot * normal;
            // Test both triangles in world space, as triangle() does. Negative
            // or collapsed dimensions still preserve the original winding.
            let vertex = |index: usize| Vertex3 {
                pos: positions[index], normal: normal.to_array(), color,
                surface, ao_strength: 1.0,
            };
            let vertices = [vertex(a), vertex(b), vertex(c), vertex(d)];
            let first = if (corners[b] - corners[a]).cross(corners[c] - corners[a]).dot(normal) < 0.0 {
                [vertices[0], vertices[2], vertices[1]]
            } else {
                [vertices[0], vertices[1], vertices[2]]
            };
            let second = if (corners[c] - corners[a]).cross(corners[d] - corners[a]).dot(normal) < 0.0 {
                [vertices[0], vertices[3], vertices[2]]
            } else {
                [vertices[0], vertices[2], vertices[3]]
            };
            self.vertices.extend_from_slice(&[
                first[0], first[1], first[2], second[0], second[1], second[2],
            ]);
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

    /// Legacy shrinking glow voxel with an erosion-front marker. Volumetric
    /// particle erosion uses `eroding_particle`, including actual cut faces.
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

    /// Opt-in spell current, measured along the beam in metres. Values below
    /// -2 in the unused glow specular channel distinguish it from particles'
    /// [-1, 0] dissolve tag and orbital mesh tags. The coordinate continues
    /// across segments, so a moving pulse follows bends without restarting.
    pub fn spell_current(
        &mut self, a: Vec3, b: Vec3, radius: f32, color: [f32; 3],
        glow: f32, alpha: f32, distance: f32,
    ) {
        let delta = b - a;
        if delta.length_squared() < 1e-8 { return; }
        let direction = delta.normalize();
        let mark = self.mark();
        self.glow_beam(a, b, radius, color, glow, alpha);
        for vertex in &mut self.vertices[mark..] {
            let along = (Vec3::from_array(vertex.pos) * self.scale - a).dot(direction);
            vertex.surface[1] = -2.0 - (distance + along).max(0.0);
        }
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

thread_local! {
    static SCENE_POOL: std::cell::RefCell<Vec<Vertex3>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Return a finished scene buffer so the next `build` reuses its allocation.
pub fn recycle(vertices: Vec<Vertex3>) {
    SCENE_POOL.with(|pool| *pool.borrow_mut() = vertices);
}

/// Added to `surface.z` of vertices whose `pos` is already relative to the
/// camera (metres / scale). `surface.z` is `kind + 64 * (block + 1)`, so the
/// flag sits far above any block key (64 * 4096); the vertex shader strips it
/// before decoding. Mirrored as `CAMERA_RELATIVE_FLAG` in `world_shader.wgsl`
/// (a test keeps them equal). Exact in f32: integers are exact below 2^24.
const CAMERA_RELATIVE_FLAG: f32 = 262_144.0;

/// How much of a prop to draw at `distance` from the viewer: 1 inside 85% of
/// `range`, falling to 0 at `range`. The one place a draw range is decided;
/// callers skip the prop at 0 and pass the rest to `Model::fade_since`.
pub fn range_fade(distance: f32, range: f32) -> f32 {
    ((range - distance) / (range * 0.15).max(1e-3)).clamp(0.0, 1.0)
}

/// Mark vertices as camera-relative (see `CAMERA_RELATIVE_FLAG`).
fn tag_camera_relative(vertices: &mut [Vertex3]) {
    for vertex in vertices {
        vertex.surface[2] += CAMERA_RELATIVE_FLAG;
    }
}

pub fn build(world: &World, player: &crate::player::Player, day: &crate::world::DayNightCycle, pixel_angle: f32, chunks: &crate::render::ChunkCache) -> Vec<Vertex3> {
    let mut trace = crate::gameplay_trace::Trace::new();
    crate::egui_ui::held_art::sync_tools(held_item::review_bank().unwrap_or(&world.adventure.tool_bank));
    let scale = world.voxel_scale.meters();

    let viewer = player.pos * scale;

    // Reuse last frame's allocation. The scene holds ~400k vertices (orbital
    // anchorages dominate); growing a fresh Vec to that size every frame cost
    // several milliseconds of reallocation and page faults.
    let mut mesh = SCENE_POOL.with(|pool| {
        let mut recycled = std::mem::take(&mut *pool.borrow_mut());
        recycled.clear();
        // Do not construct Model::new first: that allocates 65k vertices only
        // to immediately throw them away when a pooled scene is available.
        if recycled.capacity() == 0 { recycled.reserve(65536); }
        Model::with_vertices(scale, recycled)
    });
    posed::begin_frame();
    trace.mark("scene-prepare");
    chest::build(&mut mesh, world, viewer);
    crate::world::hell_gate::appearance::draw(&mut mesh, world, viewer);
    crate::world::adventure::hedrons::draw(&mut mesh, world, viewer);
    trace.mark("scene-ground-props");
    crate::manyfold::weave::draw(&mut mesh, world);
    crate::manyfold::undercroft::draw(&mut mesh, world);
    if world.world_type == crate::world::WorldType::Manyfold
        && !engine_worldgen::manyfold::flat_maze(world.generator_version)
    {
        if let Some(socket) = engine_worldgen::manyfold::socket(&world.sample_config()) {
            let up = -engine_worldgen::manyfold::acceleration_for(world.generator_version, socket).normalize_or_zero();
            let q = Quat::from_rotation_arc(Vec3::Y, up);
            let colour = if world.manyfold_weight.opened { [0.28, 0.56, 0.40] } else { [0.65, 0.43, 0.16] };
            mesh.cuboid(socket - up * 0.35, Vec3::new(1.2, 0.25, 1.2), q, [0.22, 0.19, 0.14], 0.0);
            for x in [-0.5, 0.5] { mesh.cuboid(socket + q * Vec3::new(x, -0.12, 0.0), Vec3::new(0.12, 0.4, 1.1), q, colour, 0.0); }
            for z in [-0.5, 0.5] { mesh.cuboid(socket + q * Vec3::new(0.0, -0.12, z), Vec3::new(0.9, 0.4, 0.12), q, colour, 0.0); }
        }
        let c = world.manyfold_weight.position;
        let g = engine_worldgen::manyfold::acceleration_for(world.generator_version, c);
        let up = if g.length_squared() > 1e-4 { -g.normalize() } else { Vec3::Y };
        let q = Quat::from_rotation_arc(Vec3::Y, up);
        mesh.cuboid(c, Vec3::splat(0.4), q, [0.65, 0.43, 0.16], 0.0);
        mesh.cuboid(c, Vec3::new(0.42, 0.12, 0.42), q, [0.22, 0.16, 0.09], 0.0);
    }
    // Skyhooks and stations belong to the home planet's sky, not to other realms.
    trace.mark("scene-realm");
    if !matches!(world.world_type, crate::world::WorldType::Manyfold | crate::world::WorldType::Nether) {
        orbital::build(&mut mesh, world, day, viewer, pixel_angle);
    }
    trace.mark("scene-orbital");
    if player.anchor.is_some() && world.sample_config().is_planet() {
        crate::planet::ground_mesh::draw(&mut mesh.vertices, world, player);
    }
    network_player::review(&mut mesh,player,world.adventure.time);
    traversal::draw(&mut mesh, player, world.adventure.time);

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
        if crate::world::industry::presentation::machine(&mut mesh,world,p,n,o) { continue; }

        let active = n.enabled && matches!(n.status, "Processing" | "Generating");

        let metal = [0.27, 0.33, 0.37];

        let dark = [0.085, 0.11, 0.13];

        if matches!(n.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe) {
            let near = index <= 95 && (c - viewer).length_squared() <= 38.0 * 38.0;
            conduit::draw(&mut mesh, world, p, n, o, step, near);
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

        // Induction matrix parts: steel casing with glowing seams once formed.
        if matches!(
            n.kind,
            Kind::InductionCasing | Kind::InductionPort | Kind::InductionCell | Kind::InductionProvider
        ) {
            let glow = if n.formed { 1.0 } else { 0.0 };
            match n.kind {
                Kind::InductionCell | Kind::InductionProvider => {
                    let core = if n.kind == Kind::InductionCell {
                        [0.25, 0.85, 1.0]
                    } else {
                        [1.0, 0.75, 0.25]
                    };
                    mesh.cuboid(c, Vec3::splat(0.9), Quat::IDENTITY, dark, 0.0);
                    mesh.cuboid(c, Vec3::splat(0.5), Quat::IDENTITY, core, 0.4 + 1.4 * glow);
                }
                _ => {
                    mesh.cuboid(c, Vec3::splat(0.996), Quat::IDENTITY, [0.26, 0.29, 0.32], 0.0);
                    // Corner posts and a recessed seam band on each course.
                    for x in [0.06, 0.94] {
                        for z in [0.06, 0.94] {
                            mesh.cuboid(
                                o + Vec3::new(x, 0.5, z),
                                Vec3::new(0.13, 1.0, 0.13),
                                Quat::IDENTITY,
                                metal,
                                0.0,
                            );
                        }
                    }
                    mesh.cuboid(
                        c,
                        Vec3::new(1.004, 0.05, 1.004),
                        Quat::IDENTITY,
                        [0.25, 0.80, 0.95],
                        0.9 * glow,
                    );
                    if n.kind == Kind::InductionPort {
                        let ring = if n.mode == 0 {
                            [0.30, 0.65, 1.0]
                        } else {
                            [1.0, 0.55, 0.20]
                        };
                        for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::Y] {
                            let f = d.as_vec3();
                            let flat = Vec3::ONE - f.abs();
                            mesh.cuboid(
                                c + f * 0.5,
                                flat * 0.46 + f.abs() * 0.02,
                                Quat::IDENTITY,
                                ring,
                                0.5 + 1.2 * glow,
                            );
                            mesh.cuboid(c + f * 0.505, flat * 0.26 + f.abs() * 0.02, Quat::IDENTITY, dark, 0.0);
                        }
                    }
                }
            }
            continue;
        }

        // Brick multiblocks: coursed brick bodies, no machine plinth. The
        // controller adds its door or tuyere on the outward face, glowing
        // with the hearth temperature.
        if matches!(
            n.kind,
            Kind::CokeBrick | Kind::BlastBrick | Kind::CokeOven | Kind::BlastFurnace
        ) {
            let blast = matches!(n.kind, Kind::BlastBrick | Kind::BlastFurnace);
            let brick = if blast {
                [0.37, 0.31, 0.28]
            } else {
                [0.50, 0.27, 0.17]
            };
            let mortar = if blast {
                [0.19, 0.17, 0.16]
            } else {
                [0.29, 0.24, 0.20]
            };
            mesh.cuboid(c, Vec3::splat(0.996), Quat::IDENTITY, mortar, 0.0);
            for k in 0..4 {
                // Alternate courses differ slightly, as fired bricks do.
                let tone = if k % 2 == 0 { 1.0 } else { 0.9 };
                mesh.cuboid(
                    o + Vec3::new(0.5, 0.125 + k as f32 * 0.25, 0.5),
                    Vec3::new(1.0, 0.215, 1.0),
                    Quat::IDENTITY,
                    [brick[0] * tone, brick[1] * tone, brick[2] * tone],
                    0.0,
                );
            }
            if matches!(n.kind, Kind::CokeOven | Kind::BlastFurnace) {
                let f = n.facing.as_vec3();
                let side = Vec3::new(f.z.abs(), 0.0, f.x.abs());
                let depth = f.abs();
                let max = if blast { 1650.0 } else { 1050.0 };
                let heat = ((factory::multiblock::temperature(n) as f32 - 20.0) / max).clamp(0.0, 1.0);
                let ember = [1.0, 0.30 + 0.45 * heat, 0.06 + 0.3 * heat * heat];
                if blast {
                    // Tuyere: an iron collar around a white-hot eye.
                    mesh.beam(c + f * 0.44, c + f * 0.56, 0.21, dark, 0.0);
                    mesh.beam(c + f * 0.55, c + f * 0.58, 0.16, metal, 0.0);
                    mesh.cuboid(
                        c + f * 0.585,
                        side * 0.16 + Vec3::Y * 0.16 + depth * 0.01,
                        Quat::IDENTITY,
                        if heat > 0.05 { ember } else { dark },
                        heat * 3.2,
                    );
                } else {
                    // Arched iron door with a glowing peep slit.
                    mesh.cuboid(
                        c + f * 0.51 - Vec3::Y * 0.05,
                        side * 0.62 + Vec3::Y * 0.72 + depth * 0.04,
                        Quat::IDENTITY,
                        dark,
                        0.0,
                    );
                    mesh.cuboid(
                        c + f * 0.535 + Vec3::Y * 0.12,
                        side * 0.38 + Vec3::Y * 0.07 + depth * 0.02,
                        Quat::IDENTITY,
                        if heat > 0.05 { ember } else { metal },
                        heat * 2.4,
                    );
                    for x in [-0.22, 0.22] {
                        mesh.cuboid(
                            c + f * 0.54 + side * x - Vec3::Y * 0.2,
                            side * 0.05 + Vec3::Y * 0.08 + depth * 0.03,
                            Quat::IDENTITY,
                            metal,
                            0.0,
                        );
                    }
                }
                // Status lamp beside the controller face.
                mesh.cuboid(
                    c + f * 0.52 + side * 0.4 + Vec3::Y * 0.35,
                    Vec3::splat(0.06),
                    Quat::IDENTITY,
                    if !n.formed {
                        [0.42, 0.12, 0.10]
                    } else if active || matches!(n.status, "Coking" | "Smelting") {
                        [0.16, 0.93, 0.56]
                    } else {
                        [0.73, 0.50, 0.17]
                    },
                    if n.formed { 1.0 } else { 0.0 },
                );
            }
            continue;
        }

        if let Some(asset) = machine_casing::authored(n, active) {
            machine_casing::draw_authored(&mut mesh, asset, o, n, active, world.adventure.time);
            continue;
        }
        machine_casing::draw(&mut mesh, o, n, active);

        machine_casing::module(&mut mesh, o, n, active, world.adventure.time, step);
    }

    mesh.surface_kind = AUTHORED_SURFACE;

    world.habitats.models(&mut mesh, viewer, &world.sample_config());
    freight::draw(&mut mesh, world, viewer);
    for enemy in &world.adventure.enemies {
        let coverage = if enemy.habitat.is_some() {
            range_fade(enemy.pos.distance(viewer), crate::world::habitats::DRAW_M)
        } else {
            1.0
        };
        if coverage <= 0.0 { continue; }
        let mark = mesh.mark();
        mesh.with_surface_kind(AUTHORED_SURFACE, |mesh| enemy.model(mesh, world.adventure.time, world));
        mesh.fade_since(mark, coverage);
    }
    for companion in world.adventure.companions() {
        mesh.with_surface_kind(AUTHORED_SURFACE, |mesh| companion.model(mesh, world.adventure.time, viewer));
    }
    for shot in &world.adventure.projectiles {
        if let Some(tether) = shot.tether() {
            let color = tether.color(shot.color());
            let slack = tether.anchor.distance(shot.pos).min(5.0)
                * if tether.returning { 0.016 } else { 0.048 };
            let path = std::array::from_fn(|i| {
                let t = i as f32 / 6.0;
                tether.anchor.lerp(shot.pos, t) - Vec3::Y * (4.0 * t * (1.0 - t) * slack)
            });
            mesh.spell_thread(&path, color, tether.returning);
            mesh.orb(shot.pos, 0.052, color, 1.1);
            mesh.glow_orb(shot.pos, 0.082, color, 1.2, 0.40);
            continue;
        }
        let shot_mark = mesh.mark();
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
            mesh.surface_kind = AUTHORED_SURFACE;
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
            if shot.hostile {
                // A pointed voxel dart communicates incoming travel; no soft orb shell.
                let rotation=Quat::from_rotation_arc(Vec3::Z,if dir.length_squared()>0.5 {dir}else{Vec3::Z});
                mesh.glow_voxel(shot.pos,Vec3::splat(body*0.75),rotation,hot,1.3,0.96);
                mesh.glow_voxel(shot.pos-dir*0.13,Vec3::new(body*0.55,body*0.55,0.24),rotation,color,1.0,0.9);
                mesh.glow_voxel(shot.pos-dir*0.25,Vec3::new(body*0.30,body*0.30,0.08),rotation,color,0.7,0.8);
            } else {
                // Fixed in the flight direction. A spinning ring of beads reads as sparkle.
                let face = if dir.length_squared() > 0.5 { dir } else { Vec3::Z };
                let rot = Quat::from_rotation_arc(Vec3::Z, face);
                match shot.spell {
                    2 | 13 | 81 => {
                        let len = if shot.spell == 13 { 0.36 } else { 0.48 };
                        mesh.glow_voxel(
                            shot.pos,
                            Vec3::new(0.016, 0.016, len),
                            rot,
                            hot,
                            1.15,
                            0.5,
                        );
                        mesh.glow_voxel(
                            shot.pos + face * (len * 0.42),
                            Vec3::new(0.01, 0.01, 0.06),
                            rot,
                            color,
                            0.9,
                            0.38,
                        );
                    }
                    12 | 28 | 82 => {
                        mesh.orb(shot.pos, body * 0.72, hot, 1.15);
                        mesh.glow_orb(shot.pos, body * 1.1, color, 1.2, 0.4);
                        mesh.glow_voxel(
                            shot.pos - face * 0.16 - Vec3::Y * 0.04,
                            Vec3::splat(body * 0.32),
                            Quat::IDENTITY,
                            color,
                            0.85,
                            0.32,
                        );
                    }
                    3 | 85 => {
                        mesh.orb(shot.pos, body * 0.72, hot, 1.25);
                        mesh.glow_orb(shot.pos, body * 1.4, color, 1.15, 0.38);
                    }
                    5 => {
                        for i in 0..3 {
                            let a = i as f32 * 2.094 + shot.pos.x;
                            let p = shot.pos + Vec3::new(a.cos(), 0.02, a.sin()) * 0.055;
                            mesh.glow_voxel(p, Vec3::splat(0.02), Quat::IDENTITY, color, 0.85, 0.38);
                        }
                    }
                    14 | 83 | 86 => {
                        mesh.glow_beam(shot.pos - face * 0.4, shot.pos + face * 0.06, 0.01, color, 1.15, 0.4);
                        mesh.glow_voxel(shot.pos, Vec3::splat(0.014), Quat::IDENTITY, hot, 1.2, 0.45);
                    }
                    0 | 71 | 80 | 87 => {
                        mesh.orb(shot.pos, body * 0.26, hot, 1.2);
                        mesh.glow_beam(shot.pos - face * 0.14, shot.pos, 0.008, color, 0.75, 0.28);
                    }
                    _ => {
                        mesh.orb(shot.pos, body * 0.38, hot, 1.35);
                        mesh.glow_orb(shot.pos, body * 0.9, color, 1.15, 0.4);
                        mesh.glow_voxel(
                            shot.pos - face * 0.16,
                            Vec3::splat(0.014),
                            Quat::IDENTITY,
                            color,
                            0.7,
                            0.28,
                        );
                    }
                }
                if let Some(element) = shot.mods.element {
                    let lift = match element {
                        crate::world::adventure::status::Element::Fire => Vec3::Y * 0.04,
                        crate::world::adventure::status::Element::Frost => Vec3::Y * -0.02,
                        _ => Vec3::ZERO,
                    };
                    mesh.glow_voxel(
                        shot.pos - face * 0.06 + lift,
                        Vec3::splat(0.012),
                        Quat::IDENTITY,
                        element.color(),
                        0.8,
                        0.32,
                    );
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
        // A newly launched friendly bolt can sit less than a metre from the
        // camera and cover an entire distant enemy. Bound its apparent size
        // through geometry, preserving the incoming enemy darts at full size.
        if !shot.hostile {
            let distance = (shot.pos-player.eye(world.voxel_scale)*scale).length();
            let near_scale = (distance/3.0).clamp(0.02,1.0);
            mesh.scale_since(shot_mark,shot.pos,Vec3::splat(near_scale));
        }
    }
    let time = world.adventure.time;
    for (idx, drop) in world.inventory.dropped.iter().enumerate() {
        draw_loose_item(&mut mesh, world, drop.stack, drop.pos, idx, time);
    }
    for (idx, (pos, item)) in crate::world::undercrown::plinth_displays(world).into_iter().enumerate() {
        draw_loose_item(&mut mesh, world, ItemStack::new(item, 1), pos, idx, time);
    }

    crate::world::industry::presentation::draw(&mut mesh, world, viewer);
    crate::tool_effects::draw(&mut mesh, &world.adventure.tool_debris);
    crate::tool_effects::draw_blasts(&mut mesh, &world.adventure.blasts);
    crate::tool_effects::draw_splashes(&mut mesh, &world.adventure.splashes);
    precipitation::draw(&mut mesh, world, player, day, chunks);
    realm_air::draw(&mut mesh, world, player, chunks);
    realm_air::draw_hush(&mut mesh, world, player);
    trace.mark("scene-local-effects");

    for effect in &world.adventure.effects {
        let t = 1.0 - effect.life / effect.total;
        if let Some(normal) = effect.impact_normal {
            if effect.pos.distance_squared(viewer) < 48.0 * 48.0 {
                mesh.spell_impact(effect.pos, normal, effect.color, effect.radius,
                    (1.0 - t).clamp(0.0, 1.0), effect.family);
            }
            continue;
        }

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

    let right = forward.cross(player.up_dir()).normalize_or_zero();

    let up = right.cross(forward).normalize_or_zero();

    let shell = (world.world_type == crate::world::WorldType::Manyfold
        && !engine_worldgen::manyfold::flat_maze(world.generator_version))
        .then_some(world.generator_version);
    world.adventure.magic_models(&mut mesh, eye, forward, shell,
        crate::world::workshop::has_lanterncap(&world.sample_config()));
    world.portals.circles.draw(&mut mesh, eye);
    let quest = crate::egui_ui::quest_log::marker(world, viewer);
    world.adventure.quest_beacon_models(quest.as_ref(), &mut mesh);

    for rem in &world.adventure.remains {
        if rem.model(&mut mesh, world.adventure.time) { continue; }
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

    // Everything glued to the camera (tool swing, hand, wand) is authored
    // camera-relative: an absolute f32 position far from the origin quantises
    // to millimetres or worse, which made the hand shimmer. The shader treats
    // vertices tagged `CAMERA_RELATIVE_FLAG` as offsets from the lens and
    // projects them without the view matrix, so no world term may enter: the
    // eye's sub-f32 remainder made the hand jump by a texel-sized step per frame.
    let viewmodel_mark = mesh.mark();
    mesh.set_surface_kind(AUTHORED_SURFACE);
    let eye = Vec3::ZERO;
    let hand_view=wand::WandView {eye,forward,right,up,time:world.adventure.time,
        cast:0.0,spell:[0.0;3],boost:player.movement.boost(),motion:player.viewmodel};

    // The held wand: styled from the player's atelier choices, idle in the hand
    // and drawn forward when a cast is under way. The tool takes the hand on a swing.
    let pinned = held_item::pinned();
    if (world.adventure.wand > 0 || player.viewmodel.equip > 0.01)
        && world.adventure.swing() <= 0.0 && !pinned {
        let adv = &world.adventure;
        let spell = if adv.casting() { adv.last_cast } else { adv.loadout.slot(adv.loadout.cursor) };
        let held_seed=if adv.wand>0 { adv.loadout.seed } else { player.viewmodel.seed };
        let view = wand::WandView {
                eye,
                forward,
                right,
                up,
                time: adv.time,
                boost: player.movement.boost(),
                motion: player.viewmodel,
                cast: if adv.casting() {
                    adv.cast_motion().clamp(0.0, 1.0)
                } else {
                    0.0
                },
                spell: crate::world::adventure::spell_color(spell),
        };
        if crate::world::adventure::wands::is_grimoire(adv.loadout.mode) {
            wand::draw_grimoire(&mut mesh, &view);
        } else {
            wand::draw(&mut mesh,
                crate::world::adventure::wand_items::style(held_seed, crate::wand_style::current()), &view);
        }
    }
    // Held hotbar items drop away while a wand is drawn; a melee swing takes the hand.
    let swinging = world.adventure.swing() > 0.0;
    let stowed = held_item::stowed(world, player);
    if !stowed && (pinned || swinging || world.adventure.wand == 0 && player.viewmodel.equip <= 0.01) {
        let mut held_view = held_item::View {
            eye, forward, right, up,
            time: world.adventure.time,
            sway: player.viewmodel.sway.value,
            gait: player.viewmodel.gait,
            speed: player.viewmodel.speed,
            clip: player.viewmodel.pose(),
            lowered: if pinned || swinging { 0.0 } else { player.viewmodel.equip },
            torch_lit: false,
        };
        let held=held_item::held(world,player);
        let held=if swinging { held_item::swung(world,held) } else { held };
        let holding_torch = held.and_then(|item| item.placeable_block()).is_some_and(|block| block.is_torch());
        if holding_torch {
            let position = held_item::torch_position(world, player, &held_view, false);
            held_view.torch_lit = crate::held_flame_can_burn(world, player, position);
        }
        let placement = match held {
            Some(item) => held_item::draw(&mut mesh, item, &held_view),
            None => held_item::bare_placement(&held_view),
        };
        let relax = if holding_torch {
            held_item::TORCH_RELAX
        } else if held.is_none() {
            held_item::BARE_RELAX
        } else {
            0.0
        };
        wayfarer::draw_hand(&mut mesh, &hand_view, Some(placement), relax);
    }
    if let Some(stack) = world.inventory.offhand.filter(|_| !stowed) {
        // Reflect geometry, including winding, to retain front-facing surfaces.
        let start = mesh.vertices.len();
        let mut v = held_item::View { eye, forward, right, up, time: world.adventure.time,
            sway: player.viewmodel.sway.value, gait: player.viewmodel.gait, speed: player.viewmodel.speed,
            clip: Default::default(), lowered: 0.0, torch_lit: false };
        let holding_torch = held_item::offhand_torch(world);
        if holding_torch {
            let position = held_item::torch_position(world, player, &v, true);
            v.torch_lit = crate::held_flame_can_burn(world, player, position);
        }
        let placement = held_item::draw(&mut mesh, stack.item, &v);
        let relax = if holding_torch { held_item::TORCH_RELAX } else { 0.0 };
        wayfarer::draw_hand(&mut mesh, &hand_view, Some(placement), relax);
        for vertex in &mut mesh.vertices[start..] {
            let p = Vec3::from_array(vertex.pos);
            vertex.pos = (p - right * (2.0 * (p-eye/mesh.scale).dot(right))).to_array();
            let n = Vec3::from_array(vertex.normal);
            vertex.normal = (n - right * (2.0*n.dot(right))).to_array();
        }
        for triangle in mesh.vertices[start..].chunks_exact_mut(3) { triangle.swap(1, 2); }
    }
    tag_camera_relative(&mut mesh.vertices[viewmodel_mark..]);
    for vertex in &mut mesh.vertices[viewmodel_mark..] {
        let kind=vertex.surface[2]-CAMERA_RELATIVE_FLAG;
        if kind==54.0 || kind==55.0 { vertex.ao_strength=2.0+player.viewmodel.sky; }
    }
    posed::end_frame();
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
    let right = forward.cross(player.up_dir()).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let spell = if adv.casting() { adv.last_cast } else { adv.loadout.slot(adv.loadout.cursor) };
    Some(wand::tip_light(
        crate::world::adventure::wand_items::style(adv.loadout.seed, crate::wand_style::current()),
        &wand::WandView {
            eye,
            forward,
            right,
            up,
            time: adv.time,
            boost: player.movement.boost(),
            motion: player.viewmodel,
            cast: if adv.casting() {
                adv.cast_motion().clamp(0.0, 1.0)
            } else {
                0.0
            },
            spell: crate::world::adventure::spell_color(spell),
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

    /// The viewmodel projection ignores the view matrix, so the hand must not
    /// move when only the eye's sub-f32 remainder changes.
    #[test]
    fn hand_is_rigid_to_the_lens_for_sub_f32_eye_steps() {
        use crate::{fluid::VoxelScalePreset, player::Player, world::{DayNightCycle, World, WorldType}};
        let world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 74);
        let day = DayNightCycle::new(0.5, 600.0);
        let hand = |x: f64| {
            let mut player = Player::new();
            player.precise_pos = glam::DVec3::new(x, 80.0, 1_000.0);
            player.pos = player.precise_pos.as_vec3();
            let vertices = build(&world, &player, &day, 0.0005, &crate::render::ChunkCache::new(IVec3::ZERO));
            vertices.into_iter().filter(|v| v.surface[2] >= CAMERA_RELATIVE_FLAG).map(|v| v.pos).collect::<Vec<_>>()
        };
        let near = hand(329_440.0);
        let shifted = hand(329_440.013);
        assert!(!near.is_empty() && near.len() == shifted.len(), "hand vertices: {} vs {}", near.len(), shifted.len());
        let drift = near.iter().zip(&shifted)
            .map(|(a, b)| Vec3::from(*a).distance(Vec3::from(*b))).fold(0.0, f32::max);
        assert!(drift < 1e-5, "hand moved {drift} m against the lens for a 13 mm eye step at x = 329 km");
    }

    #[test]
    fn camera_relative_flag_leaves_kind_and_block_key_decodable() {
        // surface.z = kind + 64 * (block + 1); the flag must clear every block key.
        const { assert!(CAMERA_RELATIVE_FLAG >= 64.0 * 4096.0) };
        for z in [0.0_f32, 41.0, 54.0, 55.0, 64.0 * 3.0 + 8.0, 64.0 * 256.0 + 63.0] {
            let mut v = [<Vertex3 as bytemuck::Zeroable>::zeroed()];
            v[0].surface[2] = z;
            tag_camera_relative(&mut v);
            assert!(v[0].surface[2] >= CAMERA_RELATIVE_FLAG);
            assert_eq!(v[0].surface[2] - CAMERA_RELATIVE_FLAG, z);
        }
    }

    #[test]
    fn shader_flag_matches_the_mesh_flag() {
        let shader = include_str!("../shaders/world_shader.wgsl");
        let want = format!("const CAMERA_RELATIVE_FLAG: f32 = {:.1};", CAMERA_RELATIVE_FLAG);
        assert!(shader.contains(&want), "world_shader.wgsl must declare `{want}`");
    }
}

/// A loose item hovering at `pos`: bobbing, turning, with a beacon for
/// notable gear. Used for dropped stacks and Undercrown plinth rewards.
fn draw_loose_item(mesh: &mut Model, world: &World, stack: ItemStack, pos: Vec3, idx: usize, time: f32) {
    use crate::world::inventory::{ArmorTier, Item};
    let phase = time * 2.4 + idx as f32 * 1.13 + pos.x * 0.7;
    let bob = (phase.sin() * 0.055) + 0.06;
    let rot = Quat::from_rotation_y(time * 1.65 + idx as f32 * 0.8)
        * Quat::from_rotation_z((phase * 0.7).cos() * 0.14);
    let (color, emissive, beam_col) = match stack.item {
        Item::Assembled { .. } | Item::Part { .. } => ([0.66,0.67,0.70],0.0,None),
        Item::Cast(_) => ([0.68,0.49,0.31],0.0,None),
        Item::Bucket(_) => ([0.45, 0.60, 0.72], 0.0, None),
        Item::PowerKit(_) => ([0.48, 0.76, 0.88], 0.2, None),
        Item::Block(b) => {
            let c = b.color((pos / world.voxel_scale.meters()).as_ivec3());
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
                ArmorTier::Kinetic => [0.96, 0.58, 0.22],
                ArmorTier::Ballast => [0.64, 0.43, 0.24],
                ArmorTier::Burner => [0.80, 0.40, 0.18],
                ArmorTier::Induction => [0.28, 0.74, 0.86],
                ArmorTier::Slime => [0.42, 0.78, 0.28],
                ArmorTier::Glider => [0.82, 0.72, 0.45],
                ArmorTier::Survey => [0.26, 0.73, 0.68],
                ArmorTier::Runesight => [0.67, 0.48, 0.93],
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
        Item::Wand { look, .. } => {
            let center = pos + Vec3::Y * bob;
            if look & crate::world::adventure::wand_items::GRIMOIRE_LOOK != 0 {
                wand::book(mesh, center, rot * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                    [0.62, 0.41, 0.72], 0.35);
            } else {
                crate::world::adventure::wand_items::draw_dropped(mesh, center, rot, look);
            }
            return;
        }
    };

    let center = pos + Vec3::Y * bob;
    let copies = if stack.count >= 16 {
        3
    } else if stack.count > 1 {
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
