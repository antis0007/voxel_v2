//! Wildflowers with a bent stem, a cupped head and paired basal leaves.
//! All dimensions are metres; ownership and wind remain the parent tuft's.
use super::{ChunkMesh, Detail, TUFT_KIND, Vec3, Vertex3};

// Six petals, two stem planes, two leaves and the heart: eleven quads.
pub(super) const VERTICES: usize = 44;
pub(super) const INDICES: usize = 132;

pub(super) fn append(mesh: &mut ChunkMesh, d: &Detail, scale: f32) {
    let seed = ((d.hash >> 8) & 255) as f32 / 255.0;
    let angle = (d.hash & 65535) as f32 * (std::f32::consts::TAU / 65536.0);
    let lean = Vec3::new(angle.cos(), 0.0, angle.sin());
    let h = (0.34 + seed * 0.22) / scale;
    let center = d.p + Vec3::Y * h + lean * (0.045 / scale);
    let gold = d.hash & 8 == 0;
    let petal = if gold { [0.86, 0.65, 0.13] } else { [0.80, 0.79, 0.66] };
    let stem = [0.25, 0.33, 0.20];
    let mut quad = |points: [Vec3; 4], color: [f32; 3], normal: Vec3, tip_gain: f32| {
        let base = mesh.vertices.len() as u32;
        for (i, p) in points.into_iter().enumerate() {
            let gain = if i == 1 || i == 2 { tip_gain } else { 1.0 };
            mesh.vertices.push(Vertex3 {
                pos: p.to_array(),
                color: color.map(|c| c * gain),
                normal: normal.to_array(),
                surface: [0.95, ((p.y - d.p.y) * scale).max(0.0), TUFT_KIND, 0.0],
                ao_strength: 0.72 + ((p.y - d.p.y) / h).clamp(0.0, 1.0) * 0.22,
            });
        }
        mesh.indices.extend([base, base+1, base+2, base, base+2, base+3,
            base+2, base+1, base, base+3, base+2, base]);
    };
    for axis in [Vec3::X, Vec3::Z] {
        let width = axis * (0.007 / scale);
        quad([d.p-width, d.p+width, center+width, center-width], stem,
            (axis + Vec3::Y * 0.25).normalize(), 1.0);
    }
    for side in [-1.0, 1.0] {
        let out = lean * side;
        let across = Vec3::new(-out.z, 0.0, out.x) * (0.025 / scale);
        let root = d.p + Vec3::Y * h * 0.27;
        let tip = root + out * (0.15 / scale) + Vec3::Y * h * 0.16;
        quad([root, tip-across, tip+out*(0.06/scale), tip+across],
            [0.30, 0.39, 0.23], (Vec3::Y-out*0.45).normalize(), 1.08);
    }
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::TAU / 6.0 + angle;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let across = Vec3::new(-a.sin(), 0.0, a.cos()) * (0.025 / scale);
        let reach = (0.060 + seed * 0.025) / scale;
        let tip = center + out * reach + Vec3::Y * ((0.014 + seed * 0.015) / scale);
        quad([center-across*0.28, tip-across*0.82, tip+across*0.82, center+across*0.28],
            petal, (Vec3::Y-out*0.45).normalize(), 1.04);
    }
    let r = 0.019 / scale;
    let c = center + Vec3::Y * (0.008 / scale);
    quad([c+Vec3::new(-r,0.0,-r), c+Vec3::new(r,0.0,-r),
        c+Vec3::new(r,0.0,r), c+Vec3::new(-r,0.0,r)],
        if gold { [0.29, 0.15, 0.045] } else { [0.66, 0.46, 0.09] }, Vec3::Y, 1.0);
}
