//! Persistent sparse thermodynamics and voxel rigid assemblies.
use crate::world::BlockId;
use anyhow::{Result, ensure};
use glam::{IVec3, Quat, Vec3};
use std::collections::HashSet;
pub const MAX_CELLS: usize = 32768;
pub const MAX_BODIES: usize = 64;
pub const MAX_BODY_VOXELS: usize = 128;
pub const MAX_TREE_BODY_VOXELS: usize = 4096;

/// Integer shears preserve unique cells through rotation. Motion and snapshot
/// validation both compute the trigonometry once per pose.
pub(super) struct VoxelRotation([(f32, f32, u32, usize, usize); 3]);
impl VoxelRotation {
    pub(super) fn new(rotation: Quat) -> Self {
        let (x, y, z) = rotation.to_euler(glam::EulerRot::XYZ);
        Self([(z, 0, 1), (y, 2, 0), (x, 1, 2)].map(|(angle, a, b)| {
            let steps = (angle.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as u32;
            let theta = angle / steps as f32;
            ((theta * 0.5).tan(), theta.sin(), steps, a, b)
        }))
    }
    pub(super) fn apply(&self, mut q: IVec3) -> IVec3 {
        for &(shear, sine, steps, a, b) in &self.0 {
            for _ in 0..steps {
                q[a] -= (shear * q[b] as f32).round() as i32;
                q[b] += (sine * q[a] as f32).round() as i32;
                q[a] -= (shear * q[b] as f32).round() as i32;
            }
        }
        q
    }
}
#[derive(Clone, Copy)]
pub struct ThermalCell {
    pub temperature: f32,
    pub fuel: f32,
    pub gas: u8,
}
impl Default for ThermalCell {
    fn default() -> Self {
        Self {
            temperature: 20.0,
            fuel: 0.0,
            gas: 0,
        }
    }
}
pub struct RigidBody {
    /// Only chopped trees dissolve into loot; player-built log assemblies persist.
    pub felled_tree: bool,
    pub origin: IVec3,
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub angular: Vec3,
    pub debt: f32,
    /// Felled-tree bodies hold the wood and crown while they fall. This is runtime-only
    /// presentation state; it is not serialized and restarts at zero on load.
    pub settle_time: f32,
    /// Runtime-only water-contact edge for splash emission.
    pub was_submerged: bool,
    pub voxels: Vec<(IVec3, BlockId)>,
    pub occupied: Vec<(IVec3, BlockId)>,
}
impl RigidBody {
    /// Centre of the material actually represented in the world lattice.
    /// Rebasing fragments around this point preserves total linear momentum
    /// even when the parent's continuous pose has sub-voxel displacement.
    pub(super) fn occupied_center(&self) -> Vec3 {
        let (weighted, mass) = self.occupied.iter().fold((Vec3::ZERO, 0.0_f32), |(sum, mass), (p, b)| {
            let m = (b.material().physics.density / 1000.0).max(0.01);
            (sum + p.as_vec3() * m, mass + m)
        });
        weighted / mass.max(0.01)
    }

    /// Mass in water-density voxel units; inertia in mass * voxel-length squared.
    /// Keep the full tensor: long diagonal beams must resist torque along their
    /// actual principal axes, rather than along the world grid.
    pub(super) fn mass_properties(&self) -> (f32, Vec3, glam::Mat3) {
        let mass = self.voxels.iter().map(|(_, b)|
            (b.material().physics.density / 1000.0).max(0.01)).sum::<f32>().max(0.01);
        let center = self.voxels.iter().map(|(p, b)|
            p.as_vec3() * (b.material().physics.density / 1000.0).max(0.01))
            .sum::<Vec3>() / mass;
        let mut inertia = glam::Mat3::ZERO;
        for (p, b) in &self.voxels {
            let m = (b.material().physics.density / 1000.0).max(0.01);
            let r = p.as_vec3() - center;
            let outer = glam::Mat3::from_cols(r * r.x, r * r.y, r * r.z);
            inertia += (glam::Mat3::IDENTITY * (r.length_squared() + 1.0 / 6.0) - outer) * m;
        }
        (mass, center, inertia.inverse())
    }

