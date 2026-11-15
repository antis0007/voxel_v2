//! The Gilded Cave: a pocket realm entered through a placed gilded gate. One
//! authored procession, warm to cold: a sand stair down from the arrival
//! landing, a domed palace gallery with side vaults, a cavern of coin dunes,
//! and a teal grotto where a stair winds up a rock pillar to a lit summit.
//! Everything is in world metres so the layout is the same at every voxel
//! scale; the realm runs along +z from the arrival gate at the origin.
//!
//! Every room is described by one [`Layout`]: plain numbers with a schema
//! ([`LAYOUT_FIELDS`]) that the dev tools edit live. A world's layout comes
//! from its seed ([`default_layout`]), so no two caves read as one copied
//! location, unless the dev tools set an override for that seed
//! ([`set_layout_override`], persisted by the app as a world sidecar).
//!
//! No historical world is a Gilded Cave, so this layout needs no generator
//! gate. Any later change to a seed's default must gate on `generator_version`.
use crate::{hash3, value_noise2, value_noise3, BlockId, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec3};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

/// Solid bedrock below this height.
pub const BEDROCK_M: f32 = -48.0;
/// Top of the solid roof; the column height.
pub const ROOF_TOP_M: f32 = 40.0;

/// Floor of the palace gallery and the treasure hall.
const HALL_Y: f32 = -12.0;
/// The arrival stair descends along z between these.
const STAIR_Z: (f32, f32) = (4.0, 16.0);
const GALLERY_Z0: f32 = 16.0;
/// The grotto pool surface.
const POOL_Y: f32 = -20.0;
/// Summit floor radius around the pillar axis.
const STAIR_OUT: f32 = 5.6;
const STAIR_W: f32 = 2.0;
/// Radius of the shaft of light from the dome boss to the summit.
const SHAFT_R: f32 = 1.5;
/// Reflecting channels sit this far out from the causeway axis.
const CHANNEL_X: (f32, f32) = (3.0, 6.0);

const ROCK: u32 = 0x474f_4c44;
const VAULT: u32 = 0x5641_554c;
const DUNE: u32 = 0x4455_4e45;
const CACHE: u32 = 0x4341_4348;
const PILLAR: u32 = 0x5049_4c52;
const LIGHT: u32 = 0x4c49_4754;
const PLAN: u32 = 0x504c_414e;

// --- Layout schema ------------------------------------------------------------

/// How the dev tools draw one layout field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Number,
    /// 0 or 1.
    Toggle,
    /// An index into `options`.
    Choice,
    /// sRGB packed as 0xRRGGBB (exact in an f32).
    Color,
}

/// One editable layout number with its group, label, unit and bounds.
#[derive(Clone, Copy, Debug)]
pub struct LayoutField {
    pub key: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub kind: FieldKind,
    pub options: &'static [&'static str],
    pub help: &'static str,
}

macro_rules! layout {
    ($( $group:literal { $( $name:ident : $kind:ident $label:literal $unit:literal [$min:expr, $max:expr, $step:expr] [$($opt:literal),*] $help:literal ; )* } )*) => {
        /// Every dial of one Gilded Cave, in metres, ratios, radians or packed
        /// sRGB. See [`LAYOUT_FIELDS`] for bounds and meaning.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Layout { $( $( pub $name: f32, )* )* }

        /// The schema of [`Layout`], in editor order.
        pub const LAYOUT_FIELDS: &[LayoutField] = &[ $( $( LayoutField {
            key: stringify!($name), group: $group, label: $label, unit: $unit,
            min: $min, max: $max, step: $step, kind: FieldKind::$kind, options: &[$($opt),*], help: $help,
        }, )* )* ];

        impl Layout {
            pub fn get(&self, key: &str) -> Option<f32> {
                match key { $( $( stringify!($name) => Some(self.$name), )* )* _ => None }
            }
            /// Sets one field, clamped to its bounds. False for an unknown key.
            pub fn set(&mut self, key: &str, value: f32) -> bool {
                let Some(field) = LAYOUT_FIELDS.iter().find(|f| f.key == key) else { return false };
                if !value.is_finite() { return false; }
                let v = if field.kind == FieldKind::Color { value.clamp(0.0, 16_777_215.0).round() } else { value.clamp(field.min, field.max) };
                match key { $( $( stringify!($name) => self.$name = v, )* )* _ => return false }
                true
            }
        }
    };
}

layout! {
    "Gallery" {
        gallery_len: Number "Length" "m" [24.0, 96.0, 1.0] [] "Palace gallery length; the cavern and grotto follow it.";
        gallery_half: Number "Half width" "m" [4.0, 7.0, 0.5] [] "Floor half width; columns stand 1.5 m in from the walls.";
        gallery_bay: Number "Bay spacing" "m" [4.0, 9.0, 0.5] [] "Distance between column lines and domes.";
        gallery_wall: Number "Wall height" "m" [4.0, 8.0, 0.5] [] "Height where each dome springs.";
        gallery_dome: Number "Dome rise" "m" [0.0, 7.0, 0.5] [] "Rise of the fish-scale domes above the walls.";
        vault_spacing: Number "Vault spacing" "m" [10.0, 18.0, 1.0] [] "Distance between side vaults along the gallery.";
        vault_kind: Choice "Vault rooms" "" [0.0, 4.0, 1.0] ["Mixed", "Treasuries", "Pit traps", "Coin stores", "Collapsed"] "Mixed rolls each vault from the seed.";
    }
    "Coin cavern" {
        hoard_rx: Number "Half width" "m" [20.0, 44.0, 1.0] [] "Cavern half width across the causeway.";
        hoard_ry: Number "Height" "m" [12.0, 32.0, 1.0] [] "Height of the cavern roof above the floor.";
        hoard_rz: Number "Half length" "m" [18.0, 44.0, 1.0] [] "Cavern half length along the causeway.";
        wall_noise: Number "Wall roughness" "" [0.0, 0.4, 0.01] [] "Noise in the cavern outline.";
        strata: Number "Sandstone ledges" "m" [0.0, 3.0, 0.1] [] "Depth of the banded ledges cut into the cavern walls.";
        strata_band: Number "Ledge spacing" "m" [1.5, 6.0, 0.25] [] "Height of one sandstone band.";
        dune_scale: Number "Dune height" "x" [0.0, 2.0, 0.05] [] "Scale of the rolling coin dunes.";
        rim_heap: Number "Rim heap" "m" [0.0, 10.0, 0.5] [] "How high coin piles climb against the walls.";
        heap_cell: Number "Heap spacing" "m" [7.0, 20.0, 0.5] [] "Grid of the tall pointed coin heaps.";
        heap_keep: Number "Heap gaps" "" [0.0, 1.0, 0.01] [] "Share of heap cells left empty.";
        heap_scale: Number "Heap height" "x" [0.0, 2.0, 0.05] [] "Height of the pointed heaps.";
        jewel_crust: Number "Jeweled crust" "" [0.0, 0.6, 0.01] [] "Share of dune crests crusted with jewels.";
        pillar_cell: Number "Pillar spacing" "m" [8.0, 30.0, 1.0] [] "Grid of the sandstone pillars.";
        pillar_keep: Number "Pillar chance" "" [0.0, 1.0, 0.01] [] "Share of grid cells with a pillar.";
        pillar_radius: Number "Pillar radius" "m" [0.8, 4.0, 0.1] [] "Base radius of a pillar.";
        sunstone_cell: Number "Crystal spacing" "m" [6.0, 20.0, 0.5] [] "Grid of the sunstone clusters in the roof.";
        sunstone_keep: Number "Crystal chance" "" [0.0, 1.0, 0.01] [] "Share of roof cells with a cluster.";
        sunstone_r: Number "Crystal radius" "m" [0.5, 3.0, 0.1] [] "Radius of one roof cluster.";
        torch_keep: Number "Dune torches" "" [0.0, 1.0, 0.01] [] "Share of dune cells with a standing torch.";
        cache_keep: Number "Buried caches" "" [0.0, 1.0, 0.01] [] "Share of dune cells with a half-buried chest.";
        pool_layout: Choice "Pools" "" [0.0, 2.0, 1.0] ["None", "Channels", "Basin"] "Reflecting channels beside the causeway, or one basin it crosses.";
        pool_depth_hall: Number "Pool depth" "m" [0.5, 3.0, 0.25] [] "Water over the coins in the cavern pools.";
        basin_r: Number "Basin radius" "m" [4.0, 12.0, 0.5] [] "Radius of the round basin.";
        basin_offset: Number "Basin offset" "m" [-12.0, 12.0, 0.5] [] "Basin centre along the causeway from the cavern centre.";
        door_on: Toggle "Portal" "" [0.0, 1.0, 1.0] [] "A free-standing portal over the causeway.";
        door_offset: Number "Portal position" "m" [-16.0, 16.0, 0.5] [] "Portal along the causeway from the cavern centre.";
        door_half: Number "Portal half width" "m" [2.8, 6.0, 0.1] [] "Half width of the portal opening.";
        door_h: Number "Portal height" "m" [5.0, 14.0, 0.25] [] "Height of the portal opening.";
    }
    "Passage" {
        passage_len: Number "Length" "m" [16.0, 40.0, 1.0] [] "Stepped passage from the cavern down to the grotto shore.";
    }
    "Grotto" {
        grotto_r: Number "Radius" "m" [16.0, 36.0, 0.5] [] "Grotto radius at the waterline.";
        grotto_h: Number "Height" "m" [24.0, 56.0, 0.5] [] "Dome height over the water.";
        lobe_amp: Number "Lobe depth" "" [0.0, 0.25, 0.01] [] "How far the outline bulges into lobes.";
        lobe_k: Number "Lobes" "" [1.0, 5.0, 1.0] [] "Number of lobes round the outline.";
        lobe_phase: Number "Lobe turn" "rad" [0.0, 6.28, 0.05] [] "Turns the lobes round the grotto.";
        flute: Number "Wall flutes" "" [0.0, 0.06, 0.002] [] "Depth of the vertical flowstone flutes.";
        stalactites: Number "Stalactites" "" [0.0, 1.0, 0.01] [] "Share of roof cells with a stalactite.";
        stalactite_len: Number "Stalactite length" "m" [0.0, 12.0, 0.25] [] "Longest stalactite.";
        gours: Number "Rimstone terraces" "" [0.0, 1.0, 0.05] [] "Stepped calcite pools along the shore.";
        pool_depth: Number "Pool depth" "m" [1.0, 8.0, 0.25] [] "Clear water over the boulder bed.";
        pillar_dx: Number "Pillar across" "m" [-8.0, 8.0, 0.25] [] "Pillar axis across from the grotto centre.";
        pillar_dz: Number "Pillar along" "m" [-8.0, 8.0, 0.25] [] "Pillar axis along from the grotto centre.";
        pillar_form: Choice "Pillar" "" [0.0, 1.0, 1.0] ["Boulder stack", "Monolith"] "Stacked boulders, or a banded square monolith.";
        monolith_rot: Number "Monolith turn" "rad" [0.0, 6.28, 0.05] [] "Rotation of the monolith.";
        summit_y: Number "Summit height" "m" [-4.0, 14.0, 0.5] [] "Summit floor height.";
        stair_style: Choice "Stair" "" [0.0, 1.0, 1.0] ["Hugging", "Flying"] "Spiral on the pillar, or a free sweep over the water.";
        stair_turns: Number "Stair turns" "" [0.5, 3.0, 0.05] [] "Turns from the water to the summit.";
        flying_r: Number "Flying radius" "m" [6.0, 12.0, 0.25] [] "Inner radius of a flying stair.";
        stair_turn_offset: Number "Stair start" "rad" [-3.14, 3.14, 0.05] [] "Start of the stair from the side facing the shore.";
        drips: Number "Stair drips" "" [0.0, 1.0, 0.05] [] "Calcite drips under a flying stair.";
        path_bend: Number "Path bend" "m" [-8.0, 8.0, 0.25] [] "Sideways bend of the stepping-stone path.";
        islets: Number "Islets" "" [0.0, 3.0, 1.0] [] "Rock islets with a stalagmite.";
        islet_seed: Number "Islet roll" "" [0.0, 999.0, 1.0] [] "Re-rolls islet places and sizes.";
    }
    "Light" {
        gallery_fill: Color "Gallery fill" "" [0.0, 0.0, 0.0] [] "Sky-free fill colour in the gallery.";
        gallery_fill_i: Number "Gallery fill strength" "" [0.0, 0.4, 0.005] [] "";
        hoard_fill: Color "Cavern fill" "" [0.0, 0.0, 0.0] [] "Sky-free fill colour in the coin cavern.";
        hoard_fill_i: Number "Cavern fill strength" "" [0.0, 0.6, 0.005] [] "";
        hoard_air: Color "Cavern air" "" [0.0, 0.0, 0.0] [] "Haze colour far walls fade into in the cavern.";
        hoard_air_i: Number "Cavern air strength" "" [0.0, 1.0, 0.01] [] "";
        hoard_air_range: Number "Cavern air range" "m" [10.0, 200.0, 1.0] [] "Distance to fade a wall halfway into the air.";
        grotto_fill: Color "Grotto fill" "" [0.0, 0.0, 0.0] [] "Sky-free fill colour in the grotto.";
        grotto_fill_i: Number "Grotto fill strength" "" [0.0, 0.4, 0.005] [] "";
        grotto_air: Color "Grotto air" "" [0.0, 0.0, 0.0] [] "Haze colour in the grotto.";
        grotto_air_i: Number "Grotto air strength" "" [0.0, 0.5, 0.005] [] "";
        grotto_air_range: Number "Grotto air range" "m" [10.0, 250.0, 1.0] [] "";
        grotto_water: Color "Grotto water" "" [0.0, 0.0, 0.0] [] "Glow colour of the grotto pool.";
        grotto_water_i: Number "Grotto water glow" "" [0.0, 3.0, 0.05] [] "";
        hall_water: Color "Cavern water" "" [0.0, 0.0, 0.0] [] "Glow colour of the cavern pools.";
        hall_water_i: Number "Cavern water glow" "" [0.0, 3.0, 0.05] [] "";
        shaft_color: Color "Light shaft" "" [0.0, 0.0, 0.0] [] "Colour of the shaft onto the summit.";
        shaft_i: Number "Light shaft strength" "" [0.0, 4.0, 0.05] [] "";
    }
}

