//! Villager trade rows loaded from `assets/trades.ron`.
use crate::world::{
    World,
    farming::{freight, pantry, watershed},
    inventory::{Item, SurvivalInventory},
};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stack {
    pub item: Item,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Trade {
    pub give: Vec<Stack>,
    pub get: Stack,
    /// Uses per day before the row is out of stock.
    pub uses: u16,
    #[serde(default, skip_serializing_if = "Stock::is_daily")]
    stock: Stock,
    #[serde(skip)]
    invalid: bool,
}

impl Trade {
    pub(crate) fn affordable(&self, inventory: &SurvivalInventory) -> bool {
        self.give.iter().all(|stack| {
            let needed: u64 = self
                .give
                .iter()
                .filter(|other| other.item == stack.item)
                .map(|other| u64::from(other.count))
                .sum();
            u64::from(inventory.count_item(stack.item)) >= needed
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Stock {
    #[default]
    Daily,
    LegacyMeals,
    Nursery,
    FreightSale,
    FreightDelivery,
    PantrySale,
    PantryBiomass,
    PantryMeals,
    Community,
}

impl Stock {
    fn is_daily(&self) -> bool {
        *self == Self::Daily
    }

    pub(crate) fn is_delivery(self) -> bool {
        matches!(
            self,
            Self::FreightDelivery | Self::PantryBiomass | Self::PantryMeals
        )
    }

    pub(crate) fn is_stocked(self) -> bool {
        matches!(self, Self::Nursery | Self::FreightSale | Self::PantrySale | Self::Community)
    }
}

pub(crate) struct Quote {
    pub stock: Stock,
    pub remaining: u16,
    pub issue: Option<WorkplaceIssue>,
}

const TRADES_RON: &str = include_str!("../../../../../assets/trades.ron");

/// One page per villager; villager `i` offers page `i % pages`.
static PAGES: std::sync::LazyLock<Vec<Vec<Trade>>> = std::sync::LazyLock::new(|| {
    ron::from_str(TRADES_RON).unwrap_or_else(|error| {
        eprintln!("[trades] assets/trades.ron: {error}");
        Vec::new()
    })
});

pub fn page(villager: u32) -> &'static [Trade] {
    if villager == 7 {
        return DISTRICT.get(4).map_or(&[], Vec::as_slice);
    }
    if let Some(role) = engine_worldgen::households::role(villager) {
        return DISTRICT.get(role as usize).map_or(&[], Vec::as_slice);
    }
    match PAGES.len() {
        0 => &[],
        n => &PAGES[villager as usize % n],
    }
}

static COMMUNITY: std::sync::LazyLock<Vec<Trade>> = std::sync::LazyLock::new(|| {
    ron::from_str(include_str!("../../../../../assets/community_trades.ron")).unwrap_or_else(|error| {
        eprintln!("[trades] assets/community_trades.ron: {error}");
        Vec::new()
    })
});

pub fn page_for(world: &World, villager: u32) -> &'static [Trade] {
    if crate::world::farming::jobs::communities::owner_site(world, villager).is_some() { &COMMUNITY }
    else { page(villager) }
}

static DISTRICT: std::sync::LazyLock<Vec<Vec<Trade>>> = std::sync::LazyLock::new(|| {
    let mut pages: Vec<Vec<Trade>> = ron::from_str(include_str!(
        "../../../../../assets/fernwake_trades.ron"
    ))
    .unwrap_or_else(|error| {
        eprintln!("[trades] assets/fernwake_trades.ron: {error}");
        Vec::new()
    });
    for trade in pages.iter_mut().flatten() {
        use crate::world::BlockId::GlowCap;
        let cap = Item::Block(GlowCap);
        let meal = pantry::FOOD;
        let biomass = pantry::BIOMASS;
        let payment = pantry::PAYMENT;
        let pair = match trade.stock {
            Stock::Nursery | Stock::FreightSale => Some((biomass, cap)),
            Stock::FreightDelivery => Some((cap, biomass)),
            Stock::PantrySale => Some((payment, meal)),
            Stock::PantryBiomass => Some((biomass, payment)),
            Stock::PantryMeals => Some((meal, payment)),
            Stock::Daily | Stock::LegacyMeals | Stock::Community => None,
        };
        if let Some((input, output)) = pair
            && !(matches!(trade.give.as_slice(), [s] if s.item == input && s.count > 0)
                && trade.get.item == output
                && trade.get.count > 0)
        {
            trade.invalid = true;
            eprintln!(
                "[trades] assets/fernwake_trades.ron: invalid {:?} trade",
                trade.stock
            );
        }
    }
    pages
});

/// Resolve an authored offer once for both the UI and the transaction.
pub(crate) fn quote(world: &World, villager: u32, row: usize, trade: &Trade) -> Option<Quote> {
    if trade.invalid {
        return None;
    }
    if crate::world::farming::jobs::communities::market_ready(world, villager) == Some(false) {
        return Some(Quote { stock: Stock::Community, remaining: 0, issue: Some(WorkplaceIssue::Stockpile) });
    }
    if let Some(remaining) = crate::world::farming::jobs::communities::trade_stock(world, villager, trade) {
        return Some(Quote { stock: Stock::Community, remaining, issue: None });
    }
    use Stock::*;
    let stock = match (villager, trade.stock) {
        (3, LegacyMeals) if pantry::active(world) => return None,
        (watershed::SELLER, Nursery) => watershed::active(world).then_some(Nursery)?,
        (freight::SELLER, FreightSale | FreightDelivery) => {
            freight::active(&world.sample_config()).then_some(trade.stock)?
        }
        (freight::SELLER, PantryBiomass | PantryMeals) => {
            pantry::active(world).then_some(trade.stock)?
        }
        (freight::SELLER, PantrySale) if pantry::active(world) => PantrySale,
        _ => Daily,
    };
    let farming = &world.farming;
    let remaining = match stock {
        Nursery => (farming.nursery_stock / trade.get.count) as u16,
        FreightSale => (farming.freight.market / trade.get.count) as u16,
        FreightDelivery => (farming.freight.free_space() / trade.give[0].count) as u16,
        PantrySale | PantryBiomass | PantryMeals => pantry::uses(world, stock, trade),
        Daily => world.travel.uses_left(villager, row, trade),
        LegacyMeals | Community => unreachable!("stock resolved before legacy quoting"),
    };
    let issue = match stock {
        Nursery => bench_issue(world, villager),
        PantryBiomass | PantryMeals => None,
        _ => workplace_issue(world, villager),
    };
    Some(Quote {
        stock,
        remaining,
        issue,
    })
}

/// The quoted stock is still current after the synchronous inventory swap.
pub(super) fn settle(world: &mut World, villager: u32, row: usize, trade: &Trade, stock: Stock) {
    use Stock::*;
    match stock {
        Nursery => world.farming.nursery_stock -= trade.get.count,
        FreightSale => world.farming.freight.market -= trade.get.count,
        FreightDelivery => world.farming.freight.market += trade.give[0].count,
        PantrySale | PantryBiomass | PantryMeals => pantry::settle(world, stock, trade),
        Daily => *world.travel.used.entry((villager, row as u16)).or_insert(0) += 1,
        LegacyMeals => unreachable!("legacy meals resolve to daily stock"),
        Community => {},
    }
}

pub fn name(villager: u32) -> &'static str {
    if crate::world::farming::jobs::crew::is_constructor(villager) { return "Constructor"; }
    if let Some(name) = super::residents::name(villager) {
        return name;
    }
    match engine_worldgen::households::role(villager) {
        Some(0) => "Satyr Gardener",
        Some(1) => "Boatwright",
        Some(2) => "Orchard Dryad",
        Some(3) => "Household Sprite",
        _ => "Villager",
    }
}

/// Availability derives from real editable blocks; no separate repair flag can
/// disagree with a demolished bench or drained channel after a reload.
#[cfg(test)]
pub fn workplace(world: &crate::world::World, villager: u32) -> Option<&'static str> {
    workplace_issue(world, villager).map(WorkplaceIssue::label)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkplaceIssue {
    Bench,
    Water,
    Stockpile,
}

impl WorkplaceIssue {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bench => "Workbench Missing",
            Self::Water => "Channel Dry",
            Self::Stockpile => "Stockpile Unavailable",
        }
    }
}

