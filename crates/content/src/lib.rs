mod size_class;
mod sim_traits;
mod validate;
pub use size_class::SizeClass;
pub use sim_traits::{COHESION_STABLE, SIM_TABLE, SimTraits, ThermalTraits};

use glam::IVec3;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MatterState {
    Vacuum,
    Solid,
    Powder,
    Liquid,
    Gas,
    Plasma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CollisionKind {
    Passable,
    Solid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RenderKind {
    Invisible,
    Opaque,
    Cutout,
    Liquid,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FluidTraits {
    pub viscosity: f32,
    pub compressibility: f32,
    pub pressure_response: f32,
}

impl FluidTraits {
    pub const NONE: Self = Self {
        viscosity: 0.0,
        compressibility: 0.0,
        pressure_response: 0.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialPhysicsTraits {
    pub matter_state: MatterState,
    pub collision: CollisionKind,
    pub density: f32,
    pub hardness: f32,
    pub friction: f32,
    pub restitution: f32,
    pub fluid: FluidTraits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TintPattern {
    None,
    SumXZParity,
    DiffXZParity,
    AbsSumXZParity,
    SumXZParityPlusYParity,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColorModel {
    pub base: [f32; 3],
    pub base_tint: f32,
    pub parity_tint: f32,
    pub vertical_tint: f32,
    pub pattern: TintPattern,
}

impl ColorModel {
    pub const fn flat(base: [f32; 3]) -> Self {
        Self {
            base,
            base_tint: 1.0,
            parity_tint: 0.0,
            vertical_tint: 0.0,
            pattern: TintPattern::None,
        }
    }

    pub const fn tinted(
        base: [f32; 3],
        base_tint: f32,
        parity_tint: f32,
        vertical_tint: f32,
        pattern: TintPattern,
    ) -> Self {
        Self {
            base,
            base_tint,
            parity_tint,
            vertical_tint,
            pattern,
        }
    }

    pub fn sample(self, _p: IVec3) -> [f32; 3] {
        // Keep mesh vertex colors neutral. World-space surface painting owns
        // variation; alternating voxel tints caused checkerboards and LOD seams.
        let tint = self.base_tint + self.parity_tint * 0.5 + self.vertical_tint * 0.5;
        [
            self.base[0] * tint,
            self.base[1] * tint,
            self.base[2] * tint,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialRenderTraits {
    pub kind: RenderKind,
    pub occludes_light: bool,
    pub color: ColorModel,
    pub surface: SurfaceDetail,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SurfaceDetail {
    pub material_id: u32,
    /// Procedural surface family; shared with world_shader.wgsl.
    pub texture_kind: u32,
    pub roughness: f32,
    pub specular: f32,
    pub detail_strength: f32,
    pub ao_strength: f32,
}

impl SurfaceDetail {
    pub fn shader_key(self) -> f32 {
        (self.texture_kind + self.material_id * 64) as f32
    }
    pub const fn new(
        roughness: f32,
        specular: f32,
        detail_strength: f32,
        ao_strength: f32,
    ) -> Self {
        Self {
            texture_kind: 0,
            material_id: 0,
            roughness,
            specular,
            detail_strength,
            ao_strength,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialTags {
    pub selectable: bool,
    pub hotbar_order: Option<u8>,
    pub contributes_to_heightmap: bool,
    pub render_in_lod_decor: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialDef {
    pub id: BlockId,
    #[serde(deserialize_with = "leak")]
    pub display_name: &'static str,
    pub render: MaterialRenderTraits,
    pub physics: MaterialPhysicsTraits,
    /// Cellular-simulation properties; omitted rows are a generic inert solid.
    #[serde(default, skip_serializing_if = "SimTraits::is_generic")]
    pub sim: SimTraits,
    pub tags: MaterialTags,
}

/// Declares `BlockId` and `ALL_BLOCK_IDS` in table order. The per-block
/// properties live in `assets/materials.ron`, one row per id in this order.
macro_rules! block_ids {
    ( $( $id:ident ),+ $(,)? ) => {
        #[repr(u8)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
        pub enum BlockId {
            $( $id, )+
        }

        pub const ALL_BLOCK_IDS: &[BlockId] = &[ $(BlockId::$id),+ ];
    };
}

const MATERIALS_RON: &str = include_str!("../../../assets/materials.ron");
/// One `MaterialDef` per `BlockId`, indexed by `id as usize`.
pub static MATERIAL_TABLE: std::sync::LazyLock<Vec<MaterialDef>> = std::sync::LazyLock::new(|| {
    let rows: Vec<MaterialDef> =
        ron::from_str(MATERIALS_RON).unwrap_or_else(|error| panic!("assets/materials.ron: {error}"));
    assert_eq!(rows.len(), ALL_BLOCK_IDS.len(), "assets/materials.ron must have one row per BlockId");
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.id as usize, i, "assets/materials.ron row {i} is {:?}; rows follow BlockId order", row.id);
    }
    let errors = validate::validate(&rows, MATERIALS_RON);
    assert!(errors.is_empty(), "invalid material table:
{}", errors.join("
"));
    rows
});

/// Display names live for the whole run; leaking them once keeps
/// `MaterialDef` a plain `Copy` row with a `&'static str` name.
fn leak<'de, D: serde::Deserializer<'de>>(d: D) -> Result<&'static str, D::Error> {
    <String as serde::Deserialize>::deserialize(d).map(|s| &*Box::leak(s.into_boxed_str()))
}

block_ids! {
    Air,
    Grass,
    Dirt,
    Stone,
    Sand,
    Sandstone,
    SnowGrass,
    Water,
    Log,
    Torch,
    TorchWallNorth,
    TorchWallSouth,
    TorchWallWest,
    TorchWallEast,
    Leaves,
    Bush,
    Gravel,
    Clay,
    Mud,
    Basalt,
    Ice,
    Lava,
    Planks,
    StoneBricks,
    Cobblestone,
    CoalOre,
    IronOre,
    PineLeaves,
    FungalSoil,
    MushroomStem,
    GlowCap,
    RedCap,
    Limestone,
    CopperOre,
    TinOre,
    Crystal,
    WeatheredBricks,
    SalvageCrate,
    Workbench,
    Smelter,
    CoalGenerator,
    Crusher,
    ElectricFurnace,
    FeedHopper,
    StorageCrate,
    ItemPipe,
    PowerCable,
    Bioreactor,
    SurveyCache,
    CultivatorCache,
    AlloyKiln,
    Fabricator,
    Cultivator,
    MiningDrill,
    WaterPump,
    FluidPipe,
    WaterTank,
    OreWasher,
    Fire,
    Smoke,
    Steam,
    Ash,
    RigidCore,
    CopperBlock,
    SilverBlock,
    GoldBlock,
    PlatinumBlock,
    MithrilBlock,
    MirageBrick,
    VoidStone,
    StarGlass,
    MirageVoid,
    BatteryBank,
    CokeOven,
    BlastFurnace,
    CokeOvenBrick,
    BlastBrick,
    InductionCasing,
    InductionPort,
    InductionCell,
    InductionProvider,
    Transformer,
    Assembler,
}

static HOTBAR_DEFAULTS: LazyLock<Vec<BlockId>> = LazyLock::new(|| {
    let mut slots = MATERIAL_TABLE
        .iter()
        .filter_map(|def| def.tags.hotbar_order.map(|order| (order, def.id)))
        .collect::<Vec<_>>();
    slots.sort_by_key(|(order, _)| *order);
    slots.into_iter().map(|(_, id)| id).collect()
});

/// Renderer-only families; low six bits of SurfaceDetail::shader_key.
/// These do not change BlockId or checkpoint identity.
pub mod terrain_family {
    pub const GRAVEL: u32 = 33;
    pub const TUFT: u32 = 34;
    pub const PEBBLE: u32 = 35;
    pub const CAPPED_TURF: u32 = 36;
}

impl BlockId {
    /// Attachment origin in the owning voxel; model dimensions are in metres.
    pub fn torch_anchor(self) -> glam::Vec3 {
        if self == Self::Torch {
            glam::Vec3::new(0.5, 0.0, 0.5)
        } else {
            glam::Vec3::new(0.5, 0.38, 0.5)
                + self.torch_support_offset().unwrap_or(IVec3::ZERO).as_vec3() * 0.5
        }
    }

    pub fn torch_point_meters(
        self,
        cell: glam::Vec3,
        model_point: glam::Vec3,
        scale: f32,
    ) -> glam::Vec3 {
        let anchor = self.torch_anchor();
        (cell + anchor) * scale + model_point - anchor
    }
    pub fn is_model(self) -> bool {
        matches!(
            self,
            Self::CoalGenerator
                | Self::Crusher
                | Self::ElectricFurnace
                | Self::FeedHopper
                | Self::StorageCrate
                | Self::ItemPipe
                | Self::PowerCable
                | Self::Bioreactor
                | Self::AlloyKiln
                | Self::Fabricator
                | Self::Cultivator
                | Self::MiningDrill
                | Self::WaterPump
                | Self::FluidPipe
                | Self::WaterTank
                | Self::OreWasher
                | Self::BatteryBank
                | Self::CokeOven
                | Self::BlastFurnace
                | Self::CokeOvenBrick
                | Self::BlastBrick
                | Self::InductionCasing
                | Self::InductionPort
                | Self::InductionCell
                | Self::InductionProvider
                | Self::Transformer
                | Self::Assembler
        )
    }
    pub const COUNT: usize = ALL_BLOCK_IDS.len();

    pub fn from_u32(raw: u32) -> Self {
        ALL_BLOCK_IDS
            .get(raw as usize)
            .copied()
            .unwrap_or(BlockId::Air)
    }

    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn material(self) -> &'static MaterialDef {
        &MATERIAL_TABLE[self as usize]
    }

    pub fn display_name(self) -> &'static str {
        self.material().display_name
    }

    pub fn color(self, p: IVec3) -> [f32; 3] {
        self.material().render.color.sample(p)
    }

    pub fn surface(self) -> SurfaceDetail {
        let mut surface = self.material().render.surface;
        surface.material_id = self as u32 + 1;
        surface.texture_kind = match self {
            Self::Fire => 42,
            Self::Smoke => 43,
            Self::Steam => 44,
            Self::Grass => 1,
            Self::Dirt | Self::Mud | Self::Clay => 2,
            Self::Stone | Self::Basalt => 3,
            Self::Gravel => terrain_family::GRAVEL,
            Self::Sand | Self::Sandstone => 4,
            Self::Log => 5,
            Self::Leaves | Self::Bush | Self::PineLeaves => 6,
            Self::SnowGrass => 7,
            Self::Water => 8,
            Self::Ice => 9,
            Self::Lava => 10,
            Self::Planks => 11,
            Self::StoneBricks => 12,
            Self::Cobblestone => 13,
            Self::CoalOre => 14,
            Self::IronOre => 15,
            Self::FungalSoil => 16,
            Self::MushroomStem => 17,
            Self::GlowCap => 18,
            Self::RedCap => 19,
            Self::Limestone => 3,
            Self::CopperOre => 20,
            Self::TinOre => 21,
            Self::Crystal => 22,
            Self::WeatheredBricks => 12,
            Self::SalvageCrate => 23,
            Self::Workbench => 24,
            Self::Smelter => 25,
            Self::CoalGenerator => 26,
            Self::Crusher => 27,
            Self::ElectricFurnace => 28,
            Self::FeedHopper => 29,
            Self::StorageCrate => 23,
            Self::ItemPipe => 30,
            Self::CopperBlock => 47,
            Self::SilverBlock => 48,
            Self::GoldBlock => 49,
            Self::PlatinumBlock => 50,
            Self::MithrilBlock => 51,
            Self::MirageBrick => 12,
            Self::VoidStone => 52,
            Self::StarGlass => 53,
            Self::MirageVoid => 52,
            Self::PowerCable => 31,
            Self::Bioreactor => 32,
            Self::SurveyCache => 23,
            Self::CultivatorCache => 23,
            _ => 0,
        };
        surface
    }

    /// Illusion blocks: drawn as stone, but the body passes through them.
    pub fn is_mirage(self) -> bool {
        matches!(self, Self::MirageBrick | Self::MirageVoid)
    }

    pub fn is_solid(self) -> bool {
        matches!(self.material().physics.collision, CollisionKind::Solid)
    }

    pub fn is_opaque(self) -> bool {
        if self.is_model() {
            return false;
        }
        self.material().render.occludes_light
    }

    pub fn is_empty(self) -> bool {
        matches!(self.material().render.kind, RenderKind::Invisible)
    }

    pub fn is_liquid(self) -> bool {
        matches!(self.material().render.kind, RenderKind::Liquid)
            || matches!(self.material().physics.matter_state, MatterState::Liquid)
    }

    pub fn is_torch(self) -> bool {
        matches!(
            self,
            BlockId::Torch
                | BlockId::TorchWallNorth
                | BlockId::TorchWallSouth
                | BlockId::TorchWallWest
                | BlockId::TorchWallEast
        )
    }

    pub fn is_ray_pickable(self) -> bool {
        // A mirage is an illusion for the body, not for the pickaxe.
        self.is_solid() || self.is_torch() || self.is_mirage()
    }

    pub fn torch_support_offset(self) -> Option<IVec3> {
        match self {
            BlockId::Torch => Some(IVec3::new(0, -1, 0)),
            BlockId::TorchWallNorth => Some(IVec3::new(0, 0, 1)),
            BlockId::TorchWallSouth => Some(IVec3::new(0, 0, -1)),
            BlockId::TorchWallWest => Some(IVec3::new(1, 0, 0)),
            BlockId::TorchWallEast => Some(IVec3::new(-1, 0, 0)),
            _ => None,
        }
    }

    pub fn torch_from_attachment(normal: IVec3) -> Option<Self> {
        match normal {
            IVec3 { x: 0, y: 1, z: 0 } => Some(BlockId::Torch),
            IVec3 { x: 0, y: 0, z: -1 } => Some(BlockId::TorchWallNorth),
            IVec3 { x: 0, y: 0, z: 1 } => Some(BlockId::TorchWallSouth),
            IVec3 { x: -1, y: 0, z: 0 } => Some(BlockId::TorchWallWest),
            IVec3 { x: 1, y: 0, z: 0 } => Some(BlockId::TorchWallEast),
            _ => None,
        }
    }

    pub fn hotbar_defaults() -> &'static [BlockId] {
        hotbar_defaults()
    }
}

pub fn material_defs() -> &'static [MaterialDef] {
    &MATERIAL_TABLE
}

pub fn hotbar_defaults() -> &'static [BlockId] {
    HOTBAR_DEFAULTS.as_slice()
}

pub fn hotbar_block(slot: usize) -> Option<BlockId> {
    hotbar_defaults().get(slot).copied()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leafy_occluders_are_solid_so_the_camera_cannot_enter_them() {
        for block in [BlockId::Leaves, BlockId::Bush, BlockId::PineLeaves] {
            assert!(
                block.is_solid(),
                "{} is an opaque/cutout occluder and must collide",
                block.display_name()
            );
        }
    }
}

#[cfg(test)]
mod material_table {
    use super::*;

    /// Serialising the loaded table reproduces the file (minus its comment
    /// header), so the file is the single source of truth.
    #[test]
    fn materials_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&MATERIAL_TABLE[..], ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = MATERIALS_RON.replace('\r', "");
        let body = match file.find("\n[\n") {
            Some(i) => &file[i + 1..],
            None => file.as_str(),
        };
        let (a, b) = (text.trim_end(), body.trim_end());
        if a != b {
            let i = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            let lo = i.saturating_sub(60);
            panic!(
                "assets/materials.ron differs from the loaded table at byte {i}; run `cargo test dump_materials_ron -- --ignored`.\n table: {:?}\n  file: {:?}",
                &a[lo..(i + 60).min(a.len())],
                &b[lo..(i + 60).min(b.len())]
            );
        }
    }

    /// Rewrites assets/materials.ron from the loaded table (keeps the header).
    #[test]
    #[ignore = "writes assets/materials.ron"]
    fn dump_materials_ron() {
        let header = MATERIALS_RON.split("[").next().unwrap_or("");
        let text = ron::ser::to_string_pretty(&MATERIAL_TABLE[..], ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/materials.ron");
        std::fs::write(path, format!("{header}{text}")).unwrap();
    }
}
