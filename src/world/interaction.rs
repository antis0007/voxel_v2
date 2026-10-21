//! Shared diegetic interaction prompt. Reliquaries and world mechanisms both
//! use this shape so the HUD has one prompt path and one visual language.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub title: &'static str,
    pub action: &'static str,
}
