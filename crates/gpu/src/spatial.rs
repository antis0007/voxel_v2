//! Conservative spatial grouping for GPU local effects. No game identifiers.

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SphereGroup {
    pub min_first: [f32; 4],
    pub max_count: [f32; 4],
}

/// Partition along each region's widest axis, then bound groups including influence radii.
/// Returns the source permutation and bounds whose offsets address that permutation.
pub fn sphere_groups(
    spheres: &[([f32; 3], f32)],
    group_size: usize,
) -> (Vec<usize>, Vec<SphereGroup>) {
    let mut order: Vec<_> = (0..spheres.len()).collect();
    let size = group_size.max(1);
    order_spatially(&mut order, spheres, size);
    let mut groups = Vec::new();
    for (group, indices) in order.chunks(size).enumerate() {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for i in indices {
            let (p, radius) = spheres[*i];
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis] - radius);
                max[axis] = max[axis].max(p[axis] + radius);
            }
        }
        groups.push(SphereGroup {
            min_first: [min[0], min[1], min[2], (group * size) as f32],
            max_count: [max[0], max[1], max[2], indices.len() as f32],
        });
    }
    (order, groups)
}

/// Conservative tangent-plane test for influences attached to a spherical surface.
/// An influence overlapping the horizon stays eligible, even if its centre is hidden.
pub fn above_spherical_horizon(
    position: [f32; 3],
    influence_radius: f32,
    eye: [f64; 3],
    planet_radius: f64,
) -> bool {
    let eye_length = eye.iter().map(|v| v * v).sum::<f64>().sqrt();
    if eye_length <= planet_radius {
        return true;
    }
    let dot: f64 = position
        .iter()
        .zip(eye)
        .map(|(p, e)| f64::from(*p) * e)
        .sum();
    dot + f64::from(influence_radius.max(0.0)) * eye_length >= planet_radius * planet_radius
}

/// Allocation-free bounds for an existing ordered light/effect list.
/// Keeping caller order is useful when the list encodes different source classes.
pub fn influence_bounds(
    spheres: impl IntoIterator<Item = ([f32; 3], f32)>,
) -> Option<([f32; 3], [f32; 3])> {
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for (position, radius) in spheres {
        any = true;
        for axis in 0..3 {
            low[axis] = low[axis].min(position[axis] - radius.max(0.0));
            high[axis] = high[axis].max(position[axis] + radius.max(0.0));
        }
    }
    any.then_some((low, high))
}

fn order_spatially(order: &mut [usize], spheres: &[([f32; 3], f32)], size: usize) {
    if order.len() <= size {
        return;
    }
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for i in order.iter() {
        for axis in 0..3 {
            low[axis] = low[axis].min(spheres[*i].0[axis]);
            high[axis] = high[axis].max(spheres[*i].0[axis]);
        }
    }
    let axis = (0..3)
        .max_by(|a, b| (high[*a] - low[*a]).total_cmp(&(high[*b] - low[*b])))
        .unwrap_or(0);
    order.sort_unstable_by(|a, b| {
        spheres[*a].0[axis]
            .total_cmp(&spheres[*b].0[axis])
            .then(a.cmp(b))
    });
    let split = (order.len().div_ceil(size) / 2) * size;
    let (left, right) = order.split_at_mut(split);
    order_spatially(left, spheres, size);
    order_spatially(right, spheres, size);
}
