//! Persistent survival inventory: item stacks with stack limits, 9-slot hotbar,
//! 27-slot backpack, 4 equipment slots, 2×2 hand-crafting grid, and lossless transfers.
use super::{BlockId, expedition, factory};
mod crafting;
mod descriptions;
pub mod food;
pub mod resources;
pub mod sorting;
pub use crafting::CraftBlocker;

pub const HOTBAR_SLOTS: usize = 9;
pub const PACK_SLOTS: usize = 27;
pub const EQUIP_SLOTS: usize = 4;
/// The crafting grid is 3 × 3. Away from a workbench only the top-left 2 × 2
/// (slots 0, 1, 3, 4) is usable, as in Minecraft's inventory.
pub const CRAFT_SLOTS: usize = 9;
pub const HAND_GRID: [usize; 4] = [0, 1, 3, 4];

/// Base melee damage, shared by combat and item stats before modifiers.
pub fn weapon_damage(tier: u32) -> f32 {
    match tier {
        1 => 22.0,
        2 => 32.0,
        3 => 38.0,
        4 => 28.0,
        5 => 44.0,
        _ => 14.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum FoodKind {
    FungalRation = 0,
    RoastedCap = 1,
    Trailcake = 2,
    RawMutton = 3,
    CookedMutton = 4,
}

impl FoodKind {
    pub const ALL: [Self; 5] = [Self::FungalRation, Self::RoastedCap, Self::Trailcake, Self::RawMutton, Self::CookedMutton];

    pub fn from_u8(raw: u8) -> Option<Self> {
        Self::ALL.get(raw as usize).copied()
    }

    pub fn name(self) -> &'static str {
        food::get(self).map_or("Food", |f| f.name.as_str())
    }

    pub fn hunger_restore(self) -> f32 {
        food::get(self).map_or(0.0, |f| f.hunger)
    }

    pub fn health_restore(self) -> f32 {
        food::get(self).map_or(0.0, |f| f.health)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Item {
    Block(BlockId),
    PowerKit(factory::portable::PowerKit),
    Resource(u8),
    Food(FoodKind),
    Weapon(u8),
    Armor(EquipSlot, ArmorTier),
    Tool(u8),
    /// A wand in the adventure's wand bank; `look` caches its icon (tier, shaft, head, glow).
    Wand { id: u16, look: u16 },
}

impl Item {
    pub fn placeable_block(self) -> Option<BlockId> {
        match self {
            Self::Block(b) => Some(b),
            Self::PowerKit(k) => Some(k.kind.block()),
            _ => None,
        }
    }
    pub fn canonical_block(block: BlockId) -> Option<BlockId> {
        if block == BlockId::Air
            || block.is_liquid()
            || matches!(block, BlockId::Fire | BlockId::Smoke | BlockId::Steam | BlockId::SnowLayer)
        {
            return None;
        }
        if let Some(b) = engine_content::fittings::canonical(block) { return Some(b); }
        if let Some(b) = engine_content::thin::canonical(block) { return Some(b); }
        if block.is_torch() {
            Some(BlockId::Torch)
        } else {
            Some(block)
        }
    }

    pub fn max_stack(self) -> u32 {
        match self {
            Self::PowerKit(_) => 1,
            Self::Weapon(_) | Self::Armor(_, _) | Self::Tool(_) | Self::Wand { .. } => 1,
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
                        BlockId::ItemPipe
                            | BlockId::PowerCable
                            | BlockId::FluidPipe
                            | BlockId::CokeOvenBrick
                            | BlockId::BlastBrick
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
            Self::PowerKit(k) => k.kind.block().display_name(),
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
            Self::Weapon(5) => "Kinetic maul",
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
            Self::Wand { look, .. } if look & 1 << 11 != 0 => "Crafted wand",
            Self::Wand { look, .. } => match look & 3 {
                1 => "Found wand",
                2 => "Found wand · fine",
                _ => "Found wand · masterwork",
            },
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            Self::PowerKit(_) => "Packed power assembly",
            Self::Block(b) if b.is_torch() => "Lighting block",
            Self::Block(b) if factory::Kind::from_block(b).is_some() => "Machine",
            Self::Block(BlockId::Workbench | BlockId::Smelter) => "Crafting station",
            Self::Block(_) => "Building block",
            Self::Resource(0..=5 | 13) => "Raw material",
            Self::Resource(_) => "Refined component",
            Self::Food(_) => "Food",
            Self::Weapon(_) => "Melee weapon",
            Self::Armor(slot, _) => match slot {
                EquipSlot::Head => "Equipment · Head",
                EquipSlot::Chest => "Equipment · Chest",
                EquipSlot::Legs => "Equipment · Legs",
                EquipSlot::Feet => "Equipment · Feet",
            },
            Self::Tool(1 | 2) => "Excavation tool",
            Self::Tool(_) => "Arcane focus",
            Self::Wand { .. } => "Wand",
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
            Self::PowerKit(k) => k.encode(),
            Self::Block(b) => (1, b.as_u32()),
            Self::Resource(i) => (2, i as u32),
            Self::Food(f) => (3, f as u32),
            Self::Weapon(w) => (4, w as u32),
            Self::Armor(slot, tier) => (5, ((slot as u32) << 8) | (tier as u32)),
            Self::Tool(t) => (6, t as u32),
            Self::Wand { id, look } => (8, id as u32 | (look as u32) << 16),
        }
    }

    pub fn decode_u32(kind: u32, payload: u32) -> anyhow::Result<Self> {
        use anyhow::ensure;
        if kind & 0xff == 7 {
            return Ok(Self::PowerKit(factory::portable::PowerKit::decode(kind, payload)?));
        }
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
                ensure!(
                    (1..=5).contains(&payload),
                    "Invalid weapon tier in survival inventory"
                );
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
                ensure!(
                    (1..=5).contains(&payload),
                    "Invalid tool item in survival inventory"
                );
                Self::Tool(payload as u8)
            }
            8 => {
                let look = (payload >> 16) as u16;
                ensure!(look & 3 != 0 && look >> 12 == 0, "Invalid wand item in survival inventory");
                Self::Wand { id: payload as u16, look }
            }
            _ => anyhow::bail!("Unknown survival item kind {kind}"),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CraftingRecipe {
    pub name: &'static str,
    pub output: ItemStack,
    /// Grid rows of `key` characters, `' '` for an empty cell. Empty for
    /// shapeless rows, which match by summed counts anywhere in the grid.
    pub pattern: &'static [&'static str],
    pub key: &'static [(char, Item)],
    /// One craft's total inputs. Derived from the pattern for shaped rows.
    pub ingredients: &'static [(Item, u32)],
    pub description: &'static str,
}

/// A shaped recipe laid out on the 3 × 3 grid, anchored top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub width: usize,
    pub height: usize,
    pub cells: [Option<Item>; CRAFT_SLOTS],
}

impl CraftingRecipe {
    pub fn shaped(&self) -> bool {
        !self.pattern.is_empty()
    }

    pub fn shape(&self) -> Option<Shape> {
        if !self.shaped() {
            return None;
        }
        let mut cells = [None; CRAFT_SLOTS];
        for (r, row) in self.pattern.iter().enumerate().take(3) {
            for (c, ch) in row.chars().enumerate().take(3) {
                cells[r * 3 + c] = self.key.iter().find(|(k, _)| *k == ch).map(|&(_, item)| item);
            }
        }
        let width = self.pattern.iter().map(|row| row.chars().count()).max().unwrap_or(0);
        Some(Shape { width, height: self.pattern.len(), cells })
    }

    /// Wider or taller than 2 × 2, machine kits, and rows of the bench table
    /// need the 3 × 3 grid of a workbench.
    pub fn needs_bench(&self) -> bool {
        if let Some(shape) = self.shape() {
            return shape.width > 2 || shape.height > 2 || RECIPES.bench.contains(self);
        }
        let machine = matches!(self.output.item, Item::Block(b) if factory::Kind::from_block(b).is_some());
        let slots: u32 = self.ingredients.iter().map(|&(item, n)| n.div_ceil(item.max_stack())).sum();
        machine || self.ingredients.len() > 4 || slots > 4 || RECIPES.bench.contains(self)
    }
}

/// Owned form of a recipe as it sits in `assets/recipes.ron`. Converted once
/// into the `'static` `CraftingRecipe` rows the game uses.
#[derive(serde::Deserialize, serde::Serialize)]
struct RecipeRow {
    name: String,
    output: ItemStack,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pattern: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    key: Vec<(char, Item)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ingredients: Vec<(Item, u32)>,
    description: String,
}

impl From<RecipeRow> for CraftingRecipe {
    fn from(row: RecipeRow) -> Self {
        let mut ingredients = row.ingredients;
        if !row.pattern.is_empty() {
            for ch in row.pattern.iter().flat_map(|r| r.chars()).filter(|&ch| ch != ' ') {
                let Some(&(_, item)) = row.key.iter().find(|(k, _)| *k == ch) else {
                    panic!("assets/recipes.ron: {}: pattern key '{ch}' is not defined", row.name);
                };
                match ingredients.iter_mut().find(|(i, _)| *i == item) {
                    Some(entry) => entry.1 += 1,
                    None => ingredients.push((item, 1)),
                }
            }
        }
        let pattern: Vec<&'static str> =
            row.pattern.into_iter().map(|r| &*Box::leak(r.into_boxed_str())).collect();
        CraftingRecipe {
            name: Box::leak(row.name.into_boxed_str()),
            output: row.output,
            pattern: Box::leak(pattern.into_boxed_slice()),
            key: Box::leak(row.key.into_boxed_slice()),
            ingredients: Box::leak(ingredients.into_boxed_slice()),
            description: Box::leak(row.description.into_boxed_str()),
        }
    }
}

impl From<&CraftingRecipe> for RecipeRow {
    fn from(recipe: &CraftingRecipe) -> Self {
        RecipeRow {
            name: recipe.name.into(),
            output: recipe.output,
            pattern: recipe.pattern.iter().map(|r| (*r).into()).collect(),
            key: recipe.key.to_vec(),
            ingredients: if recipe.shaped() { Vec::new() } else { recipe.ingredients.to_vec() },
            description: recipe.description.into(),
        }
    }
}

/// Hand, workbench and crystal-tech recipes from `assets/recipes.ron`.
#[derive(Debug, PartialEq)]
struct RecipeTables {
    hand: Vec<CraftingRecipe>,
    bench: Vec<CraftingRecipe>,
    crystal_tech: Vec<CraftingRecipe>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct RecipeTableRows {
    hand: Vec<RecipeRow>,
    bench: Vec<RecipeRow>,
    crystal_tech: Vec<RecipeRow>,
}

impl RecipeTables {
    #[cfg(test)]
    fn rows(&self) -> RecipeTableRows {
        let rows = |v: &[CraftingRecipe]| v.iter().map(RecipeRow::from).collect();
        RecipeTableRows { hand: rows(&self.hand), bench: rows(&self.bench), crystal_tech: rows(&self.crystal_tech) }
    }
}

const RECIPES_RON: &str = include_str!("../../../../assets/recipes.ron");
static RECIPES: std::sync::LazyLock<RecipeTables> = std::sync::LazyLock::new(|| {
    let rows: RecipeTableRows =
        ron::from_str(RECIPES_RON).unwrap_or_else(|error| panic!("assets/recipes.ron: {error}"));
    let convert = |v: Vec<RecipeRow>| v.into_iter().map(CraftingRecipe::from).collect();
    RecipeTables { hand: convert(rows.hand), bench: convert(rows.bench), crystal_tech: convert(rows.crystal_tech) }
});
pub fn hand_recipes() -> &'static [CraftingRecipe] {
    &RECIPES.hand
}

pub fn all_recipes() -> &'static [CraftingRecipe] {
    static ALL: std::sync::LazyLock<Vec<CraftingRecipe>> = std::sync::LazyLock::new(|| {
        let mut all: Vec<CraftingRecipe> = RECIPES.hand.clone();
        all.extend_from_slice(&RECIPES.bench);
        for kind in factory::Kind::ALL {
            let cost = kind.recipe();
            if kind.structural() || cost.iter().any(|&(_, n)| n % 64 != 0) {
                continue;
            }
            let ingredients: Vec<(Item, u32)> = cost
                .iter()
                .map(|&(i, n)| (Item::Resource(i as u8), n / 64))
                .collect();
            all.push(CraftingRecipe {
                name: kind.name(),
                output: ItemStack {
                    item: Item::Block(kind.block()),
                    count: kind.yield_count(),
                },
                pattern: &[],
                key: &[],
                ingredients: Box::leak(ingredients.into_boxed_slice()),
                description: "Machine assembly. Right-click to place on the metre grid.",
            });
        }
        all.extend_from_slice(&RECIPES.crystal_tech);
        all
    });
    &ALL
}

#[derive(Clone, Debug)]
pub struct DroppedStack {
    pub pos: glam::Vec3,
    pub stack: ItemStack,
}
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

/// What a left-button release should do with the carried stack.
///
/// A press picks a stack up immediately. Releasing on that same slot leaves it
/// on the cursor. Releasing on one other slot places the whole stack. Dragging
/// across several slots while already carrying a stack splits it evenly.
/// Releasing away from every slot throws the carried stack into the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerRelease {
    Keep,
    Place(SlotAddr),
    Distribute,
    /// The pointer left the slot grid with a stack in hand. The UI only turns
    /// this into a world drop when the release is outside the inventory panel,
    /// so clicking the recipe book or a tab cannot accidentally discard items.
    Drop,
}