/// One plan dial in 0..1. A full avalanche mix: `hash3` mixes a raw seed once,
/// so nearby seeds (11, 22, 33) gave near-identical caves.
fn dial(seed: u32, i: i32) -> f32 {
    let mut x = seed ^ PLAN ^ (i as u32).wrapping_mul(0x9e37_79b9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 13;
    x = x.wrapping_mul(0xc2b2_ae35);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// The layout a seed gets when nothing overrides it.
pub fn default_layout(seed: u32) -> Layout {
    let f = |i: i32| dial(seed, i);
    let grotto_r = 22.0 + 7.0 * f(0);
    let a = TAU * f(5);
    let d = grotto_r * 0.2 * f(6);
    let flying = f(9) < 0.45;
    Layout {
        gallery_len: 54.0,
        gallery_half: 5.0,
        gallery_bay: 6.0,
        gallery_wall: 5.5,
        gallery_dome: 4.0,
        vault_spacing: 12.0,
        vault_kind: 0.0,
        hoard_rx: 30.0,
        hoard_ry: 22.0,
        hoard_rz: 28.0,
        wall_noise: 0.16,
        strata: 0.6 + 1.2 * f(27),
        strata_band: 2.5 + 1.5 * f(28),
        dune_scale: 1.0,
        rim_heap: 5.5,
        heap_cell: 10.0 + 4.0 * f(20),
        heap_keep: 0.18 + 0.3 * f(21),
        heap_scale: 0.75 + 0.5 * f(22),
        jewel_crust: 0.18,
        pillar_cell: 16.0,
        pillar_keep: 0.65,
        pillar_radius: 1.5,
        sunstone_cell: 11.0,
        sunstone_keep: 0.75,
        sunstone_r: 1.8,
        torch_keep: 0.82,
        cache_keep: 0.30,
        pool_layout: if f(23) < 0.5 { 2.0 } else { 1.0 },
        pool_depth_hall: 1.25,
        basin_r: 8.5,
        basin_offset: 4.0,
        door_on: 1.0,
        door_offset: -8.0 + 20.0 * f(24),
        door_half: 3.3 + 0.9 * f(25),
        door_h: 7.0 + 3.0 * f(26),
        passage_len: 26.0,
        grotto_r,
        grotto_h: 36.0 + 9.0 * f(1),
        lobe_amp: 0.10 * f(2),
        lobe_k: if f(3) < 0.5 { 2.0 } else { 3.0 },
        lobe_phase: TAU * f(4),
        flute: 0.022,
        stalactites: 0.58,
        stalactite_len: 7.0,
        gours: 0.4 + 0.5 * f(29),
        pool_depth: 4.5,
        pillar_dx: a.sin() * d,
        pillar_dz: grotto_r * 0.05 + a.cos() * d * 0.6,
        pillar_form: if f(7) < 0.4 { 1.0 } else { 0.0 },
        monolith_rot: TAU * f(8),
        summit_y: 6.0,
        stair_style: if flying { 1.0 } else { 0.0 },
        stair_turns: if flying { 0.75 + 0.2 * f(10) } else { 1.25 + 0.5 * f(10) },
        flying_r: 7.0 + 1.5 * f(11),
        stair_turn_offset: (f(12) - 0.5) * 0.8,
        drips: 0.45,
        path_bend: (f(14) - 0.5) * 8.0,
        islets: (f(13) * 4.0).floor().min(3.0),
        islet_seed: (f(15) * 999.0).floor(),
        gallery_fill: 0xFFDCB1 as f32,
        gallery_fill_i: 0.045,
        hoard_fill: 0xFFA478 as f32,
        hoard_fill_i: 0.14,
        hoard_air: 0xFF644B as f32,
        hoard_air_i: 0.22,
        hoard_air_range: 60.0,
        grotto_fill: 0x69A4FF as f32,
        grotto_fill_i: 0.07,
        grotto_air: 0x3F8CFF as f32,
        grotto_air_i: 0.13,
        grotto_air_range: 65.0,
        grotto_water: 0x50BCFF as f32,
        grotto_water_i: 1.0,
        hall_water: 0x59D8FF as f32,
        hall_water_i: 0.75,
        shaft_color: 0xE6F7FF as f32,
        shaft_i: 1.0,
    }
}

static OVERRIDE: RwLock<Option<(u32, Layout)>> = RwLock::new(None);
static EPOCH: AtomicU32 = AtomicU32::new(0);

/// Replaces the layout of one seed (the dev tools, or a world's saved
/// layout), or with `None` returns every seed to its default. Generation
/// already running keeps the old layout; the app regenerates afterwards.
pub fn set_layout_override(seed: u32, layout: Option<Layout>) {
    if let Ok(mut slot) = OVERRIDE.write() {
        *slot = layout.map(|l| (seed, l));
    }
    EPOCH.fetch_add(1, Ordering::Release);
}

/// The layout this seed generates with now.
pub fn layout(seed: u32) -> Layout {
    let over = OVERRIDE.read().ok().and_then(|slot| *slot);
    match over {
        Some((s, l)) if s == seed => l,
        _ => default_layout(seed),
    }
}

/// True when the dev tools or a saved sidecar override this seed.
pub fn layout_overridden(seed: u32) -> bool {
    OVERRIDE.read().ok().and_then(|slot| *slot).is_some_and(|(s, _)| s == seed)
}

// --- Plan: the layout plus the places derived from it ----------------------------

#[derive(Clone, Copy, Debug)]
struct Plan {
    seed: u32,
    l: Layout,
    gallery_end: f32,
    hoard_z: f32,
    passage: (f32, f32),
    grotto_z: f32,
    pool_floor: f32,
    summit_y: f32,
    pillar: (f32, f32),
    monolith: bool,
    flying: bool,
    stair_start: f32,
    vault_count: usize,
    islets: [(f32, f32, f32, f32); 3],
    islet_count: usize,
    /// 0 none, 1 channels, 2 basin.
    pools: u32,
    basin_z: f32,
    door_z: f32,
    channel_z: (f32, f32),
}

fn plan(seed: u32) -> Plan {
    thread_local! {
        static LAST: std::cell::Cell<Option<(u32, u32, Plan)>> = const { std::cell::Cell::new(None) };
    }
    let epoch = EPOCH.load(Ordering::Acquire);
    if let Some((s, e, p)) = LAST.with(|c| c.get())
        && s == seed && e == epoch
    {
        return p;
    }
    let p = make_plan(seed, layout(seed));
    LAST.with(|c| c.set(Some((seed, epoch, p))));
    p
}

fn make_plan(seed: u32, l: Layout) -> Plan {
    let gallery_end = GALLERY_Z0 + l.gallery_len;
    let hoard_z = gallery_end + l.hoard_rz + 2.0;
    let passage = (hoard_z + l.hoard_rz - 4.0, hoard_z + l.hoard_rz - 4.0 + l.passage_len);
    let grotto_z = passage.1 + 0.85 * l.grotto_r;
    let pillar = (l.pillar_dx, grotto_z + l.pillar_dz);
    let flying = l.stair_style > 0.5;
    let shore = (0.0, grotto_z - 0.74 * l.grotto_r);
    let stair_start = (shore.1 - pillar.1).atan2(shore.0 - pillar.0) + l.stair_turn_offset;
    let vault_count = (((l.gallery_len - 14.0) / l.vault_spacing).floor() + 1.0).clamp(0.0, 8.0) as usize;
    let mut islets = [(0.0, 0.0, 0.0, 0.0); 3];
    let mut islet_count = 0;
    let roll = l.islet_seed as u32;
    for k in 0..24 {
        if islet_count >= l.islets as usize {
            break;
        }
        let g = |i: i32| dial(seed ^ roll.wrapping_mul(0x2545_f491), 100 + k * 8 + i);
        let ang = TAU * g(0);
        let dist = l.grotto_r * (0.42 + 0.3 * g(1));
        let (x, z) = (ang.sin() * dist, grotto_z + ang.cos() * dist);
        let size = 1.2 + 1.6 * g(2);
        let reach = if flying { l.flying_r + STAIR_W + 3.0 } else { 9.5 };
        if (x - pillar.0).hypot(z - pillar.1) > reach + size
            && segment_distance((x, z), shore, pillar) > 4.0 + size
            && islets[..islet_count].iter().all(|o| (o.0 - x).hypot(o.1 - z) > o.2 + size + 3.0)
        {
            islets[islet_count] = (x, z, size, 2.0 + 4.5 * g(3));
            islet_count += 1;
        }
    }
    Plan {
        seed,
        l,
        gallery_end,
        hoard_z,
        passage,
        grotto_z,
        pool_floor: POOL_Y - l.pool_depth,
        summit_y: l.summit_y.max(POOL_Y + 6.0),
        pillar,
        monolith: l.pillar_form > 0.5,
        flying,
        stair_start,
        vault_count,
        islets,
        islet_count,
        pools: l.pool_layout.round() as u32,
        basin_z: hoard_z + l.basin_offset,
        door_z: hoard_z + l.door_offset,
        channel_z: (hoard_z - l.hoard_rz * 0.65, hoard_z + l.hoard_rz * 0.57),
    }
}

fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, abz) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * abz) / (abx * abx + abz * abz).max(1e-6)).clamp(0.0, 1.0);
    (p.0 - a.0 - abx * t).hypot(p.1 - a.1 - abz * t)
}

