//! Mana hedrons: standing voxel crystals that leak one element's mana.
//!
//! Hedrons stand only beside surface sites (`engine_worldgen::sites::site_seat`), drawn on a
//! carved plinth with pylon sockets and a glyph slab. Place and element are a pure function of the world seed and the generated
//! ground height, so every peer sees the same ones and nothing is saved. It is drawn from
//! whole voxel-grid blocks and never owns terrain. Four pylons of the element's block,
//! one on each cardinal side, capture it: a captured hedron adds mana recovery for a
//! caster in reach and pays ritual circles there. The table is `assets/mana_sources.ron`.
use super::*;
use crate::world::{BlockId, WorldType};
use engine_core::hash::hash01;
use status::Element;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
enum Essence {
    Fire,
    Frost,
    Shock,
    Toxic,
    Wet,
}

impl Essence {
    fn element(self) -> Element {
        match self {
            Self::Fire => Element::Fire,
            Self::Frost => Element::Frost,
            Self::Shock => Element::Shock,
            Self::Toxic => Element::Toxic,
            Self::Wet => Element::Wet,
        }
    }
}

#[derive(serde::Deserialize)]
struct Row {
    element: Essence,
    pylon: BlockId,
    mana_per_second: f32,
}

#[derive(serde::Deserialize)]
struct Table {
    share: f32,
    reach_m: f32,
    tap_m: f32,
    matched: f32,
    hover: i32,
    pylon_offset: i32,
    pylon_height: i32,
    essences: Vec<Row>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let table: Table = ron::from_str(include_str!("../../../../../assets/mana_sources.ron"))
        .unwrap_or_else(|error| panic!("assets/mana_sources.ron: {error}"));
    assert!(!table.essences.is_empty() && table.hover >= 1, "assets/mana_sources.ron: no essences or no plinth");
    table
});

/// Half-widths of the diamond cross-section, bottom layer first, in hedron blocks.
const LAYERS: [i32; 9] = [0, 0, 1, 1, 2, 1, 1, 0, 0];
const DRAW_RANGE_M: f32 = 192.0;

#[derive(Clone, Copy)]
pub struct Hedron {
    /// Lowest corner of the bottom block, in voxels; x and z are multiples of `block`.
    base: IVec3,
    /// Generated ground height under it, in voxels.
    ground: i32,
    /// Edge of one hedron block, in voxels.
    block: i32,
    row: usize,
}

type Key = (u32, u32, i32, i32);
static SITES: LazyLock<Mutex<HashMap<Key, Option<Hedron>>>> = LazyLock::new(Default::default);
/// The hedron in reach last tick and whether it was captured, for the notice.
static NOTICED: Mutex<Option<(IVec3, bool)>> = Mutex::new(None);

fn hosts(world: &World) -> bool {
    !matches!(world.world_type, WorldType::Nether | WorldType::Manyfold | WorldType::Hall | WorldType::Realm(_) | WorldType::Superflat)
}

fn site(world: &World, cx: i32, cz: i32) -> Option<Hedron> {
    site_for(&world.sample_config(), cx, cz)
}

/// `--terrain-survey`: the hedrons nearest the origin as (element, x, z, ground), in metres.
pub fn survey(cfg: &engine_worldgen::WorldSampleConfig, count: usize) -> Vec<(String, f32, f32, f32)> {
    let vm = cfg.voxel_meters();
    let mut found: Vec<_> = (-6..6)
        .flat_map(|x| (-6..6).map(move |z| (x, z)))
        .filter_map(|(x, z)| site_for(cfg, x, z))
        .map(|hedron| {
            let at = hedron.centre(vm);
            (format!("{:?}", TABLE.essences[hedron.row].element), at.x, at.z, (hedron.ground + 1) as f32 * vm)
        })
        .collect();
    found.sort_by(|a, b| (a.1 * a.1 + a.2 * a.2).total_cmp(&(b.1 * b.1 + b.2 * b.2)));
    found.truncate(count);
    found
}

