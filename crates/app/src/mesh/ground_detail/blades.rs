//! Procedural low-poly grass clumps and leaf litter. A blade is one or three
//! triangles, so a turf column costs a fraction of an authored tussock.
use super::{ChunkMesh, Detail, TUFT_KIND, Vec3, Vertex3};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Blades {
    Short,
    Medium,
    Bent,
    SeedHead,
    Dense,
    Weed,
    Litter,
}

struct Shape {
    count: u32,
    /// Height range in metres.
    low: f32,
    high: f32,
    width: f32,
    /// Root spread radius in metres.
    spread: f32,
    lean: f32,
    /// Two-segment blades curve; one-segment blades are single triangles.
    curved: bool,
}

impl Blades {
    fn shape(self) -> Shape {
        let s = |count, low, high, width, spread, lean, curved| Shape {
            count,
            low,
            high,
            width,
            spread,
            lean,
            curved,
        };
        match self {
            Self::Short => s(4, 0.09, 0.17, 0.030, 0.09, 0.30, false),
            Self::Medium => s(5, 0.22, 0.36, 0.032, 0.13, 0.32, true),
            Self::Bent => s(5, 0.20, 0.30, 0.032, 0.10, 0.75, true),
            Self::SeedHead => s(3, 0.42, 0.64, 0.016, 0.05, 0.16, true),
            Self::Dense => s(8, 0.13, 0.24, 0.030, 0.12, 0.24, false),
            Self::Weed => s(4, 0.10, 0.15, 0.060, 0.02, 1.15, false),
            Self::Litter => s(4, 0.05, 0.09, 0.060, 0.22, 0.0, false),
        }
    }

    pub fn indices(self) -> usize {
        let shape = self.shape();
        let n = shape.count as usize;
        match self {
            // Weed leaves are quads; litter lies flat and needs no back face.
            Self::Weed => n * 12,
            Self::Litter => n * 6,
            Self::SeedHead => n * (18 + 24),
            _ => n * if shape.curved { 18 } else { 6 },
        }
    }

    pub fn vertices(self) -> usize {
        let shape = self.shape();
        let n = shape.count as usize;
        match self {
            Self::Weed | Self::Litter => n * 4,
            Self::SeedHead => n * (5 + 8),
            _ => n * if shape.curved { 5 } else { 3 },
        }
    }

    /// Tall forms that still read at the coarser near LOD.
    pub fn keeps_at_lod1(self) -> bool {
        matches!(self, Self::Medium | Self::SeedHead | Self::Bent)
    }
}

/// sRGB root and tip colours. Roots sit slightly under the turf value as
/// contact shade; tips sit above it, so a clump never reads as a dark stain.
const ROOT: [f32; 3] = [0.22, 0.31, 0.17];
const LUSH: [f32; 3] = [0.36, 0.50, 0.26];
const SAGE: [f32; 3] = [0.41, 0.50, 0.32];
const STRAW: [f32; 3] = [0.60, 0.57, 0.36];
const WEED: [f32; 3] = [0.30, 0.42, 0.22];
const HEAD: [f32; 3] = [0.70, 0.62, 0.42];
const RUSSET: [f32; 3] = [0.52, 0.39, 0.27];
const LITTER: [[f32; 3]; 3] = [[0.42, 0.33, 0.20], [0.50, 0.42, 0.24], [0.36, 0.34, 0.20]];

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn unit(h: u32, shift: u32) -> f32 {
    ((h >> shift) & 255) as f32 / 255.0
}

