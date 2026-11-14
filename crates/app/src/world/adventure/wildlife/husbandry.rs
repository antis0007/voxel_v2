//! Tended grazers: feeding, shearing, breeding cooldowns, young growth, area caps and death drops.
//! A fed wild animal joins the herd; herd members persist in the farming sidecar.
use super::*;
use crate::world::inventory::{DroppedStack, Item, ItemStack};

const SITE_Z: i32 = 7;
/// Above every wild slot, so herd identities never match a wild record.
const SLOT: u32 = 2;
pub(crate) const MAX_HERD: usize = 256;
pub(crate) const AREA_CAP: usize = 8;
const AREA_METRES: f32 = 24.0;
const ACTIVE_HERD: usize = 24;
const FEED_REACH_METRES: f32 = 4.0;
const MATE_METRES: f32 = 3.0;
const SECOND: u64 = 120;
pub(crate) const READY_TICKS: u64 = 30 * SECOND;
pub(crate) const COOLDOWN_TICKS: u64 = 300 * SECOND;
/// One default in-game day.
pub(crate) const GROW_TICKS: u64 = 480 * SECOND;
/// Fleece takes one day to grow back after shearing.
pub(crate) const FLEECE_TICKS: u64 = GROW_TICKS;
pub(crate) const SHEARS: Item = Item::Tool(11);

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Member {
    pub kind: u32,
    pub home: Vec3,
    pub pos: Vec3,
    pub health: f32,
    pub yaw: f32,
    /// Birth tick while young; `None` once grown.
    pub born: Option<u64>,
    pub cooldown_until: u64,
    pub ready_until: u64,
    /// Tick the fleece is full again (or a layer lays next); at or below the clock it can be sheared.
    pub fleece_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Herd {
    pub members: BTreeMap<u32, Member>,
    next: u32,
}

pub(super) fn member_id(e: &Enemy) -> Option<u32> {
    (passive(e.kind) && e.site.z == SITE_Z && e.slot == SLOT).then_some(e.site.x as u32)
}

impl Member {
    /// 1 at birth, 0 once grown.
    pub fn youth(&self, clock: u64) -> f32 {
        self.born.map_or(0.0, |b| 1.0 - (clock.saturating_sub(b) as f32 / GROW_TICKS as f32).min(1.0))
    }
    fn actor(&self, id: u32, clock: u64) -> Enemy {
        let mut e = Enemy::new(IVec3::new(id as i32, 0, SITE_Z), SLOT, self.kind, self.pos, 0);
        e.home = self.home;
        e.health = self.health;
        e.yaw = self.yaw;
        e.youth = self.youth(clock);
        e
    }
}

impl Herd {
    fn insert(&mut self, m: Member) -> Option<u32> {
        if self.members.len() >= MAX_HERD {
            return None;
        }
        while self.members.contains_key(&self.next) {
            self.next = self.next.wrapping_add(1) % (1 << 24);
        }
        let id = self.next;
        self.members.insert(id, m);
        self.next = self.next.wrapping_add(1) % (1 << 24);
        Some(id)
    }
    /// Members of `kind` within the breeding area around `at`, live or remembered.
    pub fn crowd(&self, kind: u32, at: Vec3) -> usize {
        self.members.values().filter(|m| m.kind == kind && m.pos.distance_squared(at) < AREA_METRES * AREA_METRES).count()
    }
    pub(crate) fn herd_encode_members(&self, actors: &[Enemy], out: &mut Vec<u8>) {
        out.extend_from_slice(&self.next.to_le_bytes());
        out.extend_from_slice(&(self.members.len() as u32).to_le_bytes());
        for (&id, m) in &self.members {
            let live = actors.iter().find(|e| member_id(e) == Some(id));
            let pos = live.map_or(m.pos, |e| e.pos);
            let health = live.map_or(m.health, |e| e.health.max(0.0));
            for word in [id, m.kind] {
                out.extend_from_slice(&word.to_le_bytes());
            }
            for v in m.home.to_array().into_iter().chain(pos.to_array()).chain([health, m.yaw]) {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&m.born.map_or(u64::MAX, |b| b).to_le_bytes());
            out.extend_from_slice(&m.cooldown_until.to_le_bytes());
            out.extend_from_slice(&m.ready_until.to_le_bytes());
        }
        // Appended after the members so files from before shearing still decode.
        out.extend_from_slice(&(self.members.len() as u32).to_le_bytes());
        for m in self.members.values() {
            out.extend_from_slice(&m.fleece_at.to_le_bytes());
        }
    }
    pub(crate) fn decode(r: &mut crate::world::farming::sidecar::Reader, clock: u64) -> anyhow::Result<Self> {
        use anyhow::ensure;
        let mut herd = Herd { next: r.u32()?, ..Herd::default() };
        let count = r.u32()? as usize;
        ensure!(count <= MAX_HERD && herd.next < 1 << 24, "Invalid herd");
        // Every stored id in file order, removed species included, so the
        // fleece section that follows still lines up.
        let mut stored = std::collections::BTreeSet::new();
        for _ in 0..count {
            let (id, kind) = (r.u32()?, r.u32()?);
            let home = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            let pos = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            let (health, yaw) = (r.f32()?, r.f32()?);
            let born = Some(r.u64()?).filter(|&b| b != u64::MAX);
            let (cooldown_until, ready_until) = (r.u64()?, r.u64()?);
            ensure!(id < 1 << 24, "Invalid herd identity");
            ensure!(stored.insert(id), "Duplicate herd identity");
            if bestiary::generated(kind) && !passive(kind) { continue; }
            ensure!(passive(kind), "Invalid herd identity");
            ensure!(home.is_finite() && pos.is_finite() && home.abs().max_element() < 50_000_000.0
                && pos.abs().max_element() < 50_000_000.0 && yaw.is_finite()
                && (0.0..=super::super::enemy_health(kind)).contains(&health)
                && born.is_none_or(|b| b <= clock), "Invalid herd state");
            let m = Member { kind, home, pos, health, yaw, born, cooldown_until, ready_until, fleece_at: 0 };
            herd.members.insert(id, m);
        }
        if r.remaining() > 0 {
            ensure!(r.u32()? as usize == stored.len(), "Invalid fleece section");
            for id in stored {
                let fleece_at = r.u64()?;
                ensure!(fleece_at <= clock.saturating_add(FLEECE_TICKS), "Invalid fleece state");
                if let Some(m) = herd.members.get_mut(&id) { m.fleece_at = fleece_at; }
            }
        }
        Ok(herd)
    }
}

impl State {
    pub(crate) fn herd_encode(&self, actors: &[Enemy], out: &mut Vec<u8>) {
        self.herd.herd_encode_members(actors, out);
    }
}

fn name(kind: u32) -> &'static str {
    bestiary::species(kind).map_or("Animal", |s| s.name)
}

