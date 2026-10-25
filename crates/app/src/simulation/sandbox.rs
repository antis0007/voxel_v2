//! Persistent sparse thermodynamics and voxel rigid assemblies.
use crate::world::BlockId;
use anyhow::{Result, ensure};
use glam::{IVec3, Quat, Vec3};
use std::collections::HashSet;
pub const MAX_CELLS: usize = 32768;
pub const MAX_BODIES: usize = 64;
pub const MAX_BODY_VOXELS: usize = 128;
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
    pub origin: IVec3,
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub angular: Vec3,
    pub debt: f32,
    /// Felled-tree bodies hold logs while they fall. This is runtime-only
    /// presentation state; it is not serialized and restarts at zero on load.
    pub settle_time: f32,
    /// Runtime-only water-contact edge for splash emission.
    pub was_submerged: bool,
    pub voxels: Vec<(IVec3, BlockId)>,
    pub occupied: Vec<(IVec3, BlockId)>,
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
        self.cells.len()
            + positions
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    !positions[..*i].contains(p) && !self.cells.contains_key(&key(**p))
                })
                .count()
            <= MAX_CELLS
    }

    pub fn note_fire(&mut self, p: IVec3) {
        if self.fire_lights.contains(&p) || self.fire_lights.len() < 64 {
            self.fire_lights.insert(p);
        }
    }

    pub fn clear_fire(&mut self, p: IVec3) {
        self.fire_lights.remove(&p);
    }

    /// Apply a bounded radial impulse to every assembly in range.
    ///
    /// Positions and radii are in metres; body velocities are stored in voxel
    /// units per second, so the caller supplies the current voxel scale. This
    /// is the shared hook used by kinetic weapons and blasts. It deliberately
    /// touches only kinematic state: no blocks are edited, so a pulse can
    /// never duplicate or delete an assembly.
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
        {
            return;
        }
        let scale = voxel_scale.max(1e-6);
        let radius_sq = radius * radius;
        for body in &mut self.bodies {
            if body.occupied.is_empty() {
                continue;
            }
            let centre_cells = body
                .occupied
                .iter()
                .map(|(p, _)| p.as_vec3() + Vec3::splat(0.5))
                .sum::<Vec3>()
                / body.occupied.len() as f32;
            let body_centre = centre_cells * scale;
            let delta = body_centre - centre;
            let distance_sq = delta.length_squared();
            if distance_sq >= radius_sq {
                continue;
            }
            let falloff = 1.0 - (distance_sq / radius_sq).sqrt();
            let direction = if delta.length_squared() > 1e-6 {
                delta.normalize()
            } else {
                Vec3::Y
            };
            // A little upward bias makes ground-level pulses read as impacts
            // rather than sliding assemblies sideways through the floor.
            let impulse =
                (direction + Vec3::Y * 0.28).normalize_or_zero() * (strength * falloff / scale);
            body.velocity += impulse;
            body.angular = (body.angular
                + Vec3::Y.cross(direction) * (strength * 0.28 * falloff / scale))
                .clamp_length_max(3.0);
        }
    }
    pub fn encode(&self, out: &mut Vec<u8>) {
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
        push(out, self.bodies.len() as u32);
        for b in &self.bodies {
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
            for list in [&b.voxels, &b.occupied] {
                push(out, list.len() as u32);
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
            let mut lists = Vec::new();
            for _ in 0..2 {
                let n = next()? as usize;
                ensure!(n > 0 && n <= MAX_BODY_VOXELS, "Invalid rigid voxel count");
                let mut list = Vec::new();
                for _ in 0..n {
                    let p = pos(next)?;
                    let raw = next()?;
                    ensure!((raw as usize) < BlockId::COUNT, "Invalid rigid material");
                    ensure!(!list.iter().any(|(q, _)| *q == p), "Duplicate rigid voxel");
                    list.push((p, BlockId::from_u32(raw)));
                }
                lists.push(list);
            }
            let occupied = lists.pop().unwrap();
            let voxels = lists.pop().unwrap();
            ensure!(occupied.len() == voxels.len(), "Rigid mass mismatch");
            ensure!(
                voxels.iter().zip(&occupied).all(|((p, b), (q, id))| b == id
                    && matches!(b, BlockId::RigidCore | BlockId::Log | BlockId::Planks)
                    && p.as_i64vec3().abs().max_element() <= MAX_BODY_VOXELS as i64
                    && origin
                        + (position + rotation.normalize() * p.as_vec3())
                            .round()
                            .as_ivec3()
                        == *q),
                "Rigid geometry disagrees with transform"
            );
            ensure!(
                s.bodies.iter().all(|body| body
                    .occupied
                    .iter()
                    .all(|(p, _)| !occupied.iter().any(|(q, _)| p == q))),
                "Overlapping rigid bodies"
            );
            s.bodies.push(RigidBody {
                origin,
                position,
                velocity,
                rotation: rotation.normalize(),
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
