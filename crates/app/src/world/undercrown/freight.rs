//! Freight controls use stored power and the actual shutter blocks.
use crate::{player::Player,world::{World,BlockId as B,interaction::Prompt,effects,factory,trace}};
use engine_worldgen::fernwake::freight as site;
use glam::{IVec3,Vec3};
use std::collections::HashSet;
use super::Outcome;
use crate::world::farming::pantry;
const POWER:u32=16;

#[derive(serde::Deserialize)]
struct Text {notice:String,archive:String}
static TEXT:std::sync::LazyLock<Text>=std::sync::LazyLock::new(||
    ron::from_str(include_str!("../../../../../assets/freight_records.ron"))
        .unwrap_or_else(|error| {eprintln!("[freight] {error}");Text{notice:String::new(),archive:String::new()}}));

fn target(w:&World,eye:Vec3,dir:Vec3)->Option<usize> {
    let cfg=w.sample_config();
    if !site::active(&cfg) || !eye.is_finite() || !dir.is_finite(){return None;}
    [site::CONTROL,site::NOTICE,site::ARCHIVE].iter().enumerate().find_map(|(id,&p)| {
        let at=site::point(&cfg,p);
        let d=at-eye;
        let along=d.dot(dir);
        let base=((at-Vec3::Y)/w.voxel_scale.meters()).floor().as_ivec3();
        let expected=match id {0=>B::CopperBlock,1=>B::Planks,_=>B::CarvedStone};
        (d.length_squared()<3.4*3.4 && along>0.0 && (d-dir*along).length_squared()<0.55*0.55
            && w.chunks.contains_key(&crate::world::chunk_coord_from_world_block(base)) && w.get(base)==expected
            && trace::solid(w,eye,at).is_none()).then_some(id)
    })
}

fn shutter(w:&World)->Result<Vec<(IVec3,B)>,&'static str> {
    let cfg=w.sample_config();let scale=w.voxel_scale.meters();
    let lo=site::point(&cfg,site::DOOR_MIN);let hi=site::point(&cfg,site::DOOR_MAX);
    let a=(lo/scale).floor().as_ivec3();let b=(hi/scale).ceil().as_ivec3();
    let mut edits=Vec::new();
    for y in a.y..b.y {for z in a.z..b.z {for x in a.x..b.x {
        let p=IVec3::new(x,y,z);let center=(p.as_vec3()+Vec3::splat(0.5))*scale;
        if center.cmplt(lo).any() || center.cmpge(hi).any(){continue;}
        if !w.chunks.contains_key(&crate::world::chunk_coord_from_world_block(p)){return Err("Area Unloaded");}
        match w.get(p) {
            B::Air=>{},
            B::IronBlock=>{
                let parked=p+IVec3::Y*(site::SHUTTER_LIFT/scale).round() as i32;
                if !w.chunks.contains_key(&crate::world::chunk_coord_from_world_block(parked)){return Err("Area Unloaded");}
                if w.get(parked)!=B::Air {return Err("Shutter Rail Obstructed");}
                edits.push((p,B::Air));edits.push((parked,B::IronBlock));
            },
            _=>return Err("Shutter Obstructed"),
        }
    }}}
    Ok(edits)
}

fn supply(w:&World)->Option<IVec3> {
    let at=site::point(&w.sample_config(),site::CONTROL);
    let s=w.voxel_scale.meters();let half=factory::step(w) as f32*0.5;
    w.factory.nodes.iter().filter(|(p,n)|n.kind==factory::Kind::Battery && n.enabled && n.energy>=POWER
        && w.chunks.contains_key(&crate::world::chunk_coord_from_world_block(**p)))
        .map(|(p,_)|(*p,((p.as_vec3()+Vec3::splat(half))*s-at).length_squared()))
        .filter(|(_,d)|*d<=6.0*6.0)
        .min_by(|(a,da),(b,db)|da.total_cmp(db).then(a.to_array().cmp(&b.to_array())))
        .map(|(p,_)|p)
}

pub(super) fn focus(w:&World,eye:Vec3,dir:Vec3)->Option<Prompt> {
    let id=target(w,eye,dir)?;
    Some(match id {
        1 if pantry::active(w)=>Prompt{title:"Fernwake Town Board",action:"[F] Read"},
        1=>Prompt{title:"Freight Notice",action:"[F] Read"},
        2=>Prompt{title:"Service Ledger",action:"[F] Read"},
        _=>Prompt{title:"Cargo Shutter",action:match shutter(w) {
            Err(reason)=>reason,Ok(edits) if edits.is_empty()=>"Open",
            Ok(_) if supply(w).is_none()=>"Battery Required · 4 kJ · 6 m",
            Ok(_)=>"[F] Open · 4 kJ",
        }},
    })
}

pub(super) fn interact(w:&mut World,p:&Player)->Option<Outcome> {
    let id=target(w,p.eye(w.voxel_scale)*w.voxel_scale.meters(),p.forward_dir())?;
    let mut out=Outcome{used:true,..Outcome::default()};
    if id == 1 && let Some(report) = pantry::report(w) {
        let mut message = format!("Fernwake Town Board · {}", report.summary);
        if let Some(cause) = report.cause {
            message.push_str(&format!("\nWaiting for a meal · {}\n{cause}", report.missing));
            message.push_str("\nBring trailcakes to the Receiving Clerk. Use the south stairs if the shutter is closed.");
        }
        w.expedition.say_for(message, 12.0);
        return Some(out);
    }
    if id!=0 {out.message=Some(if id==1 {TEXT.notice.as_str()}else{TEXT.archive.as_str()});return Some(out);}
    let edits=match shutter(w) {Ok(edits)=>edits,Err(reason)=>{out.message=Some(reason);return Some(out);}};
    if edits.is_empty(){out.message=Some("Shutter Open");return Some(out);}
    let scale=w.voxel_scale.meters();
    let (player_min,player_max)=p.bounds(w.voxel_scale);
    for &(cell,block) in &edits {
        if block==B::Air {continue;}
        let lo=cell.as_vec3()*scale;let hi=lo+Vec3::splat(scale);
        let overlaps=|min:Vec3,max:Vec3|lo.cmplt(max).all() && hi.cmpgt(min).all();
        let mut occupied=overlaps(player_min*scale,player_max*scale);
        #[cfg(feature="multiplayer")]
        if let Some(context)=w.multiplayer.as_ref(){occupied|=context.players.iter().any(|actor|overlaps(actor.min,actor.max));}
        occupied|=w.adventure.enemies.iter().any(|actor|actor.health>0.0
            && overlaps(actor.pos-Vec3::new(1.0,0.0,1.0),actor.pos+Vec3::new(1.0,3.0,1.0)));
        if occupied {out.message=Some("Shutter Rail Occupied");return Some(out);}
    }
    let Some(battery)=supply(w) else {out.message=Some("Battery Required · 4 kJ · 6 m");return Some(out);};
    let mut blocks=HashSet::new();let mut chunks=HashSet::new();
    if effects::dry_material_batch(w,&edits,&mut blocks,&mut chunks).is_err() {
        out.message=Some("Shutter Blocked");return Some(out);
    }
    // No call above changes factory nodes. Debit only after the whole door commits.
    if let Some(node)=w.factory.nodes.get_mut(&battery){node.energy-=POWER;}
    out.changed=blocks.into_iter().collect();
    out.changed.sort_by_key(|p|(p.x,p.y,p.z));
    out.message=Some("Shutter Open");Some(out)
}
