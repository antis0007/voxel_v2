//! Villager trade rows loaded from `assets/trades.ron`.
use crate::world::inventory::Item;

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
    match PAGES.len() {
        0 => &[],
        n => &PAGES[villager as usize % n],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trades_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*PAGES, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = TRADES_RON.replace('\r', "");
        let body = file.find("\n[\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end(), "rerun dump_trades_ron");
        assert!(PAGES.iter().all(|p| !p.is_empty() && p.iter().all(|t| t.uses > 0 && t.get.count > 0)));
    }

    #[test]
    #[ignore = "writes assets/trades.ron"]
    fn dump_trades_ron() {
        let header: String = TRADES_RON.lines().take_while(|l| l.starts_with("//")).map(|l| format!("{l}\n")).collect();
        let text = ron::ser::to_string_pretty(&*PAGES, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/trades.ron");
        std::fs::write(path, format!("{header}{text}\n")).unwrap();
    }
}
