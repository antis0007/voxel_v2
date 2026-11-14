//! Food names and restore values loaded from `assets/foods.ron`.
use super::FoodKind;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Food {
    pub kind: FoodKind,
    pub name: String,
    pub hunger: f32,
    pub health: f32,
    /// Hidden buffer that drains before hunger, capped by hunger after eating.
    #[serde(default)]
    pub saturation: f32,
    #[serde(default)]
    pub effects: Vec<crate::world::progression::effects::Grant>,
    /// Explicit item-sheet slot; absent on legacy food definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<u16>,
}

impl Food {
    pub fn icon_id(&self) -> usize {
        self.icon.map_or_else(
            || if (self.kind as u8) < 10 { 10 + self.kind as usize } else { 50 + self.kind as usize },
            usize::from,
        )
    }
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
    fn explicit_food_icon_preserves_legacy_slots() {
        for kind in FoodKind::ALL {
            let mut row = get(kind).expect("food row").clone();
            row.icon = None;
            let id = kind as usize;
            assert_eq!(row.icon_id(), if id < 10 { 10 + id } else { 50 + id });
            row.icon = Some(78);
            assert_eq!(row.icon_id(), 78);
        }
        let source = r#"(kind: Bread, name: "Test", hunger: 1.0, health: 0.0, icon: Some(78))"#;
        let row: Food = ron::from_str(source).expect("explicit food icon");
        assert_eq!(row.icon_id(), 78);
        assert_eq!(ron::from_str::<Food>(&ron::to_string(&row).unwrap()).unwrap(), row);
    }

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
    fn crop_foods_are_crafted_from_crops_and_raise_hunger_above_their_parts() {
        use super::super::Item;
        use crate::world::BlockId as B;
        let recipe = |kind| super::super::all_recipes().iter().find(|r| r.output.item == Item::Food(kind)).expect("recipe");
        assert!(recipe(FoodKind::Bread).ingredients.contains(&(Item::Block(B::WheatCrop), 3)));
        assert!(recipe(FoodKind::RootStew).ingredients.contains(&(Item::Block(B::CarrotCrop), 2)));
        assert!(recipe(FoodKind::RootStew).ingredients.contains(&(Item::Block(B::BeetCrop), 2)));
        let cake = recipe(FoodKind::HarvestCake);
        for crop in [B::WheatCrop, B::CarrotCrop, B::BeetCrop] {
            assert!(cake.ingredients.iter().any(|(i, _)| *i == Item::Block(crop)), "{crop:?}");
        }
        let fish = recipe(FoodKind::CookedFish);
        assert!(fish.ingredients.contains(&(Item::Food(FoodKind::RawFish), 1)));
        let hunger = |k| get(k).map_or(0.0, |f| f.hunger);
        assert!(hunger(FoodKind::RawFish) < hunger(FoodKind::CookedFish), "cooking adds hunger");
        assert!(hunger(FoodKind::Bread) > hunger(FoodKind::CookedMutton));
        assert!(!FoodKind::Bread.is_potion());
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