    pub(super) fn inverse_inertia(&self, inverse: glam::Mat3, torque: Vec3) -> Vec3 {
        self.rotation * (inverse * (self.rotation.conjugate() * torque))
    }
}
#[derive(Default)]
pub struct Sandbox {
    /// Sparse thermal lattice. Hash lookup is materially cheaper than a
    /// B-tree for the per-neighbour heat solve and fluid sandbox queries.
    /// Encoding sorts keys, so save bytes remain deterministic.
    pub cells: crate::fast_hash::FastMap<(i32, i32, i32), ThermalCell>,
    pub bodies: Vec<RigidBody>,
    pub body_cursor: usize,
    /// Bounded burning-cell index used only to feed local lights. Entries are
    /// validated against the world before use and are never serialized.
    pub fire_lights: HashSet<IVec3>,
    /// Runtime ambient; synced from the world and day clock every fixed step.
    pub climate: super::climate::Climate,
}
pub fn key(p: IVec3) -> (i32, i32, i32) {
    (p.x, p.y, p.z)
}
impl Sandbox {
    /// Detaches existing world cells into a free-flying body; false when full or overlapping.
    pub fn add_debris(&mut self, origin: IVec3, cells: &[(IVec3, BlockId)], velocity: Vec3) -> bool {
        if !velocity.is_finite() || cells.is_empty()
            || cells.len() > MAX_BODY_VOXELS
            || self.bodies.len() >= MAX_BODIES
            || self.bodies.iter().any(|b| b.occupied.iter().any(|(q, _)| cells.iter().any(|(p, _)| p == q)))
        {
            return false;
        }
        self.bodies.push(RigidBody {
            felled_tree: false,
            origin,
            position: Vec3::ZERO,
            velocity,
            rotation: Quat::IDENTITY,
            angular: Vec3::ZERO,
            debt: 0.0,
            settle_time: 0.0,
            was_submerged: false,
            voxels: cells.iter().map(|&(p, b)| (p - origin, b)).collect(),
            occupied: cells.to_vec(),
        });
        true
    }
    /// The recorded cell, or one at the local air or ground ambient.
    pub fn cell(&self, p: IVec3) -> ThermalCell {
        self.cells.get(&key(p)).copied().unwrap_or_else(|| self.ambient_cell(p, BlockId::Air))
    }
    /// The recorded cell, or one at the ambient `block` sees at `p`.
    pub fn cell_for(&self, p: IVec3, block: BlockId) -> ThermalCell {
        self.cells.get(&key(p)).copied().unwrap_or_else(|| self.ambient_cell(p, block))
    }
    pub fn ambient_cell(&self, p: IVec3, block: BlockId) -> ThermalCell {
        ThermalCell { temperature: self.climate.ambient_for(p, block), ..ThermalCell::default() }
    }
    /// True when the cell carries nothing its absence would not.
    pub fn settled(&self, p: IVec3, block: BlockId, c: ThermalCell) -> bool {
        (c.temperature - self.climate.ambient_for(p, block)).abs() < 0.25 && c.fuel <= 0.0 && c.gas == 0
    }
    pub fn set(&mut self, p: IVec3, c: ThermalCell) -> bool {
        let k = key(p);
        if self.settled(p, BlockId::Air, c) {
            self.cells.remove(&k);
            return true;
        }
        if !self.cells.contains_key(&k) && self.cells.len() >= MAX_CELLS {
            return false;
        }
        self.cells.insert(k, c);
        true
    }
    pub fn can_admit(&self, positions: &[IVec3]) -> bool {
        // The common case needs no duplicate detection or allocation at all.
        if self.cells.len().saturating_add(positions.len()) <= MAX_CELLS {
            return true;
        }
        let mut incoming = crate::fast_hash::FastSet::default();
        for &p in positions {
            if !self.cells.contains_key(&key(p)) {
                incoming.insert(p);
                if self.cells.len() + incoming.len() > MAX_CELLS { return false; }
            }
        }
        true
    }

    pub fn note_fire(&mut self, p: IVec3) {
        if self.fire_lights.contains(&p) || self.fire_lights.len() < 64 {
            self.fire_lights.insert(p);
        }
    }

    pub fn clear_fire(&mut self, p: IVec3) {
        self.fire_lights.remove(&p);
    }

