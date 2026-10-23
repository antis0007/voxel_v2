// Offload entry: the CPU gathered each block's eight cells; step them and report what changed.
struct Params { seed: u32, phase_counter: u32, count: u32, pad: u32 }

@group(0) @binding(0) var<uniform> params: Params;
// Packed `SimRules`: see `pack_rules` in lib.rs for the offsets.
@group(0) @binding(1) var<storage, read> rules: array<u32>;
@group(0) @binding(2) var<storage, read> anchors: array<vec4<i32>>;
// Sixteen words per block (eight cells of two words), rewritten in place.
@group(0) @binding(3) var<storage, read_write> blocks: array<u32>;
// Per block: bit 0 changed, bit 1 pending.
@group(0) @binding(4) var<storage, read_write> outcomes: array<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let k = id.x;
    if (k >= params.count) {
        return;
    }
    let base = k * 16u;
    var before: array<u32, 16>;
    for (var i = 0u; i < 16u; i = i + 1u) {
        before[i] = blocks[base + i];
    }
    for (var i = 0u; i < 8u; i = i + 1u) {
        cells[i] = unpack(before[2u * i], before[2u * i + 1u]);
    }
    apply_rule(params.seed, params.phase_counter, anchors[k].xyz);
    var changed = false;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let w0 = pack0(cells[i]);
        let w1 = pack1(cells[i]);
        changed = changed || w0 != before[2u * i] || w1 != before[2u * i + 1u];
        blocks[base + 2u * i] = w0;
        blocks[base + 2u * i + 1u] = w1;
    }
    outcomes[k] = select(0u, 1u, changed) | select(0u, 2u, pending);
}
