//! Engine Workbench editor for realm layouts (`op: "layout"`). Every room and
//! structure of the Gilded Cave is one `engine_worldgen::gilded::Layout`; this
//! module serves its schema and values, applies edits live (the main loop then
//! regenerates resident chunks), rerolls from another seed, jumps the camera to
//! review views and saves the layout as the world's sidecar so it survives a
//! reload. Edits are dev previews: the game never writes a layout by itself.
use crate::{player::Player, world::{World, WorldType}};
use engine_worldgen::gilded::{self, FieldKind, Layout, LAYOUT_FIELDS};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

static REGEN: AtomicBool = AtomicBool::new(false);

/// True once after a layout change; the main loop then rebuilds resident chunks.
pub fn take_regen() -> bool {
    REGEN.swap(false, Ordering::AcqRel)
}

fn kind_name(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Number => "number",
        FieldKind::Toggle => "boolean",
        FieldKind::Choice => "choice",
        FieldKind::Color => "color",
    }
}

fn gilded_seed(w: &World) -> Result<u32, String> {
    if w.world_type != WorldType::Realm(engine_voxel::RealmKind::Gilded) {
        return Err("Layout: open a Gilded Cave world (Engine workbench world 'gilded', or enter through a gilded gate)".into());
    }
    Ok(w.seed)
}

fn values(l: &Layout) -> Value {
    Value::Object(LAYOUT_FIELDS.iter().map(|f| {
        let v = l.get(f.key).unwrap_or(0.0);
        let value = match f.kind {
            FieldKind::Color => json!(format!("#{:06x}", v.max(0.0) as u32)),
            FieldKind::Toggle => json!(v > 0.5),
            _ => json!(v),
        };
        (f.key.to_string(), value)
    }).collect())
}

fn report(seed: u32) -> Value {
    let l = gilded::layout(seed);
    let fields: Vec<Value> = LAYOUT_FIELDS.iter().map(|f| json!({
        "key": f.key, "group": f.group, "label": f.label, "unit": f.unit, "type": kind_name(f.kind),
        "min": f.min, "max": f.max, "step": f.step, "options": f.options, "help": f.help,
    })).collect();
    json!({
        "realm": "gilded", "seed": seed, "overridden": gilded::layout_overridden(seed),
        "fields": fields, "values": values(&l), "views": gilded::REVIEW_VIEWS,
        "plan": gilded::plan_summary(seed), "sidecar": FILE,
        "scope": "Live preview: resident chunks regenerate; player edits in them are kept. Save writes the world sidecar.",
    })
}

