//! Persistent sparse thermodynamics and voxel rigid assemblies.
use crate::world::BlockId;
use anyhow::{Result, ensure};
use glam::{IVec3, Quat, Vec3};
use std::collections::BTreeMap;
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
    pub voxels: Vec<(IVec3, BlockId)>,
    pub occupied: Vec<(IVec3, BlockId)>,
}
#[derive(Default)]
pub struct Sandbox {
    pub cells: BTreeMap<(i32, i32, i32), ThermalCell>,
    pub bodies: Vec<RigidBody>,
    pub body_cursor: usize,
}
pub fn key(p: IVec3) -> (i32, i32, i32) {
    (p.x, p.y, p.z)
}
impl Sandbox {
    pub fn cell(&self, p: IVec3) -> ThermalCell {
        self.cells.get(&key(p)).copied().unwrap_or_default()
    }
    pub fn set(&mut self, p: IVec3, c: ThermalCell) -> bool {
        let k = key(p);
        if (c.temperature - 20.0).abs() < 0.25 && c.fuel <= 0.0 && c.gas == 0 {
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
        for (&(x, y, z), c) in &self.cells {
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
                voxels,
                occupied,
            });
        }
        Ok(s)
    }
}
