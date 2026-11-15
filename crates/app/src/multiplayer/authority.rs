//! Reuse the game's survival rules; clients supply buttons and selections only.
use super::*;
use crate::{creative,world::{edit_history::EditHistory,inventory::Item}};

/// Cells undo or redo writes per tick for one player: 61k cells a second, the
/// rate of a 4096-cell brush every 67 ms, without a wall clock in the host tick.
const HISTORY_CELLS_PER_TICK: usize = 512;
#[path = "authority_edits.rs"]
mod edits;
pub(super) use edits::{Pending, review as review_pending};

/// Keep the caster mounted while deferred crater cells award drops and fell
/// trees. The host uses the same admission and blast effects as solo play.
pub(super) fn blast_step(w: &mut World, stroke: &mut creative::CreativeState,
    sim: &mut crate::simulation::WorldSimulation, left: &mut usize,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
    if *left == 0 || !w.rules.mode.edits_blocks() { return; }
    if stroke.pending_cells() == 0 {
        let Some((point, radius)) = w.adventure.take_terrain_blast() else { return; };
        world::factory::industry::rupture(w, point, radius);
        let cells = creative::spell_blast_targets(w.voxel_scale.meters(), point, radius);
        stroke.queue_spell_blast(w, cells, point, radius);
    }
    let cfg = w.sample_config();
    for _ in 0..(*left).min(48) {
        let Some(cell) = stroke.next_edit_position(cfg) else { break; };
        *left -= 1;
        if !sim.prepare_tool_edit(w, cell) {
            if sim.tool_edit_waiting() { break; }
            stroke.next_edit(cfg);
            continue;
        }
        stroke.next_edit(cfg);
        stroke.spell_blast_cell(w, cell, blocks, chunks);
        w.environment.player_edit = false;
    }
}