/// The plan of this seed, for review receipts.
pub fn plan_summary(seed: u32) -> String {
    let p = plan(seed);
    format!(
        "gallery={:.0}m grotto_r={:.1} h={:.1} lobes={:.2}x{} pillar=({:.1},{:.1}) {} {} turns={:.2} islets={} pools={} door_z={:.1} override={}",
        p.l.gallery_len, p.l.grotto_r, p.l.grotto_h, p.l.lobe_amp, p.l.lobe_k, p.pillar.0, p.pillar.1,
        if p.monolith { "monolith" } else { "boulders" }, if p.flying { "flying" } else { "hugging" },
        p.l.stair_turns, p.islet_count, ["none", "channels", "basin"][p.pools.min(2) as usize], p.door_z,
        layout_overridden(seed),
    )
}

// --- Public geometry for the app -------------------------------------------------

pub fn column(cfg: &WorldSampleConfig, _x: i32, _z: i32) -> ColumnSample {
    ColumnSample {
        deposit_m: 0.0,
        deposit_bed_surface: BlockId::OchreSandstone,
        height: cfg.blocks_for_height(ROOF_TOP_M),
        surface: BlockId::OchreSandstone,
        filler: BlockId::OchreSandstone,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

/// Feet position in voxels: on the arrival landing beside the gate.
pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    Vec3::new(0.5, 0.02, 1.5) / cfg.voxel_meters()
}

/// The generated return gate: a 1 m block in the landing floor at the origin.
pub fn gate_at(m: Vec3) -> bool {
    (0.0..1.0).contains(&m.x) && (0.0..1.0).contains(&m.z) && (-1.0..0.0).contains(&m.y)
}

/// Light mood weights at an eye position in metres: (coin cavern, grotto),
/// each 0..1. The realm's ambient blends warm gold into cold teal by these.
pub fn mood(seed: u32, m: Vec3) -> (f32, f32) {
    let p = plan(seed);
    let grotto = smooth(p.passage.0 + 14.0, p.passage.1, m.z);
    let hoard = smooth(p.gallery_end, p.gallery_end + 10.0, m.z) * (1.0 - grotto);
    (hoard, grotto)
}

/// A still pool that glows, for the realm's light rig: centre and half size in
/// metres on x and z, the water surface height, whether it is round, and the
/// palette the renderer gives it (0 grotto, 1 cavern pools).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlowPool {
    pub centre: [f32; 2],
    pub half: [f32; 2],
    pub surface_y: f32,
    pub round: bool,
    pub palette: u32,
}

/// The grotto pool, then the coin cavern's pools. At most three.
pub fn glow_pools(seed: u32) -> Vec<GlowPool> {
    let p = plan(seed);
    let mut pools = vec![GlowPool { centre: [0.0, p.grotto_z], half: [p.l.grotto_r * 0.9; 2], surface_y: POOL_Y, round: true, palette: 0 }];
    match p.pools {
        2 => pools.push(GlowPool { centre: [0.0, p.basin_z], half: [p.l.basin_r; 2], surface_y: HALL_Y, round: true, palette: 1 }),
        1 => {
            let cx = (CHANNEL_X.0 + CHANNEL_X.1) * 0.5;
            let cz = (p.channel_z.0 + p.channel_z.1) * 0.5;
            let half = [(CHANNEL_X.1 - CHANNEL_X.0) * 0.5, (p.channel_z.1 - p.channel_z.0) * 0.5];
            for x in [-cx, cx] {
                pools.push(GlowPool { centre: [x, cz], half, surface_y: HALL_Y, round: false, palette: 1 });
            }
        }
        _ => {}
    }
    pools
}

/// The shaft of light from the dome boss down onto the summit, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightShaft {
    pub centre: [f32; 2],
    pub radius: f32,
    pub bottom_y: f32,
    pub top_y: f32,
}

pub fn light_shaft(seed: u32) -> LightShaft {
    let p = plan(seed);
    let (hr, _) = grotto_hr(&p, p.pillar.0, p.pillar.1);
    LightShaft { centre: [p.pillar.0 + 0.5, p.pillar.1 + 0.5], radius: SHAFT_R, bottom_y: p.summit_y, top_y: dome_y(&p, hr) }
}

