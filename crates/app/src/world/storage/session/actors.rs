use super::super::{DayNightCycle, Player, World};
use crate::player::Input;
use anyhow::{Result, ensure};

#[derive(serde::Serialize)]
struct Borrowed<'a> {
    player: &'a Player,
    input: &'a Input,
    day: &'a DayNightCycle,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Actors {
    pub player: Player,
    pub input: Input,
    pub day: DayNightCycle,
}

fn valid(player: &Player, input: &Input, day: &DayNightCycle) -> bool {
    player.checkpoint_valid() && input.checkpoint_valid()
        && (0.0..1.0).contains(&day.time_of_day)
        && (1.0..=86400.0).contains(&day.day_length_seconds)
        && day.cloud_controls == crate::world::sanitize_cloud_controls(day.cloud_controls)
}

pub(super) fn encode(player: &Player, input: &Input, day: &DayNightCycle) -> Result<Vec<u8>> {
    ensure!(valid(player, input, day), "Invalid actor state in exact session");
    let bytes = ron::Options::default().with_recursion_limit(64)
        .to_string(&Borrowed { player, input, day })?.into_bytes();
    ensure!(bytes.len() <= super::format::MAX_ACTOR, "Actor snapshot exceeds size limit");
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8]) -> Result<Actors> {
    let state: Actors = ron::Options::default().with_recursion_limit(64).from_bytes(bytes)?;
    ensure!(valid(&state.player, &state.input, &state.day), "Invalid saved actor state");
    Ok(state)
}

pub(super) fn model(world: &World) -> u64 {
    let sources: &[&[u8]] = &[
        include_bytes!("../../../player.rs"),
        include_bytes!("../../../player/movement.rs"),
        include_bytes!("../../../player/traversal.rs"),
        include_bytes!("../../../player/radial.rs"),
        include_bytes!("../../../player/viewmodel.rs"),
        include_bytes!("../../../player/checkpoint.rs"),
        include_bytes!("../../../simulation/fixed.rs"),
        include_bytes!("../../../simulation/tools.rs"),
        include_bytes!("../../collision.rs"),
        include_bytes!("../../../world.rs"),
        include_bytes!("actors.rs"),
    ];
    let hash = sources.iter().flat_map(|source| source.iter()).fold(0xcbf29ce484222325_u64,
        |h, byte| (h ^ u64::from(*byte)).wrapping_mul(0x100000001b3));
    hash ^ u64::from(world.generator_version)
}
