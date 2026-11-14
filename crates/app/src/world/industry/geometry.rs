use super::*;

/// Face lookups depend on the metre immediately across that face. Ground
/// lookups depend on the vertical ray below the cell, not on distant edits.
pub(super) fn invalidate(state: &mut State, changed: &[IVec3], size: i32) {
    if changed.is_empty() { return; }
    // Large writes can be cheaper to clear than to index.
    if changed.len() > 512 {
        state.topology.clear();
        state.faces.clear();
        state.ground.clear();
        return;
    }
    let metre = |p: IVec3| p.div_euclid(IVec3::splat(size)) * size;
    let mut dirty = crate::fast_hash::FastSet::default();
    let mut columns = crate::fast_hash::FastMap::<(i32, i32), (i32, i32)>::default();
    for &p in changed {
        let o = metre(p);
        // A changed conduit can also change the arms of its neighbours.
        for q in std::iter::once(o).chain(super::super::factory::DIRS.map(|d| o + d * size)) {
            if !dirty.insert(q) { continue; }
            state.topology.remove(&(q.x, q.y, q.z));
            columns.entry((q.x, q.z)).and_modify(|(low, high)| {
                *low = (*low).min(q.y);
                *high = (*high).max(q.y + size - 1);
            }).or_insert((q.y, q.y + size - 1));
        }
    }
    state.faces.retain(|&(k, side), _| {
        let d = super::super::factory::DIRS[side];
        let sample = pos(k) + d * if d.element_sum() > 0 { k.3 } else { 1 };
        !dirty.contains(&metre(sample))
    });
    state.ground.retain(|&k, _| {
        let ray = metre(pos(k) + IVec3::new(k.3 / 2, 0, k.3 / 2));
        columns.get(&(ray.x, ray.z)).is_none_or(|&(low, high)| {
            low >= k.1 || high < k.1 - size * 32
        })
    });
}

pub fn open(world: &World, p: IVec3) -> bool {
    let cfg = world.sample_config();
    p.y >= cfg.min_world_y() && p.y < cfg.max_world_y() && {
        let b = world.get(p);
        let solid = if b.is_model() {
            super::super::factory::solid_point(
                world,
                (p.as_vec3() + glam::Vec3::splat(0.5)) * world.voxel_scale.meters(),
            )
        } else {
            b.is_solid()
        };
        !solid && b != B::Water && b != B::Lava
    }
}
pub fn owner(world: &World, state: &mut State, p: IVec3) -> Key {
    let size = super::super::factory::step(world);
    let o = p.div_euclid(IVec3::splat(size)) * size;
    let key = (o.x, o.y, o.z);
    let coarse = if let Some(&v) = state.topology.get(&key) {
        v
    } else {
        let mut clear = true;
        'scan: for y in 0..size {
            for z in 0..size {
                for x in 0..size {
                    if !open(world, o + IVec3::new(x, y, z)) {
                        clear = false;
                        break 'scan;
                    }
                }
            }
        }
        state.topology.insert(key, clear);
        clear
    };
    if coarse {
        (o.x, o.y, o.z, size)
    } else {
        (p.x, p.y, p.z, 1)
    }
}
pub fn neighbours(world: &World, state: &mut State, k: Key, side: usize) -> Vec<(Key, f32)> {
    if let Some(v) = state.faces.get(&(k, side)) {
        return v.clone();
    }
    let d = super::super::factory::DIRS[side];
    let o = pos(k);
    let mut targets: BTreeMap<Key, u32> = BTreeMap::new();
    for a in 0..k.3 {
        for b in 0..k.3 {
            let p = o + if d.x != 0 {
                IVec3::new(if d.x > 0 { k.3 } else { -1 }, a, b)
            } else if d.y != 0 {
                IVec3::new(a, if d.y > 0 { k.3 } else { -1 }, b)
            } else {
                IVec3::new(a, b, if d.z > 0 { k.3 } else { -1 })
            };
            if open(world, p) {
                *targets.entry(owner(world, state, p)).or_default() += 1;
            }
        }
    }
    let out: Vec<_> = targets
        .into_iter()
        .map(|(key, n)| (key, n as f32 / (k.3 * k.3) as f32))
        .collect();
    state.faces.insert((k, side), out.clone());
    out
}

pub fn ground(world: &World, state: &mut State, k: Key) -> Option<IVec3> {
    if let Some(v) = state.ground.get(&k) {
        return *v;
    }
    let p = pos(k) + IVec3::new(k.3 / 2, 0, k.3 / 2);
    let distance = super::super::factory::step(world) * 32;
    let result = (1..=distance)
        .map(|y| p - IVec3::Y * y)
        .find(|&q| !open(world, q));
    state.ground.insert(k, result);
    result
}
