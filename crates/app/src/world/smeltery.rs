//! Factory-owned smeltery heat, finite molten cells, and physical casting regions.
use super::{BlockId as B, World, effects, factory::{self,Kind,Node}, inventory::{Item,ItemStack}};
use crate::simulation::{sandbox::ThermalCell,residency::Residency,Budget};
use glam::IVec3;
use std::{collections::HashSet,sync::LazyLock};
pub const SIDES:[IVec3;4]=[IVec3::X,IVec3::Z,IVec3::NEG_X,IVec3::NEG_Z];
pub const INGOTS_PER_BLOCK: u32 = 4;
const UNITS_PER_INGOT: u32 = 64;
#[derive(serde::Deserialize)]
pub struct Metal {pub solid:B,pub liquid:B,pub resource:usize,pub feeds:Vec<usize>}
#[derive(serde::Deserialize)]
pub struct Definition {pub fuel:usize,pub fuel_units:u32,pub fuel_ticks:u32,pub temperature_c:f32,pub heat_per_tick:f32,pub metals:Vec<Metal>}
pub static DEF:LazyLock<Definition>=LazyLock::new(||{
    let d:Definition=ron::from_str(include_str!("../../../../assets/tools/smeltery.ron")).expect("smeltery rows");
    assert!(d.fuel<factory::RESOURCE_COUNT && d.fuel_units>0 && d.fuel_ticks>0 && d.fuel_ticks<=2400);
    for m in &d.metals {assert!(m.resource<factory::RESOURCE_COUNT && m.feeds.iter().all(|&r|r<factory::RESOURCE_COUNT));assert_eq!(m.liquid.material().sim.cools_into,Some(m.solid));}
    d
});
pub fn interactive(b:B)->bool {matches!(b,B::SmelteryController|B::SmelteryDrain|B::SmelteryFaucet|B::CastingTable|B::CastingBasin)}
pub fn metal(b:B)->Option<(B,Option<usize>,f32)> {
    // Reversible water/ice phase changes do not make water a molten metal.
    // Otherwise thermal::active claims every river cell and bypasses fluid flow.
    if b==B::Water {return None;}
    let sim=b.material().sim;
    if b.material().physics.matter_state!=engine_content::MatterState::Liquid {return None;}
    let solid=sim.cools_into?;
    if solid.material().sim.melts_into!=Some(b) {return None;}
    Some((solid,DEF.metals.iter().find(|m|m.solid==solid).map(|m|m.resource),sim.thermal.cool?-273.15))
}
pub(super) fn pool_capacity(size: i32) -> u32 { (18 * size.pow(3)) as u32 }

fn pool(p:IVec3,n:&Node,size:i32)->Vec<IVec3>{
    let centre=p-n.facing*2*size;
    let mut out=Vec::with_capacity(pool_capacity(size) as usize);
    for y in 0..2*size {for z in -size..2*size {for x in -size..2*size {out.push(centre+IVec3::new(x,y,z));}}}
    out
}
pub fn interior(w:&World,p:IVec3)->Option<Vec<IVec3>>{
    let size=factory::step(w);let p=factory::anchor(p,size);let n=w.factory.nodes.get(&p)?;
    (n.kind==Kind::SmelteryController && n.formed).then(||pool(p,n,size))
}
#[derive(Clone, Copy)]
enum Mold {
    Basin,
    Ingots,
    Part { kind: u8, cost: u32 },
}

impl Mold {
    fn at(n: &Node) -> Option<Self> {
        match n.kind {
            Kind::CastingBasin => Some(Self::Basin),
            Kind::CastingTable if matches!(n.target, 0 | 256) => Some(Self::Ingots),
            Kind::CastingTable => Self::part(n.target.checked_sub(1)? as usize),
            _ => None,
        }
    }

    fn part(index: usize) -> Option<Self> {
        let part = engine_tools::PARTS.part_kinds.get(index)?;
        Some(Self::Part { kind: index as u8, cost: u32::from(part.material_cost) })
    }

    fn cells(self, table: IVec3, size: i32) -> impl Iterator<Item = IVec3> {
        let start = table + IVec3::new(size / 2, size, size / 2);
        let blocks = match self {
            Self::Part { cost, .. } => cost.div_ceil(INGOTS_PER_BLOCK).max(1),
            Self::Basin | Self::Ingots => 1,
        };
        (0..blocks).map(move |offset| start + IVec3::Y * offset as i32)
    }

