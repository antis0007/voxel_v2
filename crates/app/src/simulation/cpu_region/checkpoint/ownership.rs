use super::super::{Command, EditOutcome, EditStatus, Region, ownership::Claim, COMMAND_CAPACITY};
use super::codec::{Reader, number};
use crate::world::BlockId;
use std::collections::{BTreeMap, VecDeque};

pub(super) fn encode(region: &Region, out: &mut Vec<u8>) {
    number(out, u64::from(region.inventory_bound));
    number(out, region.owned.len() as u64);
    for (&sequence, claim) in &region.owned {
        number(out, sequence);
        number(out, u64::from(claim.block.sim_byte()));
        number(out, u64::from(claim.placement));
    }
}

pub(super) fn decode(input: &mut Reader<'_>, pending: &VecDeque<Command>,
    outcomes: &VecDeque<EditOutcome>) -> Result<(bool, BTreeMap<u64, Claim>), String> {
    let bound = input.count(1)? == 1;
    let count = input.count(COMMAND_CAPACITY)?;
    if count > 0 && !bound { return Err("unbound cellular inventory claims".into()); }
    let mut claims = BTreeMap::new();
    let mut previous = 0;
    for _ in 0..count {
        let sequence = input.number()?;
        let material = input.count(u8::MAX as usize)? as u8;
        let block = BlockId::from_u32(material as u32);
        if !matches!(block, BlockId::Stone | BlockId::Sand) || material != block.sim_byte() || sequence <= previous {
            return Err("invalid cellular inventory claim".into());
        }
        let claim = Claim { block, placement: input.count(1)? == 1 };
        let (expected, replacement) = claim.cells();
        let valid = if let Some(command) = pending.iter().find(|c| c.sequence == sequence) {
            command.expected == Some(expected) && command.cell == replacement
        } else if let Some(outcome) = outcomes.iter().find(|o| o.sequence == sequence) {
            match outcome.status {
                EditStatus::Applied => outcome.before == expected && outcome.after == replacement,
                EditStatus::Conflict => outcome.before != expected && outcome.before == outcome.after,
                _ => false,
            }
        } else { false };
        if !valid { return Err("cellular inventory claim has no matching command or result".into()); }
        previous = sequence;
        claims.insert(sequence, claim);
    }
    Ok((bound, claims))
}
