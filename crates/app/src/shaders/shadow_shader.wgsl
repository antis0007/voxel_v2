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
    // Voxel origin in xyz; w is the surface caster ceiling in voxels (0 disables it).
    render_origin: vec4<i32>,
};

@group(0) @binding(0) var<uniform> gen: ShadowGenUniform;

struct VsIn {
    @location(0) pos : vec3<f32>,
    @location(3) surface : vec4<f32>,
    @location(4) ao_strength : f32,
    // Per-draw transform at vertex slot 1 (render/origin.rs DrawTransform).
    @location(5) draw_offset : vec4<f32>,
    @location(6) axis_x : vec4<f32>,
    @location(7) axis_y : vec4<f32>,
    @location(8) axis_z : vec4<f32>,
    // Corner corrections (DrawTransform::seam), as in world_shader.wgsl.
    @location(9) seam_x0 : vec4<f32>,
    @location(10) seam_x1 : vec4<f32>,
    @location(11) seam_y0 : vec4<f32>,
    @location(12) seam_y1 : vec4<f32>,
    @location(13) seam_z0 : vec4<f32>,
    @location(14) seam_z1 : vec4<f32>,
};

fn shadow_planet_position(p: vec3<f32>, in: VsIn) -> vec3<f32> {
    let t = clamp(p / 32.0, vec3<f32>(0.0), vec3<f32>(1.0));
    let face = vec4<f32>((1.0 - t.x) * (1.0 - t.y), t.x * (1.0 - t.y), (1.0 - t.x) * t.y, t.x * t.y);
    let lo = face * (1.0 - t.z);
    let hi = face * t.z;
    return vec3<f32>(
        dot(in.seam_x0, lo) + dot(in.seam_x1, hi),
        dot(in.seam_y0, lo) + dot(in.seam_y1, hi),
        dot(in.seam_z0, lo) + dot(in.seam_z1, hi),
    );
}
struct ShadowVertex {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_xz: vec2<f32>,
    @location(1) @interpolate(flat) parent: u32,
    @location(2) chart_y: f32,
};

fn shadow_vertex(in: VsIn, ownership: bool) -> ShadowVertex {
    // Origin-frame voxels; the light matrix maps origin-frame metres.
    // Bent chunks store metres in offset.xyz. Their chart voxel offset is axis.w.
    let bent = (u32(in.draw_offset.w) & 1u) != 0u;
    let chart_offset = select(in.draw_offset.xyz, vec3<f32>(in.axis_x.w, in.axis_y.w, in.axis_z.w), bent);
    let local = in.pos + chart_offset;
    var p = local;
    var out: ShadowVertex;
    out.chart_y = local.y;
    let parent = ownership && in.ao_strength < 0.0;
    out.parent = u32(parent);
    if parent {
        // Parent masks use absolute columns even when the render origin moves.
        out.world_xz = (vec2<f32>(gen.render_origin.xz) + local.xz) * gen.world_scale;
    }
    let kind = u32(round(max(in.surface.z, 0.0))) % 64u;
    // Torch flames, embers and sparks are emissive geometry, not solid casters.
    if in.surface.w < 0.0 && in.surface.y <= -1024.0 {
        out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
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
        let absolute = vec3<f32>(gen.render_origin.xyz) + local;
        let sway = tuft_wind_offset(absolute, in.surface.y, gen.world_scale, gen.time, gen.wind);
        p = local + sway;
        if parent {
            out.world_xz = (absolute + sway).xz * gen.world_scale;
        }
    }
    if bent {
        let delta = (p - local) + in.pos - clamp(in.pos, vec3<f32>(0.0), vec3<f32>(32.0));
        let metres = shadow_planet_position(in.pos, in)
            + in.axis_x.xyz * delta.x + in.axis_y.xyz * delta.y + in.axis_z.xyz * delta.z;
        out.clip = gen.light_view_proj * vec4<f32>(metres, 1.0);
    } else {
        out.clip = gen.light_view_proj * vec4<f32>(p * gen.world_scale, 1.0);
    }
    return out;
}

@vertex
fn vs_main(in: VsIn) -> ShadowVertex {
    return shadow_vertex(in, true);
}

@vertex
fn vs_opaque(in: VsIn) -> @builtin(position) vec4<f32> {
    return shadow_vertex(in, false).clip;
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
    // Distant edit meshes can contain the same orbital voxels as child chunks.
    if gen.render_origin.w != 0 && in.chart_y > f32(gen.render_origin.w - gen.render_origin.y) {
        discard;
    }
    // The child draw list admits complete resident owners only. Suppress
    // their parent casters here using exactly the main-pass column mask.
    // Positive scene/model geometry is deliberately never fragment-masked.
    if in.parent != 0u && shadow_resident_owned(in.world_xz) { discard; }
}