/// `owned`: the body was already moved by its joiner and checked; only actions run here.
pub fn step(w:&mut World,p:&mut Player,c:&Controls,tools:&actions::Tools,creative_mode:bool,owned:bool,
    cooldown:&mut f32,mining:&mut world::mining::Progress,history:&mut EditHistory,dt:f32,_changes:&mut BTreeMap<[i32;3],u32>,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>,sim:&mut crate::simulation::WorldSimulation,pending:&mut Pending) {
    let slot=usize::from(c.slot).min(8);
    p.yaw=c.yaw.rem_euclid(std::f32::consts::TAU);p.pitch=c.pitch.clamp(-1.54,1.54);
    w.adventure.creative=creative_mode;
    p.flying=creative_mode && c.flying;
    p.gear=w.inventory.mobility();
    let mut input=c.input();
    w.adventure.hand_slot=slot;
    if owned {
        let m=w.voxel_scale.meters();
        let up=p.up_dir();
        p.on_ground=world::raycast_block(w,p.pos+up*(0.05/m),-up,0.15/m)
            .is_some_and(|(cell,_)|w.solid(cell));
        p.swimming=w.get((p.pos+up*(0.9/m)).floor().as_ivec3()).is_liquid();
    }
    crate::player::traversal::prepare(w,p,&input,dt);
    let riding = if w.adventure.is_dead() {
        w.travel.riding = None;
        w.travel.cast = None;
        false
    } else {
        world::travel::step(w, p, &input, true, dt)
    };
    if owned && !riding { crate::player::traversal::burn_jet(p,&input,w,dt); }
    if !owned && !riding {
        let before = world::portals::traverse::before(w, p);
        update_player(p,&mut input,w,dt);
        if p.manyfold_crossing == 0 {
            world::portals::traverse::step(w, p, before);
        }
        crate::manyfold::weave::feedback(w,p);
        let harm=p.take_pending_harm();
        if let Some((amount,cause))=harm {w.adventure.environment_damage(amount,cause);}
    }
    world::fittings::step(w, p, blocks, chunks);
    crate::manyfold::undercroft::tick(w,p,dt);
    w.expedition.tick(dt);w.adventure.hand_slot=slot;
    let held=w.inventory.hotbar[slot].map(|s|s.item);
    if let Some(Item::Weapon(tier))=held {let _=w.adventure.select_weapon(u32::from(tier));}
    world::industry::presentation::player_exposure(w,p,dt);
    w.adventure.shield_raised = c.place && !p.traversal.sling;
    let consumed=world::adventure::tick(w,p,dt,c.cast&&w.rules.systems.magic(),c.cycle,c.mine);
    crate::player::viewmodel::tick(p,w,&input,dt);
    world::pickups::tick(w,p,dt);
    for cell in std::mem::take(&mut w.adventure.dirty_voxels) {blocks.insert(cell);world::mark_chunk_and_neighbors_dirty(chunks,cell);}
    if !pending.busy() && let Some(message)=history.step(w,&mut world::blueprint::Sink{prepare:&mut |w:&mut World,cell|sim.prepare_tool_edit(w,cell),blocks,chunks},engine_core::Budget::UNBOUNDED,HISTORY_CELLS_PER_TICK) {w.expedition.say(message);}
    w.environment.player_edit = false;
    *cooldown=(*cooldown-dt).max(0.0);
    if !c.mine || consumed {
        mining.tick(None, false, 0.0, dt);
    }
    pending.drain(w, p, sim, creative_mode, history, blocks, chunks);
    if pending.busy() { return; }
    if !c.interact && !c.mine && !c.place && !c.use_secondary {
        return;
    }
    let eye=p.eye(w.voxel_scale);let forward=p.forward_dir();let scale=w.voxel_scale.meters();
    let hit=world::raycast_block(w,eye,forward,6.0/scale);
    let contextual_use = c.place && !input.sneak && tools.tool == 0
        && world::interaction::secondary_targets_world(w, p);
    if c.interact || contextual_use {
        let effect = world::interaction::activate(w, p, slot, input.sneak, blocks, chunks);
        if let world::interaction::Effect::Block(cell) = effect {
            world::expedition::interact(w,cell,p.pos,blocks,chunks);
        }
        if c.interact || effect != world::interaction::Effect::None { return; }
    }
    if c.use_secondary && tools.tool == 0 && w.rules.mode.interacts() && !w.adventure.is_dead()
        && world::travel::interact(w, eye * scale, forward, slot) != world::travel::Use::None
    {
        return;
    }
    let mut config=tools.clone();
    if !creative_mode {config.tool=0;}
    if let Some(Item::Tool(tier @ (1|2)))=held {config.tool=crate::creative::ToolKind::DestructorWand as u8;config.limit=if tier==1{64}else{256};}
    // Survival hand breaks follow the single-player hold time and tool tier rules.
    let mining_tool=(!creative_mode && config.tool==0 && !p.flying).then(||world::mining::tool_for_world(w,held));
    let mut mine=c.mine && !consumed;
    let timed=mining_tool.is_some() && !c.alt;
    if let Some(tool)=mining_tool.filter(|_|timed) {
        let target=hit.map(|(cell,_)|(cell,w.get(cell)));
        let needed=target.map_or(0.0,|(_,block)|world::mining::survival_break(block,tool).seconds);
        mine=mining.tick(target,mine,needed,dt);
    }
    if !c.place && !mine {return;}
    // The hold timer already paces a timed break; the cooldown paces the rest.
    if *cooldown>0.0 && (c.place || !timed) {return;}
    *cooldown=0.15;
    if c.place && config.tool == 0 && !input.sneak && w.rules.mode.interacts()
        && !w.adventure.is_dead()
        && world::farming::interact(w, hit, eye * scale, forward, slot, blocks, chunks)
    {
        return;
    }
    if c.place && world::water_tools::use_bucket(w,p,slot,input.sneak,blocks,chunks) { return; }
    if c.place && let Some(Item::Food(food))=held {let _=w.eat_food(food,Some(slot));return;}
    let selected=held.and_then(Item::placeable_block).or_else(||creative_mode.then(||BlockId::from_u32(c.block)));
    let block=selected.unwrap_or(p.selected_block);
    p.selected_block=block;
    let state=config.creative(slot,block,w.voxel_scale);
    // The raycast returns the hit cell and the empty cell before it.
    let target=hit.map(|(cell,before)|creative::RaycastTarget{hit:Some(cell),place:Some(before)})
        .unwrap_or(creative::RaycastTarget{hit:None,place:c.alt.then(||(eye+forward*(6.0/scale)).floor().as_ivec3())});
    let Some((mode,cells))=creative::edit_blocks(&state,w,p,target,c.place,mine,c.alt) else {return;};
    pending.queue(w, mode, cells, selected, slot, creative_mode, mining_tool, forward);
    pending.drain(w, p, sim, creative_mode, history, blocks, chunks);
    w.sync_from_inventory();
}
