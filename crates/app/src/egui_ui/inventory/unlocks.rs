//! Progressive disclosure: which inventory pages and HUD pieces a survival
//! player has reached. Nothing is saved; every flag is derived each frame from
//! what the player owns, has placed, or the goal they are on, so an older save
//! that already progressed opens everything it earned. Creative shows all.
use crate::world::{
    adventure,
    factory::{Kind, View as Machines},
    goals::Tier,
    inventory::{Item, SurvivalInventory},
};

/// Wild essence is the first magic resource a new player can pick up.
const WILD_ESSENCE: u8 = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::egui_ui) struct Unlocks {
    pub magic: bool,
    pub forge: bool,
    pub factory: bool,
    pub space: bool,
    pub quests: bool,
}

impl Unlocks {
    pub const ALL: Self = Self { magic: true, forge: true, factory: true, space: true, quests: true };

    /// Inventory page ids (see `tabs::TABS`) that are still locked away.
    /// Pages not listed here are always open.
    pub fn page_open(self, page: u32) -> bool {
        match page {
            1 => self.magic,
            5 => self.forge,
            3 | 7 => self.factory,
            8 => self.space,
            _ => true,
        }
    }

}

/// Magic is revealed once the player draws a wand or shows magic progress.
/// Every new world hands out a starter wand, so owning one says nothing: the
/// signal is holding it (kept for this world in the session), a found sigil, rare spell or
/// rank, or reaching the Magic goal. Older saves that progressed reopen at once.
pub(in crate::egui_ui) fn magic_open(ctx: &egui::Context, view: &adventure::View) -> bool {
    let key = egui::Id::new("progressive_magic_seen");
    let scope = super::super::goals_hud::world_scope(ctx);
    let progressed = view.active_wand.is_some()
        || view.rank > 0
        || view.sigils != 0
        || view.rare_spells != 0
        || view.discoveries != 0
        || view.heart_restored
        || reached(ctx, Tier::Magic);
    if progressed {
        ctx.data_mut(|d| d.insert_temp(key, scope));
        return true;
    }
    // Keep only the most recently progressed world, not a process-wide flag.
    // An absent scope is used by isolated UI fixtures before a world is published.
    ctx.data(|d| d.get_temp::<Option<std::sync::Arc<std::path::PathBuf>>>(key)) == Some(scope)
}

/// Whether the Wild Essence pickup toast may show: it means nothing before magic opens.
pub(in crate::egui_ui) fn pickup_toast_open(
    ctx: &egui::Context,
    item: Item,
    view: &adventure::View,
    survival: bool,
) -> bool {
    !survival || item != Item::Resource(WILD_ESSENCE) || magic_open(ctx, view)
}

/// The mana regeneration and affinity line. Poisoning is a hazard and always shows.
pub(in crate::egui_ui) fn mana_line_open(ctx: &egui::Context, view: &adventure::View, poisoned: bool) -> bool {
    view.creative || poisoned || magic_open(ctx, view)
}

fn owns(survival: &SurvivalInventory, f: impl Fn(Item) -> bool) -> bool {
    survival.hotbar.iter().chain(survival.pack.iter()).flatten().any(|s| f(s.item))
}

/// Position of a goal tier on the track; used to ask whether the current goal has reached one.
fn rank(tier: Tier) -> usize {
    Tier::ALL.iter().position(|t| *t == tier).unwrap_or(0)
}

/// The goal the player is on has reached `tier`. A finished track counts as reached.
fn reached(ctx: &egui::Context, tier: Tier) -> bool {
    match super::super::goals_hud::current_tier(ctx) {
        None => false,
        Some(None) => true,
        Some(Some(current)) => rank(current) >= rank(tier),
    }
}

pub(in crate::egui_ui) fn derive(
    ctx: &egui::Context,
    creative_catalogue: bool,
    survival: &SurvivalInventory,
    machines: &Machines,
    equipment: &adventure::View,
) -> Unlocks {
    if creative_catalogue {
        return Unlocks::ALL;
    }
    let factory = machines.count > 0
        || owns(survival, |item| matches!(item, Item::Block(b) if Kind::from_block(b).is_some()))
        || reached(ctx, Tier::Machine);
    let forge = factory
        || !equipment.tool_bank.is_empty()
        || owns(survival, |item| matches!(item, Item::Assembled { .. }));
    let magic = magic_open(ctx, equipment);
    let space = machines.meridian_terminal.is_some()
        || machines.orbital_cell
        || reached(ctx, Tier::Orbit);
    let quests = machines.workshop.is_some() || reached(ctx, Tier::Guardian);
    Unlocks { magic, forge, factory, space, quests }
}
