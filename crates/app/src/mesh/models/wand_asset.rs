//! Blender-authored voxel meshes with local paint coordinates.
use super::Model;
use crate::mesh::Vertex3;
use glam::{Mat3, Quat, Vec3};
use serde::Deserialize;

#[derive(Deserialize)]
struct PaintedVertex {
    p: [f32; 3],
    n: [f32; 3],
    c: [f32; 3],
    uv: [f32; 2],
    kind: f32,
    finish: f32,
    #[serde(default)]
    tint: u8,
    #[serde(default)]
    bone: u8,
    #[serde(default)]
    weight: f32,
}

#[derive(Deserialize)]
struct Asset {
    vertices: Vec<PaintedVertex>,
}

pub struct Mesh {
    vertices: Vec<(Vertex3, u8, u8, f32)>,
}

impl Mesh {
    pub fn load(source: &str, label: &str) -> Self {
        match ron::from_str::<Asset>(source) {
            Ok(asset) => {
                if asset.vertices.len() % 3 != 0 || asset.vertices.iter().any(|v| {
                    !Vec3::from_array(v.p).is_finite() || !Vec3::from_array(v.n).is_finite()
                        || Vec3::from_array(v.n).length_squared() < 0.5
                        || v.c.iter().chain(v.uv.iter()).any(|x| !x.is_finite())
                        || !v.finish.is_finite() || !v.weight.is_finite() || v.bone > 5
                }) {
                    eprintln!("[wand-asset] {label}: invalid geometry or skin weights");
                    return Self { vertices: Vec::new() };
                }
                let vertices: Vec<_> = asset.vertices.into_iter().map(|v| (Vertex3 {
                    pos: v.p, normal: v.n, color: v.c,
                    surface: [v.uv[0], v.uv[1], v.kind, v.finish],
                    ao_strength: 1.0,
                }, v.tint, v.bone.min(5), v.weight.clamp(0.0, 1.0))).collect();
                eprintln!("[wand-asset] {label} baked_triangles={}", vertices.len() / 3);
                Self { vertices }
            }
            Err(error) => {
                eprintln!("[wand-asset] {label}: {error}");
                Self { vertices: Vec::new() }
            }
        }
    }

    pub fn draw(&self, model: &mut Model, origin: Vec3, rotation: Quat, stretch: Vec3,
        grip: [f32; 3], spell: [f32; 3]) {
        self.draw_posed(model, origin, rotation, stretch, grip, spell, 0.0);
    }

    pub fn draw_posed(&self, model: &mut Model, origin: Vec3, rotation: Quat, stretch: Vec3,
        grip: [f32; 3], spell: [f32; 3], fingers: f32) {
        if !origin.is_finite() || !rotation.is_finite() || !stretch.is_finite()
            || stretch.min_element() <= 0.0 || !fingers.is_finite() { return; }
        model.vertices.reserve(self.vertices.len());
        let rotation = Mat3::from_quat(rotation);
        let transform = rotation * Mat3::from_diagonal(stretch / model.scale);
        let normals = rotation * Mat3::from_diagonal(stretch.recip());
        let origin = origin / model.scale;
        let bones = super::wand_pose::RIG.bones.map(|b| (
            Vec3::from_array(b.pivot), Quat::from_axis_angle(
                Vec3::from_array(b.axis).normalize_or_zero(), b.angle*fingers)));
        for (source, tint, bone, weight) in &self.vertices {
            let mut vertex = *source;
            let mut position = Vec3::from_array(source.pos);
            let mut normal = Vec3::from_array(source.normal);
            if *bone > 0 && fingers.abs() > 0.0001 {
                let (pivot, joint) = bones[usize::from(*bone)-1];
                position = position.lerp(pivot+joint*(position-pivot), *weight);
                normal = normal.lerp(joint*normal, *weight);
            }
            vertex.pos = (origin + transform * position).to_array();
            vertex.normal = (normals * normal).normalize_or_zero().to_array();
            if *tint != 0 {
                let color = if *tint == 1 { grip } else { spell };
                for (value, tint) in vertex.color.iter_mut().zip(color) { *value *= tint; }
            }
            model.vertices.push(vertex);
        }
    }
}