    fn outputs(self, metal: &Metal) -> Option<(ItemStack, Option<ItemStack>)> {
        let ingot = Item::Resource(metal.resource as u8);
        match self {
            Self::Basin => Some((ItemStack::new(Item::Block(metal.solid), 1), None)),
            Self::Ingots => Some((ItemStack::new(ingot, INGOTS_PER_BLOCK), None)),
            Self::Part { kind, cost } => {
                let material = material_for(metal.solid)?;
                let excess = cost.div_ceil(INGOTS_PER_BLOCK) * INGOTS_PER_BLOCK - cost;
                Some((
                    ItemStack::new(Item::Part { kind, material }, 1),
                    (excess > 0).then(|| ItemStack::new(ingot, excess)),
                ))
            }
        }
    }
}

/// One successful collection from a part mold already filled with cooled metal.
/// Heating, fuel, pouring and cooling are deliberately outside this projection.
pub(super) struct PartCollection {
    pub metal: ItemStack,
    pub output: ItemStack,
    pub excess: Option<ItemStack>,
}

pub(super) fn part_collection(kind: u8, solid: B, size: i32) -> Option<PartCollection> {
    if kind == 255 { return None; }
    let mold = Mold::part(usize::from(kind))?;
    let metal = DEF.metals.iter().find(|metal| metal.solid == solid)?;
    let (output, excess) = mold.outputs(metal)?;
    let count = u32::try_from(mold.cells(IVec3::ZERO, size).count()).ok()?;
    Some(PartCollection { metal: ItemStack::new(Item::Block(solid), count), output, excess })
}

fn feed_cost(resource: usize) -> u32 {
    let per_ingot = if resource == factory::GOLD_CONCENTRATE {
        factory::recipes::CONCENTRATE_PER_INGOT
    } else {
        UNITS_PER_INGOT
    };
    INGOTS_PER_BLOCK * per_ingot
}

