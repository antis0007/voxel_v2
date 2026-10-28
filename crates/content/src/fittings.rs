//! Stable block states for metre-grid hinged fittings. Appended IDs preserve saves.
use crate::BlockId as B;
use glam::{IVec3, Vec3};

#[derive(Clone, Copy)]
pub struct State {
    pub door: bool,
    pub lever: bool,
    pub z: bool,
    pub open: bool,
    pub upper: bool,
}
pub fn state(b: B) -> Option<State> {
    let i = b as u32;
    let first = B::OakDoor as u32;
    if i < first || i > B::LeverOn as u32 {
        return None;
    }
    let i = i - first;
    Some(State {
        door: i < 8,
        lever: i >= 16,
        z: if i < 8 { i & 4 != 0 } else { i & 2 != 0 },
        open: i & 1 != 0,
        upper: if i < 8 { i & 2 != 0 } else { i & 4 != 0 },
    })
}
pub fn block(s: State) -> B {
    B::from_u32(
        B::OakDoor as u32
            + if s.lever {
                16 + s.open as u32
            } else if s.door {
                s.z as u32 * 4 + s.upper as u32 * 2 + s.open as u32
            } else {
                8 + s.upper as u32 * 4 + s.z as u32 * 2 + s.open as u32
            },
    )
}
pub fn canonical(b: B) -> Option<B> {
    state(b).map(|s| {
        if s.lever {
            B::Lever
        } else if s.door {
            B::OakDoor
        } else {
            B::OakTrapdoor
        }
    })
}
pub fn step(scale: f32) -> i32 {
    scale.recip().round().max(1.0) as i32
}
pub fn anchor(p: IVec3, n: i32) -> IVec3 {
    p.div_euclid(IVec3::splat(n)) * n
}

/// Physical boxes in block coordinates, clipped to the owning voxel. Empty
/// cells still reserve the assembly's opening, as a Minecraft door does.
pub fn boxes(b: B, p: IVec3, scale: f32) -> Vec<(Vec3, Vec3)> {
    let Some(s) = state(b) else {
        return Vec::new();
    };
    let n = step(scale);
    let base = anchor(p, n).as_vec3();
    let mut authored = Vec::with_capacity(4);
    if s.lever {
        authored.push((Vec3::new(0.25, 0.0, 0.25), Vec3::new(0.75, 0.14, 0.75)));
        let lean = if s.open { 1.0 } else { -1.0 };
        for i in 0..4 {
            let y = 0.14 + i as f32 * 0.08;
            let z = 0.5 + lean * i as f32 * 0.06;
            authored.push((
                Vec3::new(0.44, y, z - 0.06),
                Vec3::new(0.56, y + 0.10, z + 0.06),
            ));
        }
    } else {
        let mut hi = Vec3::ONE;
        let mut lo = Vec3::ZERO;
        let axis = if s.door {
            if s.z ^ s.open { 0 } else { 2 }
        } else if !s.open {
            1
        } else if s.z {
            0
        } else {
            2
        };
        hi[axis] = 0.1875;
        if !s.door && !s.open && s.upper {
            lo.y = 0.8125;
            hi.y = 1.0;
        }
        if s.upper || !s.door {
            // Four real openings, shared by the oak window and hatch lattice.
            let u = if axis == 0 { 2 } else { 0 };
            let v = if axis == 1 { 2 } else { 1 };
            for (u0, v0, u1, v1) in [
                (0.0, 0.0, 1.0, 0.2),
                (0.0, 0.8, 1.0, 1.0),
                (0.0, 0.2, 0.2, 0.8),
                (0.8, 0.2, 1.0, 0.8),
                (0.45, 0.2, 0.55, 0.8),
                (0.2, 0.45, 0.8, 0.55),
            ] {
                let mut a = lo;
                let mut b = hi;
                a[u] = u0;
                a[v] = v0;
                b[u] = u1;
                b[v] = v1;
                authored.push((a, b));
            }
        } else {
            authored.push((lo, hi));
        }
        if s.door && !s.upper {
            let tangent = if axis == 0 { 2 } else { 0 };
            let mut a = Vec3::ZERO;
            let mut b = Vec3::ONE;
            a.y = 0.85;
            b.y = 0.95;
            a[tangent] = 0.78;
            b[tangent] = 0.92;
            a[axis] = 0.1875;
            b[axis] = 0.26;
            authored.push((a, b));
        }
        // Raised cross rails and functional latch, on the same metric lattice.
        if axis != 1 {
            let tangent = if axis == 0 { 2 } else { 0 };
            for y in [0.18, 0.78] {
                let mut a = lo;
                let mut b = hi;
                a.y = y;
                b.y = y + 0.08;
                a[tangent] = 0.04;
                b[tangent] = 0.96;
                a[axis] = hi[axis];
                b[axis] += 0.035;
                authored.push((a, b));
            }
        }
    }
    let cell_min = p.as_vec3();
    let cell_max = cell_min + Vec3::ONE;
    authored
        .into_iter()
        .filter_map(|(a, b)| {
            let a = (base + a * n as f32).max(cell_min);
            let b = (base + b * n as f32).min(cell_max);
            a.cmplt(b).all().then_some((a, b))
        })
        .collect()
}
