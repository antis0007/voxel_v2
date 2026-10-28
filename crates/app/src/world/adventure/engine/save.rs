//! Save v38 adds Engine records after the existing boss records. Earlier saves
//! initialize the new mechanism once; no old IDs or inventory encodings change.
use super::*;
#[derive(Clone)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(in crate::world::adventure) struct Saved {
    pos: Vec3,
    home: Vec3,
    health: f32,
    yaw: f32,
    menace: u8,
    state: State,
}
pub(in crate::world::adventure) fn snapshot(e: &Enemy) -> Saved {
    let mut state = e.engine.clone();
    state.run = None;
    Saved {
        pos: e.pos,
        home: e.home,
        health: e.health.max(0.0),
        yaw: e.yaw,
        menace: e.menace,
        state,
    }
}
pub(in crate::world::adventure) fn restore(e: &mut Enemy, s: &Saved) {
    e.pos = s.pos;
    e.home = s.home;
    e.health = s.health;
    e.yaw = s.yaw;
    e.menace = s.menace;
    e.engine = s.state.clone();
    e.engine.run = None;
    e.cooldown = 2.0;
    e.tell = 0.0;
}
fn put(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn vector(b: &mut Vec<u8>, p: Vec3) {
    for f in p.to_array() {
        put(b, f.to_bits());
    }
}
fn module(m: Option<Module>) -> u32 {
    match m {
        None => 0,
        Some(Module::Brace) => 1,
        Some(Module::Cutter) => 2,
        Some(Module::Launcher) => 3,
    }
}
fn read_module(r: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Option<Module>> {
    Ok(match r()? {
        0 => None,
        1 => Some(Module::Brace),
        2 => Some(Module::Cutter),
        3 => Some(Module::Launcher),
        _ => anyhow::bail!("Invalid Engine module"),
    })
}
fn read_vec(r: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Vec3> {
    let p = Vec3::new(
        f32::from_bits(r()?),
        f32::from_bits(r()?),
        f32::from_bits(r()?),
    );
    anyhow::ensure!(
        p.is_finite() && p.abs().max_element() < 1_000_000.0,
        "Invalid Engine position"
    );
    Ok(p)
}
fn float(r: &mut impl FnMut() -> anyhow::Result<u32>, max: f32) -> anyhow::Result<f32> {
    let v = f32::from_bits(r()?);
    anyhow::ensure!(
        v.is_finite() && (0.0..=max).contains(&v),
        "Invalid Engine scalar"
    );
    Ok(v)
}
impl Adventure {
    pub(in crate::world::adventure) fn drain_engine_salvage(&mut self, world: &mut World) {
        self.engines.retain(|site, s| {
            if s.health > 0.0 || self.records.get(site).is_none_or(|m| m & 8 == 0) {
                return true;
            }
            let mut e = Enemy::new(*site, 3, KIND, s.pos, s.menace);
            restore(&mut e, s);
            release_cargo(&mut e, world);
            s.state.cargo = e.engine.cargo;
            !s.state.cargo.is_empty()
        });
    }

    pub(crate) fn encode_engines(&self, b: &mut Vec<u8>) {
        let mut saved = self.engines.clone();
        for e in &self.enemies {
            if e.kind == KIND {
                saved.insert(e.site, snapshot(e));
            }
        }
        let mut entries: Vec<_> = saved.iter().collect();
        entries.sort_by_key(|(p, _)| (p.x, p.y, p.z));
        put(b, entries.len() as u32);
        for (site, s) in entries {
            for v in site.to_array() {
                put(b, v as u32);
            }
            vector(b, s.pos);
            vector(b, s.home);
            put(b, s.health.to_bits());
            put(b, s.yaw.to_bits());
            put(b, s.menace as u32);
            let t = &s.state;
            for v in [
                t.stock,
                t.initialized as u32,
                t.loading_damage.to_bits(),
                t.traction_damage.to_bits(),
                t.jam.to_bits(),
                t.cut_cells,
                t.stolen,
                t.knocked,
                t.secured as u32,
            ] {
                put(b, v);
            }
            put(b, module(t.installed));
            for task in [t.building, t.transfer] {
                put(b, module(task.map(|v| v.0)));
                put(b, task.map_or(0.0, |v| v.1).to_bits());
            }
            put(b, module(t.loose.map(|v| v.0)));
            vector(b, t.loose.map_or(Vec3::ZERO, |v| v.1));
            put(b, t.cargo.len() as u32);
            for c in &t.cargo {
                let (k, p) = c.item.encode_u32();
                put(b, k);
                put(b, p);
                put(b, c.count);
            }
        }
    }
    pub(crate) fn decode_engines(
        &mut self,
        r: &mut impl FnMut() -> anyhow::Result<u32>,
    ) -> anyhow::Result<()> {
        let n = r()?;
        anyhow::ensure!(n as usize <= MAX_RECORDS, "Too many Engines");
        for _ in 0..n {
            let site = IVec3::new(r()? as i32, r()? as i32, r()? as i32);
            let pos = read_vec(r)?;
            let home = read_vec(r)?;
            let health = float(r, 2000.0)?;
            let yaw = f32::from_bits(r()?);
            let menace = r()?;
            anyhow::ensure!(yaw.is_finite() && menace <= 3, "Invalid Engine actor");
            let stock = r()?;
            let initialized = r()?;
            let loading_damage = float(r, DRIVE_HEALTH)?;
            let traction_damage = float(r, DRIVE_HEALTH)?;
            let jam = float(r, 4.0)?;
            let cut_cells = r()?;
            let stolen = r()?;
            let knocked = r()?;
            let secured = r()?;
            anyhow::ensure!(
                stock <= STOCK + 2
                    && initialized <= 1
                    && cut_cells <= CUT_CAPACITY
                    && stolen <= 16
                    && secured <= 1,
                "Invalid Engine resources"
            );
            let installed = read_module(r)?;
            let building = read_module(r)?;
            let build_time = float(r, 5.0)?;
            let transfer = read_module(r)?;
            let progress = float(r, 1.0)?;
            let loose = read_module(r)?;
            let at = read_vec(r)?;
            let count = r()?;
            anyhow::ensure!(count <= 20, "Too much Engine cargo");
            let mut cargo = Vec::new();
            let mut total = 0u32;
            for _ in 0..count {
                let item = Item::decode_u32(r()?, r()?)?;
                let count = r()?;
                anyhow::ensure!(
                    (matches!(item, Item::Block(_) | Item::PowerKit(_)))
                        && count > 0
                        && count <= CUT_CAPACITY,
                    "Invalid Engine cargo"
                );
                total = total.saturating_add(count);
                cargo.push(ItemStack { item, count });
            }
            anyhow::ensure!(total <= cut_cells + 4, "Engine cargo exceeds excavation");
            anyhow::ensure!(
                self.records.get(&site).is_some_and(
                    |m| m & reliquaries::AWAKENED != 0 && (m & 8 == 0 || health == 0.0)
                ),
                "Engine without active encounter"
            );
            let state = State {
                secured: secured != 0,
                stock,
                initialized: initialized != 0,
                loading_damage,
                traction_damage,
                jam,
                cut_cells,
                stolen,
                knocked,
                installed,
                building: building.map(|m| (m, build_time)),
                transfer: transfer.map(|m| (m, progress)),
                loose: loose.map(|m| (m, at)),
                cargo,
                last_health: health,
                ..Default::default()
            };
            anyhow::ensure!(
                self.engines
                    .insert(
                        site,
                        Saved {
                            pos,
                            home,
                            health,
                            yaw,
                            menace: menace as u8,
                            state
                        }
                    )
                    .is_none(),
                "Duplicate Engine"
            );
        }
        Ok(())
    }
}
