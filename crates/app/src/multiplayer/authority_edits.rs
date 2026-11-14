//! Retain admitted player intent while cold sources and capacity are pending.
use super::*;
use std::collections::VecDeque;

#[derive(Default)]
pub(crate) struct Pending(Option<Stroke>);

struct Stroke {
    cfg: world::WorldSampleConfig,
    save: String,
    cells: VecDeque<IVec3>,
    mode: creative::BrushMode,
    selected: Option<BlockId>,
    slot: usize,
    held: Option<Item>,
    creative: bool,
    tool: Option<world::mining::Tool>,
    forward: Vec3,
}

impl Pending {
    pub(super) fn queue(&mut self, w: &World, mode: creative::BrushMode,
        cells: Vec<IVec3>, selected: Option<BlockId>, slot: usize, creative: bool,
        tool: Option<world::mining::Tool>, forward: Vec3) {
        if self.0.is_some() { return; }
        self.0 = Some(Stroke {
            cfg: w.sample_config(), save: w.regions.view.session_name().to_owned(),
            cells: cells.into_iter().take(4096).collect(),
            mode, selected, slot, held: w.inventory.hotbar[slot].map(|s| s.item),
            creative, tool, forward,
        });
    }

    pub(super) fn busy(&self) -> bool { self.0.is_some() }

    pub(super) fn drain(&mut self, w: &mut World, p: &mut Player,
        sim: &mut crate::simulation::WorldSimulation, creative: bool,
        history: &mut EditHistory, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
        let Some(stroke) = self.0.as_mut() else { return; };
        // A delayed action cannot spend a replacement stack or cross a realm.
        if stroke.cfg != w.sample_config() || stroke.save != w.regions.view.session_name()
            || stroke.creative != creative
            || !w.rules.mode.edits_blocks() || w.adventure.is_dead()
            || w.inventory.hotbar[stroke.slot].map(|s| s.item) != stroke.held {
            self.0 = None;
            history.seal();
            return;
        }
        let previous_slot = w.adventure.hand_slot;
        w.adventure.hand_slot = stroke.slot;
        for _ in 0..48 {
            let Some(&cell) = stroke.cells.front() else { break; };
            if !sim.prepare_tool_edit(w, cell) {
                if sim.tool_edit_waiting() { break; }
                stroke.cells.pop_front();
                continue;
            }
            stroke.cells.pop_front();
            let scale = w.voxel_scale.meters();
            match stroke.mode {
                creative::BrushMode::Erase => {
                    let old = w.get(cell);
                    let changed = if stroke.creative {
                        if let Some(changed) = world::farming::cut_stalk(w, cell, false, blocks, chunks, Some(history)) { changed }
                        else if let Ok(commit) = world::effects::replace(w, cell, old, BlockId::Air) {
                            commit.publish(w, blocks, chunks);
                            history.record(w, cell, old, BlockId::Air);
                            true
                        } else { false }
                    } else {
                        let tool = stroke.tool.map(|_| world::mining::tool_for_world(w, stroke.held));
                        world::mining::harvest(w, cell, tool, blocks, chunks, None, stroke.forward)
                    };
                    if changed { audio_events::block(w, cell, old, crate::audio::Contact::Break); }
                }
                creative::BrushMode::Place => {
                    let Some(block) = stroke.selected else { break; };
                    let size = if world::factory::Kind::from_block(block).is_some() { 1.0 } else { scale };
                    let a = cell.as_vec3() * scale;
                    let b = a + Vec3::splat(size);
                    if w.multiplayer.as_ref().is_some_and(|c| c.players.iter().any(|t| t.min.cmplt(b).all() && t.max.cmpgt(a).all())) { continue; }
                    if world::fittings::place(w, cell, block, p, (!stroke.creative).then_some(stroke.slot), blocks, chunks) {
                        audio_events::block(w, cell, block, crate::audio::Contact::Place);
                        continue;
                    }
                    if world::factory::place(w, cell, block, p, stroke.held, blocks, chunks) {
                        audio_events::block(w, cell, block, crate::audio::Contact::Place);
                        continue;
                    }
                    let old = w.get(cell);
                    if !old.is_empty() && !old.is_liquid() { continue; }
                    if world::effects::place(w, cell, old, block, (!stroke.creative).then_some(stroke.slot), blocks, chunks).is_ok() {
                        if stroke.creative { history.record(w, cell, old, block); }
                        audio_events::block(w, cell, block, crate::audio::Contact::Place);
                    }
                }
            }
            w.environment.player_edit = false;
        }
        w.adventure.hand_slot = previous_slot;
        w.environment.player_edit = false;
        if stroke.cells.is_empty() {
            self.0 = None;
            history.seal();
        }
        w.sync_from_inventory();
    }
}

