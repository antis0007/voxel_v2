// Local sky visibility bake (render/sky_volume.rs). One thread per probe, one
// probe per voxel. Rays cover the sky hemisphere only: the ambient model
// already accounts for open ground below. A probe stores the cosine-weighted
// sky transmittance seen by a face pointing along +y, +x, -x, +z and -z, as
// unorm8 pairs: x = +y | state << 8, y = +x | -x << 8, z = +z | -z << 8.
// State 0 is a cleared slot, 1 a solid cell, 2 a baked probe.

struct BakeParams {
    // xyz: probe window origin (voxels), w: entry count.
    window: vec4<i32>,
    // x: leaf transmittance per cell, y: ray length (voxels), z: occupancy edge.
    trace: vec4<f32>,
};

// Header (origin.xyz, edge), then solid bits, then leaf bits.
@group(0) @binding(0) var<storage, read> occupancy: array<u32>;
// xyz: brick coordinate, w: 0 bake, 1 clear.
@group(0) @binding(1) var<storage, read> entries: array<vec4<i32>>;
@group(0) @binding(2) var<uniform> params: BakeParams;
@group(0) @binding(3) var probes: texture_storage_3d<rgba16uint, write>;

const RAYS: u32 = 64u;
const BRICK: i32 = 8;

// 0 air, 1 leaf, 2 solid. Outside the window reads as air.
fn occupancy_class(cell: vec3<i32>) -> u32 {
    let edge = i32(occupancy[3]);
    let d = cell - vec3<i32>(bitcast<i32>(occupancy[0]), bitcast<i32>(occupancy[1]), bitcast<i32>(occupancy[2]));
    if any(d < vec3<i32>(0)) || any(d >= vec3<i32>(edge)) { return 0u; }
    // Toroidal storage, as render/sky_volume.rs `Window::index`.
    let w = wrap(cell, edge);
    let i = u32(w.x + edge * (w.z + edge * w.y));
    let words = u32(edge * edge * edge) / 32u;
    let bit = 1u << (i & 31u);
    if (occupancy[4u + (i >> 5u)] & bit) != 0u { return 2u; }
    if (occupancy[4u + words + (i >> 5u)] & bit) != 0u { return 1u; }
    return 0u;
}

// Fibonacci hemisphere, y up. Fixed so every bake of a cell agrees.
fn ray_direction(i: u32) -> vec3<f32> {
    let y = 1.0 - (f32(i) + 0.5) / f32(RAYS);
    let r = sqrt(max(1.0 - y * y, 0.0));
    let phi = f32(i) * 2.39996323;
    return vec3<f32>(r * cos(phi), y, r * sin(phi));
}

fn axis_setup(rd: f32, origin: f32, cell: i32) -> vec3<f32> {
    if rd > 1e-6 { return vec3<f32>(1.0, (f32(cell) + 1.0 - origin) / rd, 1.0 / rd); }
    if rd < -1e-6 { return vec3<f32>(-1.0, (f32(cell) - origin) / rd, -1.0 / rd); }
    return vec3<f32>(0.0, 1e30, 1e30);
}

// Transmittance from the cell centre along `rd` for `reach` voxels.
fn trace(start: vec3<i32>, rd: vec3<f32>, reach: f32, leaf: f32) -> f32 {
    let origin = vec3<f32>(start) + vec3<f32>(0.5);
    var cell = start;
    let sx = axis_setup(rd.x, origin.x, cell.x);
    let sy = axis_setup(rd.y, origin.y, cell.y);
    let sz = axis_setup(rd.z, origin.z, cell.z);
    var tmax = vec3<f32>(sx.y, sy.y, sz.y);
    let step = vec3<i32>(i32(sx.x), i32(sy.x), i32(sz.x));
    let tdelta = vec3<f32>(sx.z, sy.z, sz.z);
    var transmittance = 1.0;
    for (var i = 0; i < 64; i += 1) {
        var t = 0.0;
        if tmax.x <= tmax.y && tmax.x <= tmax.z {
            t = tmax.x; cell.x += step.x; tmax.x += tdelta.x;
        } else if tmax.y <= tmax.z {
            t = tmax.y; cell.y += step.y; tmax.y += tdelta.y;
        } else {
            t = tmax.z; cell.z += step.z; tmax.z += tdelta.z;
        }
        if t > reach { break; }
        let kind = occupancy_class(cell);
        if kind == 2u { return 0.0; }
        if kind == 1u {
            transmittance *= leaf;
            if transmittance < 0.02 { return 0.0; }
        }
    }
    return transmittance;
}

fn wrap(v: vec3<i32>, edge: i32) -> vec3<i32> {
    return ((v % vec3<i32>(edge)) + vec3<i32>(edge)) % vec3<i32>(edge);
}

@compute @workgroup_size(64)
fn bake(@builtin(global_invocation_id) id: vec3<u32>) {
    let entry_index = id.x / 512u;
    if entry_index >= u32(params.window.w) { return; }
    let entry = entries[entry_index];
    let local = id.x % 512u;
    let offset = vec3<i32>(i32(local % 8u), i32(local / 64u), i32((local / 8u) % 8u));
    let cell = entry.xyz * BRICK + offset;
    let slot = wrap(cell, i32(textureDimensions(probes).x));
    if entry.w == 1 {
        textureStore(probes, slot, vec4<u32>(0u));
        return;
    }
    if occupancy_class(cell) == 2u {
        textureStore(probes, slot, vec4<u32>(1u << 8u, 0u, 0u, 0u));
        return;
    }
    // Per axis: sum of T * cos and of cos, for +y, +x, -x, +z, -z.
    var lit = array<f32, 5>(0.0, 0.0, 0.0, 0.0, 0.0);
    var weight = array<f32, 5>(0.0, 0.0, 0.0, 0.0, 0.0);
    for (var r = 0u; r < RAYS; r += 1u) {
        let rd = ray_direction(r);
        let t = trace(cell, rd, params.trace.y, params.trace.x);
        let cosines = array<f32, 5>(rd.y, max(rd.x, 0.0), max(-rd.x, 0.0), max(rd.z, 0.0), max(-rd.z, 0.0));
        for (var a = 0; a < 5; a += 1) {
            lit[a] += t * cosines[a];
            weight[a] += cosines[a];
        }
    }
    var v = array<u32, 5>(0u, 0u, 0u, 0u, 0u);
    for (var a = 0; a < 5; a += 1) {
        v[a] = u32(round(clamp(lit[a] / max(weight[a], 1e-4), 0.0, 1.0) * 255.0));
    }
    textureStore(probes, slot, vec4<u32>(v[0] | (2u << 8u), v[1] | (v[2] << 8u), v[3] | (v[4] << 8u), 0u));
}