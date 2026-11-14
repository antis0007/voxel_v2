//! Services change existing plots, boats or dropped stacks; never mint stock.
use super::*;
use crate::world::farming;

pub fn accompany(w:&mut World,id:u32,eye:Vec3)->Result<(),&'static str>{
    accompany_for(w, id, eye, None)
}

pub(crate) fn invite_to_base(w: &mut World, id: u32, eye: Vec3) -> Result<(), &'static str> {
    #[cfg(feature = "multiplayer")]
    if w.multiplayer.is_some() {
        return Err("Move-in is available in local play");
    }
    if !w.rules.mode.interacts() {
        return Err("Move-in is available in local play");
    }
    follow_or_settle(w, id, eye, None, false)
}

pub(super) fn accompany_for(w:&mut World,id:u32,eye:Vec3,actor:Option<u32>)->Result<(),&'static str>{
    follow_or_settle(w, id, eye, actor, true)
}

fn follow_or_settle(w: &mut World, id: u32, eye: Vec3, actor: Option<u32>, paid: bool) -> Result<(), &'static str> {
    let p=vehicle::villager_centre(w,id).ok_or("Resident Absent")?;
    if !eye.is_finite() || p.distance(eye)>6.0{return Err("Out of Reach");}
    if w.travel.guard_posts.contains_key(&id) { return Err("Release Guard First"); }
    if w.travel.following==Some(id){
        if w.travel.follow_owner != actor { return Err("Companion Busy"); }
        if let Some(e)=w.adventure.enemies.iter().find(|e|e.site.z==8 && e.site.x==id as i32){w.travel.homes.insert(id,e.pos);}
        w.travel.following=None;
        w.travel.follow_owner=None;
        w.expedition.say(if paid { "Staying" } else { "Moved in" });
        return Ok(());
    }
    if w.travel.following.is_some(){return Err("Companion Busy");}
    if w.travel.homes.len()>=64 && !w.travel.homes.contains_key(&id){return Err("Homes Full");}
    if paid && !w.inventory.remove_item(Item::Food(super::super::inventory::FoodKind::Trailcake),1){return Err("Requires: 1 Trailcake");}
    // Save feet rather than the interaction centre; otherwise reload would lift
    // the resident through ceilings. Walking remains collision constrained.
    if let Some(e)=w.adventure.enemies.iter().find(|e|e.site.z==8 && e.site.x==id as i32){w.travel.homes.insert(id,e.pos);}
    w.travel.following=Some(id);
    w.travel.follow_owner=actor;
    if paid { w.sync_from_inventory(); }
    w.expedition.say(if paid { "Following" } else { "Lead them home, then choose Move In Here" });
    Ok(())
}

pub fn label(id: u32) -> Option<&'static str> {
    Some(match engine_worldgen::households::role(id)? {
        0=>"Tend Crops",1=>"Recover Boat",2=>"Tend Saplings",_=>"Gather Items",
    })
}
pub fn cost(id: u32) -> Option<Item> {
    matches!(engine_worldgen::households::role(id),Some(0|2)).then_some(Item::Resource(13))
}
enum Target { Plots(Vec<(i32,i32,i32)>), Boat(u32), Drops(Vec<usize>) }

fn target(w:&World,id:u32,eye:Vec3)->Result<Target,&'static str> {
    let center=vehicle::villager_centre(w,id).ok_or("Resident Absent")?;
    if !eye.is_finite() || center.distance(eye)>6.0{return Err("Out of Reach");}
    let role=engine_worldgen::households::role(id).ok_or("Unavailable")?;
    if let Some(cost)=cost(id) {if w.inventory.count_item(cost)==0{return Err("Requires: 1 Fungal Biomass");}}
    if role==0 || role==2 {
        if w.travel.day==day(w) && w.travel.used.get(&(id,u16::MAX)).copied().unwrap_or(0)>=1{return Err("Resting");}
        let scale=w.voxel_scale.meters();
        let mut plots=Vec::new();
        for (&key,plot) in &w.farming.plots {
            let p=IVec3::new(key.0,key.1,key.2);
            if ((p.as_vec3()+Vec3::splat(0.5))*scale).distance_squared(center)>12.0*12.0 {continue;}
            let Some(crop)=farming::table::get(plot.crop) else {continue;};
            if farming::watershed::manages(w, p, crop) { continue; }
            // Paid growth requires a loaded crop that still exists.
            if !w.chunks.contains_key(&super::super::chunk_coord_from_world_block(p))
                || ![crop.sprout,crop.crop].contains(&w.get(p)) {continue;}
            if crop.tree.is_some()!=(role==2) || plot.stage>=crop.mature(){continue;}
            if farming::conditions(w,p,crop).is_some_and(|c|c.soil && c.light>=crop.light_min && c.light<=crop.light_max
                && (crop.irrigation_metres.is_none() || c.wet)) {
                plots.push(key);
                if plots.len()==4 {break;}
            }
        }
        if plots.is_empty(){Err("No Growing Plots")}else{Ok(Target::Plots(plots))}
    }else if role==1 {
        let found=w.travel.boats.iter().filter(|(bid,b)|!w.travel.occupied(Ride::Boat(**bid)) && b.pos.distance_squared(center)<12.0*12.0)
            .min_by(|a,b|a.1.pos.distance_squared(center).total_cmp(&b.1.pos.distance_squared(center))).map(|(id,_)|*id);
        found.map(Target::Boat).ok_or("No Unoccupied Boat")
    }else{
        let drops=w.inventory.dropped.iter().enumerate().filter(|(_,d)|d.pos.distance_squared(center)<12.0*12.0)
            .take(8).map(|(i,_)|i).collect::<Vec<_>>();
        if drops.is_empty(){Err("No Dropped Items")}else{Ok(Target::Drops(drops))}
    }
}

pub fn availability(w:&World,id:u32,eye:Vec3)->Result<(),&'static str>{target(w,id,eye).map(|_|())}

pub fn perform(w:&mut World,id:u32,eye:Vec3)->Result<(),&'static str>{
    let action=target(w,id,eye)?;
    w.travel.restock(day(w));
    match action {
        Target::Plots(plots)=>{
            if !w.inventory.remove_item(Item::Resource(13),1){return Err("Items Missing");}
            for p in plots {if let Some(plot)=w.farming.plots.get_mut(&p){plot.stage+=1;}}
            w.travel.used.insert((id,u16::MAX),1);
            w.expedition.say("Plots Tended");
        }
        Target::Boat(id)=>{
            let (inserted,_)=w.inventory.try_insert(ItemStack::new(BOAT,1));
            if inserted==0{return Err("Inventory Full");}
            w.travel.boats.remove(&id);
            w.adventure.enemies.retain(|e| !(e.kind==vehicle::BOAT && e.site.x==id as i32 && e.site.z==9));
            w.inventory.notify_pickup(BOAT,1);
            w.expedition.say("Boat Recovered");
        }
        Target::Drops(mut indices)=>{
            // Reverse indices keep removal stable; partial insertion leaves the
            // exact unpaid remainder in the world, including when the pack fills.
            indices.reverse();
            let mut moved=0;
            for i in indices {
                let stack=w.inventory.dropped[i].stack;
                let (count,left)=w.inventory.try_insert(stack);
                moved+=count;
                if count>0{w.inventory.notify_pickup(stack.item,count);}
                if left==0{w.inventory.dropped.remove(i);}else{w.inventory.dropped[i].stack.count=left;}
            }
            if moved==0{return Err("Inventory Full");}
            w.expedition.say("Items Gathered");
        }
    }
    w.sync_from_inventory();
    Ok(())
}
