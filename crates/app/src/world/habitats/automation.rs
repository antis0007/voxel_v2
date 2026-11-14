use super::*;
use crate::world::factory::Kind;

/// Fonts supply finite receiver charge without consuming player mana.
pub(super) fn supply_mana(world:&mut World,source:&mut Source,p:Vec3) {
    if !source.emitter && !source.automatic && source.beacon == Beacon::Off { return; }
    let wanted=64u32.saturating_sub(source.charge).min(4);
    if wanted==0 {return;}
    let scale=world.voxel_scale.meters();
    let half=super::super::factory::step(world) as f32*0.5;
    let font=world.factory.nodes.iter()
        .filter(|(_,n)|n.kind==Kind::Battery && n.enabled && n.mode==3 && n.energy>=4)
        .map(|(q,_)|(*q,((q.as_vec3()+Vec3::splat(half))*scale).distance_squared(p)))
        .filter(|(_,d)|*d<=6.0*6.0)
        .min_by(|(a,da),(b,db)|da.total_cmp(db).then(a.to_array().cmp(&b.to_array())))
        .map(|(q,_)|q);
    if let Some(node)=font.and_then(|q|world.factory.nodes.get_mut(&q)) {
        let amount=wanted.min(node.energy/4);
        node.energy -= amount * 4;
        node.activity = 1.0;
        world.factory.stats.power(0, amount * 4);
        source.charge += amount;
    }
}
