// Cubic chunk constants — 32^3 interior + 1-block pad on all sides = 34^3 per chunk
const CHUNK_SIZE : i32 = 32;
const PAD : i32 = 1;
const CHUNK_GEN_W : i32 = 34; // PADDED_CHUNK_SIZE = CHUNK_SIZE + 2
const WORLD_MIN_Y_METERS : f32 = -16.0;
const WORLD_MAX_Y_METERS : f32 = 512.0;
const WATER_LEVEL_METERS : f32 = 12.0;

struct GenerationConfig {
    chunk_coord : vec4<i32>, // x, y, z, pad
    seed : u32,
    world_type : u32,
    _padding : vec2<u32>,
    generation_params : vec4<f32>,
    block_ids0 : vec4<u32>,
    block_ids1 : vec4<u32>,
    block_ids2 : vec4<u32>,
}

@group(0) @binding(0) var<uniform> cfg : GenerationConfig;
@group(0) @binding(1) var<storage, read_write> out_blocks : array<u32>;

fn block_air() -> u32 {
    return cfg.block_ids0.x;
}

fn block_grass() -> u32 {
    return cfg.block_ids0.y;
}

fn block_dirt() -> u32 {
    return cfg.block_ids0.z;
}

fn block_stone() -> u32 {
    return cfg.block_ids0.w;
}

fn block_sand() -> u32 {
    return cfg.block_ids1.x;
}

fn block_sandstone() -> u32 {
    return cfg.block_ids1.y;
}

fn block_snow_grass() -> u32 {
    return cfg.block_ids1.z;
}

fn block_water() -> u32 {
    return cfg.block_ids1.w;
}

fn block_log() -> u32 {
    return cfg.block_ids2.x;
}

fn block_leaves() -> u32 {
    return cfg.block_ids2.y;
}

fn block_bush() -> u32 {
    return cfg.block_ids2.z;
}

fn block_gravel() -> u32 {
    return cfg.block_ids2.w;
}

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

fn hash2(x: i32, z: i32, seed: u32) -> f32 {
    var h = seed ^ (u32(x) * 374761393u) ^ (u32(z) * 668265263u);
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32((h ^ (h >> 16u)) & 65535u) / 65535.0;
}

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    var h = seed ^ (u32(x) * 374761393u) ^ (u32(y) * 1103515245u) ^ (u32(z) * 668265263u);
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32((h ^ (h >> 16u)) & 65535u) / 65535.0;
}

fn smooth_interp(t: f32) -> f32 {
    return t * t * (3.0 - 2.0 * t);
}

fn value_noise2(x: f32, z: f32, seed: u32) -> f32 {
    let x0 = i32(floor(x));
    let z0 = i32(floor(z));
    let x1 = x0 + 1;
    let z1 = z0 + 1;
    let tx = x - f32(x0);
    let tz = z - f32(z0);
    let sx = smooth_interp(tx);
    let sz = smooth_interp(tz);
    let n00 = hash2(x0, z0, seed);
    let n10 = hash2(x1, z0, seed);
    let n01 = hash2(x0, z1, seed);
    let n11 = hash2(x1, z1, seed);
    let nx0 = n00 + (n10 - n00) * sx;
    let nx1 = n01 + (n11 - n01) * sx;
    return nx0 + (nx1 - nx0) * sz;
}

fn value_noise3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let x0 = i32(floor(x));
    let y0 = i32(floor(y));
    let z0 = i32(floor(z));
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;
    let tx = x - f32(x0);
    let ty = y - f32(y0);
    let tz = z - f32(z0);
    let sx = smooth_interp(tx);
    let sy = smooth_interp(ty);
    let sz = smooth_interp(tz);
    let n000 = hash3(x0, y0, z0, seed);
    let n100 = hash3(x1, y0, z0, seed);
    let n010 = hash3(x0, y1, z0, seed);
    let n110 = hash3(x1, y1, z0, seed);
    let n001 = hash3(x0, y0, z1, seed);
    let n101 = hash3(x1, y0, z1, seed);
    let n011 = hash3(x0, y1, z1, seed);
    let n111 = hash3(x1, y1, z1, seed);
    let nx00 = n000 + (n100 - n000) * sx;
    let nx10 = n010 + (n110 - n010) * sx;
    let nx01 = n001 + (n101 - n001) * sx;
    let nx11 = n011 + (n111 - n011) * sx;
    let nxy0 = nx00 + (nx10 - nx00) * sy;
    let nxy1 = nx01 + (nx11 - nx01) * sy;
    return nxy0 + (nxy1 - nxy0) * sz;
}

