//! Trait rules: predicates on material properties, evaluated per material and
//! stacked per assembly. No material is named in a rule.

use crate::materials::{Flag, MaterialProps, Prop};
use crate::parts::PartRole;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Predicate {
    Gt(Prop, f32),
    Lt(Prop, f32),
    Flag(Flag),
    Not(Box<Predicate>),
    All(Vec<Predicate>),
    Any(Vec<Predicate>),
}

impl Predicate {
    pub fn holds(&self, props: &MaterialProps) -> bool {
        match self {
            Predicate::Gt(prop, limit) => props.get(*prop) > *limit,
            Predicate::Lt(prop, limit) => props.get(*prop) < *limit,
            Predicate::Flag(flag) => props.flag(*flag),
            Predicate::Not(inner) => !inner.holds(props),
            Predicate::All(inner) => inner.iter().all(|p| p.holds(props)),
            Predicate::Any(inner) => inner.iter().any(|p| p.holds(props)),
        }
    }

    fn check(&self, errors: &mut Vec<String>, rule: &str) {
        match self {
            Predicate::Gt(_, limit) | Predicate::Lt(_, limit) => {
                if !limit.is_finite() {
                    errors.push(format!("rule {rule} compares against a non-finite limit"));
                }
            }
            Predicate::Flag(_) => {}
            Predicate::Not(inner) => inner.check(errors, rule),
            Predicate::All(inner) | Predicate::Any(inner) => {
                if inner.is_empty() {
                    errors.push(format!("rule {rule} has an empty All/Any"));
                }
                for p in inner {
                    p.check(errors, rule);
                }
            }
        }
    }
}

/// How a trait counts when several parts carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stacking {
    /// One stack if any part qualifies.
    Once,
    /// One stack per qualifying part.
    PerPart,
    /// One stack if a head-role part qualifies; other parts are ignored.
    HeadOnly,
}

/// Effect vocabulary the game systems implement; which materials get one is data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TraitEffect {
    Conductive,
    InsulatedGrip,
    FlammableHandle,
    SelfRepair,
    Jagged,
    Magnetic,
    Buoyant,
    LavaProof,
    Heavy,
    Brittle,
    Lightweight,
    Tempered,
    Bouncy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraitRule {
    pub effect: TraitEffect,
    pub display_name: String,
    pub when: Predicate,
    pub stacking: Stacking,
}

/// A trait on an assembled tool with its stack count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AppliedTrait {
    pub effect: TraitEffect,
    pub stacks: u8,
}

/// Rules a single material satisfies, in rule order.
pub fn material_traits<'a>(rules: &'a [TraitRule], props: &MaterialProps) -> Vec<&'a TraitRule> {
    rules.iter().filter(|rule| rule.when.holds(props)).collect()
}

/// Traits of an assembly from its parts' roles and materials, in rule order.
pub fn assemble_traits(rules: &[TraitRule], parts: &[(PartRole, &MaterialProps)]) -> Vec<AppliedTrait> {
    let mut out = Vec::new();
    for rule in rules {
        let qualifying = parts.iter().filter(|(role, props)| {
            (rule.stacking != Stacking::HeadOnly || *role == PartRole::Head) && rule.when.holds(props)
        });
        let count = qualifying.count();
        let stacks = match rule.stacking {
            Stacking::Once | Stacking::HeadOnly => u8::from(count > 0),
            Stacking::PerPart => count.min(usize::from(u8::MAX)) as u8,
        };
        if stacks > 0 {
            out.push(AppliedTrait { effect: rule.effect, stacks });
        }
    }
    out
}

/// Table errors: an effect ruled twice, empty groups, non-finite limits.
pub fn validate(rules: &[TraitRule]) -> Vec<String> {
    let mut errors = Vec::new();
    for (i, rule) in rules.iter().enumerate() {
        let name = format!("{:?}", rule.effect);
        if rules[..i].iter().any(|r| r.effect == rule.effect) {
            errors.push(format!("rule {name} appears twice"));
        }
        rule.when.check(&mut errors, &name);
    }
    errors
}