pub(super) fn installed_cast(n: &Node) -> Option<Item> {
    match Mold::at(n)? {
        Mold::Part { kind, .. } => Some(Item::Cast(kind)),
        Mold::Basin | Mold::Ingots => None,
    }
}
/// The mold region retains poured cells until collection or removal of its table.
pub fn contained(w:&World,p:IVec3)->bool{
    let size=factory::step(w);let base=factory::anchor(p,size);
    (1..=8).any(|down| {
        let table = base - IVec3::Y * down * size;
        w.factory.nodes.get(&table).and_then(Mold::at)
            .is_some_and(|mold| mold.cells(table, size).any(|q| q == p))
    })
}
pub(crate) fn material_for(solid:B)->Option<u8>{
    let resource=DEF.metals.iter().find(|m|m.solid==solid).map(|m|m.resource);
    engine_tools::MATERIALS.iter().enumerate().find(|(i,m)|{
        m.block.as_ref().is_some_and(|name|ron::from_str::<B>(name).ok()==Some(solid))
        || (resource.is_some() && super::tool_workshop::supply(*i).is_some_and(|r|r.resource==resource))
    }).map(|(i,_)|i as u8)
}
pub fn interact(w:&mut World,cell:IVec3,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>)->bool{
    if !interactive(w.get(cell)){return false;}
    let size=factory::step(w);let p=factory::anchor(cell,size);
    let Some(mut n)=w.factory.nodes.get(&p).cloned() else {w.expedition.say("Replace Legacy Component");return true;};
    let held=w.inventory.hotbar.get(w.adventure.hand_slot).copied().flatten().map(|s|s.item);
    if n.kind==Kind::SmelteryController {
        // An empty hand opens the normal machine panel and its bulk load controls.
        let Some(Item::Resource(r))=held else {return false;};
        let r=r as usize;
        if !n.accepts(r) { return false; }
        let reserved = w.factory.input_reserved(p, r);
        if n.input[r].saturating_add(reserved) > factory::CAPACITY - UNITS_PER_INGOT {
            w.expedition.say("Input Full");
            return true;
        }
        if w.inventory.remove_item(Item::Resource(r as u8),1){
            n.input[r] += UNITS_PER_INGOT;
            w.factory.nodes.insert(p,n);
            w.sync_from_inventory();
        }
    }else if matches!(n.kind,Kind::CastingTable|Kind::CastingBasin){
        if n.kind==Kind::CastingTable && let Some(Item::Cast(kind))=held {
            if n.target != 0 || Mold::Ingots.cells(p, size).any(|q| w.get(q) != B::Air) {
                w.expedition.say("Mold Occupied");
                return true;
            }
            n.target = u32::from(kind) + 1;
            if Mold::at(&n).is_none_or(|mold| mold.cells(p, size).any(|q| w.get(q) != B::Air)) {
                w.expedition.say("Mold Space Blocked");
                return true;
            }
            if w.inventory.remove_item(Item::Cast(kind),1){w.factory.nodes.insert(p,n);w.sync_from_inventory();}
            return true;
        }
        let Some(mold) = Mold::at(&n) else { return true; };
        let cells: Vec<_> = mold.cells(p, size).collect();
        if cells.iter().all(|&q|w.get(q)==B::Air){
            if let Mold::Part { kind, .. } = mold {
                let mut inv=w.inventory.clone();
                if inv.try_insert(ItemStack::new(Item::Cast(kind),1)).1==0 {n.target=0;w.inventory=inv;w.factory.nodes.insert(p,n);w.sync_from_inventory();}
            }else{w.expedition.say("Empty Mold");}
            return true;
        }
        let solid=w.get(cells[0]);
        let Some(metal) = DEF.metals.iter().find(|m| m.solid == solid) else {
            w.expedition.say("Cooling or Incomplete");
            return true;
        };
        if cells.iter().any(|&q|w.get(q)!=solid){w.expedition.say("Cooling or Incomplete");return true;}
        let Some((output, excess)) = mold.outputs(metal) else {
            w.expedition.say("Tool Material Unavailable");
            return true;
        };
        let mut inv=w.inventory.clone();
        if inv.try_insert(output).1 != 0 || excess.is_some_and(|stack| inv.try_insert(stack).1 != 0) {
            w.expedition.say("Inventory Full");
            return true;
        }
        if batch(w,&cells.iter().map(|&q|(q,B::Air)).collect::<Vec<_>>(),blocks,chunks){
            for q in cells {w.sandbox.cells.remove(&crate::simulation::sandbox::key(q));}
            // Clay molds are consumed by a completed cast.
            if matches!(mold, Mold::Part { .. }) {n.target=0;}
            inv.dropped = std::mem::take(&mut w.inventory.dropped);
            inv.overflow = std::mem::take(&mut w.inventory.overflow);
            w.factory.nodes.insert(p,n);w.inventory=inv;w.sync_from_inventory();
        }
    }else{
        let p=if n.kind==Kind::SmelteryDrain {SIDES.iter().map(|d|p+*d*size).find(|q|w.factory.nodes.get(q).is_some_and(|n|n.kind==Kind::SmelteryFaucet)).unwrap_or(p)}else{p};
        if let Some(n)=w.factory.nodes.get_mut(&p){if n.kind==Kind::SmelteryFaucet {n.enabled=!n.enabled;}}
    }
    true
}
fn prepare(w: &mut World, sources: &mut Residency, cells: &[IVec3]) -> bool {
    cells.iter().all(|&cell| sources.request(w, super::chunk_coord_from_world_block(cell)))
        && sources.prepare(w, cells)
}

fn pour(w:&mut World,p:IVec3,n:&Node,sources:&mut Residency,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>) -> &'static str {
    if !n.enabled{return "Paused";}
    let size=factory::step(w);
    let table=(1..=9).map(|d|p-IVec3::Y*d*size).find_map(|q|w.factory.nodes.get(&q).filter(|n|matches!(n.kind,Kind::CastingTable|Kind::CastingBasin)).map(|n|(q,n.clone())));
    let Some((table,cast))=table else{return "No Mold";};
    let Some(mold) = Mold::at(&cast) else { return "No Mold"; };
    let cells: Vec<_> = mold.cells(table, size).collect();
    let filled=cells.iter().find_map(|&q|{let b=w.get(q);metal(b).map(|(solid,_,_)|solid).or_else(||b.material().sim.melts_into.map(|_|b))});
    let Some(to)=cells.into_iter().find(|&q|w.get(q)==B::Air)else{return "Mold Full";};
    if to.y >= p.y || (to.y + 1..p.y).any(|y| w.get(IVec3::new(to.x, y, to.z)) != B::Air) {
        return "Pour Blocked";
    }
    let mut connected = false;
    for d in SIDES {
        let drain=p+d*size;
        if !w.factory.nodes.get(&drain).is_some_and(|n|n.kind==Kind::SmelteryDrain && n.formed){continue;}
        connected = true;
        // Read the whole drain face, not a single arbitrary fine voxel.
        for from in factory::cells(drain+d*size,size){
            if metal(w.get(from)).is_some_and(|(solid,_,_)|filled.is_none_or(|f|f==solid))
                && prepare(w,sources,&[from,to]) && transfer(w,from,to,blocks,chunks){return "Pouring";}
        }
    }
    if connected { "No Molten Metal" } else { "No Drain" }
}
#[derive(Default)]
pub(crate) struct Work {
    nodes: Vec<IVec3>,
    next: usize,
}