/// Review camera (eye, target) in realm metres for `VOXEL_REVIEW_DUNGEON=gilded:<name>`.
/// Every view follows this seed's layout.
pub fn review_view(seed: u32, name: &str) -> Option<(Vec3, Vec3)> {
    let p = plan(seed);
    let (px, pz) = p.pillar;
    let (r, gz, hz, ge) = (p.l.grotto_r, p.grotto_z, p.hoard_z, p.gallery_end);
    let sy = p.summit_y;
    Some(match name {
        "arrival" => (Vec3::new(0.5, 1.7, -3.5), Vec3::new(0.0, -7.0, 14.0)),
        "gallery" => (Vec3::new(0.0, -10.2, 18.5), Vec3::new(0.0, -8.0, 18.5 + p.l.gallery_len * 0.5)),
        "vault" => (Vec3::new(-3.0, -10.2, 25.5), Vec3::new(13.0, -10.5, 25.5)),
        "wall" => (Vec3::new(1.5, -10.0, 30.0), Vec3::new(5.0, -9.6, 31.5)),
        "hoard" => (Vec3::new(0.0, -7.5, ge + 4.0), Vec3::new(0.0, -10.5, hz + 12.0)),
        "dunes" => (Vec3::new(-14.0, -6.0, hz - 10.0), Vec3::new(8.0, -11.0, hz + 6.0)),
        "vista" => (Vec3::new(-0.45 * p.l.hoard_rx, HALL_Y + 0.62 * p.l.hoard_ry, hz - 0.55 * p.l.hoard_rz), Vec3::new(0.0, HALL_Y + 2.0, hz + 6.0)),
        "door" => (Vec3::new(1.5, -9.8, p.door_z - 13.0), Vec3::new(0.0, -6.0, p.door_z)),
        "hoardpool" if p.pools == 2 => (Vec3::new(-1.0, -5.0, p.basin_z - 9.0), Vec3::new(-5.0, -12.5, p.basin_z + 1.0)),
        "hoardpool" => (Vec3::new(-4.5, -10.2, p.channel_z.0 + 3.0), Vec3::new(-6.0, -12.5, p.channel_z.0 + 17.0)),
        "grotto" => (Vec3::new(0.0, -17.3, p.passage.1 - 1.0), Vec3::new(px, 2.0, pz)),
        "lake" => (Vec3::new(-0.5 * r, -18.4, gz - 0.55 * r), Vec3::new(px, -13.0, pz)),
        "pool" => (Vec3::new(px + 9.0, -13.0, pz - 15.0), Vec3::new(px, -20.0, pz - 6.0)),
        "shore" => (Vec3::new(0.0, -18.0, gz - 0.74 * r - 1.0), Vec3::new(-0.4 * r, -19.5, gz - 0.4 * r)),
        "underwater" => (Vec3::new(px - 10.0, -22.2, pz - 10.0), Vec3::new(px, -21.0, pz)),
        "stair" => (Vec3::new(12.0, -14.0, gz - 14.0), Vec3::new(px, -4.0, pz)),
        "summit" => (Vec3::new(px + 7.0, sy + 3.0, pz - 7.0), Vec3::new(px, sy + 0.5, pz)),
        "shaft" => (Vec3::new(px - 11.0, sy - 8.0, pz - 12.0), Vec3::new(px + 0.5, sy + 2.0, pz + 0.5)),
        _ => return None,
    })
}

/// Every review view name, for the dev tools.
pub const REVIEW_VIEWS: &[&str] = &["arrival", "gallery", "vault", "wall", "hoard", "dunes", "vista", "door", "hoardpool",
    "grotto", "lake", "pool", "shore", "underwater", "stair", "summit", "shaft"];

// --- Helpers ---------------------------------------------------------------------

/// The voxel holding a point given in metres.
fn cell(at: Vec3, vm: f32) -> IVec3 {
    (at / vm).floor().as_ivec3()
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Stair tread height: one rise per run, `s` metres per step.
fn descent(z: f32, z0: f32, top: f32, bottom: f32, s: f32) -> f32 {
    if z < z0 {
        return top;
    }
    (top - ((z - z0) / s).floor() * s - s).max(bottom)
}

/// Wall torch on the wall at `-x` (support toward -x) or `+x`.
fn wall_torch(toward_neg_x: bool) -> BlockId {
    if toward_neg_x { BlockId::TorchWallEast } else { BlockId::TorchWallWest }
}

// --- Arrival landing and stair -------------------------------------------------

fn stair_floor(z: f32, s: f32) -> f32 {
    descent(z, STAIR_Z.0, 0.0, HALL_Y, s)
}

fn arrival_inside(p: IVec3, m: Vec3, vm: f32, s: f32) -> Option<BlockId> {
    let ax = m.x.abs();
    if ax < 4.5 && (-4.5..STAIR_Z.0).contains(&m.z) && m.y >= 0.0
        && m.y < 6.0 + 1.5 * (1.0 - (m.x / 4.5).powi(2))
    {
        for at in [Vec3::new(-3.5, 0.5, -3.5), Vec3::new(3.5, 0.5, -3.5), Vec3::new(-3.5, 0.5, 2.5), Vec3::new(3.5, 0.5, 2.5)] {
            if p == cell(at, vm) {
                return Some(BlockId::Torch);
            }
        }
        return Some(BlockId::Air);
    }
    if ax < 3.5 && (STAIR_Z.0..STAIR_Z.1).contains(&m.z) {
        let f = stair_floor(m.z, s);
        if m.y >= f && m.y < f + 6.5 {
            for z in [6.5f32, 10.5, 14.5] {
                let y = stair_floor(z, s) + 2.5;
                for side in [-1.0f32, 1.0] {
                    if p == cell(Vec3::new(side * (3.5 - vm * 0.5), y, z), vm) {
                        return Some(wall_torch(side < 0.0));
                    }
                }
            }
            return Some(BlockId::Air);
        }
    }
    None
}

fn arrival_shell(m: Vec3, s: f32) -> Option<BlockId> {
    let ax = m.x.abs();
    if ax > 5.5 || m.z < -5.5 || m.z > STAIR_Z.1 + 0.5 || m.y < HALL_Y - 1.5 || m.y > 9.0 {
        return None;
    }
    let floor = if m.z < STAIR_Z.0 { 0.0 } else { stair_floor(m.z, s) };
    if m.y < floor && m.y >= floor - 1.0 {
        return Some(if m.z < STAIR_Z.0 { BlockId::StarTile } else { BlockId::CarvedSandstone });
    }
    let hf = m.y - floor;
    if m.z < STAIR_Z.0 {
        // Landing walls: carved ashlar with a gilded band at eye height.
        return Some(if (2.5..3.5).contains(&hf) { BlockId::GildedRelief } else { BlockId::CarvedSandstone });
    }
    None
}

// --- Palace gallery and side vaults ------------------------------------------

fn gallery_ceiling(pl: &Plan, m: Vec3) -> f32 {
    let (bay, half) = (pl.l.gallery_bay, pl.l.gallery_half);
    let dz = (m.z - GALLERY_Z0).rem_euclid(bay) - bay * 0.5;
    let dome = (1.0 - (dz / (bay * 0.5)).powi(2)).max(0.0) * (1.0 - (m.x / half).powi(2)).max(0.0);
    HALL_Y + pl.l.gallery_wall + pl.l.gallery_dome * dome
}

fn vault_centres(pl: &Plan) -> impl Iterator<Item = f32> + '_ {
    (0..pl.vault_count).map(move |i| GALLERY_Z0 + 9.0 + pl.l.vault_spacing * i as f32)
}

/// Room kind of one vault: 0 treasury, 1 pit trap, 2 coin store, 3 collapse.
fn vault_kind(pl: &Plan, zc: f32, side: f32) -> u32 {
    let forced = pl.l.vault_kind.round() as u32;
    if forced > 0 {
        return forced - 1;
    }
    let h = hash3(zc as i32, side as i32, 0, pl.seed ^ VAULT);
    ((h * 4.0) as u32).min(3)
}

/// Height of a vault's coin heap, highest against the back wall.
fn vault_heap(seed: u32, inner: f32, sx: f32, dz: f32) -> f32 {
    let back = smooth(inner, inner + 8.0, sx);
    2.6 * back * (1.0 - (dz / 4.0).powi(2)) + 0.6 * value_noise2(sx / 2.0, dz / 2.0, seed ^ DUNE)
}

fn gallery_inside(pl: &Plan, p: IVec3, m: Vec3, vm: f32) -> Option<BlockId> {
    let seed = pl.seed;
    let (half, bay_len) = (pl.l.gallery_half, pl.l.gallery_bay);
    let inner = half + 1.0;
    let ax = m.x.abs();
    let hf = m.y - HALL_Y;
    if (GALLERY_Z0..pl.gallery_end).contains(&m.z) && ax < half && hf >= 0.0 && m.y < gallery_ceiling(pl, m) {
        // Columns at each bay line, a step in from the walls.
        let bay = ((m.z - GALLERY_Z0) / bay_len).floor();
        let zb = GALLERY_Z0 + bay * bay_len;
        if bay > 0.0 && (ax - (half - 1.5)).abs() < 0.5 && (m.z - (zb + 0.5)).abs() < 0.5 {
            return Some(if (3.5..4.5).contains(&hf) { BlockId::GildedRelief } else { BlockId::CarvedSandstone });
        }
        for side in [-1.0f32, 1.0] {
            if bay > 0.0 && p == cell(Vec3::new(side * (half - vm * 0.5), HALL_Y + 2.5, zb + 0.5), vm) {
                return Some(wall_torch(side < 0.0));
            }
        }
        return Some(BlockId::Air);
    }
    for zc in vault_centres(pl) {
        let dz = m.z - zc;
        if dz.abs() >= 5.0 || ax >= inner + 8.0 {
            continue;
        }
        let side = m.x.signum();
        let sx = ax;
        // Doorway through the gallery wall.
        if (half..inner).contains(&sx) && dz.abs() < 1.0 && (0.0..3.0).contains(&hf) {
            return Some(BlockId::Air);
        }
        if sx < inner || dz.abs() >= 4.0 {
            continue;
        }
        let kind = vault_kind(pl, zc, side);
        let back = Vec3::new(side * (inner + 7.5), HALL_Y + 0.5, zc + 0.5);
        if (0.0..5.0).contains(&hf) {
            if p == cell(Vec3::new(side * (inner + 8.0 - vm * 0.5), HALL_Y + 2.5, zc - 2.5), vm) {
                return Some(wall_torch(side < 0.0));
            }
            match kind {
                0 => {
                    for at in [back, back + Vec3::new(0.0, 0.0, 2.0), back - Vec3::new(0.0, 0.0, 2.0)] {
                        if p == cell(at, vm) {
                            return Some(if at == back { BlockId::SurveyCache } else { BlockId::SalvageCrate });
                        }
                    }
                }
                2 => {
                    let top = HALL_Y + vault_heap(seed, inner, sx, dz);
                    if m.y < top {
                        return Some(BlockId::GoldCoins);
                    }
                    let crest = HALL_Y + vault_heap(seed, inner, inner + 6.5, 0.0);
                    if p == cell(Vec3::new(side * (inner + 6.5), crest + vm * 0.5, zc + 0.5), vm) {
                        return Some(BlockId::SurveyCache);
                    }
                }
                3 => {
                    let rubble = 1.6 * value_noise2(m.x / 1.7, m.z / 1.7, seed ^ VAULT) * smooth(inner + 1.0, inner + 6.0, sx);
                    if hf < rubble {
                        return Some(BlockId::OchreSandstone);
                    }
                    if p == cell(Vec3::new(side * (inner + 1.5), HALL_Y + 0.5, zc + 2.5), vm) {
                        return Some(BlockId::SalvageCrate);
                    }
                }
                _ => {}
            }
            return Some(BlockId::Air);
        }
        // Pit trap: a hidden drop under a mirage floor, with a cache below.
        if kind == 1 && (inner + 2.0..inner + 6.0).contains(&sx) && dz.abs() < 2.0 && (HALL_Y - 4.0..HALL_Y - 1.0).contains(&m.y) {
            if p == cell(Vec3::new(side * (inner + 4.5), HALL_Y - 3.5, zc + 0.5), vm) {
                return Some(BlockId::SurveyCache);
            }
            return Some(BlockId::Air);
        }
    }
    None
}

