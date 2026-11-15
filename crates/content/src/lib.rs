pub mod fabrication;
pub mod factory_resources;
pub mod spells;
pub mod runes;
mod spell_validation;
pub mod fittings;
pub mod thin;
pub mod flora;
pub mod stamp;
pub mod biomes;
pub mod climate;
pub mod shaped;
mod size_class;
pub mod descriptions;
pub mod environment;
pub mod portal_layout;
pub mod reactions;
pub mod ore;
mod sim_traits;
mod validate;
pub use size_class::SizeClass;
pub use sim_traits::{COHESION_STABLE, SIM_DIRECT, SIM_RULES, SIM_TABLE, SimTraits, ThermalTraits, sim_row};

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
    /// Health per second lost while a body overlaps the block.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub contact_damage: f32,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
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
    /// Cellular-simulation stand-in for ids at or past `sim_traits::SIM_DIRECT`,
    /// which have no row of their own in the one-byte sim table. Must be a
    /// lower id. Omitted: Stone. Ignored below `SIM_DIRECT`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sim_as: Option<BlockId>,
    pub tags: MaterialTags,
}

/// Declares `BlockId` and `ALL_BLOCK_IDS` in table order. The per-block
/// properties live in `assets/materials.ron`, one row per id in this order.
macro_rules! block_ids {
    ( $( $id:ident ),+ $(,)? ) => {
        // Two bytes: 65,535 ids. Saves, regions and the wire store ids as
        // u32/varint/u16; the cellular sim keeps a one-byte `MaterialId` (see
        // `sim_traits::SIM_DIRECT`), so never cast a `BlockId` to `u8`.
        #[repr(u16)]
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
    Bedrock,
    TombStone,
    AmberCap,
    UmberCap,
    GroveMoss,
    Bookcase,
    CarvedStone,
    GlazedTiles,
    Bed,
    RoofTiles,
    LimePlaster,
    TimberPost,
    TimberBeamX,
    TimberBeamZ,
    DressedStone,
    OakDoor, OakDoorOpen, OakDoorUpper, OakDoorUpperOpen, OakDoorZ, OakDoorZOpen, OakDoorZUpper, OakDoorZUpperOpen, OakTrapdoor, OakTrapdoorOpen, OakTrapdoorZ, OakTrapdoorZOpen, OakTrapdoorTop, OakTrapdoorTopOpen, OakTrapdoorZTop, OakTrapdoorZTopOpen, Lever, LeverOn,
    Ladder, LadderSouth, LadderWest, LadderEast, Vines, VinesSouth, VinesWest, VinesEast, Sapling,
    SnowLayer,
    SignPost, SignPostSouth, SignPostWest, SignPostEast, WallSign, WallSignSouth, WallSignWest, WallSignEast, Chest, Lectern, Tnt,
    PlankStairs, PlankStairsSouth, PlankStairsWest, PlankStairsEast, BrickStairs, BrickStairsSouth, BrickStairsWest, BrickStairsEast,
    CobbleStairs, CobbleStairsSouth, CobbleStairsWest, CobbleStairsEast, PlankSlab, BrickSlab, CobbleSlab, PlankFence, BrickFence, CobbleFence,
    SearedBrick, SmelteryController, SmelteryDrain, SmelteryFaucet, CastingTable, CastingBasin, MoltenCopper, MoltenTin, MoltenIron, MoltenBronze, TinBlock, IronBlock, BronzeBlock, MoltenSteel, SteelBlock,
    Anvil, BrewingStand, Stonecutter, Grindstone,
    WhitePlaster,
    Paydirt,
    GoldQuartz,
    Bamboo, BambooShoot,
    WheatCrop, CarrotCrop, BeetCrop, CropShoot,
    ExhaustDuct, Chimney, DustFilter, WetScrubber, GasTank, GasGenerator, IndustrialWaste,
    Hellstone,
    Concrete, SteelPlate, RustedSteel, HazardPaint, WindowGlass, ChestCopper, ChestIron, ChestSteel,
    Sugarcane, SugarcaneShoot,
    EnchantingTable,
    KilnScale, KilnLining,
    Quarry,
    SulfuricAcid,
    Kelp, Seagrass, WaterShoot, LilyPad, Cattail, SeaOats, MangrovePropagule, MangroveRoots,
    BerryBush, SweetBerries, FruitBud, Apple, Cherry, Fig, CocoaPod, TrellisVine, Grapes,
    Potato, Onion, Cabbage, Tomato, Pumpkin, Melon, GourdVine, RiceCrop,
    Lavender, Orchid, Cactus, Agave, CaveVine, GlowBerries, Dripleaf, SporeBlossom,
    ManaFlower, CollectorFlower, IronFern,
    MoltenGold,
    Chalcopyrite, Malachite, Hematite, Magnetite, Cassiterite,
    ClaimStake,
    CropHarvester,
    // Dungeon quest seals (generator 294, save 72): opened with dungeon keys.
    LockedDoor, BossDoor,
    // The Lightless (save 74): its stone, the foxfire refuges and the gate block.
    LightlessStone, FoxfireMoss, LightlessShaft,
    // The Gilded Cave (save 75): its gate, cave rock, hoard and palace blocks.
    // `gilded_surfaces.wgsl` keys on these ids; keep them contiguous.
    GildedGate, OchreSandstone, GrottoStone, GoldCoins, JeweledGold,
    ZigzagTile, LozengeTile, StarTile, LapisBrick, GildedRelief,
    InlaidMarble, CarvedSandstone, ScaleTile, Sunstone,
    // The Highfold and the Gloaming (save 76): their gates and the dusk forest.
    HighfoldGate, GloamingGate, GloamingMoss, GloamingLog, GloamingLeaves,
    MechanicalButton, MechanicalButtonOn, PressurePlate, PressurePlateOn,
}

// Packed material IDs consumed by the recovered industrial WGSL finishes.
// Append new block identities; never insert before existing identities.
const _: () = assert!(BlockId::Concrete as u32 + 1 == 192
    && BlockId::SteelPlate as u32 + 1 == 193 && BlockId::RustedSteel as u32 + 1 == 194
    && BlockId::HazardPaint as u32 + 1 == 195 && BlockId::WindowGlass as u32 + 1 == 196
    && BlockId::SulfuricAcid as u32 + 1 == 206);
// Packed material IDs of the Gilded Cave blocks, read by `gilded_surfaces.wgsl`.
const _: () = assert!(BlockId::GildedGate as u32 + 1 == 256 && BlockId::Sunstone as u32 + 1 == 269);

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
    /// Warm emissive fungus gills (Amberroot grove); also a local light source.
    pub const AMBER_GILL: u32 = 37;
    /// Loose ash: fine grey powder. Family 0 would paint it as climate turf.
    pub const ASH: u32 = 38;
    /// Use the authored voxel colour with the generic block finish.
    pub const GENERIC_BLOCK: u32 = 39;
}

