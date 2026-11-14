//! Planted timber uses ordinary growth, tree physics and hauled pickups.
use super::*;
use crate::world::{block_data, expedition};
use std::collections::HashSet;

pub(super) mod check;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::world::farming::jobs) struct State {
    pub target: u32,
    #[serde(default)] pub planted: u32,
    #[serde(default)] pub felled: u32,
}

impl State {
    pub fn valid(&self) -> bool { (1..=256).contains(&self.target) }
}

#[derive(Deserialize)]
struct Content { roots: Vec<[i32;3]>, saplings: u32, log_target: u32, residents: usize, extra_beds: Vec<Bed> }

pub(super) fn enabled() -> bool { std::env::args().any(|a| a == "--city-livelihood-woodlot") }

pub(super) fn install(w: &mut World) -> Result<(), String> {
    let content: Content = ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../assets/city_woodlot.ron")))
        .map_err(|e| e.to_string())?;
    if content.roots.is_empty() || content.roots.len() > 16 || !(1..=64).contains(&content.saplings)
        || !(1..=256).contains(&content.log_target) || !(3..=64).contains(&content.residents)
        || content.extra_beds.len()>64 { return Err("Invalid woodlot content".into()); }
    let pile = IVec3::from(w.farming.jobs.stockpile.ok_or("Missing woodlot stockpile")?);
    let base = pile - IVec3::Y;
    let mut beds: Vec<_> = content.extra_beds.iter().map(|b|Bed {at:(base+IVec3::from(b.at)).to_array(),crop:b.crop}).collect();
    if beds.iter().any(|b|!matches!(b.crop,B::CarrotCrop|B::BeetCrop)) {return Err("Unsupported woodlot crop".into());}
    beds.extend(content.roots.iter().map(|p| Bed { at:(base+IVec3::from(*p)).to_array(), crop:B::Sapling }));
    if beds.iter().any(|b| !b.valid() || !b.open(w) || (IVec3::from(b.at)-pile).as_vec3().length()>20.0) {
        return Err("Woodlot bed obstructed or out of reach".into());
    }
    let Some(s) = w.farming.jobs.production.as_mut() else { return Err("Missing production".into()) };
    s.beds.extend(beds.iter().copied());
    s.woodlot = Some(State { target:content.log_target, planted:0, felled:0 });
    if !s.valid() { return Err("Invalid woodlot production".into()); }
    let mut seed = ItemStack::new(Item::Block(B::Sapling), content.saplings);
    block_data::store_hauled(w, pile, &mut seed);
    if seed.count != 0 { return Err("Sapling stockpile full".into()); }
    for crop in [B::CarrotCrop,B::BeetCrop] {
        let mut seed=ItemStack::new(Item::Block(crop),beds.iter().filter(|b|b.crop==crop).count() as u32);
        block_data::store_hauled(w,pile,&mut seed);
        if seed.count!=0 {return Err("Field seed stockpile full".into());}
    }
    for id in 3..content.residents as u32 {
        let slot=id-3;
        let feet=base.as_vec3()+Vec3::new(6.5+(slot%3) as f32*1.2,1.0,2.5+(slot/3) as f32*1.2);
        let mut e=crate::world::adventure::Enemy::new(IVec3::new(id as i32,0,8),0,40,feet,0);
        e.home=feet;w.adventure.enemies.push(e);w.travel.homes.insert(id,feet);
        if !super::super::crew::add(w,id) {return Err(format!("Physical crew capacity reached at {id}"));}
        if let Some(s)=w.farming.jobs.production.as_mut() {s.workers.push(id);}
    }
    Ok(())
}

pub(super) fn ready(w: &World, j: &Jobs, pile: IVec3, bed: Bed) -> bool {
    let Some(f) = j.production.as_ref().and_then(|s|s.woodlot.as_ref()) else { return false };
    if bed.crop != B::Sapling || known(w, bed.at.into()) != Some(B::Log)
        || known(w, IVec3::from(bed.at)+IVec3::Y) != Some(B::Log) { return false; }
    let p=IVec3::from(bed.at);
    if farming::base::overlaps(w,p-IVec3::new(2,0,2),p+IVec3::new(2,6,2))
        || w.sandbox.bodies.len()>=crate::simulation::sandbox::MAX_BODIES
        || w.sandbox.bodies.iter().any(|b| b.occupied.iter().any(|(q,_)|*q==p||*q==p+IVec3::Y)) { return false; }
    let log=Item::Block(B::Log);
    let range = super::super::work_range(j);
    let near = |cell: IVec3| (cell-pile).as_vec3().length_squared() * w.voxel_scale.meters().powi(2) <= range*range;
    let pending=w.sandbox.bodies.iter().filter(|b|b.felled_tree && b.occupied.iter().any(|(p,_)|near(*p)))
        .map(|b|b.voxels.iter().filter(|(_,block)|*block==B::Log).count() as u32).sum::<u32>();
    let dropped=w.inventory.dropped.iter().filter(|d|d.stack.item==log
        && near((d.pos/w.voxel_scale.meters()).floor().as_ivec3())).map(|d|d.stack.count).sum::<u32>();
    count(&w.block_data.chest(pile),log)+super::super::crew::cargo_item(j,log)+pending+dropped < f.target
}

pub(super) fn execute(w: &mut World, j: &mut Jobs, bed: Bed) -> bool {
    let (Some(pile),Some(id),Some(s))=(j.stockpile.map(IVec3::from),j.worker,&j.production) else { return true };
    if j.carry.is_some() || !s.owns(id,bed) || !ready(w,j,pile,bed) { return true; }
    if !super::super::activity::work(w,j,12,bed.at.into()) { return false; }
    let root=IVec3::from(bed.at);
    let direction=(root-pile).as_vec3();
    let before=w.sandbox.bodies.len();
    let mut blocks=HashSet::new();let mut chunks=HashSet::new();
    if expedition::harvest_log(w,root,Some(direction),&mut blocks,&mut chunks) {
        w.adventure.dirty_voxels.extend(blocks);
        if w.sandbox.bodies.len()>before && let Some(f)=j.production.as_mut().and_then(|s|s.woodlot.as_mut()) {
            f.felled+=1;
        }
    }
    true
}
