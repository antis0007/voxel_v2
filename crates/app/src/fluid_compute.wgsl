struct FluidParams {
    voxel_count: u32,
    packed_count: u32,
    chunk_w: u32,
    chunk_h: u32,
    dt: f32,
    lateral: f32,
    downward: f32,
    upward: f32,
    ripple_amp: f32,
    ripple_freq: f32,
    ripple_speed: f32,
    time: f32,
};

@group(0) @binding(0) var<uniform> params: FluidParams;
@group(0) @binding(1) var<storage, read> in_levels: array<u32>;
@group(0) @binding(2) var<storage, read_write> out_levels: array<u32>;
@group(0) @binding(3) var<storage, read> solid_mask: array<u32>;

fn level_at(index: u32) -> u32 {
    let word = in_levels[index >> 2u];
    let shift = (index & 3u) * 8u;
    return (word >> shift) & 0xffu;
}

fn solid_at(index: u32) -> bool {
    let word = solid_mask[index >> 5u];
    return ((word >> (index & 31u)) & 1u) != 0u;
}

fn index_of(x: i32, y: i32, z: i32) -> i32 {
    let w = i32(params.chunk_w);
    let h = i32(params.chunk_h);
    if x < 0 || y < 0 || z < 0 || x >= w || y >= h || z >= w {
        return -1;
    }
    return (y * w + z) * w + x;
}

fn sample_level(index: i32, fallback: f32) -> f32 {
    if index < 0 {
        return fallback;
    }
    let u = u32(index);
    if solid_at(u) {
        return fallback;
    }
    return f32(level_at(u));
}

fn compute_voxel(index: u32) -> u32 {
    if solid_at(index) {
        return 0u;
    }

    let w = i32(params.chunk_w);
    let plane = w * w;
    let idx = i32(index);
    let y = idx / plane;
    let rem = idx - y * plane;
    let z = rem / w;
    let x = rem - z * w;

    let self_level = f32(level_at(index));

    let east_idx = index_of(x + 1, y, z);
    let west_idx = index_of(x - 1, y, z);
    let south_idx = index_of(x, y, z + 1);
    let north_idx = index_of(x, y, z - 1);
    let above_idx = index_of(x, y + 1, z);
    let below_idx = index_of(x, y - 1, z);

    let east = sample_level(east_idx, self_level);
    let west = sample_level(west_idx, self_level);
    let south = sample_level(south_idx, self_level);
    let north = sample_level(north_idx, self_level);
    let above = sample_level(above_idx, self_level);
    let below = sample_level(below_idx, self_level);

    let lateral_avg = (east + west + south + north) * 0.25;
    var next = self_level + (lateral_avg - self_level) * params.lateral * params.dt;

    let down_out = max(0.0, self_level - below) * params.downward * params.dt;
    let down_in = max(0.0, above - self_level) * params.downward * params.dt;
    let up_out = max(0.0, self_level - above) * params.upward * params.dt;
    let up_in = max(0.0, below - self_level) * params.upward * params.dt;
    next += down_in + up_in - down_out - up_out;

    var exposed = false;
    if above_idx < 0 {
        exposed = true;
    } else {
        let au = u32(above_idx);
        exposed = !solid_at(au) && level_at(au) == 0u;
    }

    if self_level > 0.0 && exposed {
        let phase = (f32(x) + f32(z)) * params.ripple_freq + params.time * params.ripple_speed;
        let lap = lateral_avg - self_level;
        next += (sin(phase) * params.ripple_amp + lap * 0.22) * params.dt;
    }

    next = clamp(next, 0.0, 255.0);
    return u32(next + 0.5);
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let packed_index = gid.x;
    if packed_index >= params.packed_count {
        return;
    }

    let base = packed_index * 4u;
    var out_word = 0u;

    for (var lane = 0u; lane < 4u; lane = lane + 1u) {
        let voxel_index = base + lane;
        if voxel_index >= params.voxel_count {
            break;
        }
        let level = compute_voxel(voxel_index);
        out_word = out_word | (level << (lane * 8u));
    }

    out_levels[packed_index] = out_word;
}