fn gallery_shell(pl: &Plan, m: Vec3) -> Option<BlockId> {
    let half = pl.l.gallery_half;
    let inner = half + 1.0;
    let ax = m.x.abs();
    let hf = m.y - HALL_Y;
    let top = pl.l.gallery_wall + pl.l.gallery_dome + 1.5;
    if m.z < GALLERY_Z0 - 1.0 || m.z > pl.gallery_end + 1.0 || ax > inner + 9.0 || hf < -6.0 || hf > top {
        return None;
    }
    for zc in vault_centres(pl) {
        let dz = m.z - zc;
        if dz.abs() > 5.0 || ax < half {
            continue;
        }
        // Jeweled frame around each doorway.
        if (half..inner).contains(&ax) && dz.abs() < 1.8 && hf < 3.8 {
            return Some(BlockId::JeweledGold);
        }
        if ax >= inner {
            let kind = vault_kind(pl, zc, m.x.signum());
            if kind == 1 && (inner + 2.0..inner + 6.0).contains(&ax) && dz.abs() < 2.0 {
                if (-1.0..0.0).contains(&hf) {
                    return Some(BlockId::MirageBrick);
                }
                if (-5.0..-4.0).contains(&hf) {
                    return Some(BlockId::GoldCoins);
                }
            }
            if (-1.0..0.0).contains(&hf) {
                return Some(if kind == 0 { BlockId::StarTile } else { BlockId::CarvedSandstone });
            }
            if hf >= 5.0 {
                return Some(BlockId::GildedRelief);
            }
            if hf >= 0.0 {
                return Some(if kind == 0 { BlockId::InlaidMarble } else { BlockId::LapisBrick });
            }
            return None;
        }
    }
    if ax < half && (-1.0..0.0).contains(&hf) {
        return Some(if ax > half - 1.5 { BlockId::LozengeTile } else { BlockId::ZigzagTile });
    }
    if ax < half + 1.5 && hf >= 0.0 {
        if m.y >= gallery_ceiling(pl, Vec3::new(m.x.clamp(-half + 0.01, half - 0.01), m.y, m.z)) && ax < half {
            return Some(BlockId::ScaleTile);
        }
        return Some(if hf < 3.0 {
            BlockId::LapisBrick
        } else if hf < 4.0 {
            BlockId::GildedRelief
        } else if hf < pl.l.gallery_wall + 4.0 {
            BlockId::CarvedSandstone
        } else {
            BlockId::ScaleTile
        });
    }
    None
}

// --- Coin cavern -----------------------------------------------------------------

fn hoard_open(pl: &Plan, m: Vec3) -> bool {
    let l = &pl.l;
    let up = if m.y >= HALL_Y { (m.y - HALL_Y) / l.hoard_ry } else { (m.y - HALL_Y) / 4.0 };
    let flat = Vec3::new(m.x / l.hoard_rx, 0.0, (m.z - pl.hoard_z) / l.hoard_rz).length();
    // Banded sandstone: harder beds stand proud as ledges along the walls, the
    // softer ones between are scooped back, like a wind-cut slot canyon.
    let band = (m.y - HALL_Y) / l.strata_band.max(0.5) + 0.35 * value_noise2(m.x / 9.0, m.z / 9.0, pl.seed ^ ROCK.rotate_left(17));
    let bed = (band.fract() * 2.0 - 1.0).abs();
    let ledge = l.strata * smooth(0.55, 0.95, bed) / l.hoard_rx.min(l.hoard_rz) * smooth(1.0, 3.0, m.y - HALL_Y);
    let d = Vec3::new(m.x / l.hoard_rx, up, (m.z - pl.hoard_z) / l.hoard_rz).length() + ledge * smooth(0.5, 0.9, flat);
    let n = value_noise3(m.x / 7.0, m.y / 7.0, m.z / 7.0, pl.seed ^ ROCK);
    d < 0.86 + l.wall_noise * n
}

fn on_causeway(pl: &Plan, m: Vec3) -> bool {
    m.x.abs() < 3.0 && (pl.gallery_end..pl.hoard_z + pl.l.hoard_rz).contains(&m.z)
}

/// Top of the coin dunes; piles climb toward the cavern walls.
fn dune_top(pl: &Plan, x: f32, z: f32) -> f32 {
    let (l, seed) = (&pl.l, pl.seed);
    let edge = (x / l.hoard_rx).hypot((z - pl.hoard_z) / l.hoard_rz);
    let dune = (0.8
        + 2.6 * value_noise2(x / 8.0, z / 8.0, seed ^ DUNE)
        + 1.0 * value_noise2(x / 3.1, z / 3.1, seed ^ DUNE.rotate_left(9))) * l.dune_scale
        + l.rim_heap * smooth(0.45, 0.95, edge);
    // The causeway runs in a trench between the dunes.
    let trench = smooth(3.0, 7.0, x.abs());
    HALL_Y - 1.5 + (dune * (0.25 + 0.75 * trench)).max(coin_heap(pl, x, z))
}

/// Tall pointed coin heaps on a jittered grid, piled well above the dunes,
/// each with a slumped skirt and a little spill ripple down one flank. The
/// causeway and its pools stay clear.
fn coin_heap(pl: &Plan, x: f32, z: f32) -> f32 {
    let (l, seed) = (&pl.l, pl.seed);
    let cell = l.heap_cell;
    let (gx, gz) = ((x / cell).floor() as i32, ((z - pl.hoard_z) / cell).floor() as i32);
    let mut top = 0.0f32;
    for cz in gz - 1..=gz + 1 {
        for cx in gx - 1..=gx + 1 {
            if hash3(cx, 23, cz, seed ^ DUNE) < l.heap_keep {
                continue;
            }
            let hx = (cx as f32 + 0.15 + 0.7 * hash3(cx, 24, cz, seed ^ DUNE)) * cell;
            let hz = pl.hoard_z + (cz as f32 + 0.15 + 0.7 * hash3(cx, 25, cz, seed ^ DUNE)) * cell;
            let size = hash3(cx, 26, cz, seed ^ DUNE);
            let radius = 3.5 + 3.0 * size;
            if hx.abs() < 11.0 || (pl.pools == 2 && hx.hypot(hz - pl.basin_z) < l.basin_r + radius) {
                continue;
            }
            let (dx, dz) = (x - hx, z - hz);
            let d = dx.hypot(dz);
            if d < radius * 1.5 {
                let t = (d / radius).min(1.5);
                let cone = (1.0 - t.min(1.0)).powf(1.25);
                let skirt = 0.18 * (1.0 - (t / 1.5)).max(0.0).powi(2);
                let ang = dz.atan2(dx) + TAU * hash3(cx, 27, cz, seed ^ DUNE);
                let spill = 0.08 * (ang.cos().max(0.0)) * (d * 2.2).sin().abs() * (1.0 - t / 1.5).max(0.0);
                top = top.max((4.5 + 6.5 * size) * l.heap_scale * (cone + skirt + spill));
            }
        }
    }
    top
}

/// The cavern's pool, coins under clear water inside a carved coping: twin
/// channels beside the causeway, or one round basin the causeway crosses on
/// a dyke. None outside the pool.
fn hoard_pool(pl: &Plan, m: Vec3) -> Option<BlockId> {
    const COPING: f32 = 0.6;
    let ax = m.x.abs();
    let (inner, rim) = match pl.pools {
        2 => {
            if ax < 3.0 {
                return None;
            }
            let d = m.x.hypot(m.z - pl.basin_z);
            (d < pl.l.basin_r, d < pl.l.basin_r + COPING)
        }
        1 => {
            let (z0, z1) = pl.channel_z;
            let inner = (CHANNEL_X.0..CHANNEL_X.1).contains(&ax) && (z0..z1).contains(&m.z);
            let rim = (CHANNEL_X.0..CHANNEL_X.1 + COPING).contains(&ax) && (z0 - COPING..z1 + COPING).contains(&m.z);
            (inner, rim)
        }
        _ => return None,
    };
    if !rim {
        return None;
    }
    let hf = m.y - HALL_Y;
    let depth = pl.l.pool_depth_hall;
    if !inner {
        return (hf < 0.5 && hf >= -depth - 1.0).then_some(BlockId::CarvedSandstone);
    }
    if hf < -depth {
        let gem = value_noise2(m.x / 1.3, m.z / 1.3, pl.seed ^ CACHE.rotate_left(5)) > 0.78;
        return Some(if gem { BlockId::JeweledGold } else { BlockId::GoldCoins });
    }
    if hf < 0.0 {
        return Some(BlockId::Water);
    }
    Some(BlockId::Air)
}