fn fbm2(x: f32, z: f32, seed: u32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        sum = sum + value_noise2(x * freq, z * freq, seed + i) * amp;
        amp = amp * 0.5;
        freq = freq * 2.0;
    }
    return sum;
}

fn ridge(x: f32) -> f32 {
    return 1.0 - abs(2.0 * x - 1.0);
}

fn smooth_band(value: f32, center: f32, width: f32) -> f32 {
    return saturate(1.0 - abs(value - center) / width);
}

fn inv_voxel_meters() -> f32 {
    return 1.0 / max(cfg.generation_params.x, 0.001);
}

fn world_x(x: i32) -> f32 {
    return f32(x) * cfg.generation_params.x;
}

fn world_y(y: i32) -> f32 {
    return f32(y) * cfg.generation_params.x;
}

fn world_z(z: i32) -> f32 {
    return f32(z) * cfg.generation_params.x;
}

fn blocks_for_height(meters: f32) -> i32 {
    return i32(round(meters * inv_voxel_meters()));
}

fn blocks_for_span(meters: f32) -> i32 {
    return max(i32(ceil(meters * inv_voxel_meters())), 1);
}

fn min_world_y() -> i32 {
    return i32(floor(WORLD_MIN_Y_METERS * inv_voxel_meters()));
}

fn max_world_y() -> i32 {
    return i32(ceil(WORLD_MAX_Y_METERS * inv_voxel_meters()));
}

// Returns the absolute world Y range covered by this chunk's block at local y
fn chunk_world_y_base() -> i32 {
    return cfg.chunk_coord.y * CHUNK_SIZE;
}

fn water_level() -> i32 {
    return blocks_for_height(WATER_LEVEL_METERS);
}

fn clamp_height(y: i32) -> i32 {
    return clamp(y, min_world_y() + 4, max_world_y() - 3);
}

struct ClimateSample {
    temperature : f32,
    moisture : f32,
    continentalness : f32,
}

struct BiomeWeights {
    plains : f32,
    hills : f32,
    desert : f32,
    snow : f32,
}

struct ColumnSample {
    height : i32,
    surface : u32,
    filler : u32,
    vegetation_density : f32,
}

struct BushProfile {
    anchor_x : i32,
    anchor_z : i32,
    ground_y : i32,
    base_radius : i32,
    top_dy : i32,
    lean_x : i32,
    lean_z : i32,
    valid : bool,
}

struct TreeProfile {
    cell_x : i32,
    cell_z : i32,
    anchor_x : i32,
    anchor_z : i32,
    ground_y : i32,
    trunk_h : i32,
    canopy_radius : i32,
    canopy_height : i32,
    branch_count : i32,
    branch_len : i32,
    branch_start_y : i32,
    lean_x : i32,
    lean_z : i32,
    valid : bool,
}

fn sample_climate(x: i32, z: i32) -> ClimateSample {
    let seed = cfg.seed;
    let mx = world_x(x);
    let mz = world_z(z);
    let warp_x = (fbm2(mx * 0.0012, mz * 0.0012, seed + 7u) - 0.5) * 220.0;
    let warp_z = (fbm2(mx * 0.0011, mz * 0.0011, seed + 9u) - 0.5) * 220.0;
    let wx = mx + warp_x;
    let wz = mz + warp_z;
    return ClimateSample(
        saturate(fbm2(wx * 0.0022, wz * 0.0022, seed + 11u)),
        saturate(fbm2(wx * 0.0020, wz * 0.0020, seed + 23u)),
        saturate(fbm2(wx * 0.0015, wz * 0.0015, seed + 37u)),
    );
}