/// Feeds the grazer under the crosshair its RON food. True when the use was taken.
pub(crate) fn feed(world: &mut World, eye_m: Vec3, forward: Vec3, slot: usize) -> bool {
    let Some(item) = world.inventory.hotbar.get(slot).copied().flatten().map(|s| s.item) else { return false };
    let clock = world.farming.clock;
    let a = &mut world.adventure;
    let target = aimed(a, eye_m, forward, |s| s.food == Some(item));
    let Some(i) = target else { return false };
    let kind = a.enemies[i].kind;
    let Some(id) = adopt(a, i) else {
        world.expedition.say(format!("Herd Full: {MAX_HERD}/{MAX_HERD}"));
        return true;
    };
    let Some(m) = a.wildlife.herd.members.get_mut(&id) else { return false };
    let state = if m.born.is_some() {
        "Young".to_string()
    } else if m.cooldown_until > clock {
        format!("Cooldown: {}s", (m.cooldown_until - clock).div_ceil(SECOND))
    } else if m.ready_until > clock {
        "Fed".to_string()
    } else if world.inventory.consume_from_hotbar(slot, item) {
        m.ready_until = clock + READY_TICKS;
        world.sync_from_inventory();
        "Fed".to_string()
    } else {
        return false;
    };
    world.expedition.say(format!("{} · {state}", name(kind)));
    true
}

/// The herd identity of enemy `i`; a wild grazer joins the herd. `None` when the herd is full.
fn adopt(a: &mut Adventure, i: usize) -> Option<u32> {
    if let Some(id) = member_id(&a.enemies[i]) {
        return Some(id);
    }
    let e = &a.enemies[i];
    let m = Member { kind: e.kind, home: e.home, pos: e.pos, health: e.health, yaw: e.yaw,
        born: None, cooldown_until: 0, ready_until: 0, fleece_at: 0 };
    let id = a.wildlife.herd.insert(m)?;
    if let Some(r) = a.wildlife.residents.get_mut(&super::id(&a.enemies[i])) {
        r.health = 0.0;
    }
    let e = &mut a.enemies[i];
    e.site = IVec3::new(id as i32, 0, SITE_Z);
    e.slot = SLOT;
    Some(id)
}

