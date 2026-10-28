// Poses resident rigid scene assets (mesh/models/posed.rs): one thread per
// output vertex. `pose` in posed.rs is the CPU form; keep the two equal.

struct Placement {
    rotation: vec4<f32>,
    translation: vec3<f32>,
    scale: f32,
    first: u32,
    count: u32,
    out_start: u32,
    _pad: u32,
};

struct Params {
    placements: u32,
    vertices: u32,
    _pad0: u32,
    _pad1: u32,
};

// Vertex3: pos 3, color 3, normal 3, surface 4, ao_strength 1.
const STRIDE: u32 = 14u;

@group(0) @binding(0) var<storage, read> pool: array<f32>;
@group(0) @binding(1) var<storage, read> placements: array<Placement>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> posed: array<f32>;

// glam's `Quat * Vec3`.
fn rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let b = q.xyz;
    return v * (q.w * q.w - dot(b, b)) + b * (dot(v, b) * 2.0) + cross(b, v) * (q.w * 2.0);
}

@compute @workgroup_size(64)
fn cs_pose(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.vertices {
        return;
    }
    // Last placement starting at or before i; out_start is ascending.
    var lo = 0u;
    var hi = params.placements;
    loop {
        if hi - lo <= 1u { break; }
        let mid = (lo + hi) / 2u;
        if placements[mid].out_start <= i { lo = mid; } else { hi = mid; }
    }
    let p = placements[lo];
    let s = (p.first + i - p.out_start) * STRIDE;
    let d = i * STRIDE;
    let position = (p.translation + rotate(p.rotation, vec3<f32>(pool[s], pool[s + 1u], pool[s + 2u]))) / p.scale;
    let normal = rotate(p.rotation, vec3<f32>(pool[s + 6u], pool[s + 7u], pool[s + 8u]));
    posed[d] = position.x;
    posed[d + 1u] = position.y;
    posed[d + 2u] = position.z;
    for (var k = 3u; k < 6u; k += 1u) { posed[d + k] = pool[s + k]; }
    posed[d + 6u] = normal.x;
    posed[d + 7u] = normal.y;
    posed[d + 8u] = normal.z;
    for (var k = 9u; k < STRIDE; k += 1u) { posed[d + k] = pool[s + k]; }
}
