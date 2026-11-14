//! Headless factory lab (`--factory-lab`): the engine bridge's operations on a
//! private superflat world, without a window, GPU or save. One JSON request per
//! stdin line, one JSON reply per stdout line. Scenario files drive it through
//! `tools/axiom-dev/factory-lab-cli.mjs`. `--processes=FILE` and
//! `--fabrication=FILE` read those tables from disk instead of the compiled copy.
use serde_json::{Value, json};

use super::{engine::Engine, machine};
use crate::fluid::VoxelScalePreset;
use crate::player::Player;
use crate::world::{DayNightCycle, World, WorldType, factory::{Kind, recipes}, inventory::Item};

/// Recipe ids probed per machine; ids are sparse, so this is a scan bound.
const RECIPE_SCAN: usize = 96;

fn fresh() -> World {
    World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 1)
}

fn stack((resource, units): (usize, u32)) -> Value {
    json!([Item::Resource(resource as u8).name(), units])
}

/// Processes of one machine kind as `[recipe, inputs, block, output, byproduct, ticks, kw]` rows.
fn recipe_rows(kind: Kind) -> Value {
    let rows: Vec<Value> = (0..RECIPE_SCAN).filter_map(|id| {
        let p = recipes::process(kind, id)?;
        let inputs: Vec<Value> = p.inputs.iter().filter(|(_, n)| *n > 0).map(|&i| stack(i)).collect();
        Some(json!([id, inputs, p.block_input.map(|m| format!("{m:?}")), stack(p.output), p.byproduct.map(stack), p.ticks, p.power]))
    }).collect();
    json!({"kind": kind.name(), "cols": "recipe inputs[name,units] block output[name,units] byproduct ticks kw", "rows": rows,
        "modes": kind.modes(), "units": "64 per item; one tick 0.25 s"})
}

fn lab_op(request: &Value, world: &mut World) -> Option<Result<Value, String>> {
    match request.get("op")?.as_str()? {
        "reset" => {
            *world = fresh();
            Some(Ok(json!({"reset": true})))
        }
        "recipes" => Some(match request.get("kind").and_then(Value::as_str) {
            Some(name) => machine::kind_named(name).map(recipe_rows).ok_or(format!("Unknown machine kind {name}; see machine_kinds")),
            None => Ok(Value::Array(Kind::ALL.into_iter().map(recipe_rows)
                .filter(|r| r["rows"].as_array().is_some_and(|rows| !rows.is_empty())).collect())),
        }),
        _ => None,
    }
}

pub fn run_headless() -> anyhow::Result<()> {
    use std::io::{BufRead, Write};
    let draft = |flag: &str| std::env::args().find_map(|a| a.strip_prefix(flag).map(String::from))
        .map(std::fs::read_to_string).transpose();
    if let Err(error) = recipes::load_drafts(draft("--processes=")?, draft("--fabrication=")?) {
        println!("{}", json!({"ok": false, "fatal": error}));
        return Ok(());
    }
    let (mut world, mut player, mut clock) = (fresh(), Player::new(), DayNightCycle::default());
    let mut engine = Engine::default();
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let lab = serde_json::from_str::<Value>(&line).ok().and_then(|request| lab_op(&request, &mut world));
        let reply = match lab {
            Some(Ok(data)) => json!({"ok": true, "data": data}),
            Some(Err(error)) => json!({"ok": false, "error": error}),
            None => engine.handle(&line, Some(&mut world), &mut player, &mut clock),
        };
        writeln!(out, "{reply}")?;
        out.flush()?;
    }
    Ok(())
}