fn biome_weights(x: i32, z: i32) -> BiomeWeights {
    let climate = sample_climate(x, z);
    let erosion = fbm2(world_x(x) * 0.0044, world_z(z) * 0.0044, cfg.seed + 51u);
    let cold = saturate((0.42 - climate.temperature) / 0.23);
    let hot = saturate((climate.temperature - 0.57) / 0.20);
    let humid = saturate((climate.moisture - 0.47) / 0.28);
    let arid = saturate((0.52 - climate.moisture) / 0.25);

    let desert = hot * arid * smooth_band(climate.continentalness, 0.58, 0.44);
    let snow = cold * (0.42 + humid * 0.58) * smooth_band(climate.continentalness, 0.52, 0.50);
    let hills = smooth_band(erosion, 0.77, 0.27) * smooth_band(climate.continentalness, 0.62, 0.38) * 0.9;
    let plains = 0.25 + (1.0 - min(desert + snow, 0.92)) * smooth_band(climate.moisture, 0.55, 0.42);
    let sum = max(desert + snow + hills + plains, 0.001);

    return BiomeWeights(plains / sum, hills / sum, desert / sum, snow / sum);
}

fn sample_column(x: i32, z: i32) -> ColumnSample {
    let weights = biome_weights(x, z);
    let climate = sample_climate(x, z);
    let seed = cfg.seed;
    let mx = world_x(x);
    let mz = world_z(z);
    let erosion = fbm2(mx * 0.0048, mz * 0.0048, seed + 51u);
    let macro_shape = fbm2(mx * 0.0090, mz * 0.0090, seed + 71u);
    let detail = fbm2(mx * 0.026, mz * 0.026, seed + 83u);
    let ridge_shape = ridge(fbm2(mx * 0.017, mz * 0.017, seed + 97u));
    let river = smooth_band(fbm2(mx * 0.0036, mz * 0.0036, seed + 109u), 0.50, 0.05);
    let lake = smooth_band(fbm2(mx * 0.0019, mz * 0.0019, seed + 127u), 0.50, 0.028)
        * smooth_band(climate.continentalness, 0.56, 0.34);
    let mountain_mask =
        smooth_band(climate.continentalness, 0.72, 0.20) * smooth_band(ridge_shape, 0.68, 0.40);
    let mountain_boost = mountain_mask
        * (8.0 + ridge(fbm2(mx * 0.0105, mz * 0.0105, seed + 137u)) * 20.0);
    let valley_cut =
        river * river * (5.0 + climate.moisture * 7.0) + lake * (3.0 + climate.moisture * 4.0);

    let plains_h = 8.0 + macro_shape * 5.5 + detail * 2.5;
    let hills_h = 11.0 + macro_shape * 11.0 + ridge(erosion) * 9.0 + detail * 4.0;
    let desert_h = 6.0 + macro_shape * 4.0 + ridge(detail) * 3.0;
    let snow_h = 12.0 + macro_shape * 7.5 + ridge_shape * 10.0 + detail * 3.5;
    let blended = weights.plains * plains_h
        + weights.hills * hills_h
        + weights.desert * desert_h
        + weights.snow * snow_h;
    let coast = (climate.continentalness - 0.5) * 14.0;

    var world_modifier = 0.0;
    if cfg.world_type == 0u {
        world_modifier = 2.0 + detail * 0.5;
    } else if cfg.world_type == 1u {
        world_modifier = blended + coast + mountain_boost * 0.65 - valley_cut;
    } else {
        world_modifier = blended + coast * 0.45 + ridge_shape * 18.0 + mountain_boost
            - valley_cut * 0.35;
    }

    let height = clamp_height(blocks_for_height(world_modifier));
    let level = water_level();
    let altitude_snow = saturate((world_y(height) - 18.0) / 18.0);
    let snow_bias = clamp(weights.snow + altitude_snow * 0.60, 0.0, 1.0);
    var surface = block_grass();
    if height <= level + 1 && weights.snow > 0.55 {
        surface = block_snow_grass();
    } else if height <= level + 1 {
        surface = block_sand();
    } else if snow_bias > 0.50 && weights.desert < 0.26 {
        surface = block_snow_grass();
    }
    let filler = select(block_dirt(), block_sandstone(), surface == block_sand());
    let vegetation = clamp((weights.plains * 0.95 + weights.hills * 0.40 + weights.snow * 0.08) * (1.0 - weights.desert * 0.90), 0.0, 1.0);

    return ColumnSample(height, surface, filler, vegetation);
}

fn water_table(x: i32, z: i32) -> i32 {
    let climate = sample_climate(x, z);
    let coast = saturate((0.54 - climate.continentalness) / 0.18);
    let aquifer = fbm2(world_x(x) * 0.0042, world_z(z) * 0.0042, cfg.seed + 509u) - 0.5;
    return blocks_for_height(
        (WORLD_MIN_Y_METERS + 6.0) * (1.0 - coast)
            + WATER_LEVEL_METERS * coast
            + aquifer * 2.0
            + climate.moisture * 2.5,
    );
}

