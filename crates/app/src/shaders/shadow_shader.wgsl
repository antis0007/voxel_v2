struct ShadowResidentCoverage {
    header: vec4<i32>,
    words: array<vec4<u32>, 131>,
    sparse_header: vec4<i32>,
    sparse_keys: array<vec4<i32>, 512>,
};
struct ShadowGenUniform {
    light_view_proj: mat4x4<f32>,
    world_scale: f32,
    time: f32,
    wind: vec2<f32>,
    resident_coverage: ShadowResidentCoverage,
    // Voxel origin of light_view_proj (render/origin.rs RenderOrigin).
    render_origin: vec4<i32>,
};

@group(0) @binding(0) var<uniform> gen: ShadowGenUniform;

struct VsIn {
    @location(0) pos : vec3<f32>,
    @location(3) surface : vec4<f32>,
    @location(4) ao_strength : f32,
    // Per-draw transform at vertex slot 1 (render/origin.rs DrawTransform).
    @location(5) draw_offset : vec4<f32>,
    // Reserved for bent chunks; no shader math reads it yet.
    @location(6) draw_rotation : vec4<f32>,
};
struct ShadowVertex {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_xz: vec2<f32>,
    @location(1) @interpolate(flat) parent: u32,
};

@vertex
fn vs_main(in: VsIn) -> ShadowVertex {
    // Origin-frame voxels; the light matrix maps origin-frame metres.
    let local = in.pos + in.draw_offset.xyz;
    // Ownership keys on absolute columns: rebuilding them from the integer
    // origin returns horizon vertices to their absolute values within one ulp.
    let absolute = vec3<f32>(gen.render_origin.xyz) + local;
    var p = local;
    var out: ShadowVertex;
    out.parent = select(0u, 1u, in.ao_strength < 0.0);
    out.world_xz = absolute.xz * gen.world_scale;
    let kind = u32(round(max(in.surface.z, 0.0))) % 64u;
    if kind == 8u {
        // Water still receives terrain/canopy shadows in the colour pass.
        // Its transmission is optical, not an opaque, undisplaced shadow sheet.
        // Water triangles have a homogeneous material key, including packed keys.
        out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    if kind == 34u {
        if gen.time < 0.0 {
            out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
            return out;
        }
        let sway = tuft_wind_offset(absolute, in.surface.y, gen.world_scale, gen.time, gen.wind);
        p = local + sway;
        out.world_xz = (absolute + sway).xz * gen.world_scale;
    }
    out.clip = gen.light_view_proj * vec4<f32>(p * gen.world_scale, 1.0);
    return out;
}

fn shadow_resident_owned(world_xz: vec2<f32>) -> bool {
    let header = gen.resident_coverage.header;
    if header.w == 0 || header.z <= 0 || header.z > 129 { return false; }
    let column = vec2<i32>(floor(world_xz / (32.0 * max(gen.world_scale, 1e-6))));
    if all(column >= header.xy) {
        let relative = bitcast<vec2<u32>>(column) - bitcast<vec2<u32>>(header.xy);
        if all(relative < vec2<u32>(u32(header.z))) {
            let bit = relative.y * u32(header.z) + relative.x;
            let word = bit >> 5u;
            return (gen.resident_coverage.words[word >> 2u][word & 3u] & (1u << (bit & 31u))) != 0u;
        }
    }
    return shadow_sparse_resident_owned(column);
}

fn shadow_sparse_resident_owned(column: vec2<i32>) -> bool {
    let info = gen.resident_coverage.sparse_header;
    if info.x <= 0 || info.x > 512 { return false; }
    let empty = vec2<i32>(bitcast<i32>(0x80000000u));
    if all(column == empty) { return info.y == 1; }
    let bits = bitcast<vec2<u32>>(column);
    let mixed = (bits.x * 0x9e3779b1u) ^ (bits.y * 0x85ebca77u);
    var slot = (mixed ^ (mixed >> 16u)) & 1023u;
    for (var probe = 0u; probe < 1024u; probe += 1u) {
        let packed = gen.resident_coverage.sparse_keys[slot >> 1u];
        let key = select(packed.xy, packed.zw, (slot & 1u) != 0u);
        if all(key == empty) { return false; }
        if all(key == column) { return true; }
        slot = (slot + 1u) & 1023u;
    }
    return false;
}

@fragment
fn fs_main(in: ShadowVertex) {
    // The child draw list admits complete resident owners only. Suppress
    // their parent casters here using exactly the main-pass column mask.
    // Positive scene/model geometry is deliberately never fragment-masked.
    if in.parent != 0u && shadow_resident_owned(in.world_xz) { discard; }
}