/// The grazer under the crosshair whose species passes `accepts`, nearest first.
fn aimed(a: &Adventure, eye_m: Vec3, forward: Vec3, accepts: impl Fn(&bestiary::Species) -> bool) -> Option<usize> {
    a.enemies.iter().enumerate()
        .filter(|(_, e)| passive(e.kind) && e.health > 0.0 && bestiary::species(e.kind).is_some_and(&accepts))
        .filter_map(|(i, e)| {
            let c = e.center();
            let t = (c - eye_m).dot(forward);
            let radius = bestiary::species(e.kind).map_or(0.6, |s| s.body_width * 0.5 + 0.2);
            ((0.0..=FEED_REACH_METRES).contains(&t) && (eye_m + forward * t).distance(c) <= radius).then_some((t, i))
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, i)| i)
}

/// Shears the grown grazer under the crosshair with held shears. True when the use was taken.
///
/// The yield rolls from the herd identity and the shearing tick, so a reload
/// cannot reroll it; the fleece then regrows over `FLEECE_TICKS`.
pub(crate) fn shear(world: &mut World, eye_m: Vec3, forward: Vec3, slot: usize) -> bool {
    if world.inventory.hotbar.get(slot).copied().flatten().map(|s| s.item) != Some(SHEARS) {
        return false;
    }
    let clock = world.farming.clock;
    let a = &mut world.adventure;
    let Some(i) = aimed(a, eye_m, forward, |s| s.shear.is_some()) else { return false };
    let kind = a.enemies[i].kind;
    if a.enemies[i].youth > 0.0 {
        world.expedition.say(format!("{} · Young", name(kind)));
        return true;
    }
    let Some(id) = adopt(a, i) else {
        world.expedition.say(format!("Herd Full: {MAX_HERD}/{MAX_HERD}"));
        return true;
    };
    let Some(m) = a.wildlife.herd.members.get_mut(&id) else { return false };
    if m.fleece_at > clock {
        let seconds = (m.fleece_at - clock).div_ceil(SECOND);
        world.expedition.say(format!("{} · Fleece: {}m {}s", name(kind), seconds / 60, seconds % 60));
        return true;
    }
    let Some(cut) = bestiary::species(kind).and_then(|s| s.shear) else { return false };
    let roll = engine_core::hash::hash_u32(id.wrapping_mul(0x9e37_79b9) ^ (clock as u32).wrapping_mul(0x85eb_ca6b) ^ (clock >> 32) as u32);
    let count = cut.min + roll % (cut.max.saturating_sub(cut.min) + 1);
    m.fleece_at = clock + FLEECE_TICKS;
    let centre = a.enemies[i].center();
    let (_, left) = world.inventory.try_insert(ItemStack { item: cut.item, count });
    if left > 0 && world.inventory.dropped.len() < 1024 {
        world.inventory.dropped.push(DroppedStack { pos: centre, stack: ItemStack { item: cut.item, count: left } });
    }
    world.sync_from_inventory();
    world.expedition.say(format!("{} · Sheared: {} × {count}", name(kind), cut.item.name()));
    true
}

/// Per-step herd upkeep: mirror live bodies, then once a second grow, prune, spawn and breed.
pub(crate) fn service(world: &mut World, feet: Vec3) {
    let clock = world.farming.clock;
    let a = &mut world.adventure;
    for e in a.enemies.iter_mut() {
        let Some(m) = member_id(e).and_then(|id| a.wildlife.herd.members.get_mut(&id)) else { continue };
        m.pos = e.pos;
        m.yaw = e.yaw;
        m.health = if e.pos.y <= -256.0 { 0.0 } else { e.health.max(0.0) };
        e.youth = m.youth(clock);
    }
    if !clock.is_multiple_of(SECOND) {
        return;
    }
    let herd = &mut a.wildlife.herd;
    herd.members.retain(|_, m| m.health > 0.0);
    for m in herd.members.values_mut() {
        if m.born.is_some_and(|b| clock.saturating_sub(b) >= GROW_TICKS) {
            m.born = None;
        }
    }
    let w: &World = world;
    let present: Vec<u32> = w.adventure.enemies.iter().filter_map(member_id).collect();
    let spawns: Vec<Enemy> = w.adventure.wildlife.herd.members.iter()
        .filter(|(id, m)| !present.contains(id) && m.pos.distance_squared(feet) <= 75.0 * 75.0 && free(w, m.pos, m.kind))
        .take(ACTIVE_HERD.saturating_sub(present.len()))
        .map(|(&id, m)| m.actor(id, clock))
        .collect();
    world.adventure.enemies.extend(spawns);
    lay(world, clock);
    breed(world, clock);
}