fn cave_carves(x: i32, y: i32, z: i32, column_height: i32) -> bool {
    if cfg.world_type == 0u {
        return false;
    }
    if y > column_height || y <= min_world_y() + blocks_for_span(0.75) {
        return false;
    }

    let depth = f32(column_height - y) * cfg.generation_params.x;
    let mx = world_x(x);
    let my = world_y(y);
    let mz = world_z(z);
    let spaghetti = value_noise3(mx * 0.060, my * 0.095, mz * 0.060, cfg.seed + 301u);
    let cheese = value_noise3(mx * 0.028, my * 0.040, mz * 0.028, cfg.seed + 347u);
    let pillar = ridge(value_noise3(mx * 0.020, my * 0.055, mz * 0.020, cfg.seed + 389u));
    let ravine = ridge(value_noise2(mx * 0.010, mz * 0.010, cfg.seed + 571u));
    let ravine_center = world_y(column_height) - 16.0
        - value_noise2(mx * 0.020, mz * 0.020, cfg.seed + 593u) * 10.0;
    let ravine_half_h =
        4.0 + value_noise2(mx * 0.018, mz * 0.018, cfg.seed + 617u) * 4.5;
    let bias = saturate(depth / 24.0) * 0.12;
    let surface_guard = saturate((10.0 - depth) / 10.0) * 0.16;
    return spaghetti > 0.86 + surface_guard - bias
        || cheese > 0.80 + surface_guard - bias
        || pillar > 0.93 + surface_guard * 0.6
        || (depth > 6.0 && ravine > 0.975 && abs(my - ravine_center) < ravine_half_h);
}

fn floor_div(a: i32, b: i32) -> i32 {
    var q = a / b;
    let r = a % b;
    if r != 0 && ((r > 0) != (b > 0)) {
        q = q - 1;
    }
    return q;
}

fn branch_direction(cell_x: i32, cell_z: i32, branch_idx: i32) -> vec2<i32> {
    let branch_dir = min(i32(hash3(cell_x, branch_idx, cell_z, cfg.seed + 613u) * 8.0), 7);
    switch branch_dir {
        case 0: { return vec2<i32>(1, 0); }
        case 1: { return vec2<i32>(1, 1); }
        case 2: { return vec2<i32>(0, 1); }
        case 3: { return vec2<i32>(-1, 1); }
        case 4: { return vec2<i32>(-1, 0); }
        case 5: { return vec2<i32>(-1, -1); }
        case 6: { return vec2<i32>(0, -1); }
        default: { return vec2<i32>(1, -1); }
    }
}

fn tree_trunk_center(tree: TreeProfile, y: i32) -> vec2<i32> {
    if y <= tree.branch_start_y {
        return vec2<i32>(tree.anchor_x, tree.anchor_z);
    }
    let bend_span = max(tree.trunk_h - (tree.branch_start_y - tree.ground_y), 1);
    let bend_t = f32(y - tree.branch_start_y) / f32(bend_span);
    return vec2<i32>(
        tree.anchor_x + i32(round(f32(tree.lean_x) * bend_t)),
        tree.anchor_z + i32(round(f32(tree.lean_z) * bend_t)),
    );
}