    /// Strike the assembly owning a cell, with torque about its centre of mass.
    /// Direction and strength are world-space metres per second of impulse.
    pub fn strike_body(&mut self, cell: IVec3, direction: Vec3, strength: f32, scale: f32) -> bool {
        if !direction.is_finite() || direction.length_squared() < 1e-8
            || !strength.is_finite() || strength <= 0.0 || !scale.is_finite() || scale <= 0.0 {
            return false;
        }
        let Some(index) = self.bodies.iter().position(|b| b.occupied.iter().any(|(p, _)| *p == cell)) else {
            return false;
        };
        let body = &mut self.bodies[index];
        let (mass, center, inverse) = body.mass_properties();
        let arm = cell.as_vec3() - (body.origin.as_vec3() + body.position + body.rotation * center);
        let impulse = direction.normalize() * (strength.min(40.0) / scale);
        let axis = direction.normalize();
        let response = (1.0 / mass + axis.dot(body.inverse_inertia(inverse, arm.cross(axis)).cross(arm))).max(0.0);
        let strength = strength.min(40.0);
        body.velocity = (body.velocity + impulse / mass).clamp_length_max(120.0 / scale);
        body.angular = (body.angular + body.inverse_inertia(inverse, arm.cross(impulse))).clamp_length_max(12.0);
        body.settle_time = 0.0;
        super::rigid::fracture_at(&mut self.bodies, index, cell,
            0.5 * strength * strength * response, strength * response);
        true
    }

    /// Apply a bounded radial impulse to every assembly in range.
    ///
    /// Positions and radii are in metres; body velocities are stored in voxel
    /// units per second, so the caller supplies the current voxel scale. This
    /// is the shared hook used by kinetic weapons and blasts. It deliberately
    /// changes motion and may split weak bonds. No blocks are edited or minted.
    pub fn apply_radial_impulse(
        &mut self,
        centre: Vec3,
        radius: f32,
        strength: f32,
        voxel_scale: f32,
    ) {
        if !centre.is_finite()
            || !radius.is_finite()
            || !strength.is_finite()
            || radius <= 0.0
            || strength <= 0.0
            || !voxel_scale.is_finite()
            || voxel_scale <= 0.0
        {
            return;
        }
        let scale = voxel_scale.max(1e-6);
        let radius_sq = radius * radius;
        let mut fractures = Vec::new();
        for (index, body) in self.bodies.iter_mut().enumerate() {
            if body.occupied.is_empty() {
                continue;
            }
            let (mass, local_center, inverse) = body.mass_properties();
            let center_cells = body.origin.as_vec3() + body.position
                + body.rotation * local_center + Vec3::splat(0.5);
            let mut impulse = Vec3::ZERO;
            let mut torque = Vec3::ZERO;
            let mut strongest: Option<(IVec3, f32, f32)> = None;
            // Integrate pressure over the actual cells. A blast can hit the end
            // of a beam even when its centre of mass lies outside the radius.
            for (p, material) in &body.occupied {
                let point = p.as_vec3() + Vec3::splat(0.5);
                let delta = point * scale - centre;
                let d2 = delta.length_squared();
                if d2 >= radius_sq { continue; }
                let falloff = 1.0 - d2.sqrt() / radius;
                let direction = if d2 > 1e-6 { delta / d2.sqrt() } else { Vec3::Y };
                let push = (direction + Vec3::Y * 0.28).normalize_or_zero()
                    * (strength * falloff / scale);
                impulse += push;
                torque += (point - center_cells).cross(push);
                let cell_mass = (material.material().physics.density / 1000.0).max(0.01);
                let speed = push.length() * scale / cell_mass;
                let energy = 0.5 * cell_mass * speed * speed;
                if strongest.is_none_or(|(_, e, _)| energy > e) { strongest = Some((*p, energy, speed)); }
            }
            body.velocity = (body.velocity + impulse / mass).clamp_length_max(120.0 / scale);
            body.angular = (body.angular + body.inverse_inertia(inverse, torque)).clamp_length_max(12.0);
            if impulse != Vec3::ZERO { body.settle_time = 0.0; }
            if let Some((cell, energy, speed)) = strongest { fractures.push((index, cell, energy, speed)); }
        }
        for (index, cell, energy, speed) in fractures {
            super::rigid::fracture_at(&mut self.bodies, index, cell, energy, speed);
        }
    }
    pub fn encode(&self, out: &mut Vec<u8>) {
        self.encode_cells(out);
        self.encode_bodies(out, |_| true);
    }

    #[cfg(feature = "multiplayer")]
    pub(crate) fn encode_thermal_image(&self, out: &mut Vec<u8>) {
        self.encode_cells(out);
        out.extend_from_slice(&0_u32.to_le_bytes());
    }

