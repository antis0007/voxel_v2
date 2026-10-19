//! Persistent survival inventory: item stacks with stack limits, 9-slot hotbar,
//! 27-slot backpack, 4 equipment slots, 2×2 hand-crafting grid, and lossless transfers.
use super::{BlockId, expedition, factory};

pub const HOTBAR_SLOTS: usize = 9;
pub const PACK_SLOTS: usize = 27;
pub const EQUIP_SLOTS: usize = 4;
pub const CRAFT_SLOTS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EquipSlot {
    Head = 0,
    Chest = 1,
    Legs = 2,
    Feet = 3,
}

impl EquipSlot {
    pub const ALL: [Self; 4] = [Self::Head, Self::Chest, Self::Legs, Self::Feet];

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Head),
            1 => Some(Self::Chest),
            2 => Some(Self::Legs),
            3 => Some(Self::Feet),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Chest => "Chest",
            Self::Legs => "Legs",
            Self::Feet => "Feet",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Head => "Head slot · Equip a Copper, Steel, or Lumen helm",
            Self::Chest => "Chest slot · Equip a Copper or Steel cuirass",
            Self::Legs => "Legs slot · Equip Copper or Steel greaves",
            Self::Feet => "Feet slot · Equip Copper or Steel boots",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ArmorTier {
    Copper = 1,
    Steel = 2,
    Lumen = 3,
}

impl ArmorTier {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Copper),
            2 => Some(Self::Steel),
            3 => Some(Self::Lumen),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FoodKind {
    FungalRation = 0,
    RoastedCap = 1,
    Trailcake = 2,
}