/// A free-standing portal the causeway passes through, rising from the coins
/// and the water: stepped jambs with a lapis band, a recessed marble reveal, a
/// gilded frieze and a cornice that steps out.
fn hoard_portal(pl: &Plan, m: Vec3) -> Option<BlockId> {
    let l = &pl.l;
    if l.door_on < 0.5 {
        return None;
    }
    let (z0, z1) = (pl.door_z, pl.door_z + 1.6);
    let ax = m.x.abs();
    let hf = m.y - HALL_Y;
    let outer = l.door_half + 1.3;
    if hf < -2.5 || hf >= l.door_h + 2.4 || ax >= outer + 0.9 || m.z < z0 - 0.6 || m.z >= z1 + 0.6 {
        return None;
    }
    let depth = (z0..z1).contains(&m.z);
    if hf >= l.door_h + 1.6 {
        return Some(BlockId::CarvedSandstone);
    }
    if hf >= l.door_h {
        return (depth && ax < outer + 0.3).then_some(BlockId::GildedRelief);
    }
    if depth && (l.door_half..outer).contains(&ax) {
        let band = (hf - l.door_h * 0.62).abs() < 0.5;
        return Some(if band { BlockId::LapisBrick } else { BlockId::CarvedSandstone });
    }
    // The recessed inner frame: a reveal half a metre in, set back from both faces.
    let reveal = (z0 + 0.4..z1 - 0.4).contains(&m.z) && hf >= 0.0
        && (((l.door_half - 0.5..l.door_half).contains(&ax) && hf < l.door_h)
            || (ax < l.door_half && hf >= l.door_h - 0.5));
    if reveal {
        return Some(BlockId::InlaidMarble);
    }
    // A plinth a step proud at the foot of each jamb.
    if hf < 0.8 && (l.door_half - 0.2..outer + 0.3).contains(&ax) && (z0 - 0.3..z1 + 0.3).contains(&m.z) {
        return Some(BlockId::CarvedSandstone);
    }
    None
}

/// Sandstone pillars on a jittered grid, clear of the causeway. Each swells
/// and pinches with the sandstone bands, so it reads as wind-cut rock.
fn hoard_pillar(pl: &Plan, m: Vec3) -> bool {
    let (l, seed) = (&pl.l, pl.seed);
    let cell = l.pillar_cell;
    let c = (Vec3::new(m.x, 0.0, m.z - pl.hoard_z) / cell).floor();
    let (cx, cz) = (c.x as i32, c.z as i32);
    if hash3(cx, 0, cz, seed ^ PILLAR) > l.pillar_keep {
        return false;
    }
    let px = (c.x + 0.25 + 0.5 * hash3(cx, 1, cz, seed ^ PILLAR)) * cell;
    let pz = pl.hoard_z + (c.z + 0.25 + 0.5 * hash3(cx, 2, cz, seed ^ PILLAR)) * cell;
    if px.abs() < 7.0 {
        return false;
    }
    let band = ((m.y - HALL_Y) / l.strata_band.max(0.5)).fract();
    let r = l.pillar_radius * (1.0 + 0.6 * hash3(cx, 3, cz, seed ^ PILLAR)) + 0.3 * (m.y * 0.4).sin()
        - 0.25 * l.strata.min(1.5) * smooth(0.3, 0.7, band) * smooth(1.0, 0.7, band);
    (m.x - px).hypot(m.z - pz) < r
}

fn hoard_inside(pl: &Plan, p: IVec3, m: Vec3, vm: f32) -> Option<BlockId> {
    let (l, seed) = (&pl.l, pl.seed);
    let ax = m.x.abs();
    let hf = m.y - HALL_Y;
    // Grand arch from the gallery into the cavern.
    let arch = (pl.gallery_end..pl.gallery_end + 8.0).contains(&m.z) && ax < 3.5 && hf >= 0.0
        && hf < 5.5 + 1.5 * (1.0 - (m.x / 3.5).powi(2));
    if !arch && !hoard_open(pl, m) {
        return None;
    }
    if let Some(b) = hoard_portal(pl, m) {
        return Some(b);
    }
    if hoard_pillar(pl, m) {
        return Some(BlockId::OchreSandstone);
    }
    if let Some(b) = hoard_pool(pl, m) {
        return Some(b);
    }
    if on_causeway(pl, m) {
        if hf < 0.0 {
            return Some(if ax < 2.0 { BlockId::ZigzagTile } else { BlockId::CarvedSandstone });
        }
        if ax >= 2.0 && hf < 1.0 {
            return Some(BlockId::CarvedSandstone);
        }
        // Torches stand on the kerbs every 6 m.
        let kz = ((m.z - pl.gallery_end) / 6.0).floor() * 6.0 + pl.gallery_end + 3.5;
        for side in [-1.0f32, 1.0] {
            if p == cell(Vec3::new(side * 2.5, HALL_Y + 1.0 + vm * 0.5, kz), vm) {
                return Some(BlockId::Torch);
            }
        }
        if ax < 2.0 || hf >= 1.0 {
            let top = dune_top(pl, m.x, m.z);
            if ax >= 3.0 && m.y < top {
                return Some(BlockId::GoldCoins);
            }
            return Some(BlockId::Air);
        }
    }
    let top = dune_top(pl, m.x, m.z);
    if m.y < top {
        // Jeweled crusts on some crests; the rest is loose coin.
        if m.y > top - 1.0 && value_noise2(m.x / 4.0, m.z / 4.0, seed ^ DUNE.rotate_left(3)) > 1.0 - l.jewel_crust {
            return Some(BlockId::JeweledGold);
        }
        // Half-buried caches on a 7 m grid.
        let g = (Vec3::new(m.x, 0.0, m.z) / 7.0).floor();
        let (gx, gz) = (g.x as i32, g.z as i32);
        let roll = hash3(gx, 5, gz, seed ^ CACHE);
        if roll > 1.0 - l.cache_keep {
            let cx = g.x * 7.0 + 3.5;
            let cz = g.z * 7.0 + 3.5;
            if cx.abs() > 4.0 && p == cell(Vec3::new(cx, dune_top(pl, cx, cz) - vm * 0.5, cz), vm) {
                return Some(if roll > 1.0 - l.cache_keep * 0.27 { BlockId::SurveyCache } else { BlockId::SalvageCrate });
            }
        }
        return Some(BlockId::GoldCoins);
    }
    // Stacks of gold blocks and standing torches on the dunes.
    let g = (Vec3::new(m.x, 0.0, m.z) / 5.0).floor();
    let (gx, gz) = (g.x as i32, g.z as i32);
    if hash3(gx, 7, gz, seed ^ CACHE) > 0.86 {
        let sx = g.x * 5.0 + 2.5;
        let sz = g.z * 5.0 + 2.5;
        let base = dune_top(pl, sx, sz);
        if (m.x - sx).abs() < 0.5 && (m.z - sz).abs() < 0.5 && m.y >= base - vm && m.y < base + 1.5 {
            return Some(BlockId::GoldBlock);
        }
    }
    let t = (Vec3::new(m.x, 0.0, m.z) / 9.0).floor();
    let (tx, tz) = (t.x as i32, t.z as i32);
    if hash3(tx, 9, tz, seed ^ LIGHT) > 1.0 - l.torch_keep {
        let lx = t.x * 9.0 + 2.5 + 4.0 * hash3(tx, 10, tz, seed ^ LIGHT);
        let lz = t.z * 9.0 + 2.5 + 4.0 * hash3(tx, 11, tz, seed ^ LIGHT);
        if lx.abs() > 4.0 && p == cell(Vec3::new(lx, dune_top(pl, lx, lz) + vm * 0.5, lz), vm) {
            return Some(BlockId::Torch);
        }
    }
    Some(BlockId::Air)
}

/// Sunstone clusters hang from the cavern roof on a grid.
fn hoard_shell(pl: &Plan, m: Vec3, vm: f32) -> Option<BlockId> {
    let l = &pl.l;
    if m.y < HALL_Y + 8.0 || !hoard_open(pl, m - Vec3::Y * vm) || hoard_open(pl, m) {
        return None;
    }
    let cell = l.sunstone_cell;
    let g = (Vec3::new(m.x, 0.0, m.z) / cell).floor();
    let (gx, gz) = (g.x as i32, g.z as i32);
    if hash3(gx, 13, gz, pl.seed ^ LIGHT) > l.sunstone_keep {
        return None;
    }
    let cx = (g.x + 0.5) * cell;
    let cz = (g.z + 0.5) * cell;
    ((m.x - cx).hypot(m.z - cz) < l.sunstone_r).then_some(BlockId::Sunstone)
}

// --- Passage -------------------------------------------------------------------

fn passage_floor(pl: &Plan, z: f32, s: f32) -> f32 {
    descent(z, pl.passage.0 + 6.0, HALL_Y, POOL_Y + 1.0, s)
}

fn passage_inside(pl: &Plan, p: IVec3, m: Vec3, vm: f32, s: f32) -> Option<BlockId> {
    if !(pl.passage.0..pl.passage.1).contains(&m.z) || m.x.abs() >= 1.8 {
        return None;
    }
    let f = passage_floor(pl, m.z, s);
    if m.y < f || m.y >= f + 4.0 + 0.6 * (1.0 - (m.x / 1.8).powi(2)) {
        return None;
    }
    let tz = pl.passage.0 + 9.5;
    for side in [-1.0f32, 1.0] {
        if p == cell(Vec3::new(side * (1.8 - vm * 0.5), passage_floor(pl, tz, s) + 2.5, tz), vm) {
            return Some(wall_torch(side < 0.0));
        }
    }
    Some(BlockId::Air)
}

fn passage_shell(pl: &Plan, m: Vec3, s: f32) -> Option<BlockId> {
    if !(pl.passage.0..pl.passage.1).contains(&m.z) || m.x.abs() >= 1.8 {
        return None;
    }
    let f = passage_floor(pl, m.z, s);
    (m.y < f && m.y >= f - 1.0).then_some(BlockId::CarvedSandstone)
}

// --- Grotto ----------------------------------------------------------------------