pub(crate) fn tick(w:&mut World,dt:f32,sources:&mut Residency,work:&mut Work,deadline:Budget,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>){
    let size=factory::step(w);
    work.nodes.clear();
    work.nodes.extend(w.factory.nodes.iter().filter(|(_,n)|matches!(n.kind,Kind::SmelteryController|Kind::SmelteryFaucet)).map(|(&p,_)|p));
    if work.nodes.is_empty() { return; }
    work.nodes.sort_unstable_by_key(|p|(p.x,p.y,p.z));
    for p in &work.nodes {
        if let Some(node) = w.factory.nodes.get_mut(p) {
            node.work_clock = (node.work_clock + dt).min(0.5);
        }
    }
    for _ in 0..work.nodes.len() {
        if !deadline.has_time(){break;}
        let p = work.nodes[work.next % work.nodes.len()];
        work.next = work.next.wrapping_add(1);
        let Some(mut n)=w.factory.nodes.remove(&p)else{continue;};
        if n.work_clock<0.05 {w.factory.nodes.insert(p,n);continue;}
        n.work_clock-=0.05;
        if n.kind==Kind::SmelteryFaucet {n.status=pour(w,p,&n,sources,blocks,chunks);w.factory.nodes.insert(p,n);continue;}
        if !n.enabled || !n.formed {n.status=if n.formed{"Paused"}else{"Incomplete shell"};w.factory.nodes.insert(p,n);continue;}
        let cells=pool(p,&n,size);
        // A metal cell cools into one placeable block. Charge the same ingots
        // as the cladding recipe so casting stays balanced at every voxel scale.
        let feed = DEF.metals.iter().find_map(|metal| {
            metal.feeds.iter().copied().find_map(|resource| {
                let cost = feed_cost(resource);
                (n.input[resource] >= cost).then_some((metal, resource, cost))
            })
        });
        if let Some((metal, resource, cost)) = feed
            && let Some(&q) = cells.iter().find(|&&q| w.get(q) == B::Air)
            && prepare(w, sources, &[q])
            && let Ok(commit) = effects::replace(w, q, B::Air, metal.solid)
        {
            commit.publish(w, blocks, chunks);
            n.input[resource] -= cost;
            n.target = resource as u32 + 1;
            w.activity.wake(q);
        }
        let charged=cells.iter().any(|&q|w.get(q).material().sim.melts_into.is_some() || metal(w.get(q)).is_some());
        if charged && n.burn == 0 {
            // Both solid fuels accepted by the controller can fire the bath.
            let fuel = [DEF.fuel, factory::COKE].into_iter()
                .find(|&resource| n.input[resource] >= DEF.fuel_units);
            if let Some(resource) = fuel {
                n.input[resource] -= DEF.fuel_units;
                n.burn = DEF.fuel_ticks;
            }
        }
        if charged && n.burn > 0 {
            let lined = w.get(p + IVec3::Y * (2 * size)) == B::KilnLining;
            n.burn -= 1;
            n.status = if lined { "Heating · Kiln Lining" } else { "Heating" };
            let mut heat = DEF.heat_per_tick * if lined { 1.25 } else { 1.0 };
            for _ in 0..cells.len().min(64) {
                let q=cells[n.progress as usize%cells.len()];n.progress=n.progress.wrapping_add(1)%cells.len() as u32;
                let b=w.get(q);
                if (b.material().sim.melts_into.is_none() && metal(b).is_none()) || !w.sandbox.can_admit(&[q]) || !prepare(w,sources,&[q]) {continue;}
                let mut c=w.sandbox.cell_for(q,b);
                let capacity=crate::simulation::heat::of(b).capacity*w.voxel_scale.meters().powi(3);
                let delta=(DEF.temperature_c-c.temperature).max(0.0).min(heat/capacity);
                c.temperature+=delta;heat-=delta*capacity;w.sandbox.set(q,c);w.activity.wake(q);
                if heat<=0.0 {break;}
            }
        }else{n.status=if charged{"Needs fuel"}else{"Needs input"};}
        w.factory.nodes.insert(p,n);
    }
}