fn site_for(cfg: &engine_worldgen::WorldSampleConfig, cx: i32, cz: i32) -> Option<Hedron> {
    let key = (cfg.seed, cfg.generator_version, cx, cz);
    if let Some(found) = SITES.lock().ok().and_then(|sites| sites.get(&key).copied()) {
        return found;
    }
    let table = &*TABLE;
    let word = cfg.seed ^ (cx as u32).wrapping_mul(0x9e37_79b1) ^ (cz as u32).wrapping_mul(0x85eb_ca77) ^ 0x4ed2_0a11;
    // Cells are surface-site regions: a hedron only stands seated beside a site.
    let seat = (hash01(word) < table.share).then(|| engine_worldgen::sites::site_seat(cfg, cx, cz, 0x4ed2_0a11)).flatten();
    let found = seat.map(|(sx, sz)| {
        let vm = cfg.voxel_meters();
        let block = (0.5 / vm).ceil().max(1.0) as i32;
        let snap = |m: f32| ((m / vm).floor() as i32).div_euclid(block) * block;
        let (x, z) = (snap(sx), snap(sz));
        let ground = engine_worldgen::terrain_height(cfg, x, z).max(cfg.water_level());
        let row = ((hash01(word ^ 0x7f4a_7c15) * table.essences.len() as f32) as usize).min(table.essences.len() - 1);
        Hedron { base: IVec3::new(x, ground + 1 + table.hover * block, z), ground, block, row }
    });
    if let Ok(mut sites) = SITES.lock() {
        if sites.len() > 4096 {
            sites.clear();
        }
        sites.insert(key, found);
    }
    found
}

impl Hedron {
    /// Centre of the widest layer, in metres.
    fn centre(&self, vm: f32) -> Vec3 {
        let half = self.block as f32 * 0.5;
        (self.base.as_vec3() + Vec3::new(half, self.block as f32 * LAYERS.len() as f32 * 0.5, half)) * vm
    }

    fn pylons(&self) -> [IVec3; 4] {
        let out = TABLE.pylon_offset * self.block;
        [(out, 0), (-out, 0), (0, out), (0, -out)].map(|(x, z)| IVec3::new(self.base.x + x, self.ground, self.base.z + z))
    }

    /// Four pylons stand: each cardinal column holds enough of the element's block.
    /// The column scan runs at most once per `CAPTURE_REFRESH_S` of simulation time per
    /// hedron; supply, notice and draw all read that one result.
    fn captured(&self, world: &World) -> bool {
        if *REVIEW_CAPTURED {
            return true;
        }
        let now = world.adventure.time;
        if let Some(&(held, at)) = CAPTURES.lock().ok().as_ref().and_then(|map| map.get(&self.base)) {
            if (now - at).abs() < CAPTURE_REFRESH_S {
                return held;
            }
        }
        let (table, row) = (&*TABLE, &TABLE.essences[self.row]);
        let top = self.block * (table.pylon_height + LAYERS.len() as i32);
        let held = self.pylons().iter().all(|foot| {
            let cells = (-3 * self.block..=top).filter(|&y| world.get(*foot + IVec3::Y * y) == row.pylon).count();
            cells as i32 >= table.pylon_height
        });
        if let Ok(mut map) = CAPTURES.lock() {
            if map.len() > 256 {
                map.retain(|_, (_, at)| (now - *at).abs() < 4.0 * CAPTURE_REFRESH_S);
            }
            map.insert(self.base, (held, now));
        }
        held
    }
}

const CAPTURE_REFRESH_S: f32 = 0.5;
/// Last capture scan per hedron base: (captured, simulation time of the scan).
static CAPTURES: LazyLock<Mutex<HashMap<IVec3, (bool, f32)>>> = LazyLock::new(Default::default);
static REVIEW_CAPTURED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("VOXEL_REVIEW_HEDRON").is_ok_and(|mode| mode == "captured"));

/// Review captures: `VOXEL_REVIEW_HEDRON` stands hedrons of every element
/// about 12 m from the first viewer on every side; the value `captured` lights their frames.
static REVIEW: std::sync::OnceLock<Vec<Hedron>> = std::sync::OnceLock::new();

