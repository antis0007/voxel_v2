// Interaction overlay for the cell under the crosshair.
//
// Edges are drawn as a pixel-dithered sparkle lattice rather than a solid line.
// The dither cells ride the engine's 5 cm world lattice, so the pattern holds
// still in the world while the camera moves and never crawls. Coverage and glow
// gather toward the top of the view; the lower half breaks into sparse violet
// dashes carrying mint motes.
//
// Two pipelines share this module: a normal alpha blend for ordinary previews
// and an inverted blend for the removal preview. Both keep straight alpha.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    world_params: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    camera_forward: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_m: vec3<f32>,
    @location(2) voxel_y: f32,
    @location(3) screen_y: f32,
}

const OVERLAY_CELLS_PER_M: f32 = 20.0;
const MOTE_CELLS_PER_M: f32 = 6.0;

// Cream core, warm bloom, violet deep, mint haze, gold motes, in linear HDR.
const CORE: vec3<f32> = vec3<f32>(1.00, 0.94, 0.78);
const BLOOM: vec3<f32> = vec3<f32>(0.92, 0.55, 0.24);
const DEEP: vec3<f32> = vec3<f32>(0.42, 0.34, 0.82);
const MINT: vec3<f32> = vec3<f32>(0.58, 0.92, 0.74);
const GOLD: vec3<f32> = vec3<f32>(0.90, 0.70, 0.18);

fn overlay_hash(cell: vec2<i32>) -> f32 {
    var h = bitcast<u32>(cell.x) * 0x9e3779b9u ^ bitcast<u32>(cell.y) * 0x85ebca6bu;
    h = (h ^ (h >> 15u)) * 0x7feb352du;
    h = (h ^ (h >> 13u)) * 0x846ca68bu;
    return f32((h ^ (h >> 16u)) >> 8u) * (1.0 / 16777216.0);
}

fn overlay_bayer(cell: vec2<i32>) -> f32 {
    let x = cell.x & 3;
    let y = cell.y & 3;
    let b = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0,
    );
    return b[y * 4 + x] * (1.0 / 16.0);
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_m = model.pos * camera.world_params.x;
    out.clip_pos = camera.view_proj * vec4<f32>(world_m - camera.camera_pos.xyz, 1.0);
    out.color = model.color;
    out.world_m = world_m;
    out.voxel_y = model.pos.y;
    let w = max(out.clip_pos.w, 1e-6);
    out.screen_y = clamp(out.clip_pos.y / w * 0.5 + 0.5, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Material damage bypasses the selection sparkle.
    if in.color.a < 0.0 { return vec4<f32>(in.color.rgb, 1.0); }
    let lattice = floor(in.world_m * OVERLAY_CELLS_PER_M);
    let grain = overlay_hash(vec2<i32>(lattice.xz + lattice.y));

    // Height inside the drawn cell: 0 on a horizontal edge, 0.5 mid-span. The
    // bottom and top edges fold together here; `up` separates them again.
    let local_y = fract(in.voxel_y);
    let edge_band = min(local_y, 1.0 - local_y);
    let up = in.screen_y;

    // Coverage rises with height and along horizontal edges, so cells low in the
    // view break into dashes while the top stays continuous. The floor keeps the
    // cell at the crosshair (screen middle) nearly solid: the old ramp left about
    // 35% of its edge pixels, which vanished on pale stone (HUD_LAYOUT.md).
    let coverage = clamp(0.45 + up * 0.75 + edge_band * 0.40, 0.0, 1.0);
    let threshold = overlay_bayer(vec2<i32>(in.clip_pos.xy)) * 0.6 + grain * 0.4;
    if coverage < threshold { discard; }

    // Hue is anchored to world position, so neighbouring cells differ and the
    // pattern stays put when the camera moves.
    let sweep = fract(dot(in.world_m, vec3<f32>(0.37, 0.61, 0.19)) * 0.35);
    var magic = mix(DEEP, CORE, smoothstep(0.0, 1.0, up));
    magic = mix(magic, BLOOM, 0.40 * (1.0 - up));
    magic = mix(magic, GOLD, 0.35 * sweep * (0.25 + 0.75 * up));

    // Mint motes occupy their own sparse lattice, so only a few cells glint.
    let mote = step(0.90, overlay_hash(vec2<i32>(floor(in.world_m * MOTE_CELLS_PER_M).xz)));
    magic = mix(magic, MINT, mote * 0.55);

    // The caller's colour carries meaning (amber means out of range), so it
    // stays readable underneath the magical tint.
    let tint = mix(in.color.rgb, magic, 0.45 + 0.25 * up);

    // Glow gathers upward, then falls off with distance so far cells stay calm.
    let glow = mix(0.70, 1.55, up * up);
    let dist = length(in.world_m - camera.camera_pos.xyz);
    let fade = 1.0 - smoothstep(16.0, 40.0, dist);
    let alpha = in.color.a * fade * mix(0.75, 1.0, up);

    return vec4<f32>(tint * glow, alpha);
}