pub fn pointer_release(
    spreading: bool,
    picked_from: Option<SlotAddr>,
    pointer: Option<SlotAddr>,
    visited_len: usize,
) -> PointerRelease {
    if spreading {
        if visited_len > 1 {
            PointerRelease::Distribute
        } else if let Some(addr) = pointer {
            PointerRelease::Place(addr)
        } else if visited_len == 0 && picked_from.is_some() {
            PointerRelease::Drop
        } else {
            // The player already distributed part of the stack. Keep the rest
            // on the cursor rather than silently throwing it on release.
            PointerRelease::Keep
        }
    } else if let (Some(from), Some(addr)) = (picked_from, pointer) {
        if from != addr {
            PointerRelease::Place(addr)
        } else {
            PointerRelease::Keep
        }
    } else if picked_from.is_some() {
        PointerRelease::Drop
    } else {
        PointerRelease::Keep
    }
}

impl SurvivalInventory {
    pub fn retrieve_nearby(&mut self, pos: glam::Vec3) -> bool {
        let Some(index) = self.dropped.iter().position(|d| d.pos.distance(pos) < 3.0) else {
            return false;
        };
        let stack = self.dropped[index].stack;
        let (inserted, remaining) = self.try_insert(stack);
        if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() {
            eprintln!(
                "[tool-review] retrieved {:?} inserted={} remaining={} stack_limit={}",
                stack.item,
                inserted,
                remaining,
                self.stack_limit(stack.item)
            );
        }
        if remaining == 0 {
            self.dropped.remove(index);
        } else {
            self.dropped[index].stack.count = remaining;
        }
        if inserted > 0 {
            self.notify_pickup(stack.item, inserted);
        } else {
            self.notify_overflow(stack.item);
        }
        true
    }
    pub fn stack_limit(&self, item: Item) -> u32 {
        Self::limit_at_scale(item, self.voxel_m)
    }
    fn limit_at_scale(item: Item, voxel_m: f32) -> u32 {
        let multiplier = if matches!(item, Item::Block(b) if !b.is_model()) {
            (1.0 / voxel_m.clamp(0.25, 1.0).powi(3)).round() as u32
        } else {
            1
        };
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
        if let Some(existing) = &mut self.pickup_notice
            && !existing.overflow && existing.item == item && existing.seconds > 0.2 {
                existing.count = existing.count.saturating_add(count);
                existing.seconds = 2.6;
                return;
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
            if let Some(stack) = slot
                && stack.item == item {
                    total = total.saturating_add(stack.count);
                }
        }
        if include_transient {
            for slot in &self.craft_grid {
                if let Some(stack) = slot
                    && stack.item == item {
                        total = total.saturating_add(stack.count);
                    }
            }
            if let Some(stack) = self.cursor
                && stack.item == item {
                    total = total.saturating_add(stack.count);
                }
        }
        total
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
            if let Some(existing) = slot
                && existing.item == stack.item && existing.count < max {
                    let add = remaining.min(max - existing.count);
                    existing.count += add;
                    remaining -= add;
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
            if let Some(existing) = slot
                && existing.item == stack.item && existing.count < max {
                    let add = remaining.min(max - existing.count);
                    existing.count += add;
                    remaining -= add;
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
            if let Some(stack) = slot
                && stack.item == item {
                    let take = remaining.min(stack.count);
                    stack.count -= take;
                    remaining -= take;
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
        }
        if remaining > 0
            && let Some(cursor) = &mut self.cursor
                && cursor.item == item {
                    let take = remaining.min(cursor.count);
                    cursor.count -= take;
                    remaining -= take;
                    if cursor.count == 0 {
                        self.cursor = None;
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
        if let Some(stack) = slot
            && stack.item == expected && stack.count > 0 {
                stack.count -= 1;
                if stack.count == 0 {
                    *slot = None;
                }
                return true;
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
                if cursor_stack.item == slot_stack.item
                    && slot_stack.count < self.stack_limit(slot_stack.item)
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
        if !matches!(addr, SlotAddr::Equipment(_))
            && let Some(equip_slot) = stack.item.equip_slot() {
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

        let original_count = stack.count;
        let max = self.stack_limit(stack.item);

        match addr {
            SlotAddr::Hotbar(_) => {
                // Move from Hotbar -> Pack (merge first, then empty).
                for dst in &mut self.pack {
                    if stack.count == 0 {
                        break;
                    }
                    if let Some(existing) = dst
                        && existing.item == stack.item && existing.count < max {
                            let add = stack.count.min(max - existing.count);
                            existing.count += add;
                            stack.count -= add;
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
                    if let Some(existing) = dst
                        && existing.item == stack.item && existing.count < max {
                            let add = stack.count.min(max - existing.count);
                            existing.count += add;
                            stack.count -= add;
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
        if let Some(stack_b) = b
            && !self.slot_accepts(addr, stack_b.item) {
                return;
            }
        if let Some(target) = self.slot_mut(addr) {
            *target = b;
            self.hotbar[hotbar_idx] = a;
        }
    }

    /// Double-click gather (Minecraft / Mouse Tweaks): pulls loose partial stacks of the held
    /// cursor item (or the item in `addr` if cursor is empty) into `self.cursor` up to its stack limit.
    pub fn gather_to_cursor(&mut self, addr: SlotAddr) {
        if self.cursor.is_none()
            && let Some(stack) = self.slot(addr) {
                if let Some(slot) = self.slot_mut(addr) {
                    *slot = None;
                }
                self.cursor = Some(stack);
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
                if let Some(st) = slot
                    && st.item == cursor.item && (!partial_only || st.count < max) {
                        let take = (max - cursor.count).min(st.count);
                        st.count -= take;
                        cursor.count += take;
                        if st.count == 0 {
                            *slot = None;
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
                        if let Some(ex) = dst
                            && ex.item == one.item && ex.count < max {
                                ex.count += 1;
                                moved = true;
                                break;
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
                        if let Some(ex) = dst
                            && ex.item == one.item && ex.count < max {
                                ex.count += 1;
                                moved = true;
                                break;
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
                        if let Some(st) = src
                            && st.item == target_item {
                                st.count -= 1;
                                if st.count == 0 {
                                    *src = None;
                                }
                                found = true;
                                break;
                            }
                    }
                }
                _ => {
                    for src in self.pack.iter_mut().chain(self.hotbar.iter_mut()) {
                        if let Some(st) = src
                            && st.item == target_item {
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
            if found {
                dst_stack.count += 1;
                if let Some(target) = self.slot_mut(addr) {
                    *target = Some(dst_stack);
                }
            }
        }
    }

    /// Merges, sorts and compacts the pack; the hotbar keeps its layout.
    pub fn sort_pack(&mut self) {
        let voxel_m = self.voxel_m;
        sorting::sort_slots(&mut self.pack, |item| Self::limit_at_scale(item, voxel_m));
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
                if let Some(hot_st) = h_slot
                    && hot_st.item == pack_st.item && hot_st.count < max {
                        let add = pack_st.count.min(max - hot_st.count);
                        hot_st.count += add;
                        pack_st.count -= add;
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

    fn stash_stack(&mut self, stack: ItemStack) -> Option<ItemStack> {
        let (_, rem) = self.try_insert(stack);
        (rem > 0).then_some(ItemStack {
            item: stack.item,
            count: rem,
        })
    }

    /// Ctrl + Left-Click (or Ctrl + Shift + Click): Quick-transfers EVERY matching stack of the
    /// item in `addr` across that entire inventory region (Hotbar <-> Pack) in one action.
    pub fn ctrl_click_slot(&mut self, addr: SlotAddr) -> bool {
        let Some(target_stack) = self.slot(addr) else {
            return false;
        };
        let target_item = target_stack.item;
        let mut any_moved = self.shift_click_slot(addr);
        match addr {
            SlotAddr::Hotbar(_) => {
                for i in 0..HOTBAR_SLOTS {
                    if self.hotbar[i].is_some_and(|s| s.item == target_item) {
                        any_moved |= self.shift_click_slot(SlotAddr::Hotbar(i));
                    }
                }
            }
            SlotAddr::Pack(_) => {
                for i in 0..PACK_SLOTS {
                    if self.pack[i].is_some_and(|s| s.item == target_item) {
                        any_moved |= self.shift_click_slot(SlotAddr::Pack(i));
                    }
                }
            }
            SlotAddr::CraftGrid(_) => {
                for i in 0..CRAFT_SLOTS {
                    if self.craft_grid[i].is_some_and(|s| s.item == target_item) {
                        any_moved |= self.shift_click_slot(SlotAddr::CraftGrid(i));
                    }
                }
            }
            SlotAddr::Equipment(_) => {}
        }
        any_moved
    }

    /// Middle-click (clone / pick full stack): if cursor is empty and `addr` has an item,
    /// gathers a full stack of that item into `self.cursor`.
    pub fn middle_click_slot(&mut self, addr: SlotAddr) {
        if self.cursor.is_none() && self.slot(addr).is_some() {
            self.gather_to_cursor(addr);
        }
    }

    /// Left-click-drag even distribution (Vanilla Minecraft / Mouse Tweaks):
    /// Splits `origin_stack` evenly across `visited_slots` and leaves any remainder on `self.cursor`.
    pub fn distribute_even_across(&mut self, origin: Option<ItemStack>, visited: &[SlotAddr]) {
        let Some(origin_stack) = origin else {
            return;
        };
        if visited.len() <= 1 {
            return;
        }
        // Filter to valid destination slots that are either empty or hold the same item below max_stack
        let max = self.stack_limit(origin_stack.item);
        let valid: Vec<SlotAddr> = visited
            .iter()
            .copied()
            .filter(|&addr| {
                self.slot_accepts(addr, origin_stack.item)
                    && self
                        .slot(addr)
                        .is_none_or(|s| s.item == origin_stack.item && s.count < max)
            })
            .collect();
        if valid.len() <= 1 {
            return;
        }
        let per_slot = (origin_stack.count / valid.len() as u32).max(1);
        let mut remaining = origin_stack.count;
        for addr in valid {
            if remaining == 0 {
                break;
            }
            let current = self.slot(addr).map_or(0, |s| s.count);
            let add = per_slot.min(remaining).min(max.saturating_sub(current));
            if add > 0 {
                if let Some(slot) = self.slot_mut(addr) {
                    *slot = Some(ItemStack {
                        item: origin_stack.item,
                        count: current + add,
                    });
                }
                remaining -= add;
            }
        }
        self.cursor = if remaining > 0 {
            Some(ItemStack {
                item: origin_stack.item,
                count: remaining,
            })
        } else {
            None
        };
    }

    /// Extracts either 1 item (`full_stack == false`) or the entire stack (`full_stack == true`)
    /// from `addr` so it can be dropped into the world.
    pub fn take_for_drop(&mut self, addr: SlotAddr, full_stack: bool) -> Option<ItemStack> {
        let mut stack = self.slot(addr)?;
        if full_stack || stack.count <= 1 {
            if let Some(slot) = self.slot_mut(addr) {
                *slot = None;
            }
            Some(stack)
        } else {
            stack.count -= 1;
            if let Some(slot) = self.slot_mut(addr) {
                *slot = Some(stack);
            }
            Some(ItemStack {
                item: stack.item,
                count: 1,
            })
        }
    }

    /// Returns any held cursor stack or 2×2 crafting grid stacks back into the pack/hotbar when closing UI.
    /// If pack/hotbar are full, keeps remainder in `craft_grid` or `cursor` so zero items are ever lost.
    pub fn return_transient_to_pack(&mut self) {
        let mut leftover: Vec<ItemStack> = Vec::new();
        if let Some(cursor) = self.cursor.take()
            && let Some(rest) = self.stash_stack(cursor) {
                leftover.push(rest);
            }
        let grid = std::mem::take(&mut self.craft_grid);
        for stack in grid.into_iter().flatten() {
            if let Some(rest) = self.stash_stack(stack) {
                leftover.push(rest);
            }
        }
        for rest in leftover {
            if let Some(slot) = self.craft_grid.iter_mut().find(|s| s.is_none()) {
                *slot = Some(rest);
            } else {
                self.cursor = Some(rest);
                break;
            }
        }
    }

    /// Evaluates the crafting grid and returns the matching recipe and how many
    /// batches it can produce. Away from a workbench only hand recipes match.
    /// Shaped rows are tried first, so a pattern wins over a shapeless kit
    /// that happens to use the same items.
    pub fn matching_grid_recipe(&self, bench: bool) -> Option<(&'static CraftingRecipe, u32)> {
        let usable = |r: &&CraftingRecipe| bench || !r.needs_bench();
        let shaped = all_recipes().iter().filter(|r| r.shaped()).filter(usable);
        if let Some(found) = shaped.into_iter().find_map(|r| self.shaped_batches(r).map(|b| (r, b))) {
            return Some(found);
        }
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
        for recipe in all_recipes() {
            if recipe.shaped() || recipe.ingredients.len() != present.len() || (!bench && recipe.needs_bench()) {
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
    pub fn craft_from_grid(&mut self, shift_to_pack: bool, bench: bool) -> Option<ItemStack> {
        let batches_to_craft = self.grid_craft_batches(shift_to_pack, bench).ok()?;
        let (recipe, _) = self.matching_grid_recipe(bench)?;
        let out = recipe.output;
        let max_stack = self.stack_limit(out.item);
        // A shaped craft takes one item from every occupied cell per batch.
        if recipe.shaped() {
            for slot in &mut self.craft_grid {
                if let Some(stack) = slot {
                    stack.count -= batches_to_craft;
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        let summed = if recipe.shaped() { &[][..] } else { recipe.ingredients };
        for &(req_item, req_count) in summed {
            let mut needed = req_count * batches_to_craft;
            for slot in &mut self.craft_grid {
                if needed == 0 {
                    break;
                }
                if let Some(stack) = slot
                    && stack.item == req_item {
                        let take = needed.min(stack.count);
                        stack.count -= take;
                        needed -= take;
                        if stack.count == 0 {
                            *slot = None;
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

    /// Stages `recipe` from the player's inventory into the crafting grid (the
    /// 2 × 2 corner away from a bench, all nine slots at one). Staging the
    /// recipe already in the grid adds one more batch, as the vanilla recipe
    /// book does. Returns `true` if the ingredients were available and staged.
    pub fn stage_recipe_into_grid(&mut self, recipe: &CraftingRecipe, bench: bool) -> bool {
        let mut staged = self.clone();
        if !staged.stage_recipe_in_place(recipe, bench) { return false; }
        *self = staged;
        true
    }

    fn stage_recipe_in_place(&mut self, recipe: &CraftingRecipe, bench: bool) -> bool {
        if recipe.needs_bench() && !bench {
            return false;
        }
        let staged = self
            .matching_grid_recipe(bench)
            .filter(|(r, _)| *r == recipe)
            .map_or(0, |(_, batches)| batches);
        // First return any existing craft_grid items to pack/hotbar if they fit.
        for i in 0..CRAFT_SLOTS {
            if let Some(stack) = self.craft_grid[i]
                && !self.can_insert(stack) {
                    return false;
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

        [staged + 1, staged.max(1)].into_iter().any(|batches| self.place_recipe(recipe, bench, batches))
    }

    #[cfg(test)]
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        self.encode_versioned(bytes, crate::world::storage::SAVE_VERSION);
    }
    /// Before save version 20: a 2 × 2 grid and 19 resource remainders.
    pub fn encode_versioned(&self, bytes: &mut Vec<u8>, version: u8) {
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
        if version >= 20 {
            for slot in self.craft_grid {
                encode_slot(slot, &mut put);
            }
        } else {
            for i in HAND_GRID {
                encode_slot(self.craft_grid[i], &mut put);
            }
        }
        encode_slot(self.cursor, &mut put);
        let resources = if version >= 20 {
            factory::RESOURCE_COUNT
        } else {
            factory::LEGACY_RESOURCE_COUNT
        };
        for &rem in &self.resource_remainders[..resources] {
            put(rem.min(63));
        }
    }

    #[cfg(test)]
    pub fn decode(read: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Self> {
        Self::decode_at_scale(read, 1.0)
    }
    #[cfg(test)]
    pub fn decode_at_scale(
        read: &mut impl FnMut() -> anyhow::Result<u32>,
        voxel_m: f32,
    ) -> anyhow::Result<Self> {
        Self::decode_versioned(read, voxel_m, crate::world::storage::SAVE_VERSION)
    }
    /// Saves before version 20 hold a 2 × 2 grid and 19 resource remainders.
    pub fn decode_versioned(
        read: &mut impl FnMut() -> anyhow::Result<u32>,
        voxel_m: f32,
        version: u8,
    ) -> anyhow::Result<Self> {
        use anyhow::ensure;
        let mut inv = Self::empty();
        inv.voxel_m = voxel_m;
        let decode_slot =
            |read: &mut dyn FnMut() -> anyhow::Result<u32>| -> anyhow::Result<Option<ItemStack>> {
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
        if version >= 20 {
            for slot in &mut inv.craft_grid {
                *slot = decode_slot(read)?;
            }
        } else {
            for i in HAND_GRID {
                inv.craft_grid[i] = decode_slot(read)?;
            }
        }
        inv.cursor = decode_slot(read)?;
        let resources = if version >= 20 {
            factory::RESOURCE_COUNT
        } else {
            factory::LEGACY_RESOURCE_COUNT
        };
        for rem in &mut inv.resource_remainders[..resources] {
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
        for (voxel_m, capacity) in [(1.0, 64), (0.5, 512), (0.25, 4096)] {
            let mut inv = SurvivalInventory::empty();
            inv.voxel_m = voxel_m;
            assert_eq!(inv.stack_limit(item), capacity);
            assert_eq!(
                inv.try_insert(ItemStack {
                    item,
                    count: capacity
                }),
                (capacity, 0)
            );
            assert_eq!(inv.hotbar.iter().flatten().count(), 1);
            inv.shift_click_slot(SlotAddr::Hotbar(0));
            assert_eq!(inv.pack[0].unwrap().count, capacity);
            inv.right_click_slot(SlotAddr::Pack(0));
            inv.click_slot(SlotAddr::Pack(0));
            assert_eq!(inv.pack[0].unwrap().count, capacity);
            let mut bytes = Vec::new();
            inv.encode(&mut bytes);
            let mut words = bytes.chunks_exact(4);
            let mut restored = SurvivalInventory::decode_at_scale(
                &mut || {
                    Ok(u32::from_le_bytes(
                        words.next().unwrap().try_into().unwrap(),
                    ))
                },
                voxel_m,
            )
            .unwrap();
            let stack = restored.pack[0].take().unwrap();
            restored.dropped.push(DroppedStack {
                pos: glam::Vec3::ZERO,
                stack,
            });
            assert!(!restored.retrieve_nearby(glam::Vec3::splat(5.0)));
            assert!(restored.retrieve_nearby(glam::Vec3::ZERO));
            assert_eq!(restored.count_item(item), capacity);
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
        let (recipe, batches) = inv.matching_grid_recipe(false).expect("Log should match planks");
        assert_eq!(recipe.output.item, Item::Block(BlockId::Planks));
        assert_eq!(batches, 2);

        let crafted = inv
            .craft_from_grid(false, false)
            .expect("Should craft 1 batch to cursor");
        assert_eq!(crafted.count, 4);
        assert_eq!(inv.cursor.unwrap().count, 4);
        assert_eq!(inv.craft_grid[0].unwrap().count, 1);
    }

    #[test]
    fn machine_kits_need_the_workbench_grid() {
        let mut inv = SurvivalInventory::empty();
        // Coal generator: 2 copper + 2 iron ingots.
        inv.craft_grid[0] = Some(ItemStack::new(Item::Resource(6), 2));
        inv.craft_grid[1] = Some(ItemStack::new(Item::Resource(8), 2));
        assert!(inv.matching_grid_recipe(false).is_none());
        let (recipe, _) = inv.matching_grid_recipe(true).expect("bench crafts the generator");
        assert_eq!(recipe.output.item, Item::Block(BlockId::CoalGenerator));
        assert!(recipe.needs_bench());
    }

    #[test]
    fn staging_by_hand_stays_in_the_two_by_two_corner() {
        let mut inv = SurvivalInventory::empty();
        inv.pack[0] = Some(ItemStack::new(Item::Block(BlockId::Planks), 8));
        let bench = all_recipes()
            .iter()
            .find(|r| r.output.item == Item::Block(BlockId::Workbench))
            .unwrap();
        assert!(inv.stage_recipe_into_grid(bench, false));
        for (i, slot) in inv.craft_grid.iter().enumerate() {
            assert_eq!(slot.is_some(), HAND_GRID.contains(&i), "slot {i}");
        }
    }

    #[test]
    fn version_19_grid_lands_in_the_hand_corner() {
        let mut inv = SurvivalInventory::empty();
        inv.craft_grid[4] = Some(ItemStack::new(Item::Block(BlockId::Log), 3));
        let mut bytes = Vec::new();
        inv.encode(&mut bytes);
        // Rewrite as a version 19 save: four grid slots, 19 remainders.
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        let slots_before_grid = (HOTBAR_SLOTS + PACK_SLOTS + EQUIP_SLOTS) * 3;
        let mut legacy = words[..slots_before_grid].to_vec();
        for i in HAND_GRID {
            legacy.extend_from_slice(&words[slots_before_grid + i * 3..slots_before_grid + i * 3 + 3]);
        }
        let cursor = slots_before_grid + CRAFT_SLOTS * 3;
        legacy.extend_from_slice(&words[cursor..cursor + 3]);
        legacy.extend(std::iter::repeat_n(0, factory::LEGACY_RESOURCE_COUNT));
        let mut it = legacy.into_iter();
        let back = SurvivalInventory::decode_versioned(&mut || Ok(it.next().unwrap()), 1.0, 19)
            .unwrap();
        assert!(it.next().is_none());
        assert_eq!(back.craft_grid[4].unwrap().count, 3);
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

        // Ctrl-click transfers all matching stacks across regions at once
        inv.hotbar[3] = Some(ItemStack::new(stone, 14));
        assert!(inv.ctrl_click_slot(SlotAddr::Hotbar(2)));
        assert!(inv.hotbar[2].is_none());
        assert!(inv.hotbar[3].is_none());
        assert_eq!(inv.count_item(stone), 64);

        // Middle-click gathers a full stack & left-drag splits evenly across visited slots
        let stone_slot = inv
            .pack
            .iter()
            .position(|s| s.is_some_and(|st| st.item == stone))
            .unwrap();
        inv.middle_click_slot(SlotAddr::Pack(stone_slot));
        assert_eq!(inv.cursor, Some(ItemStack::new(stone, 64)));
        let origin = inv.cursor.take();
        inv.distribute_even_across(
            origin,
            &[
                SlotAddr::Hotbar(0),
                SlotAddr::Hotbar(1),
                SlotAddr::Hotbar(2),
            ],
        );
        assert_eq!(inv.hotbar[0], Some(ItemStack::new(stone, 21)));
        assert_eq!(inv.hotbar[1], Some(ItemStack::new(stone, 21)));
        assert_eq!(inv.hotbar[2], Some(ItemStack::new(stone, 21)));
        assert_eq!(inv.cursor, Some(ItemStack::new(stone, 1)));

        let hotbar = SlotAddr::Hotbar(0);
        let pack = SlotAddr::Pack(0);
        assert_eq!(
            pointer_release(false, Some(hotbar), Some(hotbar), 0),
            PointerRelease::Keep
        );
        assert_eq!(
            pointer_release(false, Some(hotbar), Some(pack), 0),
            PointerRelease::Place(pack)
        );
        assert_eq!(
            pointer_release(false, Some(hotbar), None, 0),
            PointerRelease::Drop
        );
        assert_eq!(
            pointer_release(true, Some(hotbar), Some(hotbar), 1),
            PointerRelease::Place(hotbar)
        );
        assert_eq!(
            pointer_release(true, Some(hotbar), Some(pack), 3),
            PointerRelease::Distribute
        );
        // Releasing an untouched drag away from the grid throws the stack.
        assert_eq!(
            pointer_release(true, Some(hotbar), None, 0),
            PointerRelease::Drop
        );
        // After partially distributing, an off-grid release keeps the remainder.
        assert_eq!(
            pointer_release(true, Some(hotbar), None, 1),
            PointerRelease::Keep
        );
    }
}

#[cfg(test)]
mod recipe_table {
    use super::*;

    /// Serialising the loaded tables reproduces the file (minus its comment
    /// header), so the file is the single source of truth.
    #[test]
    fn recipes_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&RECIPES.rows(), ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = RECIPES_RON.replace('\r', "");
        let body = match file.find("\n(\n") {
            Some(i) => &file[i + 1..],
            None => file.as_str(),
        };
        let (a, b) = (text.trim_end(), body.trim_end());
        if a != b {
            let i = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            let lo = i.saturating_sub(60);
            panic!(
                "assets/recipes.ron differs from the loaded tables at byte {i}; run `cargo test dump_recipes_ron -- --ignored`.\n table: {:?}\n  file: {:?}",
                &a[lo..(i + 60).min(a.len())],
                &b[lo..(i + 60).min(b.len())]
            );
        }
    }

    /// Rewrites assets/recipes.ron from the loaded tables (keeps the header).
    #[test]
    #[ignore = "writes assets/recipes.ron"]
    fn dump_recipes_ron() {
        let header = RECIPES_RON.find("\n(").map_or("", |i| &RECIPES_RON[..=i]);
        let text = ron::ser::to_string_pretty(&RECIPES.rows(), ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/recipes.ron");
        std::fs::write(path, format!("{header}{text}")).unwrap();
    }
}