fn review(world: &World, at: Vec3) -> Option<&'static [Hedron]> {
    std::env::var_os("VOXEL_REVIEW_HEDRON")?;
    Some(REVIEW.get_or_init(|| {
        let vm = world.voxel_scale.meters();
        let block = (0.5 / vm).ceil().max(1.0) as i32;
        let feet = (at / vm).floor().as_ivec3();
        let out = (10.0 / vm) as i32;
        [(6, 10), (-6, 10), (6, -10), (-6, -10), (10, 6), (10, -6), (-10, 6), (-10, -6)]
            .map(|(x, z)| (x * out / 10, z * out / 10))
            .iter()
            .enumerate()
            .map(|(row, &(x, z))| {
                let snap = |v: i32| v.div_euclid(block) * block;
                let ground = feet.y - 1;
                Hedron {
                    base: IVec3::new(snap(feet.x + x), ground + 1 + TABLE.hover * block, snap(feet.z + z)),
                    ground,
                    block,
                    row: row % TABLE.essences.len(),
                }
            })
            .collect()
    }))
}

fn near(world: &World, at: Vec3, range: f32) -> Vec<Hedron> {
    if let Some(fixed) = review(world, at) {
        return fixed.to_vec();
    }
    if !hosts(world) {
        return Vec::new();
    }
    let cell = engine_worldgen::sites::site_region_span(&world.sample_config());
    // Site regions are centred on index * span; seats sit within 32 m of the centre.
    let reach = range + 32.0;
    let (x0, x1) = (((at.x - reach) / cell).round() as i32, ((at.x + reach) / cell).round() as i32);
    let (z0, z1) = (((at.z - reach) / cell).round() as i32, ((at.z + reach) / cell).round() as i32);
    let vm = world.voxel_scale.meters();
    (x0..=x1)
        .flat_map(|x| (z0..=z1).map(move |z| (x, z)))
        .filter_map(|(x, z)| site(world, x, z))
        .filter(|hedron| hedron.centre(vm).distance(at) <= range)
        .collect()
}

/// Mana per second that captured hedrons give a caster or circle at `at` (metres).
/// A hedron of the working `element` gives `matched` times as much.
pub fn supply(world: &World, at: Vec3, element: Option<Element>) -> f32 {
    near(world, at, TABLE.reach_m)
        .iter()
        .filter(|hedron| hedron.captured(world))
        .map(|hedron| {
            let row = &TABLE.essences[hedron.row];
            row.mana_per_second * if element == Some(row.element.element()) { TABLE.matched } else { 1.0 }
        })
        .sum()
}

/// One hedron as the dev panel reports it.
pub struct Report {
    pub element: String,
    pub centre_m: [f32; 3],
    pub distance_m: f32,
    pub captured: bool,
    pub pylon_block: String,
    /// Every voxel cell of the four pylons, lowest first.
    pub pylon_cells: Vec<[i32; 3]>,
    pub mana_per_second: f32,
}

/// Hedrons within `range` metres of `at`, nearest first.
pub fn report(world: &World, at: Vec3, range: f32) -> Vec<Report> {
    let vm = world.voxel_scale.meters();
    let mut found: Vec<_> = near(world, at, range)
        .into_iter()
        .map(|hedron| {
            let row = &TABLE.essences[hedron.row];
            let centre = hedron.centre(vm);
            Report {
                element: format!("{:?}", row.element),
                centre_m: centre.to_array(),
                distance_m: centre.distance(at),
                captured: hedron.captured(world),
                pylon_block: format!("{:?}", row.pylon),
                pylon_cells: hedron
                    .pylons()
                    .iter()
                    .flat_map(|foot| (1..=TABLE.pylon_height).map(move |y| (*foot + IVec3::Y * y).to_array()))
                    .collect(),
                mana_per_second: row.mana_per_second,
            }
        })
        .collect();
    found.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m));
    found
}

/// A generator in hedron-tap mode at `at` (metres) stands beside a captured hedron.
pub fn feeds(world: &World, at: Vec3) -> bool {
    near(world, at, TABLE.tap_m).iter().any(|hedron| hedron.captured(world))
}

