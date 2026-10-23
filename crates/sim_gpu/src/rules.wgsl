// The movement, conduction and block driver of the rule (engine_sim rules.rs); see pinned.wgsl.

// ---- rules.rs ----

fn idx(x: u32, y: u32, z: u32) -> u32 {
    return x + 2u * z + 4u * y;
}

fn movable(c: Cell) -> bool {
    let k = class_of(c.m);
    return k == VOID || k == POWDER || k == LIQUID || k == GAS;
}

fn spreads(c: Cell) -> bool {
    let k = class_of(c.m);
    return k == LIQUID || k == GAS;
}

fn transfer(src_i: u32, dst_i: u32, amount: u32) {
    let src = cells[src_i];
    var dst = cells[dst_i];
    if (dst.v == 0u) {
        dst = Cell(src.m, amount, src.t, 0u, 0u);
    } else {
        let a = amount;
        let b = dst.v;
        dst.t = (dst.t * b + src.t * a) / (a + b);
        dst.v = dst.v + amount;
    }
    cells[dst_i] = dst;
    cells[src_i].v = cells[src_i].v - amount;
    if (cells[src_i].v == 0u) {
        cells[src_i] = empty_cell();
    }
}

fn swap_cells(a: u32, b: u32) {
    let c = cells[a];
    cells[a] = cells[b];
    cells[b] = c;
}

fn fall(lo: u32, up: u32) {
    let l = cells[lo];
    let u = cells[up];
    if (!movable(l) || !movable(u)) {
        return;
    }
    if (u.m == l.m) {
        if (class_of(u.m) == LIQUID) {
            var room = 0u;
            if (VOLUME_FULL > l.v) {
                room = VOLUME_FULL - l.v;
            }
            let amount = min(u.v, room);
            if (amount > 0u) {
                transfer(up, lo, amount);
            }
        }
    } else if (density(u.m) > density(l.m)) {
        swap_cells(up, lo);
    }
}

fn slide(x: u32, z: u32) {
    let up = idx(x, 1u, z);
    if (class_of(cells[up].m) != POWDER || cells[idx(x, 0u, z)].v == 0u) {
        return;
    }
    let random = roll(1u);
    for (var k = 0u; k < 2u; k = k + 1u) {
        let pick = (k + ((random >> (2u * (x + 2u * z))) & 1u)) % 2u;
        var nx = x ^ 1u;
        var nz = z;
        if (pick == 1u) {
            nx = x;
            nz = z ^ 1u;
        }
        let n_lo = idx(nx, 0u, nz);
        let n_up = idx(nx, 1u, nz);
        if (cells[n_lo].v == 0u && cells[n_up].v == 0u) {
            swap_cells(up, n_lo);
            return;
        }
    }
}

fn level(a: u32, b: u32, salt: u32) {
    let ca = cells[a];
    let cb = cells[b];
    var src_i = 0u;
    var dst_i = 0u;
    var amount = 0u;
    var gate = 0u;
    if (ca.m == cb.m && spreads(ca)) {
        if (ca.v >= cb.v) {
            src_i = a; dst_i = b; amount = (ca.v - cb.v) / 2u;
        } else {
            src_i = b; dst_i = a; amount = (cb.v - ca.v) / 2u;
        }
        gate = flow_of(ca.m);
    } else if (cb.v == 0u && spreads(ca)) {
        src_i = a; dst_i = b; amount = ca.v / 2u; gate = flow_of(ca.m);
    } else if (ca.v == 0u && spreads(cb)) {
        src_i = b; dst_i = a; amount = cb.v / 2u; gate = flow_of(cb.m);
    } else {
        return;
    }
    if (amount == 0u) {
        return;
    }
    if (gate == 255u || (roll(salt) & 0xFFu) < gate) {
        transfer(src_i, dst_i, amount);
    } else {
        pending = true;
    }
}

fn heat_capacity(c: Cell) -> u32 {
    return c.v * thermal_mass(c.m);
}

fn steps(whole: u32, rem: u32, c: u32, random: u32) -> u32 {
    return whole + select(0u, 1u, mulhi(random, c) < rem);
}

fn is_void(c: Cell) -> bool {
    return class_of(c.m) == VOID;
}

fn sat_sub16(t: u32, d: u32) -> u32 {
    return select(0u, t - d, t > d);
}

fn sat_add16(t: u32, d: u32) -> u32 {
    return min(t + d, 65535u);
}