pub(crate) fn review() -> Result<()> {
    use world::inventory::ItemStack;
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let cell = IVec3::new(-700_000, 200, -32);
    w.set(cell, BlockId::Dirt);
    w.set(cell, BlockId::Air);
    w.activity = Default::default();
    let slot = 8;
    let dirt = Item::Block(BlockId::Dirt);
    w.inventory.hotbar[slot] = Some(ItemStack::new(dirt, 2));
    let mut p = Player::new();
    p.pos = cell.as_vec3() + Vec3::Z * 4.0;
    let mut sim = crate::simulation::WorldSimulation::new();
    let mut history = EditHistory::new(world::edit_history::REMOTE_CELLS);
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let mut pending = Pending::default();
    pending.queue(&w, creative::BrushMode::Place, vec![cell], Some(BlockId::Dirt), slot, false, None, Vec3::NEG_Z);
    for i in 0..world::activity::MAX_ACTIVE_CHUNKS {
        ensure!(w.activity.wake(IVec3::new(i as i32 * 32, 300, 10_000)), "fixture capacity");
    }
    pending.drain(&mut w, &mut p, &mut sim, false, &mut history, &mut blocks, &mut chunks);
    ensure!(pending.busy() && w.get(cell).is_empty(), "queued placement consumed under pressure");
    ensure!(w.inventory.hotbar[slot].is_some_and(|s| s.count == 2), "supply consumed before admission");
    w.activity = Default::default();
    pending.drain(&mut w, &mut p, &mut sim, false, &mut history, &mut blocks, &mut chunks);
    ensure!(!pending.busy() && w.get(cell) == BlockId::Dirt, "queued placement did not resume");
    pending.drain(&mut w, &mut p, &mut sim, false, &mut history, &mut blocks, &mut chunks);
    ensure!(w.inventory.hotbar[slot].is_some_and(|s| s.count == 1), "supply consumed more than once");
    ensure!(!w.environment.player_edit, "foreground reservation leaked");

    w.set(cell, BlockId::Air);
    pending.queue(&w, creative::BrushMode::Place, vec![cell], Some(BlockId::Dirt), slot, false, None, Vec3::NEG_Z);
    w.inventory.hotbar[slot] = Some(ItemStack::new(Item::Block(BlockId::Stone), 2));
    pending.drain(&mut w, &mut p, &mut sim, false, &mut history, &mut blocks, &mut chunks);
    ensure!(!pending.busy() && w.get(cell).is_empty(), "stale placement spent replacement supply");
    ensure!(w.inventory.hotbar[slot].is_some_and(|s| s.count == 2), "replacement supply changed");

    pending.queue(&w, creative::BrushMode::Place, vec![cell], Some(BlockId::Stone), slot, false, None, Vec3::NEG_Z);
    w.regions = world::regions::RegionStream::new();
    pending.drain(&mut w, &mut p, &mut sim, false, &mut history, &mut blocks, &mut chunks);
    ensure!(!pending.busy() && w.get(cell).is_empty(), "queued edit crossed save identity");
    println!("[edit-admission] pending-placement retained=true supply_once=true replacement_cancelled=true realm_cancelled=true");
    Ok(())
}

#[test]
fn queued_placement_retains_supply_and_cancels_stale_intent() { review().unwrap(); }
