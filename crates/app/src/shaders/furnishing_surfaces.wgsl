// Furniture shares the terrain's 20 texel/metre grid. Shapes use a few
// connected pixel clusters; no fine grain, labels, dithering or micro-detail.
// Metric coordinates preserve the design across editable voxel scales.
const FURNISHING_TEXELS: f32 = 20.0;

// Short directional brush marks, shared by fittings and machine panels.
fn furnishing_iron(base: vec3<f32>, uv_m: vec2<f32>, fp: f32) -> vec3<f32> {
    let pixel = floor(uv_m * FURNISHING_TEXELS);
    let stroke = vec2<f32>(floor(pixel.x / 8.0), pixel.y);
    let h = surface_hash(stroke);
    let tone = select(select(0.97, 1.0, h > 0.22), 1.025, h > 0.82);
    return base * mix(1.0, tone, surface_detail_weight(fp * FURNISHING_TEXELS));
}

fn furnishing_wood(base: vec3<f32>, uv_m: vec2<f32>, fp: f32) -> vec3<f32> {
    let t = floor(uv_m * FURNISHING_TEXELS);
    let board = floor(t.y / 5.0);
    let y = t.y - board * 5.0;
    let x = t.x - floor(t.x / 20.0) * 20.0;
    // Three quiet oak values; one board carries a short connected grain mark.
    var value = select(0.88, 0.96, (board - floor(board / 2.0) * 2.0) > 0.5);
    if y == 0.0 { value = 0.70; }
    if y == 4.0 { value = 1.02; }
    let mark = (x >= 6.0 && x <= 10.0 && y == 2.0)
        || (x >= 9.0 && x <= 12.0 && y == 3.0);
    if mark && board - floor(board / 4.0) * 4.0 == 1.0 { value = 0.80; }
    return base * mix(0.89, value, surface_detail_weight(fp * FURNISHING_TEXELS));
}

fn bookcase_surface(base: vec3<f32>, uv_m: vec2<f32>, normal: vec3<f32>, fp: f32) -> vec3<f32> {
    let wood = furnishing_wood(base * 0.75, uv_m, fp);
    if abs(normal.y) > 0.5 { return wood; }
    let t = floor(uv_m * FURNISHING_TEXELS);
    let row = floor(t.y / 20.0);
    let y = t.y - row * 20.0;
    let shifted = t.x + (row - floor(row / 2.0) * 2.0) * 2.0;
    let slot = floor(shifted / 4.0);
    let x = shifted - slot * 4.0;
    let roll = surface_hash(vec2<f32>(slot, row));
    let h = 13.0 + floor(roll * 5.0);
    // Three related cloth bindings, each with one broad shaded edge.
    var binding = vec3<f32>(0.16, 0.065, 0.040);
    if roll > 0.34 { binding = vec3<f32>(0.060, 0.12, 0.105); }
    if roll > 0.70 { binding = vec3<f32>(0.18, 0.13, 0.065); }
    binding *= select(1.0, 0.72, x == 1.0);
    if y == h - 1.0 { binding *= 1.18; }
    // Sparse cloth bands and a short gilt title mark form connected clusters,
    // rather than putting the same bright line on every spine.
    if roll > 0.46 && x >= 2.0 && (y == 4.0 || y == h - 4.0) {
        binding = mix(binding, vec3<f32>(0.29, 0.21, 0.10), 0.55);
    }
    if roll > 0.78 && x == 2.0 && y >= 7.0 && y <= 9.0 { binding *= 1.32; }
    let book = x >= 1.0 && y >= 2.0 && y < h;
    var face = select(vec3<f32>(0.035, 0.025, 0.019), binding, book);
    if y == 0.0 || y == 19.0 { face = wood * select(0.72, 1.10, y == 19.0); }
    return mix(base * vec3<f32>(0.31, 0.28, 0.23), face,
        surface_detail_weight(fp * FURNISHING_TEXELS));
}

fn chest_surface(base: vec3<f32>, uv_m: vec2<f32>, normal: vec3<f32>, fp: f32) -> vec3<f32> {
    let t = floor(fract(uv_m) * FURNISHING_TEXELS);
    let wood = furnishing_wood(base, select(uv_m, uv_m.yx, abs(normal.y) > 0.5), fp);
    let border = min(min(t.x,t.y),min(19.0-t.x,19.0-t.y)) < 1.0;
    var face = wood * select(1.0,0.68,border);
    let strap = (t.x >= 3.0 && t.x <= 4.0) || (t.x >= 15.0 && t.x <= 16.0);
    let iron = furnishing_iron(select(vec3<f32>(0.085,0.10,0.105),vec3<f32>(0.14,0.16,0.17),t.x == 3.0 || t.x == 15.0), uv_m, fp);
    face = select(face,iron,strap);
    if abs(normal.y) < 0.5 {
        if t.y == 13.0 { face *= 0.60; }
        if abs(normal.z) > 0.5 {
            let latch = t.x >= 9.0 && t.x <= 10.0 && t.y >= 11.0 && t.y <= 15.0;
            face = select(face,vec3<f32>(0.32,0.22,0.09),latch);
            if t.x == 9.0 && t.y == 12.0 { face = vec3<f32>(0.085,0.055,0.025); }
        } else {
            let grip = t.x >= 8.0 && t.x <= 11.0 && t.y >= 9.0 && t.y <= 10.0;
            face = select(face,vec3<f32>(0.065,0.075,0.070),grip);
        }
    }
    return mix(base * 0.82,face,surface_detail_weight(fp * FURNISHING_TEXELS));
}

// Two drawn, connected five-pixel silhouettes replace noisy circular spots.
const LICHEN_CLUSTER_A: array<u32,5> = array<u32,5>(6u,15u,30u,14u,4u);
const LICHEN_CLUSTER_B: array<u32,5> = array<u32,5>(4u,14u,15u,27u,8u);
fn lichen_colony(uv_m: vec2<f32>, fp: f32) -> vec2<f32> {
    let t = floor(uv_m * FURNISHING_TEXELS);
    let cell = floor(t / 15.0);
    let id = surface_hash(cell + vec2<f32>(61.0,17.0));
    let offset = floor(vec2<f32>(id,surface_hash(cell+23.0))*5.0) + 3.0;
    var p = vec2<i32>(t-cell*15.0-offset);
    if id > 0.72 { p = p.yx; }
    var mask = 0.0;
    var tone = 0.90;
    if all(p >= vec2<i32>(0)) && all(p < vec2<i32>(5)) && id > 0.52 {
        let bits = select(LICHEN_CLUSTER_A[u32(p.y)],LICHEN_CLUSTER_B[u32(p.y)],id > 0.68);
        mask = f32((bits >> u32(p.x)) & 1u);
        tone = select(0.90,1.05,p.x+p.y < 4);
    }
    return vec2<f32>(mix(0.045,mask,surface_detail_weight(fp*FURNISHING_TEXELS)),tone);
}