/// Horizontal radius ratio of a point in the grotto, with the plan's lobes,
/// and its angle about the grotto centre. The lobes fade out toward the
/// passage mouth (-z) so the entrance always opens onto the shore.
fn grotto_hr(p: &Plan, x: f32, z: f32) -> (f32, f32) {
    let dz = z - p.grotto_z;
    let ang = x.atan2(dz);
    let mouth = (1.0 + ang.cos()) * 0.5;
    let lobe = 1.0 + p.l.lobe_amp * (p.l.lobe_k.round() * ang + p.l.lobe_phase).cos() * mouth;
    (x.hypot(dz) / (p.l.grotto_r * lobe), ang)
}

/// Ceiling of the grotto dome at a horizontal radius ratio, without wall noise.
fn dome_y(p: &Plan, hr: f32) -> f32 {
    POOL_Y + p.l.grotto_h * (0.88 - hr * hr).max(0.0).sqrt()
}

/// Bed of the pool: a rippled rock floor with scattered boulders, deep enough
/// that the glow shows the stones through clear water.
fn pool_bed(p: &Plan, x: f32, z: f32) -> f32 {
    let swell = value_noise2(x / 6.0, z / 6.0, p.seed ^ ROCK.rotate_left(3));
    let boulder = smooth(0.62, 0.86, value_noise2(x / 2.3, z / 2.3, p.seed ^ ROCK.rotate_left(7)));
    p.pool_floor + 1.1 * swell + 1.3 * boulder
}

/// Distance from the pillar axis inside which nothing may hang: the shaft,
/// the summit and the whole stair.
fn stair_reach(p: &Plan) -> f32 {
    if p.flying { p.l.flying_r + STAIR_W + 1.5 } else { 7.5 }
}

/// Stalactites on a jittered grid hang from the dome, clear of the stair.
fn stalactite(p: &Plan, m: Vec3) -> bool {
    const CELL: f32 = 2.6;
    let seed = p.seed;
    let (dx, dz) = (m.x, m.z - p.grotto_z);
    let (gx, gz) = ((dx / CELL).floor(), (dz / CELL).floor());
    let (ix, iz) = (gx as i32, gz as i32);
    if hash3(ix, 17, iz, seed ^ ROCK) > p.l.stalactites {
        return false;
    }
    let cx = (gx + 0.2 + 0.6 * hash3(ix, 18, iz, seed ^ ROCK)) * CELL;
    let cz = (gz + 0.2 + 0.6 * hash3(ix, 19, iz, seed ^ ROCK)) * CELL + p.grotto_z;
    let (hr, _) = grotto_hr(p, cx, cz);
    if hr > 0.93 || (cx - p.pillar.0).hypot(cz - p.pillar.1) < stair_reach(p) {
        return false;
    }
    let root = dome_y(p, hr) + 1.0;
    let size = hash3(ix, 20, iz, seed ^ ROCK);
    let len = 1.5 + p.l.stalactite_len * size * size;
    let tip = root - len;
    if !(tip..root).contains(&m.y) {
        return false;
    }
    // Ringed like real dripstone: a slight bead every half metre.
    let ring = 1.0 + 0.12 * ((m.y - tip) * TAU * 2.0).sin();
    let girth = (0.3 + 0.8 * size) * (m.y - tip) / len * ring;
    (m.x - cx).hypot(m.z - cz) < girth.max(0.1)
}

fn grotto_open(p: &Plan, m: Vec3) -> bool {
    let seed = p.seed;
    let (hr, ang) = grotto_hr(p, m.x, m.z);
    if m.y >= POOL_Y {
        // Flowstone flutes: the wall noise is drawn out vertically and the
        // radius is ribbed by angle, so the walls fall in long curtains.
        let wander = value_noise3(ang.cos() * 3.0, m.y / 11.0, ang.sin() * 3.0, seed ^ ROCK.rotate_left(13)) * 2.4;
        let flute = 0.6 * (ang * 41.0 + wander).sin() + 0.4 * (ang * 97.0 + wander * 1.7).sin();
        let n = value_noise3(m.x / 6.0, m.y / 19.0, m.z / 6.0, seed ^ ROCK.rotate_left(11));
        let hf = hr * (1.0 + p.l.flute * flute);
        return hf * hf + ((m.y - POOL_Y) / p.l.grotto_h).powi(2) < 0.88 + 0.14 * n && !stalactite(p, m);
    }
    let n = value_noise3(m.x / 6.0, m.y / 6.0, m.z / 6.0, seed ^ ROCK.rotate_left(11));
    m.y >= pool_bed(p, m.x, m.z) && hr < 0.88 + 0.08 * n
}

/// Smooth core radius of the pillar: a broad foot narrowing toward the capital.
fn pillar_taper(p: &Plan, y: f32) -> f32 {
    let t = ((y - POOL_Y) / (p.summit_y - 4.0 - POOL_Y)).clamp(0.0, 1.0);
    4.6 - 1.1 * t
}

/// The pillar. A boulder stack: each 2.1 m course bulges out and pinches in
/// at its seams, with a few lobes turned per course. A monolith: a rounded
/// square shaft with proud strata ledges every 3 m.
fn pillar_solid(p: &Plan, m: Vec3) -> bool {
    let (dx, dz) = (m.x - p.pillar.0, m.z - p.pillar.1);
    if p.monolith {
        let (s, c) = p.l.monolith_rot.sin_cos();
        let (a, b) = (dx * c - dz * s, dx * s + dz * c);
        let d = (a.powi(4) + b.powi(4)).sqrt().sqrt();
        let ledge = if (m.y / 3.0).rem_euclid(1.0) < 0.14 { 0.25 } else { 0.0 };
        return d < pillar_taper(p, m.y) - 0.75 + ledge;
    }
    let course = (m.y - p.pool_floor) / 2.1;
    let k = course.floor() as i32;
    let f = course.fract() * 2.0 - 1.0;
    let ang = dx.atan2(dz);
    let spin = hash3(k, 21, 0, p.seed ^ PILLAR) * TAU;
    let lobes = 0.30 * (ang * 3.0 + spin).cos() + 0.18 * (ang * 5.0 - spin * 1.3).cos();
    let bulge = (1.0 - f * f).max(0.0).sqrt();
    dx.hypot(dz) < pillar_taper(p, m.y) - 0.45 + bulge * (0.55 + lobes)
}

/// Tread top of the stair at this point's angle, one per turn, lowest first.
/// A hugging stair rides the pillar's taper; a flying one keeps its radius.
fn helix_treads(p: &Plan, m: Vec3, s: f32) -> impl Iterator<Item = f32> + '_ {
    let (dx, dz) = (m.x - p.pillar.0, m.z - p.pillar.1);
    let r = dx.hypot(dz);
    let u = (dz.atan2(dx) - p.stair_start).rem_euclid(TAU);
    let start = POOL_Y + 1.0;
    let pitch = (p.summit_y - start) / p.l.stair_turns.max(0.25);
    (0..4).filter_map(move |turn| {
        let raw = start + (u + TAU * turn as f32) / TAU * pitch;
        if raw > p.summit_y {
            return None;
        }
        let top = start + ((raw - start) / s).floor() * s + s;
        let inner = if p.flying { p.l.flying_r } else { pillar_taper(p, top) - 0.3 };
        (inner..inner + STAIR_W).contains(&r).then_some(top)
    })
}

/// The flat bridge from a flying stair's last tread onto the summit rim.
fn on_bridge(p: &Plan, m: Vec3) -> bool {
    if !p.flying {
        return false;
    }
    let (s, c) = (p.stair_start + p.l.stair_turns * TAU).sin_cos();
    let (dx, dz) = (m.x - p.pillar.0, m.z - p.pillar.1);
    let along = dx * c + dz * s;
    let across = -dx * s + dz * c;
    across.abs() < STAIR_W * 0.5 && (STAIR_OUT - 0.6..p.l.flying_r + STAIR_W).contains(&along)
}

/// Calcite drips hanging under a flying stair's treads, one chance per tread
/// and half-metre strip, so the stair reads as an old, wet, hanging flight.
fn stair_drip(p: &Plan, m: Vec3, top: f32, s: f32) -> bool {
    let r = (m.x - p.pillar.0).hypot(m.z - p.pillar.1);
    let underside = top - 0.8;
    if !p.flying || m.y >= underside {
        return false;
    }
    let step = ((top - POOL_Y) / s).round() as i32;
    let strip = ((r - p.l.flying_r) / 0.5).floor() as i32;
    let h = hash3(step, strip, 31, p.seed ^ PILLAR);
    if h > p.l.drips {
        return false;
    }
    let len = 0.4 + 3.0 * (1.0 - h / p.l.drips.max(1e-3)).powi(2);
    m.y >= underside - len
}

/// Where the stepping-stone path starts and ends: the shore ledge under the
/// passage mouth, and the islet edge facing it.
fn path_ends(p: &Plan, islet_r: f32) -> ((f32, f32), (f32, f32)) {
    let shore = (0.0, p.grotto_z - 0.74 * p.l.grotto_r);
    let (ox, oz) = (shore.0 - p.pillar.0, shore.1 - p.pillar.1);
    let len = ox.hypot(oz).max(1e-3);
    let end = (p.pillar.0 + ox / len * islet_r, p.pillar.1 + oz / len * islet_r);
    (shore, end)
}

const ISLET_R: f32 = 6.2;

/// Foot of a flying stair: a landing rock just outside its first tread,
/// where the stepping stones end. None for a hugging stair.
fn stair_foot(p: &Plan) -> Option<(f32, f32)> {
    let (s, c) = p.stair_start.sin_cos();
    let r = p.l.flying_r + STAIR_W * 0.5;
    p.flying.then_some((p.pillar.0 + c * r, p.pillar.1 + s * r))
}

