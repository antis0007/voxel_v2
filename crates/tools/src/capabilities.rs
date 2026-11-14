//! Authored behavior belongs to a schema, not to its name or inventory class.
use serde::{Deserialize, Serialize};

macro_rules! mining_classes {
    ($($name:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum MiningClass { $($name),+ }
        impl MiningClass {
            /// Native choices for the same type parsed by runtime and authoring.
            pub const ALL: &'static [Self] = &[$(Self::$name),+];
        }
    };
}
mining_classes!(Dig, Cut, Scoop);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mining {
    pub class: MiningClass,
    /// Hold to charge a crater instead of breaking a single cell.
    pub charged: bool,
    pub cuts_bamboo: bool,
    pub fells_trees: bool,
}

/// More capabilities can be added here without new item classes or instance
/// state. Absence of mining explicitly opts out; stats alone do not enable it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub mining: Option<Mining>,
    pub blocking: bool,
}

impl Capabilities {
    pub fn validate(self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(mining) = self.mining {
            if mining.fells_trees && mining.class != MiningClass::Cut {
                errors.push("tree felling requires Cut mining".into());
            }
        }
        errors
    }
}