fn mix(h: u32, salt: u32) -> u32 {
    let mut x = h ^ salt.wrapping_mul(0x9e37_79b9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^ (x >> 15)
}

pub(super) fn append(out: &mut ChunkMesh, d: &Detail, kind: Blades, dry: f32, scale: f32) {
    let shape = kind.shape();
    let inv = 1.0 / scale;
    let yaw = unit(d.hash, 16) * std::f32::consts::TAU;
    // A clump leans together a little; Bent clumps lean hard one way.
    let clump_lean = Vec3::new(yaw.cos(), 0.0, yaw.sin());
    let tip_col = lerp3(lerp3(LUSH, SAGE, unit(d.hash, 4)), STRAW, dry * 0.6);
    for i in 0..shape.count {
        let h = mix(d.hash, i + 1);
        let around = yaw + i as f32 * 2.399_963 + (unit(h, 0) - 0.5) * 0.8;
        let radial = Vec3::new(around.cos(), 0.0, around.sin());
        let r = shape.spread * (0.25 + 0.75 * unit(h, 8).sqrt());
        let root = d.p + radial * (r * inv);
        let height = (shape.low + (shape.high - shape.low) * unit(h, 16)) * d.size();
        let lean_dir = match kind {
            Blades::Bent => (clump_lean * 0.8 + radial * 0.4).normalize(),
            _ => (radial + clump_lean * 0.35).normalize(),
        };
        let lean = (shape.lean * (0.6 + 0.8 * unit(h, 24))).min(1.3);
        let pigment = d.pigment * (0.94 + 0.12 * unit(h, 12));
        match kind {
            Blades::Litter => litter(out, root, around, height, shape.width, h, inv),
            Blades::Weed => weed_leaf(out, root, lean_dir, height, shape.width, lean, pigment, inv),
            _ => {
                let tip = if kind == Blades::Dense { lerp3(LUSH, ROOT, 0.25) } else { tip_col };
                let tip = tip.map(|c| c * pigment);
                let top = blade(out, root, lean_dir, height, shape.width, lean, shape.curved, tip, inv);
                if kind == Blades::SeedHead {
                    let head = if unit(h, 28) > 0.7 { RUSSET } else { HEAD };
                    seed_head(out, top, lean_dir, height, head.map(|c| c * pigment), inv);
                }
            }
        }
    }
}

fn vertex(pos: Vec3, normal: Vec3, color: [f32; 3], height: f32) -> Vertex3 {
    Vertex3 {
        pos: pos.to_array(),
        normal: normal.to_array(),
        color,
        surface: [0.95, height, TUFT_KIND, 0.0],
        ao_strength: 0.74 + (height / 0.18).clamp(0.0, 1.0) * 0.24,
    }
}

fn two_sided(out: &mut ChunkMesh, tris: &[[u32; 3]]) {
    for t in tris {
        out.indices.extend(*t);
        out.indices.extend([t[2], t[1], t[0]]);
    }
}

/// Tapered blade bending toward `dir`. Returns the tip, in voxel units.
#[allow(clippy::too_many_arguments)]
fn blade(
    out: &mut ChunkMesh,
    root: Vec3,
    dir: Vec3,
    height: f32,
    width: f32,
    lean: f32,
    curved: bool,
    tip_col: [f32; 3],
    inv: f32,
) -> Vec3 {
    let side = Vec3::new(-dir.z, 0.0, dir.x) * (width * 0.5 * inv);
    // Lit like a soft upward surface: the face normal of a thin blade flips
    // between bright and black as the camera walks past it.
    let normal = (Vec3::Y * 0.75 + dir * 0.35).normalize();
    let tip = root + (Vec3::Y * lean.cos() + dir * lean.sin()) * (height * inv);
    let base = out.vertices.len() as u32;
    out.vertices.push(vertex(root - side, normal, ROOT, 0.0));
    out.vertices.push(vertex(root + side, normal, ROOT, 0.0));
    if curved {
        let mid_h = height * 0.55;
        let mid = root + (Vec3::Y * (lean * 0.45).cos() + dir * (lean * 0.45).sin()) * (mid_h * inv);
        let mid_col = lerp3(ROOT, tip_col, 0.6);
        out.vertices.push(vertex(mid - side * 0.65, normal, mid_col, mid_h));
        out.vertices.push(vertex(mid + side * 0.65, normal, mid_col, mid_h));
        out.vertices.push(vertex(tip, normal, tip_col, height));
        two_sided(out, &[[base, base + 1, base + 2], [base + 1, base + 3, base + 2], [base + 2, base + 3, base + 4]]);
    } else {
        out.vertices.push(vertex(tip, normal, tip_col, height));
        two_sided(out, &[[base, base + 1, base + 2]]);
    }
    tip
}

/// Small drooping spikelet: two crossed diamonds at the blade tip.
fn seed_head(out: &mut ChunkMesh, top: Vec3, dir: Vec3, height: f32, col: [f32; 3], inv: f32) {
    let len = 0.07 * inv;
    let down = (dir * 0.6 - Vec3::Y * 0.2).normalize() * len;
    let sides = [Vec3::new(-dir.z, 0.0, dir.x), Vec3::Y];
    let base = out.vertices.len() as u32;
    for s in sides {
        let s = s * (0.012 * inv);
        out.vertices.push(vertex(top, Vec3::Y, col, height));
        out.vertices.push(vertex(top + down * 0.5 + s, Vec3::Y, col, height));
        out.vertices.push(vertex(top + down, Vec3::Y, col, height));
        out.vertices.push(vertex(top + down * 0.5 - s, Vec3::Y, col, height));
    }
    let q = |o: u32| [[base + o, base + o + 1, base + o + 2], [base + o, base + o + 2, base + o + 3]];
    let [a, b] = q(0);
    let [c, e] = q(4);
    two_sided(out, &[a, b, c, e]);
}

/// Broad rosette leaf held low, as plantain and dock grow in shade and trampled turf.
#[allow(clippy::too_many_arguments)]
fn weed_leaf(out: &mut ChunkMesh, root: Vec3, dir: Vec3, length: f32, width: f32, lean: f32, pigment: f32, inv: f32) {
    let reach = (Vec3::Y * lean.cos() + dir * lean.sin()) * (length * inv);
    let side = Vec3::new(-dir.z, 0.0, dir.x) * (width * 0.5 * inv);
    let normal = (Vec3::Y * 0.8 + dir * 0.2).normalize();
    let col = WEED.map(|c| c * pigment);
    let tip_h = reach.y / inv;
    let base = out.vertices.len() as u32;
    out.vertices.push(vertex(root, normal, lerp3(ROOT, col, 0.4), 0.0));
    out.vertices.push(vertex(root + reach * 0.5 - side, normal, col, tip_h * 0.5));
    out.vertices.push(vertex(root + reach, normal, col, tip_h));
    out.vertices.push(vertex(root + reach * 0.5 + side, normal, col, tip_h * 0.5));
    two_sided(out, &[[base, base + 1, base + 2], [base, base + 2, base + 3]]);
}

/// Fallen leaf lying on the soil. Seen from above only, so one face.
fn litter(out: &mut ChunkMesh, root: Vec3, angle: f32, length: f32, width: f32, h: u32, inv: f32) {
    let dir = Vec3::new(angle.cos(), 0.0, angle.sin()) * (length * 0.5 * inv);
    let side = Vec3::new(-dir.z, 0.0, dir.x).normalize_or_zero() * (width * 0.5 * inv);
    let lift = Vec3::Y * (0.012 * inv);
    let col = LITTER[(h % 3) as usize];
    let c = root + lift;
    let base = out.vertices.len() as u32;
    for p in [c - dir, c - side, c + dir, c + side] {
        out.vertices.push(Vertex3 {
            pos: p.to_array(),
            normal: Vec3::Y.to_array(),
            color: col,
            surface: [0.95, 0.0, super::STONE_KIND, 0.0],
            ao_strength: 0.70,
        });
    }
    // Counter-clockwise seen from above.
    out.indices.extend([base, base + 3, base + 2, base, base + 2, base + 1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Blades; 7] = [
        Blades::Short,
        Blades::Medium,
        Blades::Bent,
        Blades::SeedHead,
        Blades::Dense,
        Blades::Weed,
        Blades::Litter,
    ];

    fn detail(hash: u32) -> Detail {
        Detail { p: Vec3::new(3.5, 10.0, -7.5), hash, kind: super::super::Kind::Stone, color: [0.3; 3], pigment: 1.0, growth: 1.0, dry: 0.0 }
    }

    #[test]
    fn every_clump_matches_its_planned_size_and_stays_low_and_local() {
        for kind in ALL {
            for hash in [1u32, 0xdead_beef, 0x1234_5678, u32::MAX] {
                for scale in [1.0, 0.5, 0.25] {
                    let mut mesh = ChunkMesh::empty();
                    let d = detail(hash);
                    append(&mut mesh, &d, kind, 0.5, scale);
                    assert_eq!(mesh.vertices.len(), kind.vertices(), "{kind:?}");
                    assert_eq!(mesh.indices.len(), kind.indices(), "{kind:?}");
                    assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()));
                    for v in &mesh.vertices {
                        let p = (Vec3::from_array(v.pos) - d.p) * scale;
                        assert!(p.is_finite() && p.y >= -0.001 && p.y < 0.8, "{kind:?} {p}");
                        assert!(p.x.abs() < 0.6 && p.z.abs() < 0.6, "{kind:?} {p}");
                        assert!(v.color.iter().all(|&c| (0.0..=1.0).contains(&c)));
                        assert!((Vec3::from_array(v.normal).length() - 1.0).abs() < 0.01);
                    }
                }
            }
        }
    }
}
