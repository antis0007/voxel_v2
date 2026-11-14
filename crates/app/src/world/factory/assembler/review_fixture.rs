//! Disposable assembler fixtures and physical inventory ledgers.
use crate::{player::Player, world::{self, BlockId as B, World, factory as f,
    inventory::{Item, ItemStack, all_recipes}, scanner::policy}};
use anyhow::{Result, anyhow, ensure};
use glam::{IVec3, Vec3};

pub const NETWORK_BATCHES: u32 = 32;

fn origin(w: &World) -> IVec3 {
    let size = f::step(w);
    let ground = world::terrain_height(&w.sample_config(), 0, 0) + 1;
    f::anchor(IVec3::new(0, ground, 0), size) + IVec3::Y * size
}
pub fn machine(w: &World) -> IVec3 { origin(w) + IVec3::Z * 4 * f::step(w) }
pub fn input(w: &World) -> IVec3 { machine(w) - IVec3::X }
pub fn output(w: &World) -> IVec3 { machine(w) + IVec3::X * f::step(w) }
pub fn network_enabled() -> bool {
    std::env::args().any(|arg| arg == "--multiplayer-review-assembler")
}

pub fn install(w: &mut World, p: &mut Player, batches: u32) -> Result<()> {
    ensure!((1..=32).contains(&batches), "assembler fixture stock limit");
    let size = f::step(w);
    let origin = origin(w);
    let mut edits = Vec::new();
    for x in -6 * size..7 * size { for z in -3 * size..11 * size {
        edits.push((origin + IVec3::new(x, -1, z), B::Stone));
        for y in 0..4 * size { edits.push((origin + IVec3::new(x, y, z), B::Air)); }
    }}
    w.set_many(&edits);
    let at = machine(w);
    f::fixture_place(w, at, f::Kind::Assembler).ok_or_else(|| anyhow!("assembler placement"))?;
    f::fixture_place(w, at - IVec3::Z * size, f::Kind::Cable).ok_or_else(|| anyhow!("cable placement"))?;
    let battery = f::fixture_place(w, at - IVec3::Z * 2 * size, f::Kind::Battery)
        .ok_or_else(|| anyhow!("battery placement"))?;
    battery.upgrade = 1;
    battery.energy = f::power::battery_capacity(battery.upgrade);
    for cell in [input(w), output(w)] {
        w.set(cell, B::Chest);
        w.block_data.set_chest(cell, vec![None; B::Chest.chest_slots()]);
    }
    let source = input(w);
    let mut slots = w.block_data.chest(source);
    slots[0] = Some(ItemStack::new(Item::Resource(6), 2 * batches));
    slots[1] = Some(ItemStack::new(Item::Resource(4), batches));
    w.block_data.set_chest(source, slots);
    let destination = output(w);
    policy::set(w, destination, policy::Policy { assigned: Some(Item::Resource(16)), ..Default::default() });
    let target = all_recipes().iter().position(|r| r.name == "Hand-wound induction coil")
        .ok_or_else(|| anyhow!("coil recipe missing"))? as u32 + 1;
    p.pos = origin.as_vec3() + Vec3::new(4.0, 0.1, 8.0) * size as f32;
    p.precise_pos = p.pos.as_dvec3();
    p.vel = Vec3::ZERO;
    p.flying = false;
    let delta = at.as_vec3() + Vec3::splat(size as f32 * 0.5) - p.eye(w.voxel_scale);
    p.yaw = delta.x.atan2(-delta.z);
    p.pitch = (delta.y / delta.length()).asin();
    f::action(w, p.pos, f::Action::SetTarget(at, target));
    for (face, role) in [(0, 2), (1, 1), (2, 3), (3, 3), (4, 3), (5, 3)] {
        f::action(w, p.pos, f::Action::SetFace(at, face, role));
    }
    ensure!(w.factory.nodes.get(&at).is_some_and(|n| n.target == target), "recipe action refused");
    eprintln!("[assembler-review] installed batches={batches} voxel_m={}", w.voxel_scale.meters());
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Ledger {
    pub source_copper: u32,
    pub source_fiber: u32,
    pub destination_coils: u32,
    pub input_copper: u32,
    pub input_fiber: u32,
    pub output_units: u32,
    pub progress: u32,
    pub target: u32,
    pub mode: u32,
    pub faces: u32,
    pub enabled: bool,
    pub energy: u32,
    pub copper_total: u64,
    pub fiber_total: u64,
}
impl Ledger {
    pub fn capture(w: &World) -> Option<Self> {
        let n = w.factory.nodes.get(&machine(w))?;
        let count = |pos, item| w.block_data.chest(pos).iter().flatten()
            .filter(|s| s.item == item).map(|s| s.count).sum::<u32>();
        let source_copper = count(input(w), Item::Resource(6));
        let source_fiber = count(input(w), Item::Resource(4));
        let destination_coils = count(output(w), Item::Resource(16));
        let coils = u64::from(destination_coils) * 64 + u64::from(n.output[16]);
        Some(Self { source_copper, source_fiber, destination_coils,
            input_copper: n.input[6], input_fiber: n.input[4], output_units: n.output[16],
            progress: n.progress, target: n.target, mode: n.mode, faces: n.faces, enabled: n.enabled, energy: n.energy,
            copper_total: u64::from(source_copper) * 64 + u64::from(n.input[6]) + 2 * coils,
            fiber_total: u64::from(source_fiber) * 64 + u64::from(n.input[4]) + coils })
    }
    pub fn conserved(&self, batches: u32) -> bool {
        self.copper_total == u64::from(batches) * 128 && self.fiber_total == u64::from(batches) * 64
    }
    pub fn batch_complete(&self, completed: u32, stock: u32) -> bool {
        completed <= stock && self.conserved(stock) && self.destination_coils == completed
            && !self.enabled && self.mode == super::SINGLE_BATCH
            && self.input_copper + self.input_fiber + self.output_units + self.progress == 0
            && self.source_copper == 2 * (stock - completed) && self.source_fiber == stock - completed
    }
    pub fn complete(&self, batches: u32) -> bool {
        self.conserved(batches) && self.destination_coils == batches
            && self.input_copper + self.input_fiber + self.output_units + self.progress == 0
            && self.source_copper + self.source_fiber == 0
    }
}

pub fn report(w: &World) {
    if let Some(ledger) = Ledger::capture(w) {
        if let Ok(text) = serde_json::to_string(&ledger) { eprintln!("[assembler-review] ledger={text}"); }
    } else { eprintln!("[assembler-review] fixture unavailable"); }
}

pub fn install_network(w: &mut World, p: &mut Player) -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.iter().any(|a| a == "--multiplayer-ephemeral")
        && args.iter().any(|a| a == "--multiplayer-headless")
        && args.iter().any(|a| a == "--multiplayer-review-bases")
        && !args.iter().any(|a| a.starts_with("--multiplayer-save=") || a.starts_with("--multiplayer-open-world=")),
        "assembler review requires an ephemeral dedicated base-review host");
    install(w, p, NETWORK_BATCHES)?;
    if args.iter().any(|arg| arg == "--multiplayer-review-assembler-batch") {
        let at = machine(w);
        f::action(w, p.pos, f::Action::Toggle(at));
    }
    Ok(())
}
