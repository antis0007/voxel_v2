//! Assembled blades from the tool station, shaped per schema and coloured by part materials.
use super::{Design, Palette, Mat4, Model, Vertex3, LazyLock, append, extrude, paint_design};

/// Schema id, then blade, guard and grip material ids.
pub(crate) type Parts = [String; 4];

/// Blade schemas: id, the part kinds that colour blade, guard and grip, and the silhouette.
const SHAPES: [(&str, [&str; 3], usize, usize, usize, bool); 4] = [
    ("sword", ["blade", "guard", "handle"], 2, 3, 6, false),
    ("dagger", ["blade", "guard", "handle"], 26, 2, 4, false),
    ("broadsword", ["blade", "large_plate", "tool_rod"], 0, 5, 9, false),
    ("cleaver", ["blade", "large_plate", "tool_rod"], 12, 7, 3, true),
];

/// A bank holds a handful of tools, so the cache stays small.
static MESHES: LazyLock<std::sync::Mutex<std::collections::HashMap<Parts, Vec<Vertex3>>>> = LazyLock::new(Default::default);

/// Schema and part materials when the tool is a blade schema.
pub(crate) fn assembled_parts(tool: &engine_tools::ToolAssembly) -> Option<Parts> {
    let (id, kinds, ..) = SHAPES.iter().find(|s| s.0 == tool.schema)?;
    let part = |kind: &str| tool.parts.iter().find(|(k, _)| k.as_str() == kind).map(|(_, m)| m.0.clone());
    Some([id.to_string(), part(kinds[0])?, part(kinds[1])?, part(kinds[2])?])
}

fn design(parts: &Parts) -> Design {
    let ramp = |m: &str| engine_tools::sprite::SHEET.ramp(m).unwrap_or([[90, 90, 90], [130, 130, 130], [170, 170, 170], [210, 210, 210]]);
    let (blade, guard, grip) = (ramp(&parts[1]), ramp(&parts[2]), ramp(&parts[3]));
    let edge = blade[3].map(|c| c.saturating_add((255 - c) / 2));
    let (_, _, tip, half, guard_half, square) = SHAPES.iter().find(|s| s.0 == parts[0]).copied().unwrap_or(SHAPES[0]);
    Design { tip, half, guard_half, horns: false, square, palette: Palette {
        edge, light: blade[3], mid: blade[2], dark: blade[1], core: blade[1],
        guard: guard[2], guard_dark: guard[1], wrap: grip[2], wrap_dark: grip[1] } }
}

/// Appends an assembled blade with its grip at the frame origin.
pub(crate) fn draw_assembled(m: &mut Model, frame: Mat4, parts: &Parts) {
    let build = || extrude(&paint_design(&design(parts), false));
    match MESHES.lock() {
        Ok(mut cache) => { let mesh = cache.entry(parts.clone()).or_insert_with(build); append(m, frame, mesh); }
        Err(_) => append(m, frame, &build()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn mesh(schema: &str) -> Model {
        let mut m = Model::new(1.0);
        draw_assembled(&mut m, Mat4::IDENTITY, &[schema.into(), "iron".into(), "copper".into(), "wood".into()]);
        m
    }

    fn closed(m: &Model) -> bool {
        m.vertices.chunks_exact(3).fold(Vec3::ZERO, |acc, t| {
            let [a, b, c] = [t[0].pos, t[1].pos, t[2].pos].map(Vec3::from_array);
            acc + (b - a).cross(c - a)
        }).length() < 1e-7
    }

    fn extent(m: &Model, axis: usize) -> f32 {
        let v = m.vertices.iter().map(|v| v.pos[axis]);
        v.clone().fold(f32::MIN, f32::max) - v.fold(f32::MAX, f32::min)
    }

    #[test]
    fn every_blade_schema_is_closed_and_uses_part_colours() {
        // Every blade paints its spine in the ramp's mid tone; narrow ones have no light flank.
        let iron = engine_tools::sprite::SHEET.ramp("iron").map(|r| r[2].map(|c| c as f32 / 255.0));
        for (schema, ..) in SHAPES {
            let m = mesh(schema);
            assert!(!m.vertices.is_empty() && closed(&m), "{schema}");
            assert!(m.vertices.iter().any(|v| Some(v.color) == iron), "{schema}");
        }
    }

    #[test]
    fn schemas_have_their_own_proportions() {
        let (sword, dagger, broad, cleaver) = (mesh("sword"), mesh("dagger"), mesh("broadsword"), mesh("cleaver"));
        assert!(extent(&dagger, 1) < extent(&sword, 1) * 0.7);
        assert!(extent(&broad, 1) > extent(&sword, 1) && extent(&broad, 0) > extent(&sword, 0));
        // Chopping head: wider than a broadsword blade, shorter than a sword.
        let blade_width = |m: &Model| { let top = m.vertices.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max); m.vertices.iter().filter(|v| v.pos[1] > top - 0.02).map(|v| v.pos[0].abs()).fold(0.0, f32::max) };
        assert!(blade_width(&cleaver) > blade_width(&broad) * 2.0);
    }
}
