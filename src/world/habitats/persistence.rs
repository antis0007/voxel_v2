use super::*;
use anyhow::{Result, ensure};
use std::collections::HashSet;

impl State {
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        self.encode_versioned(bytes, 27);
    }
    pub fn encode_versioned(&self, bytes: &mut Vec<u8>, version: u8) {
        let mut push = |v: u32| bytes.extend_from_slice(&v.to_le_bytes());
        for v in [
            self.seeded as u32,
            self.next_id,
            self.clock.to_bits() as u32,
            (self.clock.to_bits() >> 32) as u32,
            self.xp as u32,
            (self.xp >> 32) as u32,
            self.lantern as u32,
            self.xp_spent,
            self.mastered,
            self.sources.len() as u32,
        ] {
            push(v);
        }
        for s in &self.sources {
            push(s.id);
            if version >= 27 {
                push(s.parent);
            }
            for v in [s.family as u32, s.pos.is_some() as u32] {
                push(v);
            }
            for v in s.pos.unwrap_or(Vec3::ZERO).to_array() {
                push(v.to_bits());
            }
            push(s.resident.is_some() as u32);
            for v in s.resident.as_ref().map_or(Vec3::ZERO, |r| r.pos).to_array() {
                push(v.to_bits());
            }
            push(s.resident.as_ref().map_or(0.0, |r| r.health).to_bits());
            let (pods, spores, work) = s
                .resident
                .as_ref()
                .map_or((0, 0, 0.0), |r| (r.pods, r.spores, r.work));
            if version >= 27 {
                for v in [
                    pods,
                    spores,
                    work.to_bits() as u32,
                    (work.to_bits() >> 32) as u32,
                ] {
                    push(v);
                }
            }
            for v in [
                s.feed,
                s.charge,
                s.development,
                s.product_clock,
                s.products,
                s.mana,
                s.xp,
                s.receiver as u32,
                s.automatic as u32,
                s.emitter as u32,
                s.reserve as u32,
                s.emit_ticks,
                s.births,
                s.harvested,
                s.feed_used,
                s.mana_made,
                s.mana_used,
                s.xp_made,
                s.deaths,
            ] {
                push(v);
            }
        }
        push(self.releases.len() as u32);
        for r in &self.releases {
            for v in r.pos.to_array() {
                push(v.to_bits());
            }
            push(r.mana);
            push(r.xp);
        }
        if version >= 27 {
            push(self.shots.len() as u32);
            for shot in &self.shots {
                push(shot.owner);
                for v in shot
                    .pos
                    .to_array()
                    .into_iter()
                    .chain(shot.velocity.to_array())
                {
                    push(v.to_bits());
                }
                push(shot.life.to_bits());
            }
        }
    }

    pub fn decode(read: &mut impl FnMut() -> Result<u32>, version: u8) -> Result<Self> {
        fn flag(read: &mut impl FnMut() -> Result<u32>) -> Result<bool> {
            let n = read()?;
            ensure!(n <= 1, "Invalid habitat flag");
            Ok(n == 1)
        }
        fn position(read: &mut impl FnMut() -> Result<u32>) -> Result<Vec3> {
            let p = Vec3::new(
                f32::from_bits(read()?),
                f32::from_bits(read()?),
                f32::from_bits(read()?),
            );
            ensure!(
                p.is_finite() && p.abs().max_element() <= 1_000_000.0,
                "Invalid habitat position"
            );
            Ok(p)
        }
        let seeded = flag(read)?;
        let next_id = read()?;
        let clock = f64::from_bits(read()? as u64 | ((read()? as u64) << 32));
        ensure!(
            clock.is_finite() && (0.0..STEP).contains(&clock),
            "Invalid habitat clock"
        );
        let xp = read()? as u64 | ((read()? as u64) << 32);
        let lantern = flag(read)?;
        let xp_spent = read()?;
        let mastered = read()?;
        ensure!(mastered & !7 == 0, "Invalid habitat mastery");
        let count = read()? as usize;
        ensure!(count <= MAX_SOURCES, "Too many habitat sources");
        let mut sources = Vec::with_capacity(count);
        let mut ids = HashSet::new();
        let mut carried = 0;
        for _ in 0..count {
            let id = read()?;
            let parent = if version >= 27 { read()? } else { 0 };
            ensure!(
                id > 0 && id < next_id && ids.insert(id),
                "Invalid habitat identity"
            );
            let family = match read()? {
                0 => Family::Nursery,
                1 => Family::Spider,
                2 => Family::Matron,
                _ => anyhow::bail!("Invalid habitat family"),
            };
            let placed = flag(read)?;
            let p = position(read)?;
            let living = flag(read)?;
            let resident_pos = position(read)?;
            let health = f32::from_bits(read()?);
            let pods = if version >= 27 { read()? } else { 0 };
            let spores = if version >= 27 { read()? } else { 0 };
            let work = if version >= 27 {
                f64::from_bits(read()? as u64 | ((read()? as u64) << 32))
            } else {
                0.0
            };
            ensure!(
                pods <= 2 && spores <= 12 && work.is_finite() && (0.0..12.1).contains(&work),
                "Invalid Matron carried stock"
            );
            ensure!(
                health.is_finite()
                    && health >= 0.0
                    && health <= adventure::habitat_health(family.kind()),
                "Invalid habitat health"
            );
            ensure!(
                !living || (placed && health > 0.0),
                "Invalid carried resident"
            );
            if !placed {
                carried += 1;
            }
            let mut s = Source::new(id, family, placed.then_some(p));
            ensure!(parent == 0 || parent < id, "Invalid brood parent");
            s.parent = parent;
            s.resident = living.then_some(Resident {
                pos: resident_pos,
                health,
                pods,
                spores,
                work,
            });
            s.feed = read()?;
            s.charge = read()?;
            s.development = read()?;
            s.product_clock = read()?;
            s.products = read()?;
            s.mana = read()?;
            s.xp = read()?;
            s.receiver = flag(read)?;
            s.automatic = flag(read)?;
            s.emitter = flag(read)?;
            s.reserve = flag(read)?;
            s.emit_ticks = read()?;
            s.births = read()?;
            s.harvested = read()?;
            s.feed_used = read()?;
            s.mana_made = read()?;
            s.mana_used = read()?;
            s.xp_made = read()?;
            s.deaths = read()?;
            ensure!(
                [s.feed, s.charge, s.products, s.mana, s.xp, s.harvested]
                    .iter()
                    .all(|n| *n <= CAP)
                    && s.development < family.growth()
                    && s.product_clock < 80
                    && (!s.automatic || s.receiver)
                    && (!s.emitter || s.receiver)
                    && s.emit_ticks <= 64,
                "Invalid habitat inventory"
            );
            sources.push(s);
        }
        ensure!(carried <= 1, "Multiple carried habitat sources");
        let count = read()? as usize;
        ensure!(count <= MAX_RELEASES, "Too many loose essence packets");
        let mut releases = Vec::with_capacity(count);
        for _ in 0..count {
            let pos = position(read)?;
            let mana = read()?;
            let xp = read()?;
            ensure!(mana > 0 || xp > 0, "Empty essence packet");
            releases.push(Release { pos, mana, xp });
        }
        let mut shots = Vec::new();
        if version >= 27 {
            let count = read()?;
            ensure!(count <= 96, "Too many farm projectiles");
            for _ in 0..count {
                let owner = read()?;
                ensure!(
                    owner > 0 && owner < next_id,
                    "Invalid farm projectile owner"
                );
                let pos = position(read)?;
                let velocity = position(read)?;
                let life = f32::from_bits(read()?);
                ensure!(
                    velocity.length() <= 100.0 && life.is_finite() && life > 0.0 && life <= 3.0,
                    "Invalid farm projectile"
                );
                shots.push(FarmShot {
                    owner,
                    pos,
                    velocity,
                    life,
                });
            }
        }
        Ok(Self {
            shots,
            seeded,
            next_id,
            clock,
            sources,
            releases,
            xp,
            lantern,
            xp_spent,
            mastered,
        })
    }
}
