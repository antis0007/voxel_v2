//! Static Blender meshes, shared by all chunk builders. No per-frame rebuilds.
use super::{ChunkMesh, Detail, Kind, STONE_KIND, TUFT_KIND, Vec3, Vertex3};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Species {
    Grass,
    Clover,
    Flower,
    Fern,
    Mushroom,
}

#[derive(Deserialize)]
struct PaintedVertex {
    p: [f32; 3],
    n: [f32; 3],
    c: [f32; 3],
}
#[derive(Deserialize)]
pub(super) struct Mesh {
    vertices: Vec<PaintedVertex>,
    indices: Vec<u32>,
    two_sided: bool,
}
#[derive(Deserialize)]
struct Library {
    assets: Vec<Mesh>,
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        ron::from_str(include_str!(
            "../../../../../assets/groundcover/library.ron"
        ))
        .expect("embedded Blender groundcover library must be valid")
    })
}

impl Mesh {
    pub(super) fn vertices(&self) -> usize {
        self.vertices.len()
    }
    pub(super) fn indices(&self) -> usize {
        self.indices.len() * if self.two_sided { 2 } else { 1 }
    }
}

pub(super) fn mesh(species: Species, variant: usize, lod: u8) -> &'static Mesh {
    &library().assets[species as usize * 4 + variant * 2 + usize::from(lod.min(1))]
}

pub(super) fn append(out: &mut ChunkMesh, d: &Detail, scale: f32, lod: u8) {
    let Kind::Plant(species) = d.kind else {
        return;
    };
    let source = mesh(species, d.variant(), lod);
    let angle = (d.hash >> 16) as f32 * (std::f32::consts::TAU / 65536.0);
    let (sin, cos) = angle.sin_cos();
    let rotate = |p: Vec3| Vec3::new(p.x * cos + p.z * sin, p.y, p.z * cos - p.x * sin);
    let size = d.size() / scale;
    let base = out.vertices.len() as u32;
    let plant = species != Species::Mushroom;
    for v in &source.vertices {
        let local = Vec3::from_array(v.p);
        let height = (local.y * d.size()).max(0.0);
        out.vertices.push(Vertex3 {
            pos: (d.p + rotate(local) * size).to_array(),
            normal: rotate(Vec3::from_array(v.n)).to_array(),
            color: std::array::from_fn(|i| v.c[i] * d.color[i] * d.pigment),
            surface: [
                0.95,
                height,
                if plant { TUFT_KIND } else { STONE_KIND },
                0.0,
            ],
            ao_strength: 0.76 + (height / 0.16).clamp(0.0, 1.0) * 0.20,
        });
    }
    for t in source.indices.chunks_exact(3) {
        // Opaque folded leaves, two faces. No alpha overdraw or extra draw call.
        out.indices.extend([base + t[0], base + t[1], base + t[2]]);
        if source.two_sided {
            out.indices.extend([base + t[2], base + t[1], base + t[0]]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_groundcover_exports_have_valid_geometry_and_bounded_lods() {
        let assets = &library().assets;
        assert_eq!(assets.len(), 20);
        for (asset_index, pair) in assets.chunks_exact(2).enumerate() {
            let bounds = |m: &Mesh| {
                m.vertices.iter().fold(
                    (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                    |(min, max), v| {
                        (
                            min.min(Vec3::from_array(v.p)),
                            max.max(Vec3::from_array(v.p)),
                        )
                    },
                )
            };
            for m in pair {
                assert!(!m.vertices.is_empty() && m.indices.len().is_multiple_of(3));
                assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
                for v in &m.vertices {
                    assert!(Vec3::from_array(v.p).is_finite());
                    assert!((Vec3::from_array(v.n).length() - 1.0).abs() < 0.002);
                    assert!(v.c.iter().all(|&c| (0.0..=1.0).contains(&c)));
                }
                for t in m.indices.chunks_exact(3) {
                    let p = |i: usize| Vec3::from_array(m.vertices[t[i] as usize].p);
                    assert!((p(1) - p(0)).cross(p(2) - p(0)).length_squared() > 1e-14);
                }
            }
            assert!(pair[1].indices.len() <= pair[0].indices.len());
            let (lo, hi) = bounds(&pair[0]);
            let (_, lod_hi) = bounds(&pair[1]);
            assert!(lo.y >= -0.02 && hi.y < 0.8);
            assert!(
                (hi.y - lod_hi.y).abs() < 0.035,
                "asset {asset_index}: LOD must retain plant height: {} -> {}",
                hi.y,
                lod_hi.y
            );
        }
    }
}
