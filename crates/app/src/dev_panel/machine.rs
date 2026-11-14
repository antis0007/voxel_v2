//! Machine inspection and live test operations shared by the F3 readout and
//! the engine bridge (`engine-cli machine …`). Reads go through
//! `factory::probe`; writes reuse the review-fixture placement path.
use engine_content::factory_resources::{NAMES, RESOURCE_COUNT};
use glam::IVec3;
use serde_json::{Value, json};

use crate::player::Player;
use crate::simulation::{Budget, STEP_SECONDS, residency::Residency};
use crate::world::{World, factory::{self, Kind, Probe}};

/// Longest fast-forward per request: 60 s of fixed steps.
pub const MAX_STEPS: u32 = (60.0 / STEP_SECONDS) as u32;

pub fn kind_named(name: &str) -> Option<Kind> {
    let key = name.trim().to_ascii_lowercase().replace(['_', '-'], " ");
    Kind::ALL.into_iter().find(|k| k.name().to_ascii_lowercase() == key
        || format!("{k:?}").to_ascii_lowercase() == key.replace(' ', ""))
}

pub fn resource_named(key: &str) -> Option<usize> {
    if let Ok(index) = key.parse::<usize>() { return (index < RESOURCE_COUNT).then_some(index); }
    if let Some(id) = engine_content::factory_resources::resource_table().id(key.trim()) { return Some(id); }
    let key = key.trim().to_ascii_lowercase().replace('_', " ");
    NAMES.iter().position(|n| n.to_ascii_lowercase() == key)
}

fn stocks(values: &[u32; RESOURCE_COUNT]) -> Value {
    Value::Object(values.iter().enumerate().filter(|(_, v)| **v > 0)
        .map(|(i, &v)| (NAMES[i].to_string(), json!(v))).collect())
}

/// Kinds as `[id, name, flags]`; flags: p powered, r processor, s structural, x excavates.
pub fn kinds() -> Value {
    Value::Array(Kind::ALL.into_iter().map(|k| {
        let flags: String = [(k.powered(), 'p'), (k.processor(), 'r'), (k.structural(), 's'), (k.excavates(), 'x')]
            .into_iter().filter_map(|(on, c)| on.then_some(c)).collect();
        json!([k as u32, k.name(), flags])
    }).collect())
}

/// Drops zero, false, null, empty and NaN values so agents read only live state.
fn trimmed(map: serde_json::Map<String, Value>) -> Value {
    Value::Object(map.into_iter().filter(|(_, v)| match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }).collect())
}

/// Machine state with defaults omitted. `on` is listed only when the machine is off.
pub fn json(p: &Probe) -> Value {
    let n = &p.node;
    let report = json!({
        "at": p.pos.to_array(), "kind": n.kind.name(), "off": !n.enabled, "status": n.status,
        "mode": n.mode, "upgrade": n.upgrade, "recipe": n.recipe, "filter": n.filter, "target": n.target,
        "stock": n.stock, "energy": n.energy, "burn": n.burn, "progress": n.progress, "flow_kw": n.flow,
        "water": n.water, "formed": n.formed,
        "shell": (n.structure.1 > 0).then(|| [n.structure.0, n.structure.1]),
        "facing": (n.facing != IVec3::ZERO).then(|| n.facing.to_array()),
        "in": stocks(&n.input), "out": stocks(&n.output),
        // Gas masses by species index (3 fuel gas, 4 steam, 5 oxygen); omitted when all zero.
        "exhaust": n.exhaust.iter().any(|v| *v > 0).then_some(n.exhaust),
        "residue": n.residue.iter().any(|v| *v > 0).then_some(n.residue),
        "grid": p.grid.map(|g| trimmed(serde_json::Map::from_iter([
            ("gen_kw".into(), json!(g.generation)), ("load_kw".into(), json!(g.load)),
            ("battery_kw".into(), json!(g.battery_flow)), ("stored".into(), json!(g.stored)),
            ("capacity".into(), json!(g.capacity)), ("machines".into(), json!(g.machines)),
            ("tier".into(), json!(g.tier))]))),
        "shell_of": p.shell_of.map(|k| k.name()),
        "pool": p.pool.map(|(stored, capacity, rate)| [stored, capacity, rate]),
        "controller": p.controller, "lab": p.lab_work.map(|(name, f)| json!([name, f])),
    });
    match report { Value::Object(map) => trimmed(map), other => other }
}

