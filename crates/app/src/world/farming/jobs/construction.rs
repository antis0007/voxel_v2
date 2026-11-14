use super::{B, IVec3, ItemStack, RANGE, State, Vec3, World, enabled, known};
use crate::world::farming::base;
use crate::world::{blueprint::Blueprint, effects, inventory::Item};
use serde::{Deserialize, Serialize};

pub(super) const LIMIT: usize = 256;
const BATCH: u32 = 8;
mod batch;
const SIDES: [IVec3; 6] = [
    IVec3::NEG_Y,
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Z,
    IVec3::NEG_Z,
    IVec3::Y,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Order {
    pub(super) at: [i32; 3],
    block: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replaces: Option<u16>,
}

impl Order {
    pub(super) fn new(cell: IVec3, block: B) -> Self {
        Self { at: cell.to_array(), block: block as u16, replaces: None }
    }

    pub(super) fn pave(cell: IVec3, expected: B) -> Self {
        Self { at: cell.to_array(), block: B::Cobblestone as u16, replaces: Some(expected as u16) }
    }

    pub(super) fn paving(self) -> bool { self.replaces.is_some() }

    fn expected(self) -> B { self.replaces.map_or(B::Air, |b| B::from_u32(u32::from(b))) }

    pub(super) fn cell(self) -> IVec3 {
        self.at.into()
    }

    pub(super) fn point(self, v: f32) -> Vec3 {
        (self.cell().as_vec3() + Vec3::splat(0.5)) * v
    }

    pub(super) fn block(self) -> B {
        B::from_u32(u32::from(self.block))
    }

    pub(super) fn material(self) -> Option<Item> {
        let block = self.block();
        (block as u16 == self.block
            && block.is_solid()
            && self.replaces.is_none_or(|b| block == B::Cobblestone
                && matches!(B::from_u32(u32::from(b)), B::Grass | B::Dirt | B::Gravel | B::Sand))
            && !(block == B::Bedrock || block.is_seal()))
        .then(|| Item::canonical_block(block).map(Item::Block))
        .flatten()
    }

    pub(super) fn ready(self, world: &World) -> bool {
        let p = self.cell();
        !base::overlaps(world, p, p)
            && (!self.paving() || (!world.farming.plots.contains_key(&(p.x,p.y+1,p.z))
                && !world.farming.jobs.production.as_ref().is_some_and(|s|
                    s.beds.iter().any(|bed|bed.at==(p+IVec3::Y).to_array()))))
            && known(world, p) == Some(self.expected())
            && SIDES
                .iter()
                .any(|&side| known(world, p + side).is_some_and(B::is_solid))
    }
}

/// Blueprint air is ignored. Existing terrain is never erased by a job.
pub(crate) fn queue_blueprint(
    world: &mut World,
    bp: &Blueprint,
    origin: IVec3,
) -> Result<usize, &'static str> {
    let jobs = &world.farming.jobs;
    if !enabled(world) || !world.rules.mode.interacts() || jobs.worker.is_none() {
        return Err("Assign a settler first");
    }
    let pile = jobs
        .stockpile
        .map(IVec3::from)
        .ok_or("Mark a stockpile chest first")?;
    if !known(world, pile).is_some_and(|b| b.chest_tier().is_some()) {
        return Err("Stockpile missing or unloaded");
    }
    let v = world.voxel_scale.meters();
    let cfg = world.sample_config();
    let mut pending = Vec::new();
    for (i, &block) in bp.cells().iter().enumerate().filter(|(_, b)| **b != B::Air) {
        let p = origin + bp.local(i);
        if known(world, p) == Some(block) {
            continue;
        }
        if base::overlaps(world, p, p) {
            return Err("Protected Base");
        }
        let order = Order {
            at: p.to_array(),
            block: block as u16,
            replaces: None,
        };
        if order.material().is_none() {
            return Err("Settlers need solid building blocks");
        }
        if p == pile
            || p.y < cfg.min_world_y()
            || p.y >= cfg.max_world_y()
            || (p - pile).as_vec3().length_squared() * v * v > RANGE * RANGE
        {
            return Err("Build within 24 m of the stockpile");
        }
        if jobs.mine.iter().any(|&(at, _)| at == order.at)
            || jobs.build.iter().any(|o| o.at == order.at && *o != order)
        {
            return Err("A different order already marks that cell");
        }
        if !jobs.build.contains(&order) {
            if jobs.build.len() + pending.len() >= LIMIT {
                return Err("Build queue full (256 blocks)");
            }
            pending.push(order);
        }
    }
    let count = pending.len();
    let jobs = &mut world.farming.jobs;
    jobs.build.extend(pending);
    jobs.reset_task();
    jobs.next_candidate = 0;
    Ok(count)
}

