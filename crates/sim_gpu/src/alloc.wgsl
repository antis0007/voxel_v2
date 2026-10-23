// The resident pool's allocator: a free stack popped with compare-and-swap, and a directory entry
// claimed with compare-and-swap; nothing ever waits on another invocation.

// Pop a free slot; NO_SLOT when the pool is full.
fn pop_slot() -> u32 {
    var slot = NO_SLOT;
    loop {
        let top = atomicLoad(&alloc[FREE_TOP]);
        if (top == 0u) {
            break;
        }
        let r = atomicCompareExchangeWeak(&alloc[FREE_TOP], top, top - 1u);
        if (r.exchanged) {
            slot = atomicLoad(&alloc[STACK + top - 1u]);
            break;
        }
    }
    return slot;
}

// The slot of window brick `wb`, taking a free one if it has none. Another block of this phase may race
// for the same brick; the loser hands its slot back (recycled before the next phase).
fn ensure_slot(wb: u32) -> u32 {
    let held = atomicLoad(&directory[wb]);
    if (held != NO_SLOT) {
        return held;
    }
    let fresh = pop_slot();
    if (fresh == NO_SLOT) {
        // Another block may have taken the last slot for this same brick: use it if so.
        let late = atomicLoad(&directory[wb]);
        if (late == NO_SLOT) {
            atomicOr(&alloc[OVERFLOW], 2u);
        }
        return late;
    }
    var slot = fresh;
    loop {
        let r = atomicCompareExchangeWeak(&directory[wb], NO_SLOT, fresh);
        if (r.exchanged) {
            break;
        }
        if (r.old_value != NO_SLOT) {
            let i = atomicAdd(&alloc[RETURNED], 1u);
            atomicStore(&alloc[STACK + params.sizes.y + i], fresh);
            slot = r.old_value;
            break;
        }
    }
    return slot;
}

