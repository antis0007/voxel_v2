// Resident-chunk mask and fade band for the globe (render/globe/band.rs).

// Same layout as ResidentCoverage in camera_lighting_shared.wgsl.
struct GlobeResidentCoverage {
    header: vec4<i32>,
    words: array<vec4<u32>, 131>,
    sparse_header: vec4<i32>,
    sparse_keys: array<vec4<i32>, 512>,
};

struct GlobeMask {
    coverage: GlobeResidentCoverage,
    // x: chunk column width m, y: resident disc radius m (<= 0: no fade), zw: eye chart x/z m
    params: vec4<f32>,
};

@group(0) @binding(7) var<uniform> globe_mask: GlobeMask;

const GLOBE_FADE_M: f32 = 256.0;

fn globe_sparse_owned(column: vec2<i32>) -> bool {
    let info = globe_mask.coverage.sparse_header;
    if info.x <= 0 || info.x > 512 { return false; }
    let empty = vec2<i32>(bitcast<i32>(0x80000000u));
    if all(column == empty) { return info.y == 1; }
    let bits = bitcast<vec2<u32>>(column);
    let mixed = (bits.x * 0x9e3779b1u) ^ (bits.y * 0x85ebca77u);
    var slot = (mixed ^ (mixed >> 16u)) & 1023u;
    for (var probe = 0u; probe < 1024u; probe += 1u) {
        let packed = globe_mask.coverage.sparse_keys[slot >> 1u];
        let key = select(packed.xy, packed.zw, (slot & 1u) != 0u);
        if all(key == empty) { return false; }
        if all(key == column) { return true; }
        slot = (slot + 1u) & 1023u;
    }
    return false;
}

// Mirrors terrain_resident_column_owned; chart metres equal world metres.
fn globe_column_owned(chart_xz: vec2<f32>) -> bool {
    let header = globe_mask.coverage.header;
    if header.w == 0 || header.z <= 0 || header.z > 129 { return false; }
    let column = vec2<i32>(floor(chart_xz / max(globe_mask.params.x, 1e-3)));
    if all(column >= header.xy) {
        let relative = bitcast<vec2<u32>>(column) - bitcast<vec2<u32>>(header.xy);
        if all(relative < vec2<u32>(u32(header.z))) {
            let bit = relative.y * u32(header.z) + relative.x;
            let word = bit >> 5u;
            return (globe_mask.coverage.words[word >> 2u][word & 3u] & (1u << (bit & 31u))) != 0u;
        }
    }
    return globe_sparse_owned(column);
}

// 0 under resident chunks, then 0 to 1 over GLOBE_FADE_M past the resident disc edge.
fn globe_mask_alpha(chart_xz: vec2<f32>) -> f32 {
    if globe_column_owned(chart_xz) { return 0.0; }
    let radius = globe_mask.params.y;
    if radius <= 0.0 { return 1.0; }
    return clamp((length(chart_xz - globe_mask.params.zw) - radius) / GLOBE_FADE_M, 0.0, 1.0);
}
