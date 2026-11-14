//! Part-based tools: every tool is an assembly of parts, each part a material.
//! Traits are rules on material properties (`assets/tools/traits.ron`), so a new
//! material row gets its traits without being named anywhere. Save integration
//! waits for a future `SAVE_VERSION` bump (docs/adr/0022-part-based-tools.md).

pub mod assembly;
pub mod capabilities;
pub mod modifiers;
pub mod codec;
pub mod materials;
pub mod parts;
pub mod progression;
pub mod sprite;
pub mod traits;

#[cfg(test)]
mod tests;

pub use assembly::{ToolAssembly, ToolError, ToolStats};
pub use codec::DecodeError;
pub use materials::{MaterialId, MaterialProps, ToolMaterial};
pub use parts::{PartKind, PartRole, PartsFile, ToolSchema};
pub use traits::{AppliedTrait, TraitEffect, TraitRule};

use std::sync::LazyLock;

pub const PARTS_RON: &str = include_str!("../../../assets/tools/parts.ron");
pub const MATERIALS_RON: &str = include_str!("../../../assets/tools/materials.ron");
pub const TRAITS_RON: &str = include_str!("../../../assets/tools/traits.ron");

fn load<T: serde::de::DeserializeOwned>(text: &str, file: &str, validate: impl Fn(&T) -> Vec<String>) -> T {
    let value: T = ron::from_str(text).unwrap_or_else(|error| panic!("assets/tools/{file}: {error}"));
    let errors = validate(&value);
    assert!(errors.is_empty(), "invalid assets/tools/{file}:\n{}", errors.join("\n"));
    value
}

/// The same schema and semantic checks for the runtime, Forge and AI drafts.
pub fn parse_parts(text: &str) -> Result<PartsFile, Vec<String>> {
    let value: PartsFile = ron::from_str(text).map_err(|error| vec![error.to_string()])?;
    let errors = parts::validate(&value);
    if errors.is_empty() { Ok(value) } else { Err(errors) }
}

/// Parse tool-material drafts with the same schema and semantic checks as the runtime.
pub fn parse_materials(text: &str) -> Result<Vec<ToolMaterial>, Vec<String>> {
    let rows: Vec<ToolMaterial> = ron::from_str(text).map_err(|error| vec![error.to_string()])?;
    let errors = materials::validate(&rows);
    if errors.is_empty() { Ok(rows) } else { Err(errors) }
}

pub static PARTS: LazyLock<PartsFile> = LazyLock::new(|| {
    parse_parts(PARTS_RON).unwrap_or_else(|errors| panic!("assets/tools/parts.ron: {}", errors.join("\n")))
});
pub static MATERIALS: LazyLock<Vec<ToolMaterial>> = LazyLock::new(|| {
    parse_materials(MATERIALS_RON).unwrap_or_else(|errors| panic!("assets/tools/materials.ron: {}", errors.join("\n")))
});
pub static TRAITS: LazyLock<Vec<TraitRule>> =
    LazyLock::new(|| load(TRAITS_RON, "traits.ron", |rows: &Vec<TraitRule>| traits::validate(rows)));

pub fn material(id: &str) -> Option<&'static ToolMaterial> {
    MATERIALS.iter().find(|m| m.id.0 == id)
}

/// Rules a material satisfies, in rule order.
pub fn material_traits(props: &MaterialProps) -> Vec<&'static TraitRule> {
    traits::material_traits(&TRAITS, props)
}

/// Mixes the three table texts into one value; changes when any row changes.
pub fn content_fingerprint() -> u32 {
    [PARTS_RON, MATERIALS_RON, TRAITS_RON]
        .iter()
        .flat_map(|text| text.replace('\r', "").into_bytes())
        .fold(0x9e37_79b9u32, |acc, byte| engine_core::hash::hash_u32(acc ^ u32::from(byte)))
}

#[cfg(test)]
mod capability_tests;

#[cfg(test)]
mod material_tests;
