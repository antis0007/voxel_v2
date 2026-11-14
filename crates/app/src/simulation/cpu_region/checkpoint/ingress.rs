use super::super::{COMMAND_CAPACITY, Command, EditOutcome, EditStatus, Region, Snapshot};
use super::codec::{Reader, cell, number, position};
use engine_core::time::PLAYER_STEPS_PER_CA;
use std::collections::VecDeque;

pub(super) fn encode(region: &Region, out: &mut Vec<u8>) {
    number(out, region.pending.len() as u64);
    for command in &region.pending {
        for value in [
            command.sequence,
            command.submitted_player_tick,
            command.earliest_ca_tick,
        ] {
            number(out, value);
        }
        position(out, command.at);
        cell(out, command.cell);
        number(out, u64::from(command.expected.is_some()));
        if let Some(expected) = command.expected {
            cell(out, expected);
        }
    }
    number(out, region.outcomes.len() as u64);
    for outcome in &region.outcomes {
        number(out, outcome.sequence);
        number(out, outcome.tick);
        position(out, outcome.at);
        cell(out, outcome.before);
        cell(out, outcome.after);
        number(
            out,
            match outcome.status {
                EditStatus::Applied => 0,
                EditStatus::NoChange => 1,
                EditStatus::Conflict => 2,
                EditStatus::Protected => 3,
            },
        );
    }
}

pub(super) fn decode(
    input: &mut Reader<'_>,
    published: &Snapshot,
    applied: u64,
    next: u64,
    player_tick: u64,
    completed: u64,
) -> Result<(VecDeque<Command>, VecDeque<EditOutcome>), String> {
    let count = input.count(COMMAND_CAPACITY)?;
    if next - 1 - applied != count as u64 {
        return Err("missing accepted CPU commands".into());
    }
    let mut pending = VecDeque::with_capacity(COMMAND_CAPACITY);
    let mut submitted = 0;
    for i in 0..count {
        let mut command = Command {
            sequence: input.number()?,
            submitted_player_tick: input.number()?,
            earliest_ca_tick: input.number()?,
            at: input.position()?,
            cell: input.cell()?,
            expected: None,
        };
        if input.count(1)? == 1 {
            command.expected = Some(input.cell()?);
        }
        if command.sequence != applied + i as u64 + 1
            || command.submitted_player_tick > player_tick
            || command.submitted_player_tick < submitted
            || command.earliest_ca_tick < command.submitted_player_tick / PLAYER_STEPS_PER_CA
                + if (command.submitted_player_tick + 1) % PLAYER_STEPS_PER_CA == 0 { 2 } else { 1 }
            || command.earliest_ca_tick > completed + 2
            || !published.contains(command.at)
        {
            return Err("invalid pending CPU command".into());
        }
        super::super::backend::validate(command.cell)?;
        if let Some(expected) = command.expected {
            super::super::backend::validate(expected)?;
        }
        submitted = command.submitted_player_tick;
        pending.push_back(command);
    }
    let count = input.count(COMMAND_CAPACITY - pending.len())?;
    let first = applied
        .checked_sub(count as u64)
        .ok_or("invalid CPU outcome sequence")?;
    let mut outcomes = VecDeque::with_capacity(COMMAND_CAPACITY);
    let mut previous_tick = 0;
    for i in 0..count {
        let sequence = input.number()?;
        let tick = input.number()?;
        let at = input.position()?;
        let before = input.cell()?;
        let after = input.cell()?;
        let status = match input.count(3)? {
            0 => EditStatus::Applied,
            1 => EditStatus::NoChange,
            2 => EditStatus::Conflict,
            _ => EditStatus::Protected,
        };
        super::super::backend::validate(before)?;
        super::super::backend::validate(after)?;
        if sequence != first + i as u64 + 1
            || tick == 0
            || tick > completed
            || tick < previous_tick
            || !published.contains(at)
            || (status == EditStatus::Applied) != (before != after)
            || (status == EditStatus::Protected
                && before.material != crate::world::BlockId::Bedrock as u8)
        {
            return Err("invalid CPU edit outcome".into());
        }
        previous_tick = tick;
        outcomes.push_back(EditOutcome {
            sequence,
            tick,
            at,
            before,
            after,
            status,
        });
    }
    Ok((pending, outcomes))
}