fn tree_profile_for_cell(cell_x: i32, cell_z: i32) -> TreeProfile {
    let cell_size = blocks_for_span(12.0);
    let offset_x = i32(hash2(cell_x, cell_z, cfg.seed + 401u) * f32(cell_size));
    let offset_z = i32(hash2(cell_x, cell_z, cfg.seed + 419u) * f32(cell_size));
    let anchor_x = cell_x * cell_size + min(offset_x, cell_size - 1);
    let anchor_z = cell_z * cell_size + min(offset_z, cell_size - 1);
    let column = sample_column(anchor_x, anchor_z);
    let ground_y = column.height;

    if column.surface != block_grass() {
        return TreeProfile(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, false);
    }
    let chance = hash2(cell_x, cell_z, cfg.seed + 433u);
    if chance > column.vegetation_density * 0.22 {
        return TreeProfile(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, false);
    }

    let h_nx = sample_column(anchor_x - 1, anchor_z).height;
    let h_px = sample_column(anchor_x + 1, anchor_z).height;
    let h_nz = sample_column(anchor_x, anchor_z - 1).height;
    let h_pz = sample_column(anchor_x, anchor_z + 1).height;
    let slope = max(
        max(abs(h_nx - ground_y), abs(h_px - ground_y)),
        max(abs(h_nz - ground_y), abs(h_pz - ground_y)),
    );
    if slope > blocks_for_span(0.9) || ground_y <= water_level() + 1 {
        return TreeProfile(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, false);
    }

    let trunk_h = blocks_for_span(4.0 + hash2(cell_x, cell_z, cfg.seed + 449u) * 3.2);
    let canopy_radius = select(blocks_for_span(2.3), blocks_for_span(2.9), hash2(cell_x, cell_z, cfg.seed + 457u) > 0.66);
    let canopy_height = blocks_for_span(3.0 + hash2(cell_x, cell_z, cfg.seed + 463u) * 1.8);
    let branch_count = 3 + i32(hash2(cell_x, cell_z, cfg.seed + 487u) * 3.0);
    let branch_len = blocks_for_span(1.5 + hash2(cell_x, cell_z, cfg.seed + 499u) * 1.5);
    let branch_start_y = ground_y + trunk_h - blocks_for_span(2.2);
    let lean_idx = min(i32(hash2(cell_x, cell_z, cfg.seed + 521u) * 5.0), 4);
    var lean_x = 0;
    var lean_z = 0;
    if lean_idx == 1 {
        lean_x = 1;
    } else if lean_idx == 2 {
        lean_x = -1;
    } else if lean_idx == 3 {
        lean_z = 1;
    } else if lean_idx == 4 {
        lean_z = -1;
    }

    return TreeProfile(
        cell_x, cell_z, anchor_x, anchor_z, ground_y, trunk_h, canopy_radius, canopy_height,
        branch_count, branch_len, branch_start_y, lean_x, lean_z, true,
    );
}

fn tree_block(tree: TreeProfile, x: i32, y: i32, z: i32) -> u32 {
    if !tree.valid {
        return block_air();
    }
    let top_y = tree.ground_y + tree.trunk_h;
    if y <= tree.ground_y || y > top_y + tree.canopy_height + 1 {
        return block_air();
    }

    let trunk_center = tree_trunk_center(tree, min(y, top_y));
    if x == trunk_center.x && z == trunk_center.y && y <= top_y {
        return block_log();
    }

    for (var branch_idx = 0; branch_idx < tree.branch_count; branch_idx = branch_idx + 1) {
        let dir = branch_direction(tree.cell_x, tree.cell_z, branch_idx);
        let start_y = tree.branch_start_y + min(branch_idx, max(top_y - tree.branch_start_y, 0));
        let start = tree_trunk_center(tree, min(start_y, top_y));
        for (var step = 1; step <= tree.branch_len; step = step + 1) {
            let rise = select(0, (step - 1) / 2, step > 1);
            let bx = start.x + dir.x * step;
            let by = min(start_y + rise, top_y + tree.canopy_height / 2);
            let bz = start.y + dir.y * step;
            if x == bx && y == by && z == bz {
                return block_log();
            }
            let twig_len = min(blocks_for_span(0.75), 2);
            for (var twig_step = 1; twig_step <= twig_len; twig_step = twig_step + 1) {
                let tx = bx + dir.x * twig_step;
                let ty = by + twig_step / 2;
                let tz = bz + dir.y * twig_step;
                if x == tx && y == ty && z == tz {
                    return block_log();
                }
            }
        }
    }

    let crown_center_y = top_y + tree.canopy_height / 3;
    let dy = y - crown_center_y;
    let crown_dx = abs(x - trunk_center.x);
    let crown_dz = abs(z - trunk_center.y);
    let canopy_rx = max(f32(tree.canopy_radius), 1.0);
    let canopy_ry = max(f32(tree.canopy_height) * 0.72, 1.0);
    let crown_shape = pow(f32(crown_dx) / canopy_rx, 2.0)
        + pow(f32(dy) / canopy_ry, 2.0)
        + pow(f32(crown_dz) / canopy_rx, 2.0);
    let trim = hash3(x, y, z, cfg.seed + 631u);
    if crown_shape <= 1.18 && !(crown_shape > 0.78 && trim > 0.82) {
        return block_leaves();
    }

    for (var branch_idx = 0; branch_idx < tree.branch_count; branch_idx = branch_idx + 1) {
        let dir = branch_direction(tree.cell_x, tree.cell_z, branch_idx);
        let start_y = tree.branch_start_y + min(branch_idx, max(top_y - tree.branch_start_y, 0));
        let start = tree_trunk_center(tree, min(start_y, top_y));
        let tip_x = start.x + dir.x * (tree.branch_len + blocks_for_span(0.5));
        let tip_y = min(start_y + tree.branch_len / 2 + 1, top_y + tree.canopy_height);
        let tip_z = start.y + dir.y * (tree.branch_len + blocks_for_span(0.5));
        let dx = abs(x - tip_x);
        let ddy = abs(y - tip_y);
        let dz = abs(z - tip_z);
        let tuft_radius = blocks_for_span(1.2);
        if max(max(dx, ddy), dz) <= tuft_radius && (dx + ddy + dz <= tuft_radius * 2 + 1 || trim < 0.35) {
            return block_leaves();
        }
    }

    return block_air();
}