fn conduct(a: u32, b: u32, salt: u32) -> bool {
    let void_a = is_void(cells[a]);
    let void_b = is_void(cells[b]);
    if (void_a && void_b) {
        return false;
    }
    let ta = cells[a].t;
    let tb = cells[b].t;
    let d = max(ta, tb) - min(ta, tb);
    if (d < HEAT_DEADBAND) {
        return false;
    }
    let ka = conductivity(cells[a].m);
    let kb = conductivity(cells[b].m);
    let ca = heat_capacity(cells[a]);
    let cb = heat_capacity(cells[b]);
    if (ka == 0u || kb == 0u || (!void_a && ca == 0u) || (!void_b && cb == 0u)) {
        return false;
    }
    let same = cells[a].m == cells[b].m && cells[a].v == cells[b].v;
    var k = ka;
    if (!same) {
        k = mul_div_rem(2u * ka, kb, ka + kb).x;
    }
    if (void_a || void_b) {
        var cell = a;
        var env_temp = tb;
        var c = ca;
        if (void_a) {
            cell = b;
            env_temp = ta;
            c = cb;
        }
        let g = min(k * HEAT_RATE * HEAT_TICK_INTERVAL, c / 2u);
        let wr = mul_div_rem(g, d, c);
        let moved = min(steps(wr.x, wr.y, c, roll(salt)), 65535u);
        let t = cells[cell].t;
        if (t > env_temp) {
            cells[cell].t = sat_sub16(t, moved);
        } else {
            cells[cell].t = sat_add16(t, moved);
        }
        return true;
    }
    var equalise = 0u;
    if (same) {
        equalise = ca / 2u;
    } else {
        equalise = mul_div_rem(ca, cb, ca + cb).x;
    }
    let g = min(k * HEAT_RATE * HEAT_TICK_INTERVAL, equalise / 2u);
    var hot = b;
    var cold = a;
    if (ta > tb) {
        hot = a;
        cold = b;
    }
    let c_hot = heat_capacity(cells[hot]);
    let c_cold = heat_capacity(cells[cold]);
    let wh = mul_div_rem(g, d, c_hot);
    var wc = wh;
    if (!same) {
        wc = mul_div_rem(g, d, c_cold);
    }
    let down = steps(wh.x, wh.y, c_hot, roll(salt));
    let up = steps(wc.x, wc.y, c_cold, roll(salt + 1u));
    cells[hot].t = sat_sub16(cells[hot].t, min(down, 65535u));
    cells[cold].t = sat_add16(cells[cold].t, min(up, 65535u));
    return true;
}

// ---- the block ----

fn unpack(w0: u32, w1: u32) -> Cell {
    return Cell(w0 & 0xFFu, (w0 >> 8u) & 0xFFu, w0 >> 16u, w1 & 0xFFFFu, w1 >> 16u);
}

fn pack0(c: Cell) -> u32 {
    return c.m | (c.v << 8u) | (c.t << 16u);
}

fn pack1(c: Cell) -> u32 {
    return c.p | (c.q << 16u);
}

// `block_rule`: steps the eight cells in `cells`, setting `pending`.
fn apply_rule(seed: u32, pc: u32, anchor: vec3<i32>) {
    pending = false;
    dice_base = block_base(seed, pc, anchor);
    fall(idx(0u, 0u, 0u), idx(0u, 1u, 0u));
    fall(idx(1u, 0u, 0u), idx(1u, 1u, 0u));
    fall(idx(0u, 0u, 1u), idx(0u, 1u, 1u));
    fall(idx(1u, 0u, 1u), idx(1u, 1u, 1u));
    slide(0u, 0u);
    slide(1u, 0u);
    slide(0u, 1u);
    slide(1u, 1u);
    for (var y = 0u; y < 2u; y = y + 1u) {
        level(idx(0u, y, 0u), idx(1u, y, 0u), 2u + y * 4u);
        level(idx(0u, y, 1u), idx(1u, y, 1u), 3u + y * 4u);
        level(idx(0u, y, 0u), idx(0u, y, 1u), 4u + y * 4u);
        level(idx(1u, y, 0u), idx(1u, y, 1u), 5u + y * 4u);
    }
    if (is_heat_phase(pc)) {
        var flat_t = true;
        for (var i = 1u; i < 8u; i = i + 1u) {
            flat_t = flat_t && cells[i].t == cells[0].t;
        }
        if (!flat_t) {
            var salt = 16u;
            for (var a = 0u; a < 8u; a = a + 1u) {
                for (var bit = 1u; bit <= 4u; bit = bit << 1u) {
                    if ((a & bit) == 0u) {
                        pending = conduct(a, a | bit, salt) || pending;
                        salt = salt + 2u;
                    }
                }
            }
        }
        react((pc & 1u) == 0u);
    }
}