pub fn workplace_issue(world: &crate::world::World, villager: u32) -> Option<WorkplaceIssue> {
    if let Some(ready) = crate::world::farming::jobs::communities::market_ready(world, villager) {
        return (!ready).then_some(WorkplaceIssue::Stockpile);
    }
    if let Some(issue) = bench_issue(world, villager) {
        return Some(issue);
    }
    let (water, _) = engine_worldgen::fernwake::workshop_cells(&world.sample_config(), villager)?;
    let cell = (water / world.voxel_scale.meters()).floor().as_ivec3();
    (world.get(cell) != crate::world::BlockId::Water).then_some(WorkplaceIssue::Water)
}

fn bench_issue(world: &World, villager: u32) -> Option<WorkplaceIssue> {
    let cfg = world.sample_config();
    let cell = if villager == freight::SELLER {
        freight::bench_cell(&cfg)
    } else {
        let (_, bench) = engine_worldgen::fernwake::workshop_cells(&cfg, villager)?;
        (bench / cfg.voxel_meters()).floor().as_ivec3()
    };
    (world.get(cell) != crate::world::BlockId::Planks).then_some(WorkplaceIssue::Bench)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trades_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*PAGES, ron::ser::PrettyConfig::new())
            .unwrap()
            .replace('\r', "");
        let file = TRADES_RON.replace('\r', "");
        let body = file.find("\n[\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end(), "rerun dump_trades_ron");
        assert!(
            PAGES
                .iter()
                .all(|p| !p.is_empty() && p.iter().all(|t| t.uses > 0 && t.get.count > 0))
        );
    }

    #[test]
    #[ignore = "writes assets/trades.ron"]
    fn dump_trades_ron() {
        let header: String = TRADES_RON
            .lines()
            .take_while(|l| l.starts_with("//"))
            .map(|l| format!("{l}\n"))
            .collect();
        let text = ron::ser::to_string_pretty(&*PAGES, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/trades.ron");
        std::fs::write(path, format!("{header}{text}\n")).unwrap();
    }
}
