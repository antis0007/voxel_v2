//! Food names and restore values loaded from `assets/foods.ron`.
use super::FoodKind;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Food {
    pub kind: FoodKind,
    pub name: String,
    pub hunger: f32,
    pub health: f32,
}

const FOODS_RON: &str = include_str!("../../../../../assets/foods.ron");

/// The asset is compiled in and checked by a test; a parse failure leaves every food at zero.
static FOODS: std::sync::LazyLock<Vec<Food>> = std::sync::LazyLock::new(|| {
    ron::from_str(FOODS_RON).unwrap_or_else(|error| {
        eprintln!("[food] assets/foods.ron: {error}");
        Vec::new()
    })
});

pub fn get(kind: FoodKind) -> Option<&'static Food> {
    FOODS.iter().find(|f| f.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foods_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*FOODS, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = FOODS_RON.replace('\r', "");
        let body = file.find("\n[\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end(), "rerun dump_foods_ron");
    }

    #[test]
    fn every_food_has_one_row() {
        for kind in FoodKind::ALL {
            assert_eq!(FOODS.iter().filter(|f| f.kind == kind).count(), 1, "{kind:?}");
            assert_eq!(FoodKind::from_u8(kind as u8), Some(kind));
        }
        assert_eq!(FOODS.len(), FoodKind::ALL.len());
        let hunger = |k| get(k).map_or(0.0, |f| f.hunger);
        assert!(hunger(FoodKind::RawMutton) < hunger(FoodKind::CookedMutton), "cooking adds hunger");
    }

    #[test]
    fn cooked_mutton_is_crafted_from_raw_mutton_and_coal() {
        let cooked = super::super::Item::Food(FoodKind::CookedMutton);
        let recipe = super::super::hand_recipes().iter().find(|r| r.output.item == cooked).expect("cooking recipe");
        assert!(recipe.ingredients.contains(&(super::super::Item::Food(FoodKind::RawMutton), 1)), "{recipe:?}");
        assert!(recipe.ingredients.contains(&(super::super::Item::Resource(3), 1)), "coal: {recipe:?}");
    }

    #[test]
    fn eating_mutton_restores_the_table_values() {
        use crate::world::{World, WorldType};
        let mut w = World::with_seed(WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter, 7);
        for kind in [FoodKind::RawMutton, FoodKind::CookedMutton] {
            let row = get(kind).expect("row");
            w.adventure.hunger = 20.0;
            w.adventure.health = 1.0;
            w.inventory.hotbar[0] = Some(super::super::ItemStack::new(super::super::Item::Food(kind), 2));
            assert!(w.eat_food(kind, Some(0)));
            assert_eq!(w.adventure.hunger, 20.0 + row.hunger, "{kind:?}");
            assert_eq!(w.adventure.health, 1.0 + row.health, "{kind:?}");
        }
    }

    #[test]
    #[ignore = "writes assets/foods.ron"]
    fn dump_foods_ron() {
        let header = FOODS_RON.split("\n[").next().unwrap_or("");
        let text = ron::ser::to_string_pretty(&*FOODS, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/foods.ron");
        std::fs::write(path, format!("{header}\n{text}\n")).unwrap();
    }
}
