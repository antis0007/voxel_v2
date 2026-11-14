//! Existing factory resource identities shared with authoring tools.
pub const RESOURCE_COUNT: usize = 25;
pub const NAMES: [&str; RESOURCE_COUNT] = [
    "Copper ore",
    "Tin ore",
    "Iron ore",
    "Coal",
    "Lumen fibre",
    "Resonant crystal",
    "Copper ingots",
    "Tin ingots",
    "Iron ingots",
    "Copper dust",
    "Tin dust",
    "Iron dust",
    "Circuit boards",
    "Fungal biomass",
    "Bronze ingots",
    "Steel ingots",
    "Induction coils",
    "Arcane cores",
    "Wild essence",
    "Coal coke",
    "Creosote oil",
    "Bio-ethanol",
    "Gold Concentrate",
    "Gold Ingot",
    "Feathers",
];

const SOURCE: &str = include_str!("../../../assets/resources.ron");

/// One factory resource. Its index in [`ResourceTable::resources`] is its
/// `Item::Resource` id.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResourceDef {
    /// Stable lowercase id (`copper_ore`); data files may refer to it.
    pub key: String,
    /// Player-visible label.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResourceTable {
    pub resources: Vec<ResourceDef>,
}

impl ResourceTable {
    /// Id of the resource with this key.
    pub fn id(&self, key: &str) -> Option<usize> {
        self.resources.iter().position(|r| r.key == key)
    }
}

fn parse(source: &str) -> Result<ResourceTable, String> {
    let table: ResourceTable = ron::from_str(source).map_err(|e| e.to_string())?;
    if table.resources.is_empty() {
        return Err("no resources".into());
    }
    if table.resources.len() > usize::from(u8::MAX) {
        return Err(format!("{} resources exceed the u8 id space", table.resources.len()));
    }
    let mut keys = std::collections::HashSet::new();
    for r in &table.resources {
        let valid = !r.key.is_empty()
            && r.key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !valid {
            return Err(format!("invalid key {:?}", r.key));
        }
        if !keys.insert(r.key.as_str()) {
            return Err(format!("duplicate key {}", r.key));
        }
        if r.name.trim().is_empty() {
            return Err(format!("empty name for {}", r.key));
        }
    }
    Ok(table)
}

/// The parsed `assets/resources.ron`, loaded once. The file is bundled and
/// validated by tests, so a failure here is a build defect.
pub fn resource_table() -> &'static ResourceTable {
    static TABLE: std::sync::OnceLock<ResourceTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| parse(SOURCE).unwrap_or_else(|e| panic!("assets/resources.ron: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_ron_parses() {
        assert!(parse(SOURCE).is_ok(), "{:?}", parse(SOURCE).err());
    }

    /// Ids are row indices and saves store them, so the legacy rows must
    /// keep their order forever. New rows go after them.
    #[test]
    fn resources_ron_keeps_legacy_order() {
        let table = parse(SOURCE).unwrap();
        assert!(table.resources.len() >= RESOURCE_COUNT);
        for (i, name) in NAMES.iter().enumerate() {
            assert_eq!(table.resources[i].name, *name, "resource {i} moved or renamed");
        }
    }

    /// Until the save stores its own resource count (plan step 3), an extra
    /// row would change the save layout without a version bump.
    #[test]
    fn resources_ron_matches_save_width() {
        assert_eq!(parse(SOURCE).unwrap().resources.len(), RESOURCE_COUNT);
    }

    #[test]
    fn resources_ron_rejects_bad_rows() {
        assert!(parse("(resources: [])").is_err());
        assert!(parse(r#"(resources: [(key: "a", name: "A"), (key: "a", name: "B")])"#).is_err());
        assert!(parse(r#"(resources: [(key: "Bad Key", name: "A")])"#).is_err());
        assert!(parse(r#"(resources: [(key: "a", name: " ")])"#).is_err());
    }

    #[test]
    fn resources_ron_round_trips() {
        let table = parse(SOURCE).unwrap();
        let text = ron::to_string(&table).unwrap();
        assert_eq!(parse(&text).unwrap(), table);
    }
}
