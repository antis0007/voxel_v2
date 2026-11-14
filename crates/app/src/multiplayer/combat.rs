//! The host alone resolves player damage; clients cannot nominate a victim or amount.
use super::*;

pub fn apply(world: &mut World, host: &mut Player, actors: &mut BTreeMap<PlayerId, Actor>,
    hits: Vec<gameplay::Hit>, headless: bool) {
    if !world.rules.pvp { return; }
    for hit in hits {
        if hit.id == HOST && !headless {
            let before=world.adventure.health;
            world.adventure.multiplayer_damage(hit.amount,hit.direction,world.inventory.armor_reduction());
            if world.adventure.health<before {
                host.vel-=hit.direction.normalize_or_zero()*2.5/world.voxel_scale.meters();
                host.vel.y=host.vel.y.max(2.0/world.voxel_scale.meters());
            }
        } else if let Some(actor) = actors.get_mut(&hit.id) {
            let before = actor.personal.adventure.health;
            actor.personal.adventure.multiplayer_damage(hit.amount, hit.direction, actor.personal.inventory.armor_reduction());
            if actor.personal.adventure.health < before {
                eprintln!("[pvp] target={} damage={:.1} health={:.1}", hit.id, before-actor.personal.adventure.health, actor.personal.adventure.health);
                // The existing correction channel carries host impulses as well as teleports.
                actor.player.vel -= hit.direction.normalize_or_zero()*2.5/world.voxel_scale.meters();
                actor.player.vel.y = actor.player.vel.y.max(2.0/world.voxel_scale.meters());
                actor.queue_correction();
            }
        }
    }
}
