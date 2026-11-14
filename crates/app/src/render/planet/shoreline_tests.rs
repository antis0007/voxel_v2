use super::*;
use std::sync::Mutex;

type Probe = [u32; 7];
type Mesh = (Vec<Vertex3>, Vec<f32>, Vec<[u32; 3]>);

fn terrain(p: Vec3, normal: Vec3, footprint: f32, scale: f32, wet: bool) -> (Vertex3, f32) {
    let x = p.x * scale;
    let z = p.z * scale;
    let bed = if wet { (x * 0.0013).sin() * 6.0 + (z * 0.0017).cos() * 5.0 }
        else { 20.0 + (x * 0.004).sin() * 9.0 + (z * 0.003).cos() * 7.0 } / scale;
    let water = wet && bed < 0.0;
    let material = if water { BlockId::Water } else { BlockId::Stone }.surface();
    (Vertex3 { pos: [p.x, bed.max(0.0), p.z], color: [bed, footprint, 0.3],
        normal: normal.to_array(), surface: [material.roughness, material.specular,
            if water { material.shader_key() } else { 0.0 }, material.shader_key()],
        ao_strength: -1.0 }, bed)
}

fn fixture(side: usize, scale: f32, face: usize, wet: bool) -> Mesh {
    let mut vertices = Vec::new();
    let mut beds = Vec::new();
    let mut triangles = Vec::new();
    for z in 0..=side {
        for x in 0..=side {
            let p = Vec3::new((face as f32 * 65536.0 - 12000.0 + x as f32 * 24000.0 / side as f32) / scale,
                0.0, (-12000.0 + z as f32 * 24000.0 / side as f32) / scale);
            let (v, b) = terrain(p, Vec3::Y, 375.0, scale, wet);
            vertices.push(v);
            beds.push(b);
        }
    }
    for z in 0..side { for x in 0..side {
        let a = (z * (side + 1) + x) as u32;
        let b = a + 1;
        let c = a + side as u32 + 1;
        triangles.extend([[a, c, b], [b, c, c + 1]]);
    }}
    (vertices, beds, triangles)
}

fn run(side: usize, scale: f32, face: usize, ordered: bool, wet: bool, workers: usize) -> (Mesh, Vec<Probe>) {
    let (mut vertices, mut beds, mut triangles) = fixture(side, scale, face, wet);
    let log = Mutex::new(Vec::new());
    let no_scarps = |_: Vec3| false;
    refine(&mut vertices, &mut beds, &mut triangles, 0, side, scale, 0.0, true,
        |_| false, |_| false,
        if ordered { Some(&no_scarps) } else { None }, None, false, workers,
        |p, normal, footprint, dry| {
            assert!(!dry);
            log.lock().unwrap().push([p.x.to_bits(), p.y.to_bits(), p.z.to_bits(),
                normal.x.to_bits(), normal.y.to_bits(), normal.z.to_bits(), footprint.to_bits()]);
            terrain(p, normal, footprint, scale, wet)
        });
    ((vertices, beds, triangles), log.into_inner().unwrap())
}

fn same_mesh(a: &Mesh, b: &Mesh) {
    assert_eq!(bytemuck::cast_slice::<Vertex3, u8>(&a.0), bytemuck::cast_slice::<Vertex3, u8>(&b.0));
    assert_eq!(bytemuck::cast_slice::<f32, u8>(&a.1), bytemuck::cast_slice::<f32, u8>(&b.1));
    assert_eq!(a.2, b.2);
}

fn compare(side: usize, scale: f32, face: usize, wet: bool) {
    let (a, mut ordered) = run(side, scale, face, true, wet, 1);
    let (b, mut batched) = run(side, scale, face, false, wet, 1);
    same_mesh(&a, &b);
    assert!(batched.len() <= 3 * 2048);
    if side == 64 && wet {
        assert_eq!(ordered.len(), 3 * 2048);
        assert_ne!(ordered, batched);
    }
    ordered.sort_unstable();
    batched.sort_unstable();
    assert_eq!(ordered, batched);
    let clip = |mesh: Mesh| {
        let (mut vertices, beds, triangles) = mesh;
        let mut indices = Vec::new();
        let mut edges = Edges::new();
        for triangle in triangles { edges.triangle(triangle, &mut vertices, &mut indices, |i| beds[i as usize]); }
        (vertices, indices)
    };
    let (av, ai) = clip(a);
    let (bv, bi) = clip(b);
    assert_eq!(bytemuck::cast_slice::<Vertex3, u8>(&av), bytemuck::cast_slice::<Vertex3, u8>(&bv));
    assert_eq!(ai, bi);
}

#[test]
fn shoreline_batch_preserves_budget_and_clipped_geometry() { compare(64, 1.0, 0, true); }
#[test]
fn shoreline_batch_preserves_scaled_face_coordinates() { compare(64, 0.25, 5, true); }
#[test]
fn shoreline_batch_preserves_small_work_and_empty_coasts() {
    compare(8, 4.0, 2, true);
    compare(8, 1.0, 0, false);
}
#[test]
fn shoreline_batch_preserves_nonzero_base_and_detail() {
    let compare = |ordered: bool| {
        let (mut vertices, mut beds, mut triangles) = fixture(16, 2.0, 3, true);
        let prefix = vertices[0];
        vertices.splice(0..0, [prefix; 7]);
        for t in &mut triangles { for i in t { *i += 7; } }
        let no_scarps = |_: Vec3| false;
        refine(&mut vertices, &mut beds, &mut triangles, 7, 16, 2.0, 0.0, false,
            |p| p.z < 0.0, |p| p.z >= 0.0,
            if ordered { Some(&no_scarps) } else { None }, None, false, 1,
            |p, n, f, dry| { assert!(!dry); terrain(p, n, f, 2.0, true) });
        assert_eq!(bytemuck::cast_slice::<Vertex3, u8>(&vertices[..7]), bytemuck::cast_slice::<Vertex3, u8>(&[prefix; 7]));
        (vertices, beds, triangles)
    };
    same_mesh(&compare(true), &compare(false));
}

#[test]
fn shoreline_batch_does_not_exceed_unsplit_probe_budget() {
    let (mut vertices, mut beds, mut triangles) = fixture(64, 1.0, 0, false);
    for vertex in &mut vertices { vertex.pos[1] = 1.0; }
    beds.fill(1.0);
    let before = (vertices.clone(), beds.clone(), triangles.clone());
    let calls = Mutex::new(Vec::new());
    refine(&mut vertices, &mut beds, &mut triangles, 0, 64, 1.0, 0.0, true,
        |_| false, |_| false, None, None, false, 1, |p, n, f, dry| {
            assert!(!dry);
            calls.lock().unwrap().push([p.x.to_bits(),p.z.to_bits()]);
            let (mut vertex, _) = terrain(p, n, f, 1.0, false);
            vertex.pos[1] = 1.0;
            (vertex, 1.0)
        });
    let mut calls = calls.into_inner().unwrap();
    assert_eq!(calls.len(), 2048);
    calls.sort_unstable();
    calls.dedup();
    assert_eq!(calls.len(), 2048);
    same_mesh(&before, &(vertices,beds,triangles));
}
#[test]
fn shoreline_parallel_sampling_preserves_mesh_and_probes() {
    let (expected, mut probes) = run(64, 0.25, 5, false, true, 1);
    probes.sort_unstable();
    for workers in [2, 4] {
        let (actual, mut parallel_probes) = run(64, 0.25, 5, false, true, workers);
        same_mesh(&expected, &actual);
        parallel_probes.sort_unstable();
        assert_eq!(probes, parallel_probes);
    }
}
