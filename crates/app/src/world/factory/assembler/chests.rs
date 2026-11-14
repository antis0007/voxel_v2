//! Assembler entry points retain the shared physical chest transfer contract.
pub(in crate::world::factory) use super::super::chest_io::{has_output, transfer};
#[cfg(test)]
pub(super) use super::super::chest_io::INTERVAL;