/// Rimstone terraces (gours) along the shore: stepped calcite pools whose
/// rims stand a hand above the water, in a band just inside the wall.
fn gour(p: &Plan, m: Vec3, hr: f32) -> Option<BlockId> {
    if p.l.gours <= 0.0 || !(0.70..0.86).contains(&hr) || m.z - p.grotto_z < -0.6 * p.l.grotto_r {
        return None;
    }
    let field = value_noise2(m.x / 2.4, m.z / 2.4, p.seed ^ ROCK.rotate_left(23));
    if field > p.l.gours {
        return None;
    }
    // Each terrace steps up toward the wall; its rim is the contour edge.
    let step = ((hr - 0.70) / 0.04).floor();
    let top = POOL_Y - 0.3 + step * 0.25;
    let rim = ((hr - 0.70) / 0.04).fract() > 0.78;
    if m.y < top - 1.2 {
        return Some(BlockId::GrottoStone);
    }
    if m.y < top {
        return Some(if rim { BlockId::InlaidMarble } else { BlockId::GrottoStone });
    }
    if rim && m.y < top + 0.25 {
        return Some(BlockId::InlaidMarble);
    }
    (m.y < top + 0.2 && m.y >= POOL_Y).then_some(BlockId::Water)
}

fn grotto_inside(pl: &Plan, p: IVec3, m: Vec3, vm: f32, s: f32) -> Option<BlockId> {
    if !grotto_open(pl, m) {
        return None;
    }
    let seed = pl.seed;
    let sy = pl.summit_y;
    let (px, pz) = pl.pillar;
    let r = (m.x - px).hypot(m.z - pz);
    if m.y < sy - 4.0 && pillar_solid(pl, m) {
        return Some(BlockId::GrottoStone);
    }
    // Stair treads, drips under them, and headroom through the capital and rim.
    for top in helix_treads(pl, m, s) {
        if m.y < top && m.y >= top - 0.8 {
            return Some(BlockId::CarvedSandstone);
        }
        if m.y >= top && m.y < top + 2.2 {
            return Some(BlockId::Air);
        }
        if stair_drip(pl, m, top, s) {
            return Some(BlockId::GrottoStone);
        }
    }
    if on_bridge(pl, m) {
        if (sy - 0.8..sy).contains(&m.y) {
            return Some(BlockId::CarvedSandstone);
        }
        if (sy..sy + 2.2).contains(&m.y) {
            return Some(BlockId::Air);
        }
    }
    // The flared capital and the summit floor.
    if (sy - 4.0..sy - 1.0).contains(&m.y) && r < pillar_taper(pl, m.y) + (m.y - (sy - 4.0)) * 0.55 {
        return Some(BlockId::GrottoStone);
    }
    if (sy - 1.0..sy).contains(&m.y) && r < STAIR_OUT {
        if p == cell(Vec3::new(px + 2.5, sy - 0.5, pz + 0.5), vm) {
            return Some(BlockId::GildedGate);
        }
        return Some(if r >= 4.6 { BlockId::JeweledGold } else { BlockId::InlaidMarble });
    }
    // The pedestal and its cache stand in the shaft of light.
    if m.y >= sy && r < STAIR_OUT {
        if p == cell(Vec3::new(px + 0.5, sy + 0.5, pz + 0.5), vm) {
            return Some(BlockId::CarvedSandstone);
        }
        if p == cell(Vec3::new(px + 0.5, sy + 1.5, pz + 0.5), vm) {
            return Some(BlockId::SurveyCache);
        }
    }
    // Islets with a stalagmite spire, standing out of the water.
    for &(ix, iz, ir, spire) in &pl.islets[..pl.islet_count] {
        let d = (m.x - ix).hypot(m.z - iz);
        let ragged = ir * (0.8 + 0.4 * value_noise2(m.x / 1.7, m.z / 1.7, seed ^ ROCK.rotate_left(19)));
        if m.y < POOL_Y + 0.6 && d < ragged {
            return Some(BlockId::GrottoStone);
        }
        let rise = m.y - (POOL_Y + 0.6);
        let ring = 1.0 + 0.12 * (rise * TAU * 2.0).sin();
        if rise >= 0.0 && rise < spire && d < (ir * 0.55 * (1.0 - rise / spire) + 0.12) * ring {
            return Some(BlockId::GrottoStone);
        }
    }
    if m.y < POOL_Y + 1.0 {
        // Shore ledge under the passage mouth.
        if m.z - pl.grotto_z < -0.77 * pl.l.grotto_r && m.x.abs() < 6.0 {
            return Some(BlockId::GrottoStone);
        }
        // A ragged rock islet carries the pillar foot just above the water.
        if r < ISLET_R + 1.3 * value_noise2(m.x / 2.5, m.z / 2.5, seed ^ PILLAR) {
            return Some(BlockId::GrottoStone);
        }
        // A flying stair rises from its own landing rock over open water.
        if let Some((fx, fz)) = stair_foot(pl)
            && (m.x - fx).hypot(m.z - fz) < 1.9 + 0.4 * value_noise2(m.x / 1.5, m.z / 1.5, seed ^ PILLAR)
        {
            return Some(BlockId::GrottoStone);
        }
        // Stepping stones along a bent path from the shore to the islet, or
        // to the landing rock of a flying stair.
        let (a, mut b) = path_ends(pl, ISLET_R + 0.4);
        if let Some(foot) = stair_foot(pl) {
            b = foot;
        }
        let (ox, oz) = (b.0 - a.0, b.1 - a.1);
        let len = ox.hypot(oz).max(1e-3);
        let bend = (a.0 + ox * 0.5 - oz / len * pl.l.path_bend, a.1 + oz * 0.5 + ox / len * pl.l.path_bend);
        let count = (len / 2.3).ceil().max(1.0) as i32;
        for i in 0..count {
            let t = (i as f32 + 0.5) / count as f32;
            let w = (1.0 - t) * (1.0 - t);
            let v = 2.0 * t * (1.0 - t);
            let x = w * a.0 + v * bend.0 + t * t * b.0;
            let z = w * a.1 + v * bend.1 + t * t * b.1;
            let size = 0.8 + 0.3 * hash3(i, 33, 0, seed ^ ROCK);
            if (m.x - x).hypot(m.z - z) < size {
                return Some(BlockId::GrottoStone);
            }
        }
        let (hr, _) = grotto_hr(pl, m.x, m.z);
        if let Some(b) = gour(pl, m, hr) {
            return Some(b);
        }
    }
    if m.y < POOL_Y {
        return Some(BlockId::Water);
    }
    Some(BlockId::Air)
}

fn grotto_shell(pl: &Plan, m: Vec3, vm: f32) -> Option<BlockId> {
    let seed = pl.seed;
    let (hr, _) = grotto_hr(pl, m.x, m.z);
    if hr > 1.2 {
        return None;
    }
    // A sunstone boss in the dome above the pillar feeds the shaft of light.
    let rp = (m.x - pl.pillar.0).hypot(m.z - pl.pillar.1);
    if rp < SHAFT_R + 1.1 && m.y > pl.summit_y + 6.0 && grotto_open(pl, m - Vec3::Y * vm) {
        return Some(BlockId::Sunstone);
    }
    // Foxfire along the walls just above the water.
    let r = m.x.hypot(m.z - pl.grotto_z);
    if (POOL_Y..POOL_Y + 10.0).contains(&m.y) && r > 6.0 {
        let inward = Vec3::new(-m.x, 0.0, pl.grotto_z - m.z) / r;
        if grotto_open(pl, m + inward * vm)
            && value_noise3(m.x / 3.0, m.y / 3.0, m.z / 3.0, seed ^ LIGHT) > 0.52
        {
            return Some(BlockId::FoxfireMoss);
        }
    }
    None
}

// --- Rock -------------------------------------------------------------------

fn rock(pl: &Plan, m: Vec3) -> BlockId {
    let seed = pl.seed;
    let grotto = m.z + 6.0 * value_noise2(m.x / 6.0, m.y / 6.0, seed ^ ROCK) > pl.grotto_z - 1.12 * pl.l.grotto_r;
    if grotto {
        return BlockId::GrottoStone;
    }
    // Gold-bearing quartz veins run through the sandstone.
    if value_noise3(m.x / 5.0, m.y / 4.0, m.z / 5.0, seed ^ ROCK.rotate_left(5)) > 0.84 {
        return BlockId::GoldQuartz;
    }
    BlockId::OchreSandstone
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3) -> BlockId {
    let vm = cfg.voxel_meters();
    let m = (p.as_vec3() + Vec3::splat(0.5)) * vm;
    if m.y < BEDROCK_M {
        return BlockId::Bedrock;
    }
    if m.y > ROOF_TOP_M {
        return BlockId::Air;
    }
    if gate_at(m) {
        return BlockId::GildedGate;
    }
    let pl = plan(cfg.seed);
    let pl = &pl;
    // Stairs step a quarter metre at fine scales, one voxel at coarse ones.
    let s = vm.max(0.25);
    let hoard_reach = pl.hoard_z + pl.l.hoard_rz * 1.2;
    let grotto_near = m.z > pl.grotto_z - pl.l.grotto_r * 1.35;
    let inside = if m.z < STAIR_Z.1 + 1.0 { arrival_inside(p, m, vm, s) } else { None }
        .or_else(|| if m.z < pl.gallery_end + 1.0 { gallery_inside(pl, p, m, vm) } else { None })
        .or_else(|| if m.z > pl.gallery_end - 1.0 && m.z < hoard_reach { hoard_inside(pl, p, m, vm) } else { None })
        .or_else(|| passage_inside(pl, p, m, vm, s))
        .or_else(|| if grotto_near { grotto_inside(pl, p, m, vm, s) } else { None });
    if let Some(b) = inside {
        return b;
    }
    let shell = arrival_shell(m, s)
        .or_else(|| gallery_shell(pl, m))
        .or_else(|| passage_shell(pl, m, s))
        .or_else(|| if m.z > pl.gallery_end && m.z < hoard_reach { hoard_shell(pl, m, vm) } else { None })
        .or_else(|| if grotto_near { grotto_shell(pl, m, vm) } else { None });
    shell.unwrap_or_else(|| rock(pl, m))
}
