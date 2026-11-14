//! Distinct mineral blocks can enter the same refining chain.
use crate::BlockId as B;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Copper,
    Tin,
    Iron,
    Coal,
    Crystal,
    GoldQuartz,
}

pub fn family(block: B) -> Option<Family> {
    Some(match block {
        B::CopperOre | B::Chalcopyrite | B::Malachite => Family::Copper,
        B::TinOre | B::Cassiterite => Family::Tin,
        B::IronOre | B::Hematite | B::Magnetite => Family::Iron,
        B::CoalOre => Family::Coal,
        B::Crystal => Family::Crystal,
        B::GoldQuartz => Family::GoldQuartz,
        _ => return None,
    })
}