/// Fields that changed between two `json` reports, as `{field: [before, after]}`.
pub fn delta(before: &Value, after: &Value) -> Value {
    let empty = serde_json::Map::new();
    let (a, b) = (before.as_object().unwrap_or(&empty), after.as_object().unwrap_or(&empty));
    let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).filter(|k| *k != "at").collect();
    Value::Object(keys.into_iter().filter_map(|k| {
        let (x, y) = (a.get(k).unwrap_or(&Value::Null), b.get(k).unwrap_or(&Value::Null));
        (x != y).then(|| (k.clone(), json!([x, y])))
    }).collect())
}

/// Compact lines for the F3 readout; empty when nothing is targeted.
pub fn lines(p: &Probe) -> Vec<String> {
    let n = &p.node;
    let mut out = vec![format!("Machine {}  @ {} {} {}{}", n.kind.name(), p.pos.x, p.pos.y, p.pos.z,
        if n.enabled { "" } else { "  (off)" })];
    let mut state = format!("mode {}  upg {}  progress {}", n.mode, n.upgrade, n.progress);
    if let Some(recipe) = n.recipe { state += &format!("  recipe {recipe}"); }
    if !n.status.is_empty() { state += &format!("  · {}", n.status); }
    out.push(state);
    if n.kind.powered() || n.energy > 0 || n.flow != 0 {
        out.push(format!("energy {}  flow {} kW  burn {}", n.energy, n.flow, n.burn));
    }
    let list = |values: &[u32; RESOURCE_COUNT]| values.iter().enumerate().filter(|(_, v)| **v > 0)
        .map(|(i, v)| format!("{} {v}", NAMES[i])).collect::<Vec<_>>().join(", ");
    let (input, output) = (list(&n.input), list(&n.output));
    if !input.is_empty() { out.push(format!("in  {input}")); }
    if !output.is_empty() { out.push(format!("out {output}")); }
    if let Some(g) = p.grid {
        out.push(format!("grid gen {} / load {} kW  stored {}/{}  tier {}", g.generation, g.load, g.stored, g.capacity, g.tier));
    }
    if n.structure.1 > 0 { out.push(format!("shell {}/{}{}", n.structure.0, n.structure.1, if n.formed { " formed" } else { "" })); }
    out
}

pub fn nearby(w: &World, centre: IVec3, radius: i32, kind: Option<Kind>) -> Value {
    let r2 = i64::from(radius) * i64::from(radius);
    let mut found: Vec<_> = w.factory.nodes.iter()
        .filter(|(p, n)| kind.is_none_or(|k| n.kind == k) && (**p - centre).as_i64vec3().length_squared() <= r2)
        .map(|(p, n)| (*p, n)).collect();
    found.sort_by_key(|(p, _)| ((*p - centre).as_i64vec3().length_squared(), p.x, p.y, p.z));
    let total = found.len();
    // Rows are [x, y, z, kind, progress, energy] plus status when set and "off" when disabled.
    let rows: Vec<_> = found.into_iter().take(64).map(|(p, n)| {
        let mut row = vec![json!(p.x), json!(p.y), json!(p.z), json!(n.kind.name()), json!(n.progress), json!(n.energy)];
        if !n.status.is_empty() { row.push(json!(n.status)); }
        if !n.enabled { row.push(json!("off")); }
        Value::Array(row)
    }).collect();
    let mut out = json!({"rows": rows, "cols": "x y z kind progress energy [status] [off]"});
    if total > 64 { out["total"] = json!(total); }
    out
}

/// Fixed-step fast-forward of power and machine service, as the live
/// simulation runs it. A private residency means terrain edits by drills and
/// quarries apply to resident chunks only.
pub fn step(w: &mut World, player: &Player, ticks: u32) {
    let (mut sources, mut cursor) = (Residency::default(), 0usize);
    let (mut blocks, mut chunks) = (Default::default(), Default::default());
    for _ in 0..ticks {
        factory::tick_power(w, STEP_SECONDS);
        factory::tick_world(w, player, STEP_SECONDS, &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
    }
}
