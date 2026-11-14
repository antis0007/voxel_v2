// Appended to the world shader program by art_refiner. It calls the same
// block_surface and tilt functions as fs_main, so edits to those show up here.
struct Tile {
    kind: i32,
    face: u32,
    rough: f32,
    refl: f32,
    // xyz: linear base colour. w: metallic.
    base: vec4<f32>,
    // BlockId + 1, shared with the game material ABI.
    block: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
}

struct Refine {
    tile_px: u32,
    cols: u32,
    count: u32,
    pad0: u32,
    metres_per_tile: f32,
    fp_scale: f32,
    ambient: f32,
    sun: f32,
    origin: vec4<f32>,
    light: vec4<f32>,
    // Eye direction in the face frame: x along u, y along v, z along the normal.
    view: vec4<f32>,
    tiles: array<Tile, 128>,
}

@group(1) @binding(60) var<uniform> refine: Refine;
@group(1) @binding(61) var<storage, read_write> out_px: array<u32>;

fn refine_is_natural(kind: i32) -> bool {
    return kind == 0 || kind == 1 || kind == 2 || kind == 3 || kind == 4 || kind == 7
        || kind == 16 || kind == 33 || kind == 36 || kind == 38;
}

fn refine_encode(c: vec3<f32>) -> u32 {
    let l = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    let s = select(1.055 * pow(l, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), l * 12.92, l <= vec3<f32>(0.0031308));
    let b = vec3<u32>(round(s * 255.0));
    return b.x | (b.y << 8u) | (b.z << 16u) | (255u << 24u);
}

@compute @workgroup_size(8, 8, 1)
fn refine_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let t_px = refine.tile_px;
    let rows = (refine.count + refine.cols - 1u) / refine.cols;
    let width = refine.cols * t_px;
    let height = rows * t_px * 4u;
    if id.x >= width || id.y >= height { return; }
    let band = id.y / t_px;
    let idx = (band / 4u) * refine.cols + id.x / t_px;
    var rgb = vec3<f32>(0.02);
    if idx < refine.count {
        let tile = refine.tiles[idx];
        let local = (vec2<f32>(f32(id.x % t_px), f32(id.y % t_px)) + vec2<f32>(0.5)) / f32(t_px);
        let span = refine.metres_per_tile;
        var p: vec3<f32>;
        var n: vec3<f32>;
        if tile.face == 0u {
            p = vec3<f32>(refine.origin.x + local.x * span, refine.origin.y, refine.origin.z + local.y * span);
            n = vec3<f32>(0.0, 1.0, 0.0);
        } else if tile.face == 1u {
            p = vec3<f32>(refine.origin.x + local.x * span, refine.origin.y + (1.0 - local.y) * span, refine.origin.z);
            n = vec3<f32>(0.0, 0.0, 1.0);
        } else {
            p = vec3<f32>(refine.origin.x, refine.origin.y + (1.0 - local.y) * span, refine.origin.z + local.x * span);
            n = vec3<f32>(1.0, 0.0, 0.0);
        }
        let fp = span / f32(t_px) * refine.fp_scale;
        natural_tilt = vec3<f32>(0.0);
        surface_finish = vec3<f32>(1.0, 1.0, 0.0);
        gilded_metal = 0.0;
        gilded_glow = vec3<f32>(0.0);
        var albedo = block_surface(tile.base.xyz, p, n, tile.kind, tile.block, fp);
        if tile.kind != 8 && tile.kind != 9 && !refine_is_natural(tile.kind) && !(tile.kind >= 64 && tile.kind <= 68)
            && tile.kind != 5 && tile.kind != 6 && tile.kind != 23 && tile.kind != 45
            && !(tile.kind == 12 && is_gilded_block(tile.block)) {
            albedo = terrain_pixel_albedo(albedo, p, n, fp);
        }
        var tilt = natural_tilt;
        if tile.kind == 13 { tilt = cobble_tilt(natural_face_uv(p, n), n, fp); }
        let nn = terrain_safe_unit3(n - (tilt - n * dot(n, tilt)), n);
        let l = normalize(refine.light.xyz);
        let mode = band % 4u;
        if mode == 0u {
            rgb = albedo;
        } else if mode == 1u {
            rgb = albedo * (refine.ambient + refine.sun * max(dot(nn, l), 0.0)) + gilded_glow;
        } else {
            // Bands 3 and 4. Same terms as fs_main: per-texel finish scales the material's roughness and
            // reflectance, GGX direct specular, and the bounded sky-environment lobe.
            let rough = clamp(tile.rough * surface_finish.x, 0.06, 1.0);
            let refl = clamp(tile.refl * surface_finish.y, 0.0, 1.0);
            let metallic = max(tile.base.w, gilded_metal);
            let f0 = mix(vec3<f32>(0.16 * refl * refl), albedo, metallic);
            let vf = normalize(refine.view.xyz);
            var v: vec3<f32>;
            if tile.face == 0u { v = vec3<f32>(vf.x, vf.z, vf.y); }
            else if tile.face == 1u { v = vec3<f32>(vf.x, vf.y, vf.z); }
            else { v = vec3<f32>(vf.z, vf.y, vf.x); }
            let sun = vec3<f32>(1.0, 0.96, 0.88) * refine.sun;
            let diffuse = albedo * (vec3<f32>(1.0) - f0) * (1.0 - metallic)
                * (vec3<f32>(refine.ambient) + sun * max(dot(nn, l), 0.0));
            let direct = sun * terrain_specular(nn, v, l, rough, f0);
            let r = reflect(-v, nn);
            let sky = mix(vec3<f32>(0.62, 0.68, 0.76), vec3<f32>(0.26, 0.42, 0.72), sqrt(saturate1(r.y)));
            let env_mean = vec3<f32>(0.45, 0.52, 0.62);
            let env = mix(sky, env_mean, rough * rough);
            let grazing = vec3<f32>(max(1.0 - rough, 0.0));
            let fres = f0 + (max(grazing, f0) - f0) * terrain_pow5(1.0 - clamp(dot(nn, v), 0.0, 1.0));
            let reflected = env * fres * mix(0.35, 1.0, metallic) * 0.55;
            rgb = diffuse + direct + reflected + gilded_glow;
            // Band 4: highlights and reflection alone, doubled, to judge roughness and glint.
            if mode == 3u { rgb = (direct + reflected) * 2.0; }
        }
    }
    out_px[id.y * width + id.x] = refine_encode(rgb);
}
