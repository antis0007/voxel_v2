// Resident entry: the brick pool, its directory and the awake bitmaps live on the GPU, and a tick needs
// no readback. A tick is nine dispatches (see `GpuWorld::step`): `bits` START, `compact`, `finalize`, `blocks`
// phase 0, `bits` MID, `compact`, `finalize`, `blocks` phase 1, `bits` END. They reproduce `engine_sim::step_with`:
//   A = dirty at the start; phase 0 steps the blocks touching A and wakes W0;
//   phase 1 steps the blocks touching A | W0; it wakes W1;
//   dirty afterwards = W0 | W1, plus A on a tick without heat; every set masked to present bricks.
// A brick is present when the directory holds a slot for it; a block that writes into an absent brick
// takes one from the free stack. Bricks are indexed in a window of chunks: `chunk * 64 + brick`.

struct Params {
    seed: u32,
    phase_counter: u32,
    // 0 or 1: the Margolus phase; for `bits`, the mode.
    phase: u32,
    heat_tick: u32,
    // Window origin in chunks.
    origin: vec4<i32>,
    // Window size in chunks; w = bricks in the window.
    dims: vec4<u32>,
    // x: words per bitmap; y: pool capacity in bricks.
    sizes: vec4<u32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> rules: array<u32>;
// Two words per cell, `BRICK_CELLS` cells per slot.
@group(0) @binding(2) var<storage, read_write> pool: array<u32>;
// Slot of each window brick, or NO_SLOT.
@group(0) @binding(3) var<storage, read_write> directory: array<atomic<u32>>;
// Five bitmaps of `sizes.x` words: dirty, A, S (this phase's active set), W0, W1.
@group(0) @binding(4) var<storage, read_write> bits: array<atomic<u32>>;
// Indirect dispatch arguments (x, y, z), the active brick count, then the active brick list.
@group(0) @binding(5) var<storage, read_write> list: array<atomic<u32>>;
// free_top, returned_count, overflow, changed_blocks; the free stack; slots returned this phase.
@group(0) @binding(6) var<storage, read_write> alloc: array<atomic<u32>>;

const NO_SLOT: u32 = 0xFFFFFFFFu;
const DIRTY: u32 = 0u;
const ACTIVE: u32 = 1u;
const STEPPED: u32 = 2u;
const WOKEN0: u32 = 3u;
const MODE_START: u32 = 0u;
const MODE_MID: u32 = 1u;
const MODE_END: u32 = 2u;
const MODE_WAKE_ALL: u32 = 3u;
const FREE_TOP: u32 = 0u;
const RETURNED: u32 = 1u;
const OVERFLOW: u32 = 2u;
const CHANGED: u32 = 3u;
const STACK: u32 = 4u;

fn word(region: u32, w: u32) -> u32 {
    return region * params.sizes.x + w;
}

fn bit_set(region: u32, b: u32) -> bool {
    return ((atomicLoad(&bits[word(region, b >> 5u)]) >> (b & 31u)) & 1u) != 0u;
}

// Present bricks of bitmap word `w`.
fn present(w: u32) -> u32 {
    var m = 0u;
    for (var j = 0u; j < 32u; j = j + 1u) {
        if (atomicLoad(&directory[w * 32u + j]) != NO_SLOT) {
            m = m | (1u << j);
        }
    }
    return m;
}

const LIST: u32 = 4u;
// Workgroups per dispatch dimension; more active bricks spill into y.
const MAX_GROUPS: u32 = 65535u;

fn reset_list() {
    atomicStore(&list[0], 0u);
    atomicStore(&list[1], 1u);
    atomicStore(&list[2], 1u);
    atomicStore(&list[3], 0u);
}

// Slots that lost an allocation race go back on the free stack; nothing pops between phases.
fn recycle() {
    let n = atomicLoad(&alloc[RETURNED]);
    let top = atomicLoad(&alloc[FREE_TOP]);
    let returned = STACK + params.sizes.y;
    for (var i = 0u; i < n; i = i + 1u) {
        atomicStore(&alloc[STACK + top + i], atomicLoad(&alloc[returned + i]));
    }
    atomicStore(&alloc[FREE_TOP], top + n);
    atomicStore(&alloc[RETURNED], 0u);
}

@compute @workgroup_size(64)
fn bits_pass(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = id.x;
    if (w == 0u) {
        reset_list();
        recycle();
    }
    if (w >= params.sizes.x) {
        return;
    }
    let mode = params.phase;
    if (mode == MODE_START) {
        let d = atomicLoad(&bits[word(DIRTY, w)]);
        atomicStore(&bits[word(ACTIVE, w)], d);
        atomicStore(&bits[word(STEPPED, w)], d);
        atomicStore(&bits[word(WOKEN0, w)], 0u);
        atomicStore(&bits[word(WOKEN0 + 1u, w)], 0u);
        atomicStore(&bits[word(DIRTY, w)], 0u);
    } else if (mode == MODE_MID) {
        let s = atomicLoad(&bits[word(ACTIVE, w)]) | (atomicLoad(&bits[word(WOKEN0, w)]) & present(w));
        atomicStore(&bits[word(STEPPED, w)], s);
    } else if (mode == MODE_END) {
        var d = atomicLoad(&bits[word(WOKEN0, w)]) | atomicLoad(&bits[word(WOKEN0 + 1u, w)]);
        if (params.heat_tick == 0u) {
            d = d | atomicLoad(&bits[word(ACTIVE, w)]);
        }
        atomicStore(&bits[word(DIRTY, w)], d & present(w));
    } else if (mode == MODE_WAKE_ALL) {
        atomicStore(&bits[word(DIRTY, w)], present(w));
    }
}

@compute @workgroup_size(64)
fn compact(@builtin(global_invocation_id) id: vec3<u32>) {
    let b = id.x;
    if (b >= params.dims.w || !bit_set(STEPPED, b)) {
        return;
    }
    let i = atomicAdd(&list[3], 1u);
    atomicStore(&list[LIST + i], b);
}

// The indirect arguments for `blocks_pass`, from the count `compact` left.
@compute @workgroup_size(1)
fn finalize() {
    let n = atomicLoad(&list[3]);
    atomicStore(&list[0], min(n, MAX_GROUPS));
    atomicStore(&list[1], (n + MAX_GROUPS - 1u) / MAX_GROUPS);
}

// Window brick index of the brick holding voxel `p`, or NO_SLOT outside the window; `cell` gets the
// cell's index inside the brick (x + 8 z + 64 y, as `brick_and_cell`).
// Floor division by a positive power of two (an arithmetic shift), exact for every i32.
fn floor_shift(a: vec3<i32>, bits_: u32) -> vec3<i32> {
    return a >> vec3<u32>(bits_);
}

fn locate(p: vec3<i32>, cell: ptr<function, u32>) -> u32 {
    let gb = floor_shift(p, 3u);
    let chunk = floor_shift(gb, 2u) - params.origin.xyz;
    if (any(chunk < vec3<i32>(0)) || any(vec3<u32>(chunk) >= params.dims.xyz)) {
        return NO_SLOT;
    }
    let lb = vec3<u32>(gb - (chunk + params.origin.xyz) * 4);
    let lc = vec3<u32>(p - gb * 8);
    *cell = lc.x + 8u * lc.z + 64u * lc.y;
    let c = vec3<u32>(chunk);
    let chunk_index = c.x + params.dims.x * (c.z + params.dims.z * c.y);
    return chunk_index * 64u + lb.x + 4u * lb.z + 16u * lb.y;
}

const OFFSETS: array<vec3<i32>, 8> = array<vec3<i32>, 8>(
    vec3<i32>(0, 0, 0), vec3<i32>(1, 0, 0), vec3<i32>(0, 0, 1), vec3<i32>(1, 0, 1),
    vec3<i32>(0, 1, 0), vec3<i32>(1, 1, 0), vec3<i32>(0, 1, 1), vec3<i32>(1, 1, 1),
);

// One workgroup per active brick; one invocation per block that overlaps it (64 in phase 0, 125 in
// phase 1). A block overlapping several active bricks is stepped by the lowest-indexed of them.
@compute @workgroup_size(128)
fn blocks_pass(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) t: u32) {
    let slot = group.x + group.y * MAX_GROUPS;
    if (slot >= atomicLoad(&list[3])) {
        return;
    }
    let brick = atomicLoad(&list[LIST + slot]);
    let phase = params.phase;
    let n = 4u + phase;
    if (t >= n * n * n) {
        return;
    }
    let chunk_index = brick / 64u;
    let bi = brick % 64u;
    let cw = vec3<i32>(
        i32(chunk_index % params.dims.x),
        i32(chunk_index / (params.dims.x * params.dims.z)),
        i32((chunk_index / params.dims.x) % params.dims.z),
    );
    let origin = (params.origin.xyz + cw) * 32 + vec3<i32>(i32(bi % 4u), i32(bi / 16u), i32((bi / 4u) % 4u)) * 8;
    let k = vec3<i32>(i32(t % n), i32(t / (n * n)), i32((t / n) % n));
    let anchor = origin + 2 * k - vec3<i32>(i32(phase));

    var where_: array<u32, 8>;
    var cell_at: array<u32, 8>;
    var owner = NO_SLOT;
    for (var i = 0u; i < 8u; i = i + 1u) {
        var c = 0u;
        let wb = locate(anchor + OFFSETS[i], &c);
        where_[i] = wb;
        cell_at[i] = c;
        if (wb != NO_SLOT && bit_set(STEPPED, wb)) {
            owner = min(owner, wb);
        }
    }
    if (owner != brick) {
        return;
    }
    var slots: array<u32, 8>;
    var before: array<u32, 16>;
    var held = false;
    for (var i = 0u; i < 8u; i = i + 1u) {
        var s = NO_SLOT;
        if (where_[i] != NO_SLOT) {
            s = atomicLoad(&directory[where_[i]]);
        }
        slots[i] = s;
        if (s == NO_SLOT) {
            before[2u * i] = pack0(empty_cell());
            before[2u * i + 1u] = pack1(empty_cell());
        } else {
            held = true;
            let at = (s * 512u + cell_at[i]) * 2u;
            before[2u * i] = pool[at];
            before[2u * i + 1u] = pool[at + 1u];
        }
        cells[i] = unpack(before[2u * i], before[2u * i + 1u]);
    }
    if (!held) {
        return;
    }
    apply_rule(params.seed, params.phase_counter, anchor);
    var changed = false;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let w0 = pack0(cells[i]);
        let w1 = pack1(cells[i]);
        if (w0 == before[2u * i] && w1 == before[2u * i + 1u]) {
            continue;
        }
        changed = true;
        if (where_[i] == NO_SLOT) {
            atomicOr(&alloc[OVERFLOW], 1u);
            continue;
        }
        var s = slots[i];
        if (s == NO_SLOT) {
            s = ensure_slot(where_[i]);
            if (s == NO_SLOT) {
                continue;
            }
        }
        let at = (s * 512u + cell_at[i]) * 2u;
        pool[at] = w0;
        pool[at + 1u] = w1;
    }
    if (changed) {
        atomicAdd(&alloc[CHANGED], 1u);
    }
    if (changed || pending) {
        let region = WOKEN0 + phase;
        for (var i = 0u; i < 8u; i = i + 1u) {
            let wb = where_[i];
            if (wb != NO_SLOT) {
                atomicOr(&bits[word(region, wb >> 5u)], 1u << (wb & 31u));
            }
        }
    }
}