fn bush_profile_for_cell(cell_x: i32, cell_z: i32) -> BushProfile {
    let cell_size = blocks_for_span(8.0);
    let border = max(min(blocks_for_span(1.5), max(cell_size - 1, 1) / 2), 1);
    let inner_span = max(cell_size - border * 2, 1);
    let offset_x = border + min(i32(hash2(cell_x, cell_z, cfg.seed + 467u) * f32(inner_span)), inner_span - 1);
    let offset_z = border + min(i32(hash2(cell_x, cell_z, cfg.seed + 479u) * f32(inner_span)), inner_span - 1);
    let anchor_x = cell_x * cell_size + offset_x;
    let anchor_z = cell_z * cell_size + offset_z;

    let column = sample_column(anchor_x, anchor_z);
    let ground_y = column.height;
    if column.surface != block_grass() || column.vegetation_density <= 0.24 || ground_y <= water_level() + 1 {
        return BushProfile(0, 0, 0, 0, 0, 0, 0, false);
    }

    let chance = hash2(cell_x, cell_z, cfg.seed + 461u);
    if chance > column.vegetation_density * 0.26 {
        return BushProfile(0, 0, 0, 0, 0, 0, 0, false);
    }

    let h_nx = sample_column(anchor_x - 1, anchor_z).height;
    let h_px = sample_column(anchor_x + 1, anchor_z).height;
    let h_nz = sample_column(anchor_x, anchor_z - 1).height;
    let h_pz = sample_column(anchor_x, anchor_z + 1).height;
    let slope = max(
        max(abs(h_nx - ground_y), abs(h_px - ground_y)),
        max(abs(h_nz - ground_y), abs(h_pz - ground_y)),
    );
    if slope > blocks_for_span(0.85) {
        return BushProfile(0, 0, 0, 0, 0, 0, 0, false);
    }

    let base_radius = select(blocks_for_span(1.1), blocks_for_span(1.6), hash2(cell_x, cell_z, cfg.seed + 487u) > 0.58);
    let top_dy = select(blocks_for_span(1.1), blocks_for_span(1.7), hash2(cell_x, cell_z, cfg.seed + 503u) > 0.74);
    let lean_index = min(i32(hash2(cell_x, cell_z, cfg.seed + 509u) * 5.0), 4);
    var lean_x = 0;
    var lean_z = 0;
    if lean_index == 1 {
        lean_x = 1;
    } else if lean_index == 2 {
        lean_x = -1;
    } else if lean_index == 3 {
        lean_z = 1;
    } else if lean_index == 4 {
        lean_z = -1;
    }

    return BushProfile(anchor_x, anchor_z, ground_y, base_radius, top_dy, lean_x, lean_z, true);
}

fn bush_contains(bush: BushProfile, x: i32, y: i32, z: i32) -> bool {
    if !bush.valid {
        return false;
    }

    let dy = y - (bush.ground_y + 1);
    if dy < 0 || dy > bush.top_dy {
        return false;
    }

    var center_x = bush.anchor_x;
    var center_z = bush.anchor_z;
    if dy >= 1 {
        center_x = center_x + bush.lean_x;
        center_z = center_z + bush.lean_z;
    }

    let dx = abs(x - center_x);
    let dz = abs(z - center_z);
    let trim = hash3(x, y, z, cfg.seed + 521u);

    if dy == 0 && bush.base_radius > 1 {
        let manhattan = dx + dz;
        return manhattan <= 2 || (manhattan == 3 && max(dx, dz) == 2 && trim < 0.35);
    }
    if dy == 0 {
        return max(dx, dz) <= 1 && (dx != 1 || dz != 1 || trim < 0.45);
    }
    if dy == 1 {
        return max(dx, dz) <= 1 && (dx != 1 || dz != 1 || trim < 0.32);
    }
    return dx + dz == 0
        || (bush.lean_x == 0 && bush.lean_z == 0 && dx + dz == 1 && trim < 0.18);
}