    #[cfg(feature = "multiplayer")]
    pub(crate) fn encode_rigid_image(&self, out: &mut Vec<u8>, listener: Vec3, reach: f32) {
        out.extend_from_slice(&0_u32.to_le_bytes());
        let r2 = reach * reach;
        self.encode_bodies(out, |body| {
            body.origin.as_vec3().distance_squared(listener) <= r2
                || body.occupied.iter().any(|(p, _)| p.as_vec3().distance_squared(listener) <= r2)
        });
    }

    fn encode_cells(&self, out: &mut Vec<u8>) {
        fn push(o: &mut Vec<u8>, v: u32) {
            o.extend_from_slice(&v.to_le_bytes());
        }
        fn pos(o: &mut Vec<u8>, p: IVec3) {
            for v in p.to_array() {
                push(o, v as u32);
            }
        }
        push(out, self.cells.len() as u32);
        let mut cells: Vec<_> = self.cells.iter().collect();
        cells.sort_unstable_by_key(|entry| *entry.0);
        for (&(x, y, z), c) in cells {
            pos(out, IVec3::new(x, y, z));
            for v in [c.temperature, c.fuel] {
                push(out, v.to_bits());
            }
            push(out, c.gas as u32);
        }
    }

    fn encode_bodies(&self, out: &mut Vec<u8>, include: impl Fn(&RigidBody) -> bool) {
        fn push(o: &mut Vec<u8>, v: u32) { o.extend_from_slice(&v.to_le_bytes()); }
        fn pos(o: &mut Vec<u8>, p: IVec3) { for v in p.to_array() { push(o, v as u32); } }
        let selected: Vec<_> = self.bodies.iter().filter(|b| include(b)).collect();
        push(out, selected.len() as u32);
        for b in selected {
            pos(out, b.origin);
            for v in b
                .position
                .to_array()
                .into_iter()
                .chain(b.velocity.to_array())
                .chain(b.rotation.to_array())
                .chain(b.angular.to_array())
                .chain([b.debt])
            {
                push(out, v.to_bits());
            }
            for (index, list) in [&b.voxels, &b.occupied].into_iter().enumerate() {
                // Tag the first list count without adding words to the save.
                // Untagged older bodies retain their legacy all-log meaning.
                let flags = if index == 0 { 0x8000_0000 | ((b.felled_tree as u32) << 30) } else { 0 };
                push(out, list.len() as u32 | flags);
                for &(p, id) in list {
                    pos(out, p);
                    push(out, id.as_u32());
                }
            }
        }
    }
    pub fn decode(next: &mut impl FnMut() -> Result<u32>) -> Result<Self> {
        fn pos(n: &mut impl FnMut() -> Result<u32>) -> Result<IVec3> {
            let p = IVec3::new(n()? as i32, n()? as i32, n()? as i32);
            ensure!(
                p.as_i64vec3().abs().max_element() <= 2_000_000,
                "Physics coordinate out of range"
            );
            Ok(p)
        }
        fn float(n: &mut impl FnMut() -> Result<u32>) -> Result<f32> {
            let v = f32::from_bits(n()?);
            ensure!(v.is_finite(), "Invalid physics value");
            Ok(v)
        }
        fn vec(n: &mut impl FnMut() -> Result<u32>) -> Result<Vec3> {
            Ok(Vec3::new(float(n)?, float(n)?, float(n)?))
        }
        let mut s = Self::default();
        let count = next()? as usize;
        ensure!(count <= MAX_CELLS, "Thermal state too large");
        for _ in 0..count {
            let p = pos(next)?;
            let c = ThermalCell {
                temperature: float(next)?,
                fuel: float(next)?,
                gas: next()?.try_into()?,
            };
            ensure!(
                (-273.0..=5000.0).contains(&c.temperature)
                    && (0.0..=10000.0).contains(&c.fuel)
                    && c.gas <= 8,
                "Invalid thermal cell"
            );
            ensure!(
                s.cells.insert(key(p), c).is_none(),
                "Duplicate thermal cell"
            );
        }
        let count = next()? as usize;
        ensure!(count <= MAX_BODIES, "Too many rigid bodies");
        let mut claimed = crate::fast_hash::FastSet::default();
        for _ in 0..count {
            let origin = pos(next)?;
            let position = vec(next)?;
            let velocity = vec(next)?;
            let rotation = Quat::from_xyzw(float(next)?, float(next)?, float(next)?, float(next)?);
            let angular = vec(next)?;
            let debt = float(next)?;
            ensure!((0.0..=0.25).contains(&debt), "Invalid rigid clock");
            ensure!(
                (0.9..=1.1).contains(&rotation.length())
                    && position.abs().max_element() < 4096.0
                    && velocity.length() < 10000.0
                    && angular.length() < 100.0,
                "Invalid rigid transform"
            );
            let rotation = rotation.normalize();
            let lattice = VoxelRotation::new(rotation);
            let base = origin + position.round().as_ivec3();
            let mut lists = Vec::new();
            let mut tree_tag = None;
            for index in 0..2 {
                let raw_count = next()?;
                if index == 0 && raw_count & 0x8000_0000 != 0 {
                    tree_tag = Some(raw_count & 0x4000_0000 != 0);
                }
                let n = if index == 0 && tree_tag.is_some() { raw_count & 0x3fff_ffff } else { raw_count } as usize;
                ensure!(n > 0 && n <= MAX_TREE_BODY_VOXELS, "Invalid rigid voxel count");
                let mut list = Vec::with_capacity(n);
                let mut seen = crate::fast_hash::FastSet::default();
                for _ in 0..n {
                    let p = pos(next)?;
                    let raw = next()?;
                    ensure!((raw as usize) < BlockId::COUNT, "Invalid rigid material");
                    ensure!(seen.insert(p), "Duplicate rigid voxel");
                    list.push((p, BlockId::from_u32(raw)));
                }
                lists.push(list);
            }
            let occupied = lists.pop().unwrap();
            let voxels = lists.pop().unwrap();
            ensure!(occupied.len() == voxels.len(), "Rigid mass mismatch");
            let all_logs = voxels.iter().all(|(_, b)| *b == BlockId::Log);
            let tree = tree_tag.unwrap_or(all_logs);
            let tree_material = |b| matches!(b, BlockId::Log | BlockId::Leaves | BlockId::PineLeaves);
            ensure!(!tree || voxels.iter().all(|(_, b)| tree_material(*b)), "Invalid felled tree material");
            ensure!(tree || all_logs || voxels.len() <= MAX_BODY_VOXELS, "Oversized rigid assembly");
            ensure!(
                voxels.iter().zip(&occupied).all(|((p, b), (q, id))| b == id
                    && ((tree && tree_material(*b)) || matches!(b, BlockId::RigidCore | BlockId::Log | BlockId::Planks) || b.is_solid())
                    && p.as_i64vec3().abs().max_element() <= if tree || all_logs { MAX_TREE_BODY_VOXELS as i64 } else { MAX_BODY_VOXELS as i64 }
                    && (base + lattice.apply(*p) == *q || origin
                        + (position + rotation * p.as_vec3())
                            .round()
                            .as_ivec3()
                        == *q)),
                "Rigid geometry disagrees with transform"
            );
            ensure!(
                occupied.iter().all(|(p, _)| claimed.insert(*p)),
                "Overlapping rigid bodies"
            );
            s.bodies.push(RigidBody {
                felled_tree: tree,
                origin,
                position,
                velocity,
                rotation,
                angular,
                debt,
                settle_time: 0.0,
                was_submerged: false,
                voxels,
                occupied,
            });
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(center: IVec3) -> RigidBody {
        RigidBody {
            felled_tree: false,
            origin: center,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angular: Vec3::ZERO,
            debt: 0.0,
            settle_time: 0.0,
            was_submerged: false,
            voxels: vec![(IVec3::ZERO, BlockId::Planks)],
            occupied: vec![(center, BlockId::Planks)],
        }
    }

    #[test]
    fn fire_light_index_deduplicates_and_clears() {
        let mut sandbox = Sandbox::default();
        let p = IVec3::new(4, 5, 6);
        sandbox.note_fire(p);
        sandbox.note_fire(p);
        assert_eq!(sandbox.fire_lights.len(), 1);
        sandbox.clear_fire(p);
        assert!(sandbox.fire_lights.is_empty());
    }

    #[test]
    fn radial_impulse_moves_near_bodies_and_ignores_far_ones() {
        let mut sandbox = Sandbox::default();
        sandbox.bodies.push(body(IVec3::new(1, 1, 0)));
        sandbox.bodies.push(body(IVec3::new(20, 1, 0)));
        sandbox.apply_radial_impulse(Vec3::ZERO, 4.0, 6.0, 1.0);
        assert!(sandbox.bodies[0].velocity.length() > 0.0);
        assert!(sandbox.bodies[0].velocity.y > 0.0);
        assert_eq!(sandbox.bodies[1].velocity, Vec3::ZERO);
    }
}