/// Says once when the hedron in reach is captured or its frame is broken.
pub fn notice(world: &mut World, at: Vec3) {
    let vm = world.voxel_scale.meters();
    let nearest = near(world, at, TABLE.reach_m)
        .into_iter()
        .min_by(|a, b| a.centre(vm).distance_squared(at).total_cmp(&b.centre(vm).distance_squared(at)));
    let now = nearest.map(|hedron| (hedron.base, hedron.captured(world)));
    let Ok(mut last) = NOTICED.lock() else { return };
    if let (Some((base, held)), Some(hedron)) = (now, nearest) {
        let row = &TABLE.essences[hedron.row];
        match *last {
            Some((seen, was)) if seen == base && was != held => {
                if held {
                    world.goals.note(crate::world::goals::Event::HedronCaptured);
                }
                world.expedition.say(if held {
                    format!("{:?} Hedron Captured: +{:.0} Mana/s", row.element, row.mana_per_second)
                } else {
                    format!("{:?} Hedron Released", row.element)
                });
            }
            Some((seen, _)) if seen == base => {}
            _ if !held => world.expedition.say(format!("{:?} Hedron · Raise 4 {} Pylons", row.element, engine_content::MATERIAL_TABLE[row.pylon as usize].display_name)),
            _ => {}
        }
    }
    *last = now;
}

pub fn draw(mesh: &mut Model, world: &World, viewer: Vec3) {
    let vm = world.voxel_scale.meters();
    for hedron in near(world, viewer, DRAW_RANGE_M) {
        let row = &TABLE.essences[hedron.row];
        let coverage = crate::mesh::models::range_fade(hedron.centre(vm).distance(viewer), DRAW_RANGE_M);
        if coverage <= 0.0 {
            continue;
        }
        let close = hedron.centre(vm).distance_squared(viewer) < 64.0 * 64.0;
        let hedron_mark = mesh.mark();
        let held = close && hedron.captured(world);
        let tint = row.element.element().color();
        let pulse = 0.5 + 0.5 * (world.adventure.time * 1.7).sin();
        let glow = if held { 0.55 + 0.2 * pulse } else { 0.18 + 0.06 * pulse };
        let b = hedron.block as f32;
        let spine = glam::Vec2::new(hedron.base.x as f32 + b * 0.5, hedron.base.z as f32 + b * 0.5) * vm;
        let mut layer = 0;
        while layer < LAYERS.len() {
            // Equal layers stack into one box per row of the diamond.
            let radius = LAYERS[layer];
            let run = LAYERS[layer..].iter().take_while(|&&r| r == radius).count();
            for dz in -radius..=radius {
                let wide = (2 * (radius - dz.abs()) + 1) as f32 * b;
                let low = hedron.base.as_vec3() + Vec3::new(b * 0.5, layer as f32 * b, (dz as f32 + 0.5) * b);
                let centre = low + Vec3::Y * run as f32 * b * 0.5;
                // The spine reads brighter than the facets.
                // Facets darken away from the spine so the diamond reads as a solid.
                let core = if dz == 0 { 1.0 } else { 0.78 - 0.14 * (dz.abs() - 1) as f32 };
                // Cut crystal (surface kind 56) with the hedron tag the shader reads
                // (world_shader.wgsl `hedron`): w = 2 + glow, and x/y = the spine's
                // x/z offset from each vertex in metres.
                let mark = mesh.mark();
                mesh.with_surface_kind(56.0, |mesh| {
                    mesh.cuboid(centre * vm, Vec3::new(wide, run as f32 * b, b) * vm, Quat::IDENTITY, tint.map(|c| c * c), 0.0);
                });
                let scale = mesh.scale_meters();
                for vertex in &mut mesh.vertices[mark..] {
                    let at = Vec3::from_array(vertex.pos) * scale;
                    vertex.surface[0] = spine.x - at.x;
                    vertex.surface[1] = spine.y - at.z;
                    vertex.surface[3] = 2.0 + glow * core;
                }
            }
            layer += run;
        }
        draw_seat(mesh, &hedron, vm, tint);
        if held {
            // A lit cap over each pylon shows the frame is live.
            for foot in hedron.pylons() {
                let top = foot + IVec3::Y * (1 + TABLE.pylon_height.max(1) * hedron.block);
                let cap = (top.as_vec3() + Vec3::new(0.5, 0.2 + 0.1 * pulse, 0.5)) * vm;
                mesh.cuboid(cap, Vec3::splat(0.3 * vm.max(0.5)), Quat::IDENTITY, tint, 0.8);
            }
        }
        mesh.fade_since(hedron_mark, coverage);
    }
}

