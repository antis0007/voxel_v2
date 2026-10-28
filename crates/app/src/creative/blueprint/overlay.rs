//! Line overlay for the selection box and the armed paste ghost.
use super::{BlueprintTool, ToolKind};
use crate::mesh::LineVertex;
use glam::{IVec3, Vec3};

const SELECTION_COLOR: [f32; 4] = [1.0, 0.82, 0.32, 0.9];
const PASTE_COLOR: [f32; 4] = [0.3, 0.85, 1.0, 0.75];
const PASTE_BOX_COLOR: [f32; 4] = [0.3, 0.85, 1.0, 0.35];

impl BlueprintTool {
    /// Selection box while the Area Tool is held, and the armed paste ghost.
    pub fn overlay(&self, tool: ToolKind, eye: Vec3, nudge: f32, out: &mut Vec<LineVertex>) {
        let mut push = |a: Vec3, b: Vec3, color: [f32; 4]| {
            for p in [a, b] {
                let p = p + (eye - p).normalize_or_zero() * nudge;
                out.push(LineVertex { pos: p.to_array(), color });
            }
        };
        if tool == ToolKind::AreaTool
            && let Some(region) = self.selection()
        {
            for [a, b] in box_edges(region.min.as_vec3(), (region.max + IVec3::ONE).as_vec3()) {
                push(a, b, SELECTION_COLOR);
            }
        }
        if let (Some(origin), Some(bp)) = (self.origin, self.clipboard.as_deref()) {
            let o = origin.as_vec3();
            for [a, b] in box_edges(o, o + bp.size().as_vec3()) {
                push(a, b, PASTE_BOX_COLOR);
            }
            for [a, b] in &self.ghost {
                push(o + *a, o + *b, PASTE_COLOR);
            }
        }
    }
}

fn box_edges(min: Vec3, max: Vec3) -> [[Vec3; 2]; 12] {
    let c = |x: bool, y: bool, z: bool| Vec3::new(if x { max.x } else { min.x }, if y { max.y } else { min.y }, if z { max.z } else { min.z });
    let mut out = [[Vec3::ZERO; 2]; 12];
    let mut i = 0;
    for a in [false, true] {
        for b in [false, true] {
            out[i] = [c(false, a, b), c(true, a, b)];
            out[i + 1] = [c(a, false, b), c(a, true, b)];
            out[i + 2] = [c(a, b, false), c(a, b, true)];
            i += 3;
        }
    }
    out
}
