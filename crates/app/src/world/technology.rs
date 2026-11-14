//! Capability branches over the live recipe graph, not an unrelated unlock tree.
//! Salvage and manufacturing may reach the same item; recipes remain authoritative.
//! These branches cover existing content, not promises of unimplemented science.
use super::{
    BlockId,
    factory::Kind,
    inventory::Item,
    recipe_index::{self, Station},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    Building,
    Equipment,
    Metallurgy,
    Power,
    Logistics,
    Processing,
    Biology,
    Control,
    Arcana,
}

impl Branch {
    pub const ALL: [Self; 9] = [
        Self::Building,
        Self::Equipment,
        Self::Metallurgy,
        Self::Power,
        Self::Logistics,
        Self::Processing,
        Self::Biology,
        Self::Control,
        Self::Arcana,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Building => "Building",
            Self::Equipment => "Equipment",
            Self::Metallurgy => "Metallurgy",
            Self::Power => "Power",
            Self::Logistics => "Logistics",
            Self::Processing => "Processing",
            Self::Biology => "Biology",
            Self::Control => "Control",
            Self::Arcana => "Arcana",
        }
    }

    fn machine(kind: Kind) -> Self {
        match kind {
            Kind::Generator | Kind::GasGenerator
            | Kind::Cable
            | Kind::Battery
            | Kind::Transformer
            | Kind::InductionCasing
            | Kind::InductionPort
            | Kind::InductionCell
            | Kind::InductionProvider => Self::Power,
            Kind::Feeder
            | Kind::Storage
            | Kind::Pipe
            | Kind::Pump
            | Kind::FluidPipe
            | Kind::Tank | Kind::ExhaustDuct | Kind::Chimney | Kind::GasTank => Self::Logistics,
            Kind::Crusher
            | Kind::Washer
            | Kind::Furnace
            | Kind::AlloyKiln
            | Kind::CokeOven
            | Kind::BlastFurnace
            | Kind::CokeBrick
            | Kind::BlastBrick | Kind::SearedBrick | Kind::SmelteryController | Kind::SmelteryDrain | Kind::SmelteryFaucet | Kind::CastingTable | Kind::CastingBasin => Self::Metallurgy,
            Kind::Cultivator | Kind::Bioreactor => Self::Biology,
            Kind::Fabricator | Kind::Assembler => Self::Control,
            Kind::MiningDrill | Kind::Quarry | Kind::DustFilter | Kind::WetScrubber => Self::Processing,
        }
    }

    pub fn of(item: Item) -> Self {
        match item {
            Item::Assembled { .. } | Item::Part { .. } | Item::Cast(_) => Self::Equipment,
            Item::Bucket(_) => Self::Equipment,
            Item::PowerKit(k) => Self::machine(k.kind),
            Item::Block(b) => Kind::from_block(b).map(Self::machine).unwrap_or(match b {
                BlockId::Workbench | BlockId::Smelter => Self::Processing,
                _ => Self::Building,
            }),
            Item::Resource(0..=2 | 6..=11 | 14 | 15 | 19 | 22 | 23) => Self::Metallurgy,
            Item::Resource(4 | 13 | 20 | 21) | Item::Food(_) => Self::Biology,
            Item::Resource(12) => Self::Control,
            Item::Resource(3 | 16) => Self::Power,
            Item::Resource(_) | Item::Wand { .. } => Self::Arcana,
            Item::Tool(3..=5) => Self::Arcana,
            Item::Weapon(_) | Item::Armor(_, _) | Item::Tool(_) => Self::Equipment,
        }
    }
}

/// JEI-style query: whitespace AND, `|` OR, `-` negation, `"..."` phrases.
/// Prefixes address actual fields, never an invented research level. The
/// base-game source is `@base`/`@livingworld`.
pub fn matches(item: Item, query: &str) -> bool {
    let query = query.to_lowercase();
    query
        .split('|')
        .any(|alt| terms(alt).iter().all(|t| match t.strip_prefix('-') {
            Some(rest) if !rest.is_empty() => !term_matches(item, rest),
            _ => term_matches(item, t),
        }))
}

fn terms(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in query.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn term_matches(item: Item, q: &str) -> bool {
    if let Some(tag) = q.strip_prefix('#') {
        item.category().to_lowercase().contains(tag)
            || Branch::of(item).name().to_lowercase().contains(tag)
            || recipe_index::Group::of(item).name().to_lowercase().contains(tag)
    } else if let Some(station) = q.strip_prefix('^') {
        recipe_index::producing(item)
            .iter()
            .chain(recipe_index::using(item).iter())
            .any(|e| e.station.name().to_lowercase().contains(station))
    } else if let Some(material) = q.strip_prefix('$') {
        (matches!(item, Item::Resource(_)) && item.name().to_lowercase().contains(material))
            || recipe_index::producing(item).iter().any(|e| {
                e.inputs.iter().any(|(i, _)| {
                    matches!(i, Item::Resource(_)) && i.name().to_lowercase().contains(material)
                })
            })
    } else if let Some(source) = q.strip_prefix('@') {
        "base".contains(source) || "livingworld".contains(source)
    } else {
        item.name().to_lowercase().contains(q)
            || item.category().to_lowercase().contains(q)
            || Branch::of(item).name().to_lowercase().contains(q)
            || recipe_index::Group::of(item).name().to_lowercase().contains(q)
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    #[test]
    fn query_syntax_or_not_and_phrases() {
        assert_eq!(terms(r#"a "b c"  d"#), ["a", "b c", "d"]);
        let item = Item::Block(BlockId::Workbench);
        let name = item.name().to_lowercase();
        let word = name.split_whitespace().next().unwrap_or("").to_string();
        assert!(matches(item, &word));
        assert!(!matches(item, &format!("-{word}")));
        assert!(matches(item, &format!("zzqx | {word}")));
        assert!(!matches(item, &format!("zzqx {word}")));
        assert!(matches(item, &format!("\"{name}\"")));
        assert!(matches(item, "-"));
    }
}

/// Reusable facilities required by a resolved recipe plan. Each appears once,
/// independently of batch counts; possession and proximity are UI concerns.
pub fn facilities(plan: &recipe_index::Plan) -> Vec<Item> {
    fn visit(plan: &recipe_index::Plan, out: &mut Vec<Item>) {
        if let Some(recipe) = plan.recipe {
            if recipe.station != Station::Hand {
                let item = recipe.station.icon();
                if !out.contains(&item) {
                    out.push(item);
                }
            }
        }
        for child in &plan.children {
            visit(child, out);
        }
    }
    let mut out = Vec::new();
    visit(plan, &mut out);
    out
}