/// Grown layers in the herd drop their product, then wait one regrowth.
///
/// The regrowth clock is the fleece timer, so the herd sidecar keeps its
/// layout; no species both sheds a fleece and lays.
fn lay(world: &mut World, clock: u64) {
    let (a, inventory) = (&mut world.adventure, &mut world.inventory);
    for e in a.enemies.iter().filter(|e| e.health > 0.0) {
        let Some(product) = bestiary::species(e.kind).and_then(|s| s.lays) else { continue };
        let Some(id) = member_id(e) else { continue };
        let Some(m) = a.wildlife.herd.members.get_mut(&id) else { continue };
        if m.born.is_some() || m.fleece_at > clock { continue; }
        m.fleece_at = clock + FLEECE_TICKS;
        let roll = engine_core::hash::hash_u32(id.wrapping_mul(0x9e37_79b9) ^ (clock as u32).wrapping_mul(0x85eb_ca6b) ^ (clock >> 32) as u32);
        let count = product.min + roll % (product.max.saturating_sub(product.min) + 1);
        if count > 0 && inventory.dropped.len() < 1024 {
            inventory.dropped.push(DroppedStack { pos: e.pos + Vec3::Y * 0.1, stack: ItemStack::new(product.item, count) });
        }
    }
}

/// Pairs ready adults of one kind; a pair inside the area cap gets a young one and a cooldown.
fn breed(world: &mut World, clock: u64) {
    let a = &world.adventure;
    let ready: Vec<(u32, u32, Vec3)> = a.enemies.iter().filter(|e| e.health > 0.0).filter_map(|e| {
        let id = member_id(e)?;
        let m = a.wildlife.herd.members.get(&id)?;
        (m.born.is_none() && m.ready_until > clock && m.cooldown_until <= clock).then_some((id, e.kind, e.pos))
    }).collect();
    let mut used = Vec::new();
    for (i, &(x, kind, px)) in ready.iter().enumerate() {
        if used.contains(&x) {
            continue;
        }
        let Some(&(y, _, py)) = ready[i + 1..].iter()
            .find(|(y, k, p)| *k == kind && !used.contains(y) && p.distance(px) <= MATE_METRES) else { continue };
        used.extend([x, y]);
        let mid = (px + py) * 0.5;
        let herd = &mut world.adventure.wildlife.herd;
        let crowd = herd.crowd(kind, mid);
        let full = crowd >= AREA_CAP || herd.members.len() >= MAX_HERD;
        for id in [x, y] {
            if let Some(m) = herd.members.get_mut(&id) {
                m.ready_until = 0;
                if !full {
                    m.cooldown_until = clock + COOLDOWN_TICKS;
                }
            }
        }
        if full {
            world.expedition.say(format!("{} · Herd Full: {crowd}/{AREA_CAP}", name(kind)));
            continue;
        }
        let at = if free(world, mid, kind) { mid } else { px };
        let young = Member { kind, home: at, pos: at, health: super::super::enemy_health(kind), yaw: 0.0,
            born: Some(clock), cooldown_until: 0, ready_until: 0, fleece_at: 0 };
        if let Some(id) = world.adventure.wildlife.herd.insert(young.clone()) {
            world.adventure.enemies.push(young.actor(id, clock));
        }
    }
}

fn roll(e: &Enemy, index: usize) -> u32 {
    engine_core::hash::hash_u32((e.site.x as u32).wrapping_mul(0x9e37_79b9) ^ (e.site.y as u32).wrapping_mul(0x85eb_ca6b)
        ^ (e.site.z as u32).wrapping_mul(0xc2b2_ae35) ^ e.slot.wrapping_mul(0x27d4_eb2f)
        ^ e.kind.wrapping_mul(0x1656_67b1) ^ (index as u32).wrapping_mul(0xd3a2_646c))
}

/// Drops a dead grazer's RON loot once; young drop nothing. Forgets a herd member.
pub(in crate::world::adventure) fn fall(state: &mut State, world: &mut World, e: &Enemy) {
    super::vehicle::fell(world, e);
    if let Some(id) = member_id(e) {
        state.herd.members.remove(&id);
    }
    if e.youth > 0.0 {
        return;
    }
    let Some(species) = bestiary::species(e.kind) else { return };
    for (i, d) in species.drops.iter().enumerate() {
        let count = d.min + roll(e, i) % (d.max.saturating_sub(d.min) + 1);
        if count > 0 && world.inventory.dropped.len() < 1024 {
            world.inventory.dropped.push(DroppedStack { pos: e.center(), stack: ItemStack::new(d.item, count) });
        }
    }
}