const STONE: [f32; 3] = [0.40, 0.39, 0.37];
const CUT: [f32; 3] = [0.22, 0.21, 0.20];

/// The carved plinth the hedron rests on, a socket at each pylon foot and an upright
/// slab on the diagonal carrying the capture glyph: a diamond with four marks around it.
fn draw_seat(mesh: &mut Model, hedron: &Hedron, vm: f32, tint: [f32; 3]) {
    let b = hedron.block as f32;
    let axis = Vec3::new(hedron.base.x as f32 + b * 0.5, 0.0, hedron.base.z as f32 + b * 0.5);
    let top = (hedron.ground + 1) as f32 + TABLE.hover as f32 * b;
    let floor = (hedron.ground + 1) as f32;
    // Footing runs 3 blocks into the ground so slopes never show a gap under it.
    let deep = floor - 3.0 * b;
    let step = floor + b * 0.5;
    let block = |mesh: &mut Model, lo: f32, hi: f32, half: f32, color: [f32; 3]| {
        let centre = Vec3::new(axis.x, (lo + hi) * 0.5, axis.z);
        mesh.cuboid(centre * vm, Vec3::new(2.0 * half, hi - lo, 2.0 * half) * vm, Quat::IDENTITY, color, 0.0);
    };
    block(mesh, deep, step, 2.5 * b, STONE);
    block(mesh, step, top - 0.15 * b, 1.5 * b, STONE);
    // Cap stone with an element inlay the hedron tip points at.
    block(mesh, top - 0.15 * b, top, 1.5 * b, CUT);
    block(mesh, top - 0.05 * b, top + 0.02 * b, 0.5 * b, tint);
    for foot in hedron.pylons() {
        let at = Vec3::new(foot.x as f32 + b * 0.5, floor + 0.04 * b, foot.z as f32 + b * 0.5);
        mesh.cuboid(at * vm, Vec3::new(1.1 * b, 0.12 * b, 1.1 * b) * vm, Quat::IDENTITY, CUT, 0.0);
        mesh.cuboid((at + Vec3::Y * 0.07 * b) * vm, Vec3::new(0.4 * b, 0.04 * b, 0.4 * b) * vm, Quat::IDENTITY, tint, 0.25);
    }
    // Glyph slab on the +x+z diagonal, its face turned toward the hedron.
    let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    let out = 3.2 * b;
    let slab = axis + Vec3::new(out, floor + 1.1 * b, out);
    mesh.cuboid(slab * vm, Vec3::new(2.0 * b, 2.6 * b, 0.35 * b) * vm, turn, STONE, 0.0);
    let face = slab - turn * Vec3::Z * 0.19 * b;
    let mark = |mesh: &mut Model, dx: f32, dy: f32, size: f32, color: [f32; 3], glow: f32| {
        let at = face + turn * Vec3::new(dx * b, dy * b, 0.0);
        mesh.cuboid(at * vm, Vec3::new(size * b, size * b, 0.05 * b) * vm, turn, color, glow);
    };
    let diamond = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
    let at = face + turn * Vec3::new(0.0, 0.25 * b, 0.0);
    mesh.cuboid(at * vm, Vec3::new(0.32 * b, 0.32 * b, 0.05 * b) * vm, turn * diamond, tint, 0.35);
    for (dx, dy) in [(0.62, 0.0), (-0.62, 0.0), (0.0, 0.62), (0.0, -0.62)] {
        mark(mesh, dx, 0.25 + dy, 0.2, CUT, 0.0);
    }
}

/// Parse and check the table now; called once at startup.
pub fn validate() {
    LazyLock::force(&TABLE);
}