pub fn transfer(w: &mut World, from: IVec3, to: IVec3, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> bool {
    let b = w.get(from);
    if metal(b).is_none() || w.get(to)!=B::Air || !w.sandbox.can_admit(&[from,to]) { return false; }
    let heat = w.sandbox.cell_for(from,b);
    if !batch(w, &[(from,B::Air),(to,b)], blocks, chunks) { return false; }
    w.sandbox.cells.remove(&crate::simulation::sandbox::key(from));
    w.sandbox.set(to,heat); w.activity.wake(to);
    true
}

fn batch(w:&mut World,edits:&[(IVec3,B)],blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>)->bool {
    effects::dry_material_batch(w,edits,blocks,chunks).is_ok()
}

/// Apply exact authored volume ratios within a face-connected liquid region.
/// A bounded search avoids a global tank scan. Every cell must be fully molten.
pub fn alloy(w:&mut World,p:IVec3,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>)->Option<usize> {
    use engine_content::reactions::{Reactant, Threshold, ROWS};
    for row in ROWS.iter().filter(|r| r.batch_counts.is_some()) {
        let (Reactant::Block(a), Some(Reactant::Block(b)), Some(product), Some((na, nb))) =
            (row.a, row.b, row.a_into, row.batch_counts) else { continue };
        if w.get(p) != a && w.get(p) != b { continue; }
        let lo = match row.a_above { Some(Threshold::K(k)) => k - 273.15, _ => f32::NEG_INFINITY };
        let hi = match row.a_below { Some(Threshold::K(k)) => k - 273.15, _ => f32::INFINITY };
        let mut queue = std::collections::VecDeque::from([p]);
        let mut seen = HashSet::from([p]);
        let mut aa = Vec::new(); let mut bb = Vec::new();
        while let Some(q) = queue.pop_front() {
            let id = w.get(q);
            if id != a && id != b { continue; }
            let c = w.sandbox.cell_for(q,id);
            if c.temperature < lo || c.temperature > hi || c.fuel != 0.0 { continue; }
            if id == a && aa.len() < na as usize { aa.push(q); }
            if id == b && bb.len() < nb as usize { bb.push(q); }
            if aa.len() == na as usize && bb.len() == nb as usize { break; }
            for d in [IVec3::X,IVec3::NEG_X,IVec3::Y,IVec3::NEG_Y,IVec3::Z,IVec3::NEG_Z] {
                if seen.len() < 128 && seen.insert(q+d) { queue.push_back(q+d); }
            }
        }
        if aa.len() != na as usize || bb.len() != nb as usize { continue; }
        aa.extend(bb);
        if !w.sandbox.can_admit(&aa) { continue; }
        // Preserve the sparse solver's sensible heat across different capacities.
        let energy: f32 = aa.iter().map(|&q| {
            w.sandbox.cell_for(q,w.get(q)).temperature * crate::simulation::heat::of(w.get(q)).capacity
        }).sum();
        let temperature = energy / (aa.len() as f32 * crate::simulation::heat::of(product).capacity);
        let edits:Vec<_> = aa.iter().map(|&q|(q,product)).collect();
        if !batch(w,&edits,blocks,chunks) { continue; }
        let count = aa.len();
        for q in aa { w.sandbox.set(q,ThermalCell {temperature,fuel:0.0,gas:0});w.activity.wake(q); }
        return Some(count);
    }
    None
}

#[cfg(test)]
mod forge_tests {
    use super::*;

    #[test]
    fn forge_collection_projection_matches_runtime_cells_and_outputs() {
        for size in [1, 2, 4] {
            for kind in 0..engine_tools::PARTS.part_kinds.len().min(255) {
                let mold = Mold::part(kind).unwrap();
                for metal in &DEF.metals {
                    let expected = mold.outputs(metal);
                    let collection = part_collection(kind as u8, metal.solid, size);
                    assert_eq!(collection.is_some(), expected.is_some());
                    let Some(collection) = collection else { continue; };
                    let (output, excess) = expected.unwrap();
                    assert_eq!(collection.metal.item, Item::Block(metal.solid));
                    assert_eq!(
                        collection.metal.count as usize,
                        mold.cells(IVec3::new(7, 11, 13), size).count(),
                    );
                    assert_eq!(collection.output, output);
                    assert_eq!(collection.excess, excess);
                }
            }
        }
    }
}