pub(super) fn fetch(world: &mut World, j: &mut State, pile: IVec3, order: Order) {
    let Some(item) = order.material() else { return };
    if j.carry.is_some() || super::supply::source(world,j,order) != Some(pile)
        || !j.available(order) || !j.build.contains(&order) || !order.ready(world) { return; }
    let funded = j.growth_funded_order(order);
    let nearby = batch::nearest::<{ BATCH as usize - 1 }, _>(
        j.build.iter().copied().filter(|&other| other != order),
        |other| (other.cell() - order.cell()).as_vec3().length_squared(),
        |other| other.material() == Some(item) && j.available(other)
            && j.growth_funded_order(other) == funded
            && super::supply::source(world,j,other) == Some(pile)
            && !super::crew::planned(j, super::Task::Fetch(pile, other))
            && other.ready(world),
    );
    let pending: Vec<_> = std::iter::once(order).chain(nearby).take(BATCH as usize).collect();
    let allowed = j.build_stock(world,pile,order).min(pending.len() as u32);
    let count = crate::world::block_data::take_hauled(world, pile, item, allowed);
    if count > 0 {
        j.carry = Some(ItemStack::new(item, count));
        j.cargo = Some(super::supply::Cargo::Build { source: pile.to_array() });
        j.building_material = true;
        j.reserve(pending, count as usize);
        j.status = "Carrying building materials";
    }
}

pub(super) fn unoccupied(world: &World, order: Order, player: Vec3) -> bool {
    let v = world.voxel_scale.meters();
    let min = order.cell().as_vec3() * v;
    let max = min + Vec3::splat(v);
    let intersects = |feet: Vec3| {
        let lo = feet - Vec3::new(0.4, 0.0, 0.4);
        let hi = feet + Vec3::new(0.4, 1.8, 0.4);
        lo.cmplt(max).all() && hi.cmpgt(min).all()
    };
    if intersects(player)
        || world
            .adventure
            .enemies
            .iter()
            .any(|e| e.health > 0.0 && intersects(e.pos))
    {
        return false;
    }
    true
}

pub(super) fn workers_clear(j: &State, order: Order, scale: f32) -> bool {
    let min = order.cell().as_vec3() * scale;
    let max = min + Vec3::splat(scale);
    super::crew::ids(j).filter_map(|id| super::crew::view(j, id).and_then(|w| w.pos))
        .map(Vec3::from).all(|feet| {
            let lo = feet - Vec3::new(0.4, 0.0, 0.4);
            let hi = feet + Vec3::new(0.4, 1.8, 0.4);
            !(lo.cmplt(max).all() && hi.cmpgt(min).all())
        })
}

pub(super) fn place(world: &mut World, j: &mut State, order: Order, player: Vec3) {
    if !j.owns(order) || !super::supply::matches_source(j,j.cargo,order)
        || super::supply::source(world,j,order).is_none() { j.status = "Build claim changed"; return; }
    let Some(mut stack) = j.carry.filter(|stack| Some(stack.item) == order.material()) else {
        return;
    };
    let v = world.voxel_scale.meters();
    if !unoccupied(world, order, player) || !workers_clear(j, order, v) {
        j.status = "Build site occupied";
        return;
    }
    if !order.ready(world) {
        j.status = "Build site blocked or unsupported";
        return;
    }
    let displaced = order.replaces.and_then(|_| crate::world::block_data::blast_drop(order.expected()));
    if displaced.is_some() && world.inventory.dropped.len() >= 1024 {
        j.status = "Clear displaced road material";
        return;
    }
    let Ok(commit) = effects::replace(world, order.cell(), order.expected(), order.block()) else {
        j.status = "Waiting to place block";
        return;
    };
    let mut changed = Vec::new();
    commit.publish_voxel(world, &mut changed);
    world.adventure.dirty_voxels.extend(changed);
    if let Some(item) = displaced {
        world.inventory.dropped.push(crate::world::inventory::DroppedStack {
            pos: order.point(v) + Vec3::Y * v, stack: ItemStack::new(item,1),
        });
    }
    stack.count -= 1;
    j.carry = (stack.count != 0).then_some(stack);
    j.building_material = j.carry.is_some();
    if j.carry.is_none() { j.cargo = None; }
    j.build.retain(|&o| o != order);
    j.claims.retain(|c| c.order != order);
    j.last_placement = Some((world.farming.clock, order.point(v)));
    super::groups::committed(j, order, world.farming.clock);
    j.status = "Built";
}
