//! Automatic nearby collection, run only by the owning gameplay authority.
use super::{World, raycast_block};
use crate::player::Player;
#[cfg(test)]
use glam::Vec3;

pub fn tick(world: &mut World, player: &Player, dt: f32) {
    let dt = dt.clamp(0.0, 0.05);
    world.inventory.pickup_cooldown = (world.inventory.pickup_cooldown - dt).max(0.0);
    if dt == 0.0 || world.inventory.pickup_cooldown > 0.0 || world.adventure.is_dead() {
        return;
    }
    let scale = world.voxel_scale.meters();
    let feet = player.pos * scale;
    let height = super::PLAYER_HEIGHT * player.scale.max(0.1);
    let target = feet + player.up_dir() * (height * 0.45);
    let mut drops = std::mem::take(&mut world.inventory.dropped);
    let mut collected = false;
    drops.retain_mut(|drop| {
        // Distance to the body, not the camera, so a stack on the ground is
        // reachable while standing and behaves the same at every voxel scale.
        let nearest = feet + player.up_dir() * (drop.pos - feet).dot(player.up_dir()).clamp(0.0, height);
        if drop.pos.distance_squared(nearest) > 2.0_f32.powi(2)
            || !world.inventory.can_insert(super::inventory::ItemStack::new(drop.stack.item,1)) {
            return true;
        }
        let delta = drop.pos - target;
        let distance = delta.length();
        if distance > 0.01
            && raycast_block(world, target / scale, delta / distance, distance / scale).is_some() {
            return true;
        }
        if drop.pos.distance_squared(nearest) <= 0.85_f32.powi(2) {
            let (inserted, remaining) = world.inventory.try_insert(drop.stack);
            if inserted > 0 {
                world.inventory.notify_pickup(drop.stack.item, inserted);
                collected = true;
            }
            drop.stack.count = remaining;
            return remaining > 0;
        }
        drop.pos -= delta * (dt * 8.0 / distance.max(0.001)).min(1.0);
        true
    });
    world.inventory.dropped = drops;
    if collected { world.sync_from_inventory(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::{WorldType,BlockId,inventory::{SurvivalInventory,Item,ItemStack,DroppedStack}}};

    fn scene(scale:VoxelScalePreset) -> (World,Player) {
        let mut w=World::with_seed(WorldType::Superflat,scale,7);
        w.inventory=SurvivalInventory::empty();w.inventory.voxel_m=scale.meters();
        let mut p=Player::new();p.pos=Vec3::new(8.0,300.0,8.0)/scale.meters();
        (w,p)
    }

    #[test]
    fn walking_over_drops_collects_at_every_voxel_scale() {
        for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::HalfMeter,VoxelScalePreset::QuarterMeter] {
            let (mut w,p)=scene(scale);let item=Item::Block(BlockId::Stone);
            w.inventory.dropped.push(DroppedStack{pos:p.pos*scale.meters()+Vec3::new(0.4,0.2,0.0),stack:ItemStack::new(item,3)});
            tick(&mut w,&p,1.0/120.0);
            assert!(w.inventory.dropped.is_empty());assert_eq!(w.inventory.count_item(item),3);
        }
    }

    #[test]
    fn partial_pickup_keeps_overflow_and_throw_grace_prevents_recollection() {
        let (mut w,p)=scene(VoxelScalePreset::OneMeter);let item=Item::Block(BlockId::Stone);
        for slot in w.inventory.hotbar.iter_mut().chain(w.inventory.pack.iter_mut()) {*slot=Some(ItemStack::new(Item::Resource(0),64));}
        let limit=w.inventory.stack_limit(item);w.inventory.hotbar[0]=Some(ItemStack::new(item,limit-2));
        w.inventory.dropped.push(DroppedStack{pos:p.pos+Vec3::new(0.4,0.2,0.0),stack:ItemStack::new(item,5)});
        w.inventory.pickup_cooldown=0.6;tick(&mut w,&p,0.05);
        assert_eq!(w.inventory.dropped[0].stack.count,5);
        w.inventory.pickup_cooldown=0.0;tick(&mut w,&p,0.05);
        assert_eq!(w.inventory.count_item(item),limit);assert_eq!(w.inventory.dropped[0].stack.count,3);
    }

    #[test]
    fn nearby_drops_do_not_cross_a_solid_wall() {
        let (mut w,p)=scene(VoxelScalePreset::HalfMeter);let item=Item::Block(BlockId::Stone);
        let at=p.pos*0.5+Vec3::new(1.6,0.81,0.0);
        w.set(glam::IVec3::new(18,601,16),BlockId::Stone);
        w.inventory.dropped.push(DroppedStack{pos:at,stack:ItemStack::new(item,2)});
        tick(&mut w,&p,0.05);
        assert_eq!(w.inventory.dropped[0].pos,at);assert_eq!(w.inventory.count_item(item),0);
    }
}