/// Parses one JSON value for a field: numbers, booleans, or "#rrggbb" colours.
fn parse(kind: FieldKind, value: &Value) -> Option<f32> {
    match (kind, value) {
        (FieldKind::Color, Value::String(s)) => u32::from_str_radix(s.trim_start_matches('#'), 16).ok().map(|v| v as f32),
        (_, Value::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
        (_, Value::Number(n)) => n.as_f64().map(|v| v as f32),
        _ => None,
    }
}

pub const FILE: &str = "gilded_layout.ron";

/// Loads a Gilded Cave world's saved layout, if it has one. A damaged file
/// leaves the seed's own layout and logs why.
pub fn load(world_type: WorldType, seed: u32, world_dir: &std::path::Path) {
    if world_type != WorldType::Realm(engine_voxel::RealmKind::Gilded) {
        return;
    }
    let path = world_dir.join(FILE);
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    match ron::from_str::<BTreeMap<String, f32>>(&text) {
        Ok(map) => {
            let mut l = gilded::default_layout(seed);
            for (key, v) in &map {
                if !l.set(key, *v) {
                    eprintln!("[gilded-layout] {}: unknown field {key:?} ignored", path.display());
                }
            }
            gilded::set_layout_override(seed, Some(l));
        }
        Err(e) => eprintln!("[gilded-layout] {}: {e}; using the seed layout", path.display()),
    }
}

fn save(w: &World, seed: u32) -> Result<String, String> {
    let dir = w.regions.view.root.clone();
    if !dir.is_dir() {
        return Err("Layout: this world has no save folder yet; save the world once first".into());
    }
    let path = dir.join(FILE);
    if !gilded::layout_overridden(seed) {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Layout: cannot remove {}: {e}", path.display())),
            _ => Ok("No override: removed the sidecar; the world uses its seed layout".into()),
        };
    }
    let l = gilded::layout(seed);
    let map: BTreeMap<&str, f32> = LAYOUT_FIELDS.iter().map(|f| (f.key, l.get(f.key).unwrap_or(0.0))).collect();
    let text = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| format!("Layout: cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("Layout: cannot publish {}: {e}", path.display()))?;
    Ok(format!("Saved {}", path.display()))
}

fn apply(seed: u32, l: Layout) {
    gilded::set_layout_override(seed, Some(l));
    REGEN.store(true, Ordering::Release);
}

/// `action`: describe (default), set, reset, reroll, save, export, view.
pub fn handle(w: &mut World, player: &mut Player, action: &str, set: &BTreeMap<String, Value>, seed: Option<u32>,
    view: Option<&str>) -> Result<Value, String> {
    let world_seed = gilded_seed(w)?;
    let mut note = Value::Null;
    match action {
        "describe" => {}
        "set" => {
            let mut l = gilded::layout(world_seed);
            for (key, value) in set {
                let field = LAYOUT_FIELDS.iter().find(|f| f.key == key).ok_or_else(|| format!("Layout: unknown field {key:?}"))?;
                let v = parse(field.kind, value).ok_or_else(|| format!("Layout: {key} needs a {}", kind_name(field.kind)))?;
                l.set(key, v);
            }
            apply(world_seed, l);
        }
        "reset" => {
            gilded::set_layout_override(world_seed, None);
            REGEN.store(true, Ordering::Release);
        }
        "reroll" => {
            let from = seed.ok_or("Layout: reroll needs seed")?;
            // Keep the light so a reroll explores shapes, not colours.
            let mut l = gilded::default_layout(from);
            let current = gilded::layout(world_seed);
            for f in LAYOUT_FIELDS.iter().filter(|f| f.group == "Light") {
                l.set(f.key, current.get(f.key).unwrap_or(0.0));
            }
            apply(world_seed, l);
        }
        "save" => note = json!(save(w, world_seed)?),
        "export" => {
            let l = gilded::layout(world_seed);
            let map: BTreeMap<&str, f32> = LAYOUT_FIELDS.iter().map(|f| (f.key, l.get(f.key).unwrap_or(0.0))).collect();
            note = json!(ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?);
        }
        "view" => {
            let name = view.ok_or("Layout: view needs a view name")?;
            let (eye, target) = gilded::review_view(world_seed, name).ok_or_else(|| format!("Layout: unknown view {name:?}"))?;
            if !w.rules.mode.flight() {
                return Err("Layout: views need creative or spectator mode".into());
            }
            let scale = w.voxel_scale.meters();
            let eye_offset = player.eye(w.voxel_scale) - player.pos;
            player.pos = eye / scale - eye_offset;
            player.precise_pos = player.pos.as_dvec3();
            player.vel = glam::Vec3::ZERO;
            player.anchor = None;
            player.gravity_frame = glam::Quat::IDENTITY;
            let direction = (target - eye).normalize_or_zero();
            player.yaw = direction.x.atan2(-direction.z);
            player.pitch = direction.y.clamp(-0.99, 0.99).asin();
            note = json!(format!("Camera at {name}"));
        }
        other => return Err(format!("Layout: unknown action {other:?}; use describe, set, reset, reroll, save, export or view")),
    }
    let mut out = report(world_seed);
    if let Value::Object(map) = &mut out {
        map.insert("note".into(), note);
    }
    Ok(out)
}