impl FoodKind {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::FungalRation),
            1 => Some(Self::RoastedCap),
            2 => Some(Self::Trailcake),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::FungalRation => "Fungal ration",
            Self::RoastedCap => "Roasted mushroom",
            Self::Trailcake => "Spore trailcake",
        }
    }

    pub fn hunger_restore(self) -> f32 {
        match self {
            Self::FungalRation => 38.0,
            Self::RoastedCap => 24.0,
            Self::Trailcake => 32.0,
        }
    }

    pub fn health_restore(self) -> f32 {
        match self {
            Self::FungalRation => 8.0,
            Self::RoastedCap => 4.0,
            Self::Trailcake => 6.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Block(BlockId),
    Resource(u8),
    Food(FoodKind),
    Weapon(u8),
    Armor(EquipSlot, ArmorTier),
    Tool(u8),
}

impl Item {
    pub fn canonical_block(block: BlockId) -> Option<BlockId> {
        if block == BlockId::Air || block.is_liquid() || matches!(block, BlockId::Fire | BlockId::Smoke | BlockId::Steam) {
            return None;
        }
        if block.is_torch() {
            Some(BlockId::Torch)
        } else {
            Some(block)
        }
    }

    pub fn max_stack(self) -> u32 {
        match self {
            Self::Weapon(_) | Self::Armor(_, _) | Self::Tool(_) => 1,
            Self::Food(_) => 16,
            Self::Block(block) => {
                if matches!(
                    block,
                    BlockId::Workbench
                        | BlockId::Smelter
                        | BlockId::SalvageCrate
                        | BlockId::SurveyCache
                        | BlockId::CultivatorCache
                ) || (block.is_model()
                    && !matches!(
                        block,
                        BlockId::ItemPipe | BlockId::PowerCable | BlockId::FluidPipe
                    ))
                {
                    16
                } else {
                    64
                }
            }
            Self::Resource(_) => 64,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Block(block) => block.display_name(),
            Self::Resource(index) => expedition::NAMES
                .get(index as usize)
                .copied()
                .unwrap_or("Material"),
            Self::Food(food) => food.name(),
            Self::Weapon(1) => "Copper sword",
            Self::Weapon(2) => "Steel sword",
            Self::Weapon(3) => "Cinderbrand",
            Self::Weapon(4) => "Winterglass",
            Self::Weapon(_) => "Blade",
            Self::Armor(EquipSlot::Head, ArmorTier::Copper) => "Copper helm",
            Self::Armor(EquipSlot::Chest, ArmorTier::Copper) => "Copper cuirass",
            Self::Armor(EquipSlot::Legs, ArmorTier::Copper) => "Copper greaves",
            Self::Armor(EquipSlot::Feet, ArmorTier::Copper) => "Copper boots",
            Self::Armor(EquipSlot::Head, ArmorTier::Steel) => "Steel helm",
            Self::Armor(EquipSlot::Chest, ArmorTier::Steel) => "Steel cuirass",
            Self::Armor(EquipSlot::Legs, ArmorTier::Steel) => "Steel greaves",
            Self::Armor(EquipSlot::Feet, ArmorTier::Steel) => "Steel boots",
            Self::Armor(EquipSlot::Head, ArmorTier::Lumen) => "Lumen miner's helm",
            Self::Armor(_, ArmorTier::Lumen) => "Lumen armor",
            Self::Tool(1) => "Bronze bore",
            Self::Tool(2) => "Resonant bore",
            Self::Tool(3) => "Lumen wand",
            Self::Tool(4) => "Frost wand",
            Self::Tool(_) => "Resonant wand",
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            Self::Block(b) if b.is_torch() => "Lighting block",
            Self::Block(b) if factory::Kind::from_block(b).is_some() => "Machine assembly",
            Self::Block(BlockId::Workbench | BlockId::Smelter) => "Crafting station",
            Self::Block(_) => "Building block",
            Self::Resource(0..=5 | 13) => "Raw material",
            Self::Resource(_) => "Refined component",
            Self::Food(_) => "Provision · Edible",
            Self::Weapon(_) => "Melee weapon",
            Self::Armor(slot, _) => match slot {
                EquipSlot::Head => "Equipment · Head",
                EquipSlot::Chest => "Equipment · Chest",
                EquipSlot::Legs => "Equipment · Legs",
                EquipSlot::Feet => "Equipment · Feet",
            },
            Self::Tool(1 | 2) => "Excavation tool",
            Self::Tool(_) => "Arcane focus",
        }
    }

    // Concise stat line for tooltip (detailed mechanics documented in code comments).
    pub fn description(self) -> &'static str {
        match self {
            Self::Block(b) if b.is_torch() => "Light 14 · Wall / Floor",
            Self::Block(BlockId::Workbench) => "[F] Engineering & Arcane station",
            Self::Block(BlockId::Smelter) => "[F] Smelts ore + 0.25 coal",
            Self::Block(b) if factory::Kind::from_block(b).is_some() => "1m³ Machine assembly",
            Self::Block(_) => "Placeable block",
            Self::Resource(3) => "Fuel · Smelting & Power",
            Self::Resource(4) => "Luminescent · Bore fuel",
            Self::Resource(5) => "Resonant crystal",
            Self::Resource(13) => "Biomass · Rations & Bioreactor",
            Self::Resource(_) => "Crafting material",
            Self::Food(FoodKind::FungalRation) => "+38 Hunger · +8 HP",
            Self::Food(FoodKind::RoastedCap) => "+24 Hunger · +4 HP",
            Self::Food(FoodKind::Trailcake) => "+32 Hunger · +6 HP",
            Self::Weapon(1) => "22 DMG · Melee",
            Self::Weapon(3) => "38 DMG · Flame cleave finisher · +8 MP",
            Self::Weapon(4) => "28 DMG · Chill, shatter & freezing shards",
            Self::Weapon(_) => "32 DMG · Cleave",
            Self::Armor(EquipSlot::Head, ArmorTier::Lumen) => "+12% DEF · Lantern beam",
            Self::Armor(_, ArmorTier::Lumen) => "+12% DEF",
            Self::Armor(_, ArmorTier::Copper) => "+8% DEF",
            Self::Armor(_, ArmorTier::Steel) => "+14% DEF",
            Self::Tool(1) => "1.6m Seam bore [Alt+LMB]",
            Self::Tool(2) => "2.8m Seam bore [Alt+LMB]",
            Self::Tool(_) => "Spell focus [R / T]",
        }
    }

    pub fn equip_slot(self) -> Option<EquipSlot> {
        match self {
            Self::Armor(slot, _) => Some(slot),
            _ => None,
        }
    }

    pub fn armor_reduction(self) -> f32 {
        match self {
            Self::Armor(_, ArmorTier::Copper) => 0.08,
            Self::Armor(_, ArmorTier::Steel) => 0.14,
            Self::Armor(_, ArmorTier::Lumen) => 0.12,
            _ => 0.0,
        }
    }

    pub fn encode_u32(self) -> (u32, u32) {
        match self {
            Self::Block(b) => (1, b.as_u32()),
            Self::Resource(i) => (2, i as u32),
            Self::Food(f) => (3, f as u32),
            Self::Weapon(w) => (4, w as u32),
            Self::Armor(slot, tier) => (5, ((slot as u32) << 8) | (tier as u32)),
            Self::Tool(t) => (6, t as u32),
        }
    }

    pub fn decode_u32(kind: u32, payload: u32) -> anyhow::Result<Self> {
        use anyhow::ensure;
        Ok(match kind {
            1 => {
                let block = BlockId::from_u32(payload);
                ensure!(
                    block.as_u32() == payload && block != BlockId::Air,
                    "Invalid block item in survival inventory"
                );
                Self::Block(Self::canonical_block(block).unwrap_or(block))
            }
            2 => {
                ensure!(
                    (payload as usize) < factory::RESOURCE_COUNT,
                    "Invalid resource index in survival inventory"
                );
                Self::Resource(payload as u8)
            }
            3 => {
                let food = FoodKind::from_u8(payload as u8)
                    .ok_or_else(|| anyhow::anyhow!("Invalid food item in survival inventory"))?;
                Self::Food(food)
            }
            4 => {
                ensure!((1..=4).contains(&payload), "Invalid weapon tier in survival inventory");
                Self::Weapon(payload as u8)
            }
            5 => {
                let slot = EquipSlot::from_index(((payload >> 8) & 0xFF) as usize)
                    .ok_or_else(|| anyhow::anyhow!("Invalid armor slot in survival inventory"))?;
                let tier = ArmorTier::from_u8((payload & 0xFF) as u8)
                    .ok_or_else(|| anyhow::anyhow!("Invalid armor tier in survival inventory"))?;
                if tier == ArmorTier::Lumen {
                    ensure!(slot == EquipSlot::Head, "Lumen helm only fits head slot");
                }
                Self::Armor(slot, tier)
            }
            6 => {
                ensure!((1..=5).contains(&payload), "Invalid tool item in survival inventory");
                Self::Tool(payload as u8)
            }
            _ => anyhow::bail!("Unknown survival item kind {kind}"),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub item: Item,
    pub count: u32,
}

impl ItemStack {
    pub fn new(item: Item, count: u32) -> Self {
        Self {
            item,
            count: count.clamp(1, item.max_stack()),
        }
    }

    pub fn max_stack(self) -> u32 {
        self.item.max_stack()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotAddr {
    Hotbar(usize),
    Pack(usize),
    Equipment(usize),
    CraftGrid(usize),
}

#[derive(Clone, Debug)]
pub struct PickupNotice {
    pub item: Item,
    pub count: u32,
    pub seconds: f32,
    pub overflow: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CraftingRecipe {
    pub name: &'static str,
    pub output: ItemStack,
    pub ingredients: &'static [(Item, u32)],
    pub description: &'static str,
}

pub const HAND_RECIPES: &[CraftingRecipe] = &[
    CraftingRecipe {
        name: "Oak planks",
        output: ItemStack {
            item: Item::Block(BlockId::Planks),
            count: 4,
        },
        ingredients: &[(Item::Block(BlockId::Log), 1)],
        description: "Split timber logs into sturdy building planks.",
    },
    CraftingRecipe {
        name: "Artificer workbench",
        output: ItemStack {
            item: Item::Block(BlockId::Workbench),
            count: 1,
        },
        ingredients: &[(Item::Block(BlockId::Planks), 4)],
        description: "2×2 plank bench. Place and press F to assemble machines and bores.",
    },
    CraftingRecipe {
        name: "Torch (Coal)",
        output: ItemStack {
            item: Item::Block(BlockId::Torch),
            count: 4,
        },
        ingredients: &[(Item::Block(BlockId::Planks), 1), (Item::Resource(3), 1)],
        description: "Wall or floor torch crafted from planks and coal.",
    },
    CraftingRecipe {
        name: "Torch (Lumen)",
        output: ItemStack {
            item: Item::Block(BlockId::Torch),
            count: 4,
        },
        ingredients: &[(Item::Block(BlockId::Planks), 1), (Item::Resource(4), 1)],
        description: "Wall or floor torch fuelled by luminous mushroom fibre.",
    },
    CraftingRecipe {
        name: "Torch (Log & Coal)",
        output: ItemStack {
            item: Item::Block(BlockId::Torch),
            count: 4,
        },
        ingredients: &[(Item::Block(BlockId::Log), 1), (Item::Resource(3), 1)],
        description: "Field torch split directly from raw timber and coal.",
    },
    CraftingRecipe {
        name: "Stone bricks",
        output: ItemStack {
            item: Item::Block(BlockId::StoneBricks),
            count: 4,
        },
        ingredients: &[(Item::Block(BlockId::Stone), 4)],
        description: "Dress four stone blocks into fitted masonry.",
    },
    CraftingRecipe {
        name: "Cobblestone",
        output: ItemStack {
            item: Item::Block(BlockId::Cobblestone),
            count: 2,
        },
        ingredients: &[(Item::Block(BlockId::Stone), 2)],
        description: "Knap rough stone into high-grip cobblestone.",
    },
    CraftingRecipe {
        name: "Ore processor",
        output: ItemStack {
            item: Item::Block(BlockId::Smelter),
            count: 1,
        },
        ingredients: &[(Item::Block(BlockId::Cobblestone), 4)],
        description: "Field smelter. Place and press F to smelt ores with coal.",
    },
    CraftingRecipe {
        name: "Sandstone",
        output: ItemStack {
            item: Item::Block(BlockId::Sandstone),
            count: 2,
        },
        ingredients: &[(Item::Block(BlockId::Sand), 4)],
        description: "Compact loose sand into load-bearing sandstone.",
    },
    CraftingRecipe {
        name: "Fungal rations",
        output: ItemStack {
            item: Item::Food(FoodKind::FungalRation),
            count: 2,
        },
        ingredients: &[(Item::Resource(13), 1), (Item::Resource(4), 1)],
        description: "Restores +38 hunger and +8 health per ration.",
    },
    CraftingRecipe {
        name: "Roasted mushroom",
        output: ItemStack {
            item: Item::Food(FoodKind::RoastedCap),
            count: 2,
        },
        ingredients: &[(Item::Block(BlockId::RedCap), 1), (Item::Resource(3), 1)],
        description: "Roast ember mushroom caps over coal (+24 hunger, +4 health).",
    },
    CraftingRecipe {
        name: "Spore trailcake",
        output: ItemStack {
            item: Item::Food(FoodKind::Trailcake),
            count: 2,
        },
        ingredients: &[(Item::Block(BlockId::GlowCap), 1), (Item::Resource(13), 1)],
        description: "Travel ration made from luminous caps and fungal biomass (+32 hunger).",
    },
    CraftingRecipe {
        name: "Copper sword",
        output: ItemStack {
            item: Item::Weapon(1),
            count: 1,
        },
        ingredients: &[(Item::Resource(6), 2), (Item::Resource(4), 1)],
        description: "Balanced melee blade (22 base damage).",
    },
    CraftingRecipe {
        name: "Copper helm",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Head, ArmorTier::Copper),
            count: 1,
        },
        ingredients: &[(Item::Resource(6), 2), (Item::Resource(4), 1)],
        description: "Head protection (+8% damage reduction).",
    },
    CraftingRecipe {
        name: "Copper cuirass",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Chest, ArmorTier::Copper),
            count: 1,
        },
        ingredients: &[(Item::Resource(6), 4)],
        description: "Chest protection (+8% damage reduction).",
    },
    CraftingRecipe {
        name: "Copper greaves",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Legs, ArmorTier::Copper),
            count: 1,
        },
        ingredients: &[(Item::Resource(6), 3)],
        description: "Leg protection (+8% damage reduction).",
    },
    CraftingRecipe {
        name: "Copper boots",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Feet, ArmorTier::Copper),
            count: 1,
        },
        ingredients: &[(Item::Resource(6), 2)],
        description: "Footwear protection (+8% damage reduction).",
    },
    CraftingRecipe {
        name: "Lumen miner's helm",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Head, ArmorTier::Lumen),
            count: 1,
        },
        ingredients: &[
            (Item::Resource(6), 2),
            (Item::Resource(5), 1),
            (Item::Resource(4), 2),
        ],
        description: "Head armor (+12% reduction) with built-in underground lantern beam.",
    },
    CraftingRecipe {
        name: "Steel helm",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Head, ArmorTier::Steel),
            count: 1,
        },
        ingredients: &[(Item::Resource(15), 2), (Item::Resource(4), 1)],
        description: "Heavy head armor (+14% damage reduction).",
    },
    CraftingRecipe {
        name: "Steel cuirass",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Chest, ArmorTier::Steel),
            count: 1,
        },
        ingredients: &[(Item::Resource(15), 4)],
        description: "Heavy chest armor (+14% damage reduction).",
    },
    CraftingRecipe {
        name: "Steel greaves",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Legs, ArmorTier::Steel),
            count: 1,
        },
        ingredients: &[(Item::Resource(15), 3)],
        description: "Heavy leg armor (+14% damage reduction).",
    },
    CraftingRecipe {
        name: "Steel boots",
        output: ItemStack {
            item: Item::Armor(EquipSlot::Feet, ArmorTier::Steel),
            count: 1,
        },
        ingredients: &[(Item::Resource(15), 2)],
        description: "Heavy boots (+14% damage reduction).",
    },
    CraftingRecipe {
        name: "Steel sword",
        output: ItemStack {
            item: Item::Weapon(2),
            count: 1,
        },
        ingredients: &[(Item::Resource(15), 2), (Item::Resource(16), 1)],
        description: "Heavy steel blade (32 base damage).",
    },
];

