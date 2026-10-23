// The reaction pass (engine_sim react.rs); see pinned.wgsl.

// ---- react.rs ----

// Turn cells[i] into `into`; returns whether its material changed.
fn become_(i: u32, into: u32, temp: u32) -> bool {
    if (into == NONE || (into == cells[i].m && cells[i].v > 0u)) {
        if (temp != 0u) {
            cells[i].t = temp;
        }
        return false;
    }
    if (class_of(into) == VOID) {
        cells[i] = empty_cell();
        return true;
    }
    var volume = cells[i].v;
    if (volume == 0u) {
        volume = VOLUME_FULL;
    }
    var t = cells[i].t;
    if (temp != 0u) {
        t = temp;
    }
    cells[i].m = into;
    cells[i].v = volume;
    cells[i].t = t;
    return true;
}

fn fires(rate: u32, salt: u32) -> bool {
    let fire = rate == ALWAYS || roll(salt) < rate;
    if (!fire) {
        pending = true;
    }
    return fire;
}

fn holds(row: u32, t: u32) -> bool {
    let w = rules[ROW_BASE + row * 4u + 1u];
    return (w & 0xFFFFu) <= t && t <= (w >> 16u);
}

fn edge(e: u32) -> vec2<u32> {
    switch e {
        case 0u: { return vec2<u32>(0u, 1u); }
        case 1u: { return vec2<u32>(0u, 2u); }
        case 2u: { return vec2<u32>(0u, 4u); }
        case 3u: { return vec2<u32>(1u, 3u); }
        case 4u: { return vec2<u32>(1u, 5u); }
        case 5u: { return vec2<u32>(2u, 3u); }
        case 6u: { return vec2<u32>(2u, 6u); }
        case 7u: { return vec2<u32>(3u, 7u); }
        case 8u: { return vec2<u32>(4u, 5u); }
        case 9u: { return vec2<u32>(4u, 6u); }
        case 10u: { return vec2<u32>(5u, 7u); }
        default: { return vec2<u32>(6u, 7u); }
    }
}

fn phase_change(i: u32) -> u32 {
    let m = cells[i].m;
    let t = cells[i].t;
    let melt = melt_point(m);
    let boil = boil_point(m);
    let cool = cool_point(m);
    if (melt != 0u && melts_into(m) != NONE && t >= melt) {
        return melts_into(m);
    }
    if (boil != 0u && boils_into(m) != NONE && t >= boil) {
        return boils_into(m);
    }
    if (cool != 0u && cools_into(m) != NONE && t <= cool) {
        return cools_into(m);
    }
    return NONE;
}

fn react(first_phase: bool) {
    var has = false;
    for (var i = 0u; i < 8u; i = i + 1u) {
        has = has || reactive(cells[i].m);
    }
    if (!has) {
        return;
    }
    var done = 0u;
    for (var e = 0u; e < 12u; e = e + 1u) {
        let ab = edge(e);
        let a = ab.x;
        let b = ab.y;
        if ((done & ((1u << a) | (1u << b))) != 0u) {
            continue;
        }
        let entry = byte_at(PAIR_BASE, cells[a].m * 256u + cells[b].m);
        if (entry == 0u) {
            continue;
        }
        let row = entry - 1u;
        let w3 = rules[ROW_BASE + row * 4u + 3u];
        var conditioned = cells[a].t;
        if (((w3 >> 16u) & 1u) != 0u) {
            conditioned = cells[b].t;
        }
        if (holds(row, conditioned) && fires(rules[ROW_BASE + row * 4u], SALT_PAIR + e)) {
            let w2 = rules[ROW_BASE + row * 4u + 2u];
            if (become_(a, w3 & 0xFFu, w2 & 0xFFFFu)) {
                done = done | (1u << a);
            }
            if (become_(b, (w3 >> 8u) & 0xFFu, w2 >> 16u)) {
                done = done | (1u << b);
            }
        }
    }
    for (var i = 0u; i < 8u; i = i + 1u) {
        if (!first_phase) {
            break;
        }
        let entry = byte_at(UNARY_BASE, cells[i].m);
        if (entry == 0u || (done & (1u << i)) != 0u) {
            continue;
        }
        let row = entry - 1u;
        if (holds(row, cells[i].t) && fires(rules[ROW_BASE + row * 4u], SALT_UNARY + i)) {
            let w2 = rules[ROW_BASE + row * 4u + 2u];
            let w3 = rules[ROW_BASE + row * 4u + 3u];
            if (become_(i, w3 & 0xFFu, w2 & 0xFFFFu)) {
                done = done | (1u << i);
            }
        }
    }
    for (var i = 0u; i < 8u; i = i + 1u) {
        let m = cells[i].m;
        let ignite = ignite_point(m);
        let burning = first_phase && ignite != 0u && burns_into(m) != NONE && cells[i].t >= ignite;
        if ((done & (1u << i)) == 0u && burning && fires(burn_rate(fuel_of(m)), SALT_BURN + i)) {
            if (become_(i, burns_into(m), 0u)) {
                done = done | (1u << i);
            }
        }
    }
    for (var i = 0u; i < 8u; i = i + 1u) {
        if ((done & (1u << i)) == 0u) {
            let into = phase_change(i);
            if (into != NONE) {
                become_(i, into, 0u);
            }
        }
    }
}

