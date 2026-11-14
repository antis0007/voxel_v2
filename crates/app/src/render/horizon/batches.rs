//! Spatial index ranges share one vertex/index allocation. Partition off-thread;
//! per-frame visibility needs six cheap plane/AABB checks and no allocations.
use crate::mesh::ChunkMesh;
use glam::{Vec3, Vec4};
use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub enum Variant {Common,Collared,Uncollared}

#[derive(Clone)]
pub struct DrawBatch {
    pub variant:Variant,
    pub indices: Range<u32>,
    center: Vec3,
    extent: Vec3,
}
impl DrawBatch {
    pub fn selected(&self,collared:bool)->bool {
        self.variant==Variant::Common || self.variant==if collared {Variant::Collared}else{Variant::Uncollared}
    }
    pub fn conditional(&self)->bool {self.variant!=Variant::Common}

    /// Whether the batch centre lies inside the globe's chart clip.
    pub fn within_clip(&self, clip: super::clip::ChartClip) -> bool {
        super::clip::within(clip, f64::from(self.center.x), f64::from(self.center.z))
    }

    pub fn visible(&self, eye: Vec3, planes: &[Vec4; 6]) -> bool {
        let center = self.center - eye;
        planes
            .iter()
            .all(|p| p.truncate().dot(center) + p.w + p.truncate().abs().dot(self.extent) >= 0.0)
    }

    pub fn nearby(&self, eye: Vec3, radius: f32) -> bool {
        let delta = ((self.center - eye).abs() - self.extent).max(Vec3::ZERO);
        delta.x * delta.x + delta.z * delta.z <= radius * radius
    }
}

#[cfg(test)]
pub fn partition(mesh: &mut ChunkMesh, scale: f32, origin: (i32, i32)) -> Vec<DrawBatch> {
    partition_variants(mesh,scale,origin,None)
}

pub fn partition_variants(mesh:&mut ChunkMesh,scale:f32,origin:(i32,i32),variants:Option<&super::collar::Ranges>)->Vec<DrawBatch> {
    // Keep both variants of a spatial region adjacent so page packing can
    // share their unchanged vertices instead of uploading two landscapes.
    let mut groups: BTreeMap<(u32, i32, i32, Variant), (Vec<u32>, Vec3, Vec3)> = BTreeMap::new();
    for (number,tri) in mesh.indices.chunks_exact(3).enumerate() {
        let index=(number*3) as u32;
        let variant=variants.map_or(Variant::Common,|v|{
            if v.collared.contains(&index){Variant::Collared}
            else if v.uncollared.contains(&index){Variant::Uncollared}else{Variant::Common}
        });
        let p = [tri[0], tri[1], tri[2]]
            .map(|i| Vec3::from_array(mesh.vertices[i as usize].pos) * scale);
        let center = (p[0] + p[1] + p[2]) / 3.0;
        let reach = (center.x - origin.0 as f32)
            .abs()
            .max((center.z - origin.1 as f32).abs());
        let level = ((reach / 512.0).max(1.0) as u32).ilog2().min(5);
        let width = (256u32 << level) as f32;
        let key = (
            level,
            (center.x / width).floor() as i32,
            (center.z / width).floor() as i32,
            variant,
        );
        let group = groups.entry(key).or_insert_with(|| {
            (
                Vec::new(),
                Vec3::splat(f32::INFINITY),
                Vec3::splat(f32::NEG_INFINITY),
            )
        });
        group.0.extend_from_slice(tri);
        for v in p {
            group.1 = group.1.min(v);
            group.2 = group.2.max(v);
        }
    }
    let mut indices = Vec::with_capacity(mesh.indices.len());
    let mut batches = Vec::with_capacity(groups.len());
    for ((_,_,_,variant), (group, min, max)) in groups {
        let start = indices.len() as u32;
        indices.extend(group);
        batches.push(DrawBatch {
            variant,
            indices: start..indices.len() as u32,
            center: (min + max) * 0.5,
            extent: (max - min) * 0.5 + Vec3::splat(4.0),
        });
    }
    mesh.indices = indices;
    batches
}