impl BlockId {
    pub fn chest_tier(self) -> Option<u8> {
        match self { Self::Chest => Some(0), Self::ChestCopper => Some(1), Self::ChestIron => Some(2), Self::ChestSteel => Some(3), _ => None }
    }
    pub fn chest_slots(self) -> usize { self.chest_tier().map_or(0, |tier| 27 + usize::from(tier) * 9) }

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
                | Self::Quarry
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
                | Self::ExhaustDuct | Self::Chimney | Self::DustFilter
                | Self::WetScrubber | Self::GasTank | Self::GasGenerator
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
        if let Some(base) = shaped::base(self) {
            surface.texture_kind = base.surface().texture_kind;
            return surface;
        }
        surface.texture_kind = match self {
            Self::Fire => 42,
            Self::Smoke => 43,
            Self::Steam => 44,
            Self::Ash => terrain_family::ASH,
            Self::Grass | Self::GloamingMoss => 1,
            Self::Dirt | Self::Mud | Self::Clay => 2,
            Self::Stone | Self::Basalt | Self::Hellstone | Self::LightlessStone | Self::LightlessShaft => 3,
            Self::Gravel | Self::Paydirt => terrain_family::GRAVEL,
            Self::Sand | Self::Sandstone => 4,
            Self::Log | Self::GloamingLog => 5,
            Self::Leaves | Self::Bush | Self::PineLeaves | Self::GloamingLeaves => 6,
            Self::Vines | Self::VinesSouth | Self::VinesWest | Self::VinesEast | Self::Sapling => 6,
            Self::Ladder | Self::LadderSouth | Self::LadderWest | Self::LadderEast => 11,
            Self::SnowGrass => 7,
            Self::Water => 8,
            Self::Ice | Self::SnowLayer => 9,
            Self::Lava | Self::SulfuricAcid => 10,
            Self::Planks | Self::Bed | Self::Tnt => 11,
            Self::RoofTiles => 46,
            Self::LimePlaster | Self::WhitePlaster => 54,
            Self::TimberPost => 55,
            Self::TimberBeamX => 56,
            Self::TimberBeamZ => 57,
            Self::DressedStone | Self::TombStone => 58,
            Self::Bookcase => 45,
            Self::CarvedStone | Self::GlazedTiles => 12,
            Self::Concrete | Self::SteelPlate | Self::RustedSteel | Self::HazardPaint | Self::WindowGlass => 12,
            Self::ChestCopper | Self::ChestIron | Self::ChestSteel => 11,
            Self::StoneBricks => 12,
            Self::Cobblestone => 13,
            Self::CoalOre => 14,
            Self::IronOre | Self::Hematite | Self::Magnetite | Self::GoldQuartz => 15,
            Self::FungalSoil | Self::GroveMoss => 16,
            Self::MushroomStem => 17,
            Self::GlowCap => 18,
            Self::RedCap => 19,
            Self::AmberCap => terrain_family::AMBER_GILL,
            // Fungal flesh streaks; family 0 would take the climate turf paint.
            Self::UmberCap => 17,
            Self::Limestone => 3,
            Self::CopperOre | Self::Chalcopyrite | Self::Malachite => 20,
            Self::TinOre | Self::Cassiterite => 21,
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
            Self::CopperBlock | Self::TinBlock | Self::IronBlock | Self::BronzeBlock | Self::SteelBlock => 47,
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
            // Gilded Cave blocks share the masonry family; the shader picks
            // each surface by block id (see `gilded_surfaces.wgsl`).
            Self::GildedGate | Self::OchreSandstone | Self::GrottoStone | Self::GoldCoins
            | Self::JeweledGold | Self::ZigzagTile | Self::LozengeTile | Self::StarTile
            | Self::LapisBrick | Self::GildedRelief | Self::InlaidMarble | Self::CarvedSandstone
            | Self::ScaleTile | Self::Sunstone => 12,
            _ => terrain_family::GENERIC_BLOCK,
        };
        surface
    }

    /// Illusion blocks: drawn as stone, but the body passes through them.
    /// Undiggable: only mechanisms (keys, puzzles, reliquaries) remove it.
    pub fn is_seal(self) -> bool {
        matches!(self, Self::TombStone | Self::LockedDoor | Self::BossDoor)
    }

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

    /// Health per second a body overlapping this block loses.
    pub fn contact_damage(self) -> f32 {
        self.material().tags.contact_damage
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

    /// Blocks kept in `ChunkData::torches`, the index the local light list reads.
    pub fn is_light_source(self) -> bool {
        self.is_torch() || matches!(self, BlockId::AmberCap | BlockId::GlowBerries | BlockId::FoxfireMoss | BlockId::LightlessShaft
            | BlockId::GildedGate | BlockId::Sunstone | BlockId::HighfoldGate | BlockId::GloamingGate)
    }

    pub fn is_ray_pickable(self) -> bool {
        // A mirage is an illusion for the body, not for the pickaxe.
        self.is_solid() || self.is_torch() || self.is_mirage() || thin::is_thin(self)
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

    /// Floor when the face points along local up, otherwise a wall mount.
    /// A downward world face is a floor wherever local up is not world up.
    pub fn torch_for_face(normal: IVec3, up: glam::Vec3) -> Option<Self> {
        let n = normal.as_vec3();
        let up = up.normalize_or_zero();
        if n.length_squared() > 0.0 && n.normalize().dot(up) > 0.55 {
            return Some(BlockId::Torch);
        }
        Self::torch_from_attachment(normal)
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
    use glam::{IVec3, Vec3};

    #[test]
    fn a_floor_facing_local_up_takes_a_standing_torch() {
        assert_eq!(BlockId::torch_for_face(IVec3::Y, Vec3::Y), Some(BlockId::Torch));
        assert_eq!(BlockId::torch_for_face(IVec3::NEG_Y, Vec3::NEG_Y), Some(BlockId::Torch));
        assert_eq!(BlockId::torch_for_face(IVec3::X, Vec3::X), Some(BlockId::Torch));
        assert_eq!(
            BlockId::torch_for_face(IVec3::X, Vec3::Y),
            Some(BlockId::TorchWallEast)
        );
        assert_eq!(BlockId::torch_for_face(IVec3::NEG_Y, Vec3::Y), None);
    }

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