fn structure_block(x: i32, y: i32, z: i32, column: ColumnSample) -> u32 {
    if y <= column.height {
        return block_air();
    }

    let cell_size = blocks_for_span(12.0);
    let base_cell_x = floor_div(x, cell_size);
    let base_cell_z = floor_div(z, cell_size);

    for (var cell_z = base_cell_z - 2; cell_z <= base_cell_z + 2; cell_z = cell_z + 1) {
        for (var cell_x = base_cell_x - 2; cell_x <= base_cell_x + 2; cell_x = cell_x + 1) {
            let tree = tree_profile_for_cell(cell_x, cell_z);
            let block = tree_block(tree, x, y, z);
            if block != block_air() {
                return block;
            }
        }
    }

    let bush_cell_size = blocks_for_span(8.0);
    let bush_cell_x = floor_div(x, bush_cell_size);
    let bush_cell_z = floor_div(z, bush_cell_size);
    for (var cell_z = bush_cell_z - 1; cell_z <= bush_cell_z + 1; cell_z = cell_z + 1) {
        for (var cell_x = bush_cell_x - 1; cell_x <= bush_cell_x + 1; cell_x = cell_x + 1) {
            let bush = bush_profile_for_cell(cell_x, cell_z);
            if bush_contains(bush, x, y, z) {
                return block_bush();
            }
        }
    }

    return block_air();
}

fn sample_block_in_column(x: i32, y: i32, z: i32, column: ColumnSample) -> u32 {
    if y < min_world_y() || y >= max_world_y() {
        return block_air();
    }


    if y > column.height {
        let structure = structure_block(x, y, z, column);
        if structure != block_air() {
            return structure;
        }
        if y <= water_level() {
            return block_water();
        }
        return block_air();
    }

    if cave_carves(x, y, z, column.height) {
        if y <= water_table(x, z) {
            return block_water();
        }
        return block_air();
    }

    let depth = column.height - y;
    if column.height <= water_level() + 1 {
        if depth == 0 {
            return block_sand();
        }
        if depth <= 2 {
            if hash3(x, y, z, cfg.seed + 541u) > 0.72 {
                return block_gravel();
            }
            return block_sand();
        }
    }

    if depth == 0 {
        return column.surface;
    }
    if depth <= 3 {
        return column.filler;
    }
    if depth <= 5 && hash3(x, y, z, cfg.seed + 563u) > 0.76 {
        return block_gravel();
    }
    return block_stone();
}

fn linear_index(x: i32, y: i32, z: i32) -> u32 {
    // Layout matches Rust: (y * PADDED + z) * PADDED + x
    return u32((y * CHUNK_GEN_W + z) * CHUNK_GEN_W + x);
}

// One invocation owns a complete vertical column. Terrain/climate sampling is
// shared by all 34 Y voxels without cross-workgroup synchronization.
@compute @workgroup_size(8, 1, 8)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if i32(id.x) >= CHUNK_GEN_W || i32(id.z) >= CHUNK_GEN_W { return; }
    let wx = cfg.chunk_coord.x * CHUNK_SIZE + i32(id.x) - PAD;
    let wz = cfg.chunk_coord.z * CHUNK_SIZE + i32(id.z) - PAD;
    let base_y = cfg.chunk_coord.y * CHUNK_SIZE - PAD;
    if base_y + CHUNK_GEN_W <= min_world_y() || base_y >= max_world_y() {
        for (var y = 0; y < CHUNK_GEN_W; y = y + 1) {
            out_blocks[linear_index(i32(id.x), y, i32(id.z))] = block_air();
        }
        return;
    }
    let column = sample_column(wx, wz);
    for (var y = 0; y < CHUNK_GEN_W; y = y + 1) {
        let wy = base_y + y;
        out_blocks[linear_index(i32(id.x), y, i32(id.z))] = sample_block_in_column(wx, wy, wz, column);
    }
}
