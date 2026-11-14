use super::*;
use crate::planetary::snapshot::{Decoder, Encoder, NODES, Result, require};

fn mesh() -> &'static Mesh {
    static MESH: OnceLock<Mesh> = OnceLock::new();
    MESH.get_or_init(|| Mesh::with_side(RIFTED_SIDE))
}

fn mesh_hash(mesh: &Mesh) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut word = |value: u64| { hash = (hash ^ value).wrapping_mul(0x100000001b3); };
    for dir in &mesh.directions { for x in dir.to_array() { word(x.to_bits()); } }
    for &area in &mesh.area { word(area.to_bits()); }
    for neighbours in &mesh.neighbours {
        word(neighbours.len() as u64);
        for &(i, distance) in neighbours { word(i as u64); word(distance.to_bits()); }
    }
    for &i in mesh.face_vertices() { word(i as u64); }
    hash
}

impl Drift {
    pub(in crate::planetary) fn snapshot_write(&self, e: &mut Encoder) -> Result<()> {
        require(!self.mesh.is_irregular() && self.mesh.directions.len() == NODES
            && self.mesh.face_vertices().len() == 6 * 85 * 85 && self.crust.len() == NODES,
            "snapshot physical mesh mismatch")?;
        require(self.sea_m.to_bits() == SEA.to_bits(), "snapshot precedes final datum adjustment")?;
        e.len(RIFTED_SIDE)?; e.f64(engine_core::cube_sphere::atlas::RADIUS_M)?;
        e.u64(mesh_hash(self.mesh))?;
        e.len(self.crust.len())?;
        for c in &self.crust {
            for v in [c.continental, c.relief_m, c.ocean_age_myr, c.continental_km,
                c.sediment_m, c.strain, c.slip_m, c.carbonate] { e.f64(v)?; }
            require(c.lineage <= 18, "invalid crust lineage")?;
            e.u16(c.lineage)?;
        }
        e.f64(self.sea_m)?;
        e.flag(self.history.is_some())?;
        self.history.as_ref().ok_or("missing history state")?.snapshot_write(e)?;
        e.flag(self.ice.is_some())?;
        self.ice.as_ref().ok_or("missing ice state")?.snapshot_write(e)?;
        e.flag(self.deep.is_some())?;
        self.deep.as_ref().ok_or("missing deep-time state")?.snapshot_write(e)
    }

    pub(in crate::planetary) fn snapshot_read(d: &mut Decoder<'_>) -> Result<Self> {
        require(d.u32()? as usize == RIFTED_SIDE, "snapshot physical mesh mismatch")?;
        require(d.f64()?.to_bits() == engine_core::cube_sphere::atlas::RADIUS_M.to_bits(),
            "snapshot coordinate mismatch")?;
        let hash = d.u64()?;
        let count = d.count(NODES, NODES, 66)?;
        // Mesh::build assigns IDs by fixed face traversal, never hash-map iteration.
        let mesh = mesh();
        require(hash == mesh_hash(mesh), "snapshot topology mismatch")?;
        let crust = d.items(count, 66, |d| {
            let c = Crust { continental: d.f64()?, relief_m: d.f64()?, ocean_age_myr: d.f64()?,
                continental_km: d.f64()?, sediment_m: d.f64()?, strain: d.f64()?,
                slip_m: d.f64()?, carbonate: d.f64()?, lineage: d.u16()? };
            require(c.lineage <= 18, "invalid crust lineage")?;
            Ok(c)
        })?;
        let sea_m = d.f64()?;
        require(sea_m.to_bits() == SEA.to_bits(), "snapshot precedes final datum adjustment")?;
        require(d.flag()?, "missing history state")?;
        let history = Some(history::Field::snapshot_read(d)?);
        require(d.flag()?, "missing ice state")?;
        let ice = Some(ice::Ice::snapshot_read(d)?);
        require(d.flag()?, "missing deep-time state")?;
        let deep = Some(deep_time::Evolution::snapshot_read(d)?);
        Ok(Self { mesh, crust, history, sea_m, ice, deep })
    }
}


#[cfg(test)]
impl Drift {
    pub(in crate::planetary) fn snapshot_fixture(seed: u32) -> Self {
        let crust = (0..NODES).map(|i| {
            let x = (i % 19) as f64 * 0.01;
            Crust { continental: x, relief_m: x - 10.0, ocean_age_myr: x * 30.0,
                continental_km: x * 28.0, sediment_m: x * 0.4, strain: x * 2.0,
                slip_m: -0.0, lineage: (i % 19) as u16, carbonate: x * 0.3 }
        }).collect();
        Self { mesh: mesh(), crust, history: Some(history::Field::snapshot_fixture()), sea_m: SEA,
            ice: Some(ice::Ice::snapshot_fixture()), deep: Some(deep_time::Evolution::snapshot_fixture(seed)) }
    }
    pub(in crate::planetary) fn snapshot_assert_equal(&self, b: &Self) {
        assert_eq!(mesh_hash(self.mesh), mesh_hash(b.mesh));
        assert_eq!(self.sea_m.to_bits(), b.sea_m.to_bits());
        assert_eq!(self.crust.len(), b.crust.len());
        for (a,b) in self.crust.iter().zip(&b.crust) {
            assert_eq!([a.continental,a.relief_m,a.ocean_age_myr,a.continental_km,
                a.sediment_m,a.strain,a.slip_m,a.carbonate].map(f64::to_bits),
                [b.continental,b.relief_m,b.ocean_age_myr,b.continental_km,
                b.sediment_m,b.strain,b.slip_m,b.carbonate].map(f64::to_bits));
            assert_eq!(a.lineage, b.lineage);
        }
        self.history.as_ref().unwrap().snapshot_assert_equal(b.history.as_ref().unwrap());
        self.ice.as_ref().unwrap().snapshot_assert_equal(b.ice.as_ref().unwrap());
        self.deep.as_ref().unwrap().snapshot_assert_equal(b.deep.as_ref().unwrap());
    }
}