#[derive(Clone, Debug)]
pub struct DroppedStack { pub pos: glam::Vec3, pub stack: ItemStack }
#[derive(Clone, Debug)]
pub struct SurvivalInventory {
    pub dropped: Vec<DroppedStack>,
    pub voxel_m: f32,
    pub hotbar: [Option<ItemStack>; HOTBAR_SLOTS],
    pub pack: [Option<ItemStack>; PACK_SLOTS],
    pub equipment: [Option<ItemStack>; EQUIP_SLOTS],
    pub craft_grid: [Option<ItemStack>; CRAFT_SLOTS],
    pub cursor: Option<ItemStack>,
    pub resource_remainders: [u32; factory::RESOURCE_COUNT],
    pub pickup_notice: Option<PickupNotice>,
    pub eat_timer: f32,
    pub starve_timer: f32,
}

impl Default for SurvivalInventory {
    fn default() -> Self {
        Self::new_starter()
    }
}

impl SurvivalInventory {
    pub fn retrieve_nearby(&mut self, pos: glam::Vec3) -> bool {
        let Some(index) = self.dropped.iter().position(|d| d.pos.distance(pos) < 3.0) else { return false; };
        let stack = self.dropped[index].stack;
        let (inserted, remaining) = self.try_insert(stack);
        if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() { eprintln!("[tool-review] retrieved {:?} inserted={} remaining={} stack_limit={}",stack.item,inserted,remaining,self.stack_limit(stack.item)); }
        if remaining == 0 { self.dropped.remove(index); }
        else { self.dropped[index].stack.count = remaining; }
        if inserted > 0 { self.notify_pickup(stack.item, inserted); }
        else { self.notify_overflow(stack.item); }
        true
    }
    pub fn stack_limit(&self, item: Item) -> u32 { Self::limit_at_scale(item, self.voxel_m) }
    fn limit_at_scale(item: Item, voxel_m: f32) -> u32 {
        let multiplier = if matches!(item, Item::Block(b) if !b.is_model()) {
            (1.0 / voxel_m.clamp(0.25, 1.0).powi(3)).round() as u32
        } else { 1 };
        item.max_stack().saturating_mul(multiplier)
    }

    pub fn empty() -> Self {
        Self {
            dropped: Vec::new(),
            voxel_m: 1.0,
            hotbar: [None; HOTBAR_SLOTS],
            pack: [None; PACK_SLOTS],
            equipment: [None; EQUIP_SLOTS],
            craft_grid: [None; CRAFT_SLOTS],
            cursor: None,
            resource_remainders: [0; factory::RESOURCE_COUNT],
            pickup_notice: None,
            eat_timer: 0.0,
            starve_timer: 0.0,
        }
    }

    pub fn new_starter() -> Self {
        let mut inv = Self::empty();
        inv.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Torch), 8));
        inv.hotbar[1] = Some(ItemStack::new(Item::Block(BlockId::Planks), 16));
        inv.hotbar[2] = Some(ItemStack::new(Item::Block(BlockId::Cobblestone), 16));
        inv.hotbar[3] = Some(ItemStack::new(Item::Food(FoodKind::FungalRation), 3));
        inv.hotbar[4] = Some(ItemStack::new(Item::Block(BlockId::Workbench), 1));
        inv
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.25);
        self.eat_timer = (self.eat_timer - dt).max(0.0);
        if let Some(notice) = &mut self.pickup_notice {
            notice.seconds -= dt;
            if notice.seconds <= 0.0 {
                self.pickup_notice = None;
            }
        }
    }

    pub fn notify_pickup(&mut self, item: Item, count: u32) {
        if count == 0 {
            return;
        }
        if let Some(existing) = &mut self.pickup_notice {
            if !existing.overflow && existing.item == item && existing.seconds > 0.2 {
                existing.count = existing.count.saturating_add(count);
                existing.seconds = 2.6;
                return;
            }
        }
        self.pickup_notice = Some(PickupNotice {
            item,
            count,
            seconds: 2.6,
            overflow: false,
        });
    }

    pub fn notify_overflow(&mut self, item: Item) {
        self.pickup_notice = Some(PickupNotice {
            item,
            count: 0,
            seconds: 2.8,
            overflow: true,
        });
    }

    pub fn slot(&self, addr: SlotAddr) -> Option<ItemStack> {
        match addr {
            SlotAddr::Hotbar(i) => self.hotbar.get(i).copied().flatten(),
            SlotAddr::Pack(i) => self.pack.get(i).copied().flatten(),
            SlotAddr::Equipment(i) => self.equipment.get(i).copied().flatten(),
            SlotAddr::CraftGrid(i) => self.craft_grid.get(i).copied().flatten(),
        }
    }

    pub fn slot_mut(&mut self, addr: SlotAddr) -> Option<&mut Option<ItemStack>> {
        match addr {
            SlotAddr::Hotbar(i) => self.hotbar.get_mut(i),
            SlotAddr::Pack(i) => self.pack.get_mut(i),
            SlotAddr::Equipment(i) => self.equipment.get_mut(i),
            SlotAddr::CraftGrid(i) => self.craft_grid.get_mut(i),
        }
    }

    pub fn slot_accepts(&self, addr: SlotAddr, item: Item) -> bool {
        match addr {
            SlotAddr::Equipment(index) => {
                EquipSlot::from_index(index).is_some_and(|slot| item.equip_slot() == Some(slot))
            }
            SlotAddr::Hotbar(i) => i < HOTBAR_SLOTS,
            SlotAddr::Pack(i) => i < PACK_SLOTS,
            SlotAddr::CraftGrid(i) => i < CRAFT_SLOTS,
        }
    }

    /// Total count of `item` across hotbar and backpack (plus craft_grid/cursor if `include_transient`).
    pub fn count_item(&self, item: Item) -> u32 {
        self.count_item_with_transient(item, true)
    }

    pub fn count_item_with_transient(&self, item: Item, include_transient: bool) -> u32 {
        let mut total = 0u32;
        for slot in self.hotbar.iter().chain(self.pack.iter()) {
            if let Some(stack) = slot {
                if stack.item == item {
                    total = total.saturating_add(stack.count);
                }
            }
        }
        if include_transient {
            for slot in &self.craft_grid {
                if let Some(stack) = slot {
                    if stack.item == item {
                        total = total.saturating_add(stack.count);
                    }
                }
            }
            if let Some(stack) = self.cursor {
                if stack.item == item {
                    total = total.saturating_add(stack.count);
                }
            }
        }
        total
    }

    pub fn used_pack_slots(&self) -> usize {
        self.pack.iter().filter(|s| s.is_some()).count()
    }

    pub fn armor_reduction(&self) -> f32 {
        self.equipment
            .iter()
            .flatten()
            .map(|stack| stack.item.armor_reduction())
            .sum::<f32>()
            .clamp(0.0, 0.65)
    }

    pub fn armor_points(&self) -> u32 {
        (self.armor_reduction() * 20.0).round() as u32
    }

    pub fn has_lumen_helm(&self) -> bool {
        self.equipment[EquipSlot::Head as usize]
            .is_some_and(|s| s.item == Item::Armor(EquipSlot::Head, ArmorTier::Lumen))
    }

    /// Remaining capacity for `item` across hotbar and backpack.
    pub fn remaining_capacity_for(&self, item: Item) -> u32 {
        let max = self.stack_limit(item);
        let mut cap = 0u32;
        for slot in self.hotbar.iter().chain(self.pack.iter()) {
            match slot {
                Some(existing) if existing.item == item && existing.count < max => {
                    cap = cap.saturating_add(max - existing.count);
                }
                None => {
                    cap = cap.saturating_add(max);
                }
                _ => {}
            }
        }
        cap
    }

    pub fn can_insert(&self, stack: ItemStack) -> bool {
        self.remaining_capacity_for(stack.item) >= stack.count
    }

    /// Inserts as much of `stack` as fits into hotbar/pack (matching partial stacks first, then empty slots).
    /// Returns `(inserted_count, remainder_count)`. Never loses or duplicates items.
    pub fn try_insert(&mut self, stack: ItemStack) -> (u32, u32) {
        if stack.count == 0 {
            return (0, 0);
        }
        let max = self.stack_limit(stack.item);
        let mut remaining = stack.count;

        // Pass 1: merge into existing partial stacks (hotbar first, then pack).
        for slot in self.hotbar.iter_mut().chain(self.pack.iter_mut()) {
            if remaining == 0 {
                break;
            }
            if let Some(existing) = slot {
                if existing.item == stack.item && existing.count < max {
                    let add = remaining.min(max - existing.count);
                    existing.count += add;
                    remaining -= add;
                }
            }
        }

        // Pass 2: fill empty slots (hotbar first, then pack).
        for slot in self.hotbar.iter_mut().chain(self.pack.iter_mut()) {
            if remaining == 0 {
                break;
            }
            if slot.is_none() {
                let add = remaining.min(max);
                *slot = Some(ItemStack {
                    item: stack.item,
                    count: add,
                });
                remaining -= add;
            }
        }

        (stack.count - remaining, remaining)
    }

    /// Inserts into `pack` first, then `hotbar` (used when returning crafting/equipment items).
    pub fn try_insert_pack_first(&mut self, stack: ItemStack) -> (u32, u32) {
        if stack.count == 0 {
            return (0, 0);
        }
        let max = self.stack_limit(stack.item);
        let mut remaining = stack.count;

        for slot in self.pack.iter_mut().chain(self.hotbar.iter_mut()) {
            if remaining == 0 {
                break;
            }
            if let Some(existing) = slot {
                if existing.item == stack.item && existing.count < max {
                    let add = remaining.min(max - existing.count);
                    existing.count += add;
                    remaining -= add;
                }
            }
        }

        for slot in self.pack.iter_mut().chain(self.hotbar.iter_mut()) {
            if remaining == 0 {
                break;
            }
            if slot.is_none() {
                let add = remaining.min(max);
                *slot = Some(ItemStack {
                    item: stack.item,
                    count: add,
                });
                remaining -= add;
            }
        }

        (stack.count - remaining, remaining)
    }

    /// Removes up to `amount` of `item` across pack and hotbar (only if all `amount` are available).
    pub fn remove_item(&mut self, item: Item, amount: u32) -> bool {
        if amount == 0 {
            return true;
        }
        if self.count_item(item) < amount {
            return false;
        }
        let mut remaining = amount;
        for slot in self
            .pack
            .iter_mut()
            .chain(self.hotbar.iter_mut())
            .chain(self.craft_grid.iter_mut())
        {
            if remaining == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.item == item {
                    let take = remaining.min(stack.count);
                    stack.count -= take;
                    remaining -= take;
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        if remaining > 0 {
            if let Some(cursor) = &mut self.cursor {
                if cursor.item == item {
                    let take = remaining.min(cursor.count);
                    cursor.count -= take;
                    remaining -= take;
                    if cursor.count == 0 {
                        self.cursor = None;
                    }
                }
            }
        }
        debug_assert_eq!(remaining, 0);
        true
    }

    /// Consumes 1 item from a specific hotbar slot if it matches `expected_item` (or any block/food).
    pub fn consume_from_hotbar(&mut self, slot_index: usize, expected: Item) -> bool {
        let Some(slot) = self.hotbar.get_mut(slot_index) else {
            return false;
        };
        if let Some(stack) = slot {
            if stack.item == expected && stack.count > 0 {
                stack.count -= 1;
                if stack.count == 0 {
                    *slot = None;
                }
                return true;
            }
        }
        self.remove_item(expected, 1)
    }

    /// Left-click interaction on `addr`: pick up, place, merge, or swap with `self.cursor`.
    pub fn click_slot(&mut self, addr: SlotAddr) {
        if !self
            .cursor
            .is_none_or(|cursor| self.slot_accepts(addr, cursor.item))
        {
            return;
        }
        let current = self.slot(addr);
        match (self.cursor, current) {
            (None, None) => {}
            (None, Some(slot_stack)) => {
                if let Some(target) = self.slot_mut(addr) {
                    *target = None;
                    self.cursor = Some(slot_stack);
                }
            }
            (Some(cursor_stack), None) => {
                if let Some(target) = self.slot_mut(addr) {
                    *target = Some(cursor_stack);
                    self.cursor = None;
                }
            }
            (Some(mut cursor_stack), Some(mut slot_stack)) => {
                if cursor_stack.item == slot_stack.item {
                    let max = self.stack_limit(slot_stack.item);
                    if slot_stack.count < max {
                        let transfer = cursor_stack.count.min(max - slot_stack.count);
                        slot_stack.count += transfer;
                        cursor_stack.count -= transfer;
                        if let Some(target) = self.slot_mut(addr) {
                            *target = Some(slot_stack);
                        }
                        self.cursor = if cursor_stack.count > 0 {
                            Some(cursor_stack)
                        } else {
                            None
                        };
                    }
                } else if let Some(target) = self.slot_mut(addr) {
                    *target = Some(cursor_stack);
                    self.cursor = Some(slot_stack);
                }
            }
        }
    }

    /// Right-click interaction on `addr`: half-stack pickup when cursor is empty, or place 1 item when holding a stack.
    pub fn right_click_slot(&mut self, addr: SlotAddr) {
        let current = self.slot(addr);
        match (self.cursor, current) {
            (None, None) => {}
            (None, Some(mut slot_stack)) => {
                let take = slot_stack.count.div_ceil(2);
                slot_stack.count -= take;
                if let Some(target) = self.slot_mut(addr) {
                    *target = if slot_stack.count > 0 {
                        Some(slot_stack)
                    } else {
                        None
                    };
                }
                self.cursor = Some(ItemStack {
                    item: slot_stack.item,
                    count: take,
                });
            }
            (Some(mut cursor_stack), None) => {
                if !self.slot_accepts(addr, cursor_stack.item) {
                    return;
                }
                if let Some(target) = self.slot_mut(addr) {
                    *target = Some(ItemStack {
                        item: cursor_stack.item,
                        count: 1,
                    });
                    cursor_stack.count -= 1;
                    self.cursor = if cursor_stack.count > 0 {
                        Some(cursor_stack)
                    } else {
                        None
                    };
                }
            }
            (Some(mut cursor_stack), Some(mut slot_stack)) => {
                if cursor_stack.item == slot_stack.item && slot_stack.count < self.stack_limit(slot_stack.item)
                {
                    slot_stack.count += 1;
                    cursor_stack.count -= 1;
                    if let Some(target) = self.slot_mut(addr) {
                        *target = Some(slot_stack);
                    }
                    self.cursor = if cursor_stack.count > 0 {
                        Some(cursor_stack)
                    } else {
                        None
                    };
                }
            }
        }
    }

    /// Shift-click quick transfer between hotbar, backpack, equipment, and crafting grid.
    /// Preserves exact item counts; if target region is full, retains remaining items in the source slot.
    pub fn shift_click_slot(&mut self, addr: SlotAddr) -> bool {
        let Some(mut stack) = self.slot(addr) else {
            return false;
        };

        // 1. If the item is armor and not already in Equipment, try moving to its matching Equipment slot first.
        if !matches!(addr, SlotAddr::Equipment(_)) {
            if let Some(equip_slot) = stack.item.equip_slot() {
                let equip_idx = equip_slot as usize;
                if self.equipment[equip_idx].is_none() {
                    self.equipment[equip_idx] = Some(ItemStack {
                        item: stack.item,
                        count: 1,
                    });
                    stack.count -= 1;
                    if let Some(src) = self.slot_mut(addr) {
                        *src = if stack.count > 0 { Some(stack) } else { None };
                    }
                    return true;
                }
            }
        }

        let original_count = stack.count;
        let max = self.stack_limit(stack.item);

        match addr {
            SlotAddr::Hotbar(_) => {
                // Move from Hotbar -> Pack (merge first, then empty).
                for dst in &mut self.pack {
                    if stack.count == 0 {
                        break;
                    }
                    if let Some(existing) = dst {
                        if existing.item == stack.item && existing.count < max {
                            let add = stack.count.min(max - existing.count);
                            existing.count += add;
                            stack.count -= add;
                        }
                    }
                }
                for dst in &mut self.pack {
                    if stack.count == 0 {
                        break;
                    }
                    if dst.is_none() {
                        let add = stack.count.min(max);
                        *dst = Some(ItemStack {
                            item: stack.item,
                            count: add,
                        });
                        stack.count -= add;
                    }
                }
            }
            SlotAddr::Pack(_) => {
                // Move from Pack -> Hotbar (merge first, then empty).
                for dst in &mut self.hotbar {
                    if stack.count == 0 {
                        break;
                    }
                    if let Some(existing) = dst {
                        if existing.item == stack.item && existing.count < max {
                            let add = stack.count.min(max - existing.count);
                            existing.count += add;
                            stack.count -= add;
                        }
                    }
                }
                for dst in &mut self.hotbar {
                    if stack.count == 0 {
                        break;
                    }
                    if dst.is_none() {
                        let add = stack.count.min(max);
                        *dst = Some(ItemStack {
                            item: stack.item,
                            count: add,
                        });
                        stack.count -= add;
                    }
                }
            }
            SlotAddr::Equipment(_) | SlotAddr::CraftGrid(_) => {
                let (_, rem) = self.try_insert_pack_first(stack);
                stack.count = rem;
            }
        }

        let moved = stack.count < original_count;
        if let Some(src) = self.slot_mut(addr) {
            *src = if stack.count > 0 { Some(stack) } else { None };
        }
        moved
    }

    /// Swaps a hovered slot with `hotbar[hotbar_idx]` when pressing keys `1–9`.
    pub fn swap_slot_with_hotbar(&mut self, addr: SlotAddr, hotbar_idx: usize) {
        if hotbar_idx >= HOTBAR_SLOTS || addr == SlotAddr::Hotbar(hotbar_idx) {
            return;
        }
        let a = self.slot(addr);
        let b = self.hotbar[hotbar_idx];
        if let Some(stack_b) = b {
            if !self.slot_accepts(addr, stack_b.item) {
                return;
            }
        }
        if let Some(target) = self.slot_mut(addr) {
            *target = b;
            self.hotbar[hotbar_idx] = a;
        }
    }

    /// Double-click gather (Minecraft / Mouse Tweaks): pulls loose partial stacks of the held
    /// cursor item (or the item in `addr` if cursor is empty) into `self.cursor` up to its stack limit.
    pub fn gather_to_cursor(&mut self, addr: SlotAddr) {
        if self.cursor.is_none() {
            if let Some(stack) = self.slot(addr) {
                if let Some(slot) = self.slot_mut(addr) {
                    *slot = None;
                }
                self.cursor = Some(stack);
            }
        }
        let Some(mut cursor) = self.cursor else {
            return;
        };
        let max = self.stack_limit(cursor.item);
        if cursor.count >= max {
            return;
        }
        // Pull from partial stacks first so full stacks remain undisturbed unless needed.
        for partial_only in [true, false] {
            for slot in self
                .craft_grid
                .iter_mut()
                .chain(self.pack.iter_mut())
                .chain(self.hotbar.iter_mut())
            {
                if cursor.count >= max {
                    break;
                }
                if let Some(st) = slot {
                    if st.item == cursor.item && (!partial_only || st.count < max) {
                        let take = (max - cursor.count).min(st.count);
                        st.count -= take;
                        cursor.count += take;
                        if st.count == 0 {
                            *slot = None;
                        }
                    }
                }
            }
        }
        self.cursor = Some(cursor);
    }

    /// Mouse Tweaks wheel interaction on `addr`:
    /// - `delta > 0` (scroll up): push 1 item from `addr` into the opposite inventory region (or cursor).
    /// - `delta < 0` (scroll down): pull 1 matching item into `addr` from the pack/hotbar (or cursor).
    pub fn wheel_slot(&mut self, addr: SlotAddr, delta: i32) {
        if delta == 0 {
            return;
        }
        if self.cursor.is_some() {
            if delta < 0 {
                // Deposit 1 item from cursor into slot
                if self
                    .cursor
                    .is_some_and(|c| self.slot(addr).is_none_or(|s| s.item == c.item))
                {
                    self.right_click_slot(addr);
                }
            } else if let (Some(mut cursor), Some(mut slot_stack)) = (self.cursor, self.slot(addr))
            {
                // Pull 1 item from slot onto cursor
                let max = self.stack_limit(cursor.item);
                if cursor.item == slot_stack.item && cursor.count < max {
                    cursor.count += 1;
                    slot_stack.count -= 1;
                    if let Some(target) = self.slot_mut(addr) {
                        *target = if slot_stack.count > 0 {
                            Some(slot_stack)
                        } else {
                            None
                        };
                    }
                    self.cursor = Some(cursor);
                }
            }
            return;
        }

        if delta > 0 {
            // Scroll up: push 1 item from `addr` to opposite container
            let Some(mut src) = self.slot(addr) else {
                return;
            };
            let one = ItemStack {
                item: src.item,
                count: 1,
            };
            let max = self.stack_limit(src.item);
            let mut moved = false;
            match addr {
                SlotAddr::Hotbar(_) => {
                    for dst in &mut self.pack {
                        if let Some(ex) = dst {
                            if ex.item == one.item && ex.count < max {
                                ex.count += 1;
                                moved = true;
                                break;
                            }
                        }
                    }
                    if !moved {
                        for dst in &mut self.pack {
                            if dst.is_none() {
                                *dst = Some(one);
                                moved = true;
                                break;
                            }
                        }
                    }
                }
                _ => {
                    for dst in &mut self.hotbar {
                        if let Some(ex) = dst {
                            if ex.item == one.item && ex.count < max {
                                ex.count += 1;
                                moved = true;
                                break;
                            }
                        }
                    }
                    if !moved {
                        for dst in &mut self.hotbar {
                            if dst.is_none() {
                                *dst = Some(one);
                                moved = true;
                                break;
                            }
                        }
                    }
                }
            }
            if moved {
                src.count -= 1;
                if let Some(target) = self.slot_mut(addr) {
                    *target = if src.count > 0 { Some(src) } else { None };
                }
            }
        } else {
            // Scroll down: pull 1 matching item into `addr`
            let Some(mut dst_stack) = self.slot(addr) else {
                return;
            };
            let max = self.stack_limit(dst_stack.item);
            if dst_stack.count >= max {
                return;
            }
            let target_item = dst_stack.item;
            let mut found = false;
            match addr {
                SlotAddr::Hotbar(_) => {
                    for src in &mut self.pack {
                        if let Some(st) = src {
                            if st.item == target_item {
                                st.count -= 1;
                                if st.count == 0 {
                                    *src = None;
                                }
                                found = true;
                                break;
                            }
                        }
                    }
                }
                _ => {
                    for src in self.pack.iter_mut().chain(self.hotbar.iter_mut()) {
                        if let Some(st) = src {
                            if st.item == target_item {
                                st.count -= 1;
                                if st.count == 0 {
                                    *src = None;
                                }
                                found = true;
                                break;
                            }
                        }
                    }
                }
            }
            if found {
                dst_stack.count += 1;
                if let Some(target) = self.slot_mut(addr) {
                    *target = Some(dst_stack);
                }
            }
        }
    }

    /// Inventory Profiles Next style sort & compact for the 27-slot backpack:
    /// merges partial stacks and sorts by category (Tools/Weapons/Armor -> Food -> Resources -> Blocks) and count.
    pub fn sort_pack(&mut self) {
        let mut combined: Vec<ItemStack> = Vec::new();
        for slot in &mut self.pack {
            if let Some(st) = slot.take() {
                if let Some(existing) = combined.iter_mut().find(|e| e.item == st.item) {
                    existing.count = existing.count.saturating_add(st.count);
                } else {
                    combined.push(st);
                }
            }
        }
        let sort_key = |item: Item| -> (u8, u32) {
            let (kind, payload) = item.encode_u32();
            let order = match item {
                Item::Weapon(_) | Item::Tool(_) => 0,
                Item::Armor(_, _) => 1,
                Item::Food(_) => 2,
                Item::Resource(_) => 3,
                Item::Block(_) => 4,
            };
            (order, (kind << 16) | (payload & 0xFFFF))
        };
        combined.sort_by(|a, b| {
            sort_key(a.item)
                .cmp(&sort_key(b.item))
                .then_with(|| b.count.cmp(&a.count))
        });

        let mut idx = 0usize;
        for mut st in combined {
            let max = self.stack_limit(st.item);
            while st.count > 0 && idx < PACK_SLOTS {
                let take = st.count.min(max);
                self.pack[idx] = Some(ItemStack {
                    item: st.item,
                    count: take,
                });
                st.count -= take;
                idx += 1;
            }
        }
    }

    /// Quick-Stacks backpack items into any existing matching partial stacks in the hotbar,
    /// then compacts the backpack.
    pub fn quick_stack_to_hotbar(&mut self) {
        for p_idx in 0..PACK_SLOTS {
            let Some(mut pack_st) = self.pack[p_idx] else {
                continue;
            };
            let max = self.stack_limit(pack_st.item);
            for h_slot in &mut self.hotbar {
                if pack_st.count == 0 {
                    break;
                }
                if let Some(hot_st) = h_slot {
                    if hot_st.item == pack_st.item && hot_st.count < max {
                        let add = pack_st.count.min(max - hot_st.count);
                        hot_st.count += add;
                        pack_st.count -= add;
                    }
                }
            }
            self.pack[p_idx] = if pack_st.count > 0 {
                Some(pack_st)
            } else {
                None
            };
        }
        self.sort_pack();
    }

    /// Returns any held cursor stack or 2×2 crafting grid stacks back into the pack/hotbar when closing UI.
    /// If pack/hotbar are full, keeps remainder in `craft_grid` or `cursor` so zero items are ever lost.
    pub fn return_transient_to_pack(&mut self) {
        if let Some(cursor) = self.cursor.take() {
            let (_, rem) = self.try_insert(cursor);
            if rem > 0 {
                for slot in &mut self.craft_grid {
                    if slot.is_none() {
                        *slot = Some(ItemStack {
                            item: cursor.item,
                            count: rem,
                        });
                        return;
                    }
                }
                self.cursor = Some(ItemStack {
                    item: cursor.item,
                    count: rem,
                });
            }
        }
    }

    /// Evaluates the 2×2 crafting grid and returns the matching recipe and how many batches the grid can produce.
    pub fn matching_grid_recipe(&self) -> Option<(&'static CraftingRecipe, u32)> {
        let mut present: Vec<(Item, u32)> = Vec::new();
        for stack in self.craft_grid.iter().flatten() {
            if let Some(entry) = present.iter_mut().find(|(i, _)| *i == stack.item) {
                entry.1 += stack.count;
            } else {
                present.push((stack.item, stack.count));
            }
        }
        if present.is_empty() {
            return None;
        }
        for recipe in HAND_RECIPES {
            if recipe.ingredients.len() != present.len() {
                continue;
            }
            let mut batches = u32::MAX;
            let mut all_match = true;
            for &(req_item, req_count) in recipe.ingredients {
                if let Some(&(_, have_count)) = present.iter().find(|(i, _)| *i == req_item) {
                    if have_count < req_count {
                        all_match = false;
                        break;
                    }
                    batches = batches.min(have_count / req_count);
                } else {
                    all_match = false;
                    break;
                }
            }
            if all_match && batches > 0 && batches != u32::MAX {
                return Some((recipe, batches));
            }
        }
        None
    }

    /// Crafts 1 batch from `craft_grid` onto `self.cursor` (or into inventory if `shift_to_pack`).
    /// Never consumes ingredients unless the output is guaranteed to fit without loss.
    pub fn craft_from_grid(&mut self, shift_to_pack: bool) -> Option<ItemStack> {
        let (recipe, max_batches) = self.matching_grid_recipe()?;
        let out = recipe.output;
        let max_stack = self.stack_limit(out.item);

        let batches_to_craft = if shift_to_pack {
            let cap = self.remaining_capacity_for(out.item);
            (cap / out.count).min(max_batches)
        } else {
            match self.cursor {
                None => 1,
                Some(cursor)
                    if cursor.item == out.item && cursor.count + out.count <= max_stack =>
                {
                    1
                }
                _ => 0,
            }
        };

        if batches_to_craft == 0 {
            return None;
        }

        // Deduct ingredients from craft_grid.
        for &(req_item, req_count) in recipe.ingredients {
            let mut needed = req_count * batches_to_craft;
            for slot in &mut self.craft_grid {
                if needed == 0 {
                    break;
                }
                if let Some(stack) = slot {
                    if stack.item == req_item {
                        let take = needed.min(stack.count);
                        stack.count -= take;
                        needed -= take;
                        if stack.count == 0 {
                            *slot = None;
                        }
                    }
                }
            }
            debug_assert_eq!(needed, 0);
        }

        let produced = ItemStack {
            item: out.item,
            count: out.count * batches_to_craft,
        };
        if shift_to_pack {
            let mut rem = produced.count;
            while rem > 0 {
                let chunk = rem.min(max_stack);
                let (ins, leftover) = self.try_insert_pack_first(ItemStack {
                    item: out.item,
                    count: chunk,
                });
                debug_assert_eq!(leftover, 0);
                debug_assert_eq!(ins, chunk);
                rem -= chunk;
            }
        } else {
            match &mut self.cursor {
                None => self.cursor = Some(produced),
                Some(cursor) => cursor.count += produced.count,
            }
        }
        Some(produced)
    }

    /// Stages 1 batch of `recipe` from the player's inventory into the 2×2 `craft_grid`.
    /// Returns `true` if all ingredients were available and staged.
    pub fn stage_recipe_into_grid(&mut self, recipe: &CraftingRecipe) -> bool {
        // First return any existing craft_grid items to pack/hotbar if they fit.
        for i in 0..CRAFT_SLOTS {
            if let Some(stack) = self.craft_grid[i] {
                if !self.can_insert(stack) {
                    return false;
                }
            }
        }
        for i in 0..CRAFT_SLOTS {
            if let Some(stack) = self.craft_grid[i].take() {
                let (_, rem) = self.try_insert_pack_first(stack);
                if rem > 0 {
                    self.craft_grid[i] = Some(ItemStack {
                        item: stack.item,
                        count: rem,
                    });
                    return false;
                }
            }
        }

        // Check if player owns all ingredients for 1 batch (or if already staged in grid).
        if !recipe
            .ingredients
            .iter()
            .all(|&(item, count)| self.count_item(item) >= count)
        {
            return false;
        }

        // Pull ingredients into craft_grid slots.
        let mut slot_idx = 0usize;
        for &(item, count) in recipe.ingredients {
            if !self.remove_item(item, count) {
                return false;
            }
            // If an ingredient is 4 planks or 4 stone, spread across the 2×2 grid nicely if single-ingredient 4-count.
            if recipe.ingredients.len() == 1 && count == 4 && item.max_stack() >= 1 {
                for s in 0..4 {
                    self.craft_grid[s] = Some(ItemStack { item, count: 1 });
                }
                slot_idx = 4;
            } else if slot_idx < CRAFT_SLOTS {
                self.craft_grid[slot_idx] = Some(ItemStack { item, count });
                slot_idx += 1;
            }
        }
        true
    }

    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        let encode_slot = |slot: Option<ItemStack>, put: &mut dyn FnMut(u32)| {
            if let Some(stack) = slot {
                let (kind, payload) = stack.item.encode_u32();
                put(kind);
                put(payload);
                put(stack.count);
            } else {
                put(0);
                put(0);
                put(0);
            }
        };
        for slot in self.hotbar {
            encode_slot(slot, &mut put);
        }
        for slot in self.pack {
            encode_slot(slot, &mut put);
        }
        for slot in self.equipment {
            encode_slot(slot, &mut put);
        }
        for slot in self.craft_grid {
            encode_slot(slot, &mut put);
        }
        encode_slot(self.cursor, &mut put);
        for rem in self.resource_remainders {
            put(rem.min(63));
        }
    }

    pub fn decode(read: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Self> {
        Self::decode_at_scale(read, 1.0)
    }
    pub fn decode_at_scale(read: &mut impl FnMut() -> anyhow::Result<u32>, voxel_m: f32) -> anyhow::Result<Self> {
        use anyhow::ensure;
        let mut inv = Self::empty();
        inv.voxel_m = voxel_m;
        let decode_slot = |read: &mut dyn FnMut() -> anyhow::Result<u32>| -> anyhow::Result<Option<ItemStack>> {
            let kind = read()?;
            let payload = read()?;
            let count = read()?;
            if kind == 0 {
                ensure!(payload == 0 && count == 0, "Non-zero empty survival slot");
                return Ok(None);
            }
            let item = Item::decode_u32(kind, payload)?;
            ensure!(
                (1..=Self::limit_at_scale(item, voxel_m)).contains(&count),
                "Survival stack count {count} exceeds limit {} for {:?}",
                item.max_stack(),
                item
            );
            Ok(Some(ItemStack { item, count }))
        };

        for slot in &mut inv.hotbar {
            *slot = decode_slot(read)?;
        }
        for slot in &mut inv.pack {
            *slot = decode_slot(read)?;
        }
        for i in 0..inv.equipment.len() {
            let decoded = decode_slot(read)?;
            if let Some(stack) = decoded {
                ensure!(
                    inv.slot_accepts(SlotAddr::Equipment(i), stack.item),
                    "Invalid item in survival equipment slot {i}"
                );
            }
            inv.equipment[i] = decoded;
        }
        for slot in &mut inv.craft_grid {
            *slot = decode_slot(read)?;
        }
        inv.cursor = decode_slot(read)?;
        for rem in &mut inv.resource_remainders {
            let v = read()?;
            ensure!(v < 64, "Invalid survival resource remainder");
            *rem = v;
        }
        Ok(inv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_stacks_survive_transfers_save_and_retrieval() {
        let item = Item::Block(BlockId::Stone);
        for (voxel_m, capacity) in [(1.0,64), (0.5,512), (0.25,4096)] {
            let mut inv = SurvivalInventory::empty();
            inv.voxel_m = voxel_m;
            assert_eq!(inv.stack_limit(item), capacity);
            assert_eq!(inv.try_insert(ItemStack {item,count:capacity}), (capacity,0));
            assert_eq!(inv.hotbar.iter().flatten().count(), 1);
            inv.shift_click_slot(SlotAddr::Hotbar(0));
            assert_eq!(inv.pack[0].unwrap().count,capacity);
            inv.right_click_slot(SlotAddr::Pack(0));
            inv.click_slot(SlotAddr::Pack(0));
            assert_eq!(inv.pack[0].unwrap().count,capacity);
            let mut bytes=Vec::new(); inv.encode(&mut bytes);
            let mut words=bytes.chunks_exact(4);
            let mut restored=SurvivalInventory::decode_at_scale(&mut || Ok(u32::from_le_bytes(words.next().unwrap().try_into().unwrap())),voxel_m).unwrap();
            let stack=restored.pack[0].take().unwrap();
            restored.dropped.push(DroppedStack {pos:glam::Vec3::ZERO, stack});
            assert!(!restored.retrieve_nearby(glam::Vec3::splat(5.0)));
            assert!(restored.retrieve_nearby(glam::Vec3::ZERO));
            assert_eq!(restored.count_item(item),capacity);
            assert!(restored.dropped.is_empty());
        }
    }

    #[test]
    fn stack_insertion_merges_partials_and_clamps_overflow() {
        let mut inv = SurvivalInventory::empty();
        inv.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Stone), 50));
        let (inserted, rem) = inv.try_insert(ItemStack::new(Item::Block(BlockId::Stone), 30));
        assert_eq!(inserted, 30);
        assert_eq!(rem, 0);
        assert_eq!(inv.hotbar[0].unwrap().count, 64);
        assert_eq!(inv.hotbar[1].unwrap().count, 16);

        // Fill all remaining slots to test overflow protection.
        for slot in inv.hotbar.iter_mut().chain(inv.pack.iter_mut()) {
            *slot = Some(ItemStack::new(Item::Block(BlockId::Dirt), 64));
        }
        inv.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Stone), 60));
        let (inserted, rem) = inv.try_insert(ItemStack::new(Item::Block(BlockId::Stone), 10));
        assert_eq!(inserted, 4);
        assert_eq!(rem, 6);
        assert_eq!(inv.hotbar[0].unwrap().count, 64);
    }

    #[test]
    fn click_split_and_shift_transfer_preserve_item_counts() {
        let mut inv = SurvivalInventory::empty();
        inv.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Log), 15));

        // Right-click picks up half (8), leaving 7.
        inv.right_click_slot(SlotAddr::Hotbar(0));
        assert_eq!(inv.cursor.unwrap().count, 8);
        assert_eq!(inv.hotbar[0].unwrap().count, 7);

        // Place cursor into Pack(0), then right-click Pack(1) after picking up again.
        inv.click_slot(SlotAddr::Pack(0));
        assert!(inv.cursor.is_none());
        assert_eq!(inv.pack[0].unwrap().count, 8);

        // Shift-click Pack(0) merges back into Hotbar(0) -> 7 + 8 = 15.
        assert!(inv.shift_click_slot(SlotAddr::Pack(0)));
        assert!(inv.pack[0].is_none());
        assert_eq!(inv.hotbar[0].unwrap().count, 15);
    }

    #[test]
    fn armor_shift_clicks_to_matching_equipment_slot() {
        let mut inv = SurvivalInventory::empty();
        let helm = Item::Armor(EquipSlot::Head, ArmorTier::Copper);
        inv.pack[3] = Some(ItemStack::new(helm, 1));
        assert!(inv.shift_click_slot(SlotAddr::Pack(3)));
        assert!(inv.pack[3].is_none());
        assert_eq!(inv.equipment[0].unwrap().item, helm);
        assert!(inv.armor_reduction() > 0.0);
    }

    #[test]
    fn crafting_2x2_consumes_exact_ingredients_and_prevents_loss() {
        let mut inv = SurvivalInventory::empty();
        inv.craft_grid[0] = Some(ItemStack::new(Item::Block(BlockId::Log), 2));
        let (recipe, batches) = inv.matching_grid_recipe().expect("Log should match planks");
        assert_eq!(recipe.output.item, Item::Block(BlockId::Planks));
        assert_eq!(batches, 2);

        let crafted = inv.craft_from_grid(false).expect("Should craft 1 batch to cursor");
        assert_eq!(crafted.count, 4);
        assert_eq!(inv.cursor.unwrap().count, 4);
        assert_eq!(inv.craft_grid[0].unwrap().count, 1);
    }

    #[test]
    fn encode_decode_roundtrip_preserves_inventory() {
        let mut inv = SurvivalInventory::new_starter();
        inv.pack[5] = Some(ItemStack::new(Item::Resource(6), 42));
        inv.equipment[0] = Some(ItemStack::new(
            Item::Armor(EquipSlot::Head, ArmorTier::Lumen),
            1,
        ));
        inv.resource_remainders[0] = 24;

        let mut bytes = Vec::new();
        inv.encode(&mut bytes);
        let mut offset = 0usize;
        let decoded = SurvivalInventory::decode(&mut || {
            let word = u32::from_le_bytes(bytes[offset..offset + 4].try_into()?);
            offset += 4;
            Ok(word)
        })
        .expect("decode should succeed");

        assert_eq!(decoded.hotbar, inv.hotbar);
        assert_eq!(decoded.pack, inv.pack);
        assert_eq!(decoded.equipment, inv.equipment);
        assert_eq!(decoded.resource_remainders, inv.resource_remainders);
    }

    #[test]
    fn mouse_tweaks_gather_wheel_sort_and_quick_stack() {
        let mut inv = SurvivalInventory::empty();
        let stone = Item::Block(BlockId::Stone);
        let wood = Item::Block(BlockId::Log);
        inv.pack[0] = Some(ItemStack::new(stone, 12));
        inv.pack[5] = Some(ItemStack::new(stone, 18));
        inv.pack[9] = Some(ItemStack::new(wood, 7));

        // Double-click gather collects partial stone stacks into cursor (12 + 18 = 30)
        inv.gather_to_cursor(SlotAddr::Pack(0));
        assert_eq!(inv.cursor, Some(ItemStack::new(stone, 30)));
        assert!(inv.pack[0].is_none());
        assert!(inv.pack[5].is_none());

        // Mouse wheel down (-1) deposits 1 item from cursor into Pack(0); up (+1) pulls 1 back
        inv.wheel_slot(SlotAddr::Pack(0), -1);
        assert_eq!(inv.pack[0], Some(ItemStack::new(stone, 1)));
        assert_eq!(inv.cursor, Some(ItemStack::new(stone, 29)));
        inv.wheel_slot(SlotAddr::Pack(0), 1);
        assert!(inv.pack[0].is_none());
        assert_eq!(inv.cursor, Some(ItemStack::new(stone, 30)));

        // Place cursor into Pack(12) and test sort_pack + quick_stack_to_hotbar
        inv.click_slot(SlotAddr::Pack(12));
        inv.pack[20] = Some(ItemStack::new(stone, 10));
        inv.sort_pack();
        // Pack should now be compacted into two slots (stone=40, wood=7)
        assert_eq!(inv.count_item(stone), 40);
        assert_eq!(inv.count_item(wood), 7);
        assert!(inv.pack[0].is_some());
        assert!(inv.pack[1].is_some());
        assert!(inv.pack[2].is_none());

        // Quick-stack merges matching pack items into existing hotbar partial stacks
        inv.hotbar[2] = Some(ItemStack::new(stone, 10));
        inv.quick_stack_to_hotbar();
        assert_eq!(inv.hotbar[2], Some(ItemStack::new(stone, 50)));
        assert_eq!(inv.count_item(stone), 50);
    }
}
