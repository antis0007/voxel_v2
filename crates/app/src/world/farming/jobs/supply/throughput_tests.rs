use super::*;
use crate::world::{farming::sidecar, storage};
use std::path::Path;

#[test]
fn single_supply_record_keeps_legacy_quantity_contract() {
    for count in [1, 8, 16, 64] {
        let j=State {worker:Some(17),stockpile:Some([0,1,0]),
            carry:Some(ItemStack::new(Item::Block(B::Planks),count)),
            cargo:Some(Cargo::Supply{source:[0,1,0],target:[16,1,0],project:10}),
            ..State::default()};
        let encoded=ron::to_string(&j).unwrap();
        let restored:State=ron::from_str(&encoded).unwrap();
        assert!(restored.valid());
        assert_eq!(restored.carry,j.carry);
        assert_eq!(restored.cargo,j.cargo);
        assert_eq!(destination(&restored,IVec3::ZERO),IVec3::new(16,1,0));
    }
}

fn root()->std::path::PathBuf {
    let p=std::path::PathBuf::from(std::env::var_os("VOXEL_COURIER_CHECK_ROOT").expect("Explicit disposable root"));
    assert!(p.is_absolute());
    assert_eq!(storage::save_directory(),p.join("disposable-saves"));
    p
}

#[test]
#[ignore = "Requires a copied completed colony and explicit disposable save root"]
fn native_service_throughput() {
    super::super::routine::check::services::run(&root()).unwrap();
}

#[test]
#[ignore = "Requires a copied completed colony and explicit disposable save root"]
fn native_completed_daily() {
    super::super::routine::check::run(&root()).unwrap();
}

#[test]
#[ignore = "Requires a copied colony and explicit disposable save root"]
fn native_legacy_growth() {
    super::super::routine::check::neighborhood::run(&root()).unwrap();
}

#[test]
#[ignore = "Requires an explicit disposable save root"]
fn native_groups() {
    super::super::groups::check::run(&root()).unwrap();
}

fn identity(w:&World)->serde_json::Value {
    let j=&w.farming.jobs;
    let primary=j.stockpile.unwrap();
    let hub=frontier::hub_plan(j).unwrap().stockpile;
    serde_json::json!({"jobs":ron::to_string(j).unwrap(),
        "primary":w.block_data.chest(primary.into()),"hub":w.block_data.chest(hub),
        "roster_bits":crew::saved_workers(w).iter().map(|(id,p)|(*id,p.to_array().map(f32::to_bits))).collect::<Vec<_>>(),
        "supply_counts":crew::loads(j).filter(|a|matches!(a.cargo,Some(Cargo::Supply{..})))
            .map(|a|a.carry.unwrap().count).collect::<Vec<_>>()})
}

fn copy_tree(from:&Path,to:&Path)->std::io::Result<()> {
    std::fs::create_dir(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry=entry?;let kind=entry.file_type()?;
        if kind.is_symlink() {return Err(std::io::Error::other("Refusing linked test checkpoint"));}
        let dest=to.join(entry.file_name());
        if kind.is_dir() {copy_tree(&entry.path(),&dest)?;} else {std::fs::copy(entry.path(),dest)?;}
    }
    Ok(())
}

pub(crate) fn capture(root:&Path,w:&World)->anyhow::Result<()> {
    if std::env::var_os("VOXEL_COURIER_CHECK_ROOT").is_none() {return Ok(());}
    let target=root.join("old-reader-checkpoint");
    anyhow::ensure!(!target.exists(),"Cannot overwrite compatibility checkpoint");
    std::fs::create_dir(&target)?;
    copy_tree(&root.join("disposable-saves"),&target.join("disposable-saves"))?;
    std::fs::write(target.join("expected.json"),serde_json::to_vec_pretty(&identity(w))?)?;
    let chunks:Vec<_>=w.chunks.keys().map(|p|p.to_array()).collect();
    std::fs::write(target.join("chunks.json"),serde_json::to_vec(&chunks)?)?;
    std::fs::write(target.join("farming.vxf"),sidecar::encode(w))?;
    Ok(())
}

#[test]
#[ignore = "Reads only a captured disposable courier checkpoint"]
fn native_old_reader_checkpoint() {
    let root=root();
    let expected:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("expected.json")).unwrap()).unwrap();
    let bytes=std::fs::read(root.join("farming.vxf")).unwrap();
    let (side,_)=sidecar::decode(&bytes).unwrap();
    assert_eq!(ron::to_string(&side.jobs).unwrap(),expected["jobs"].as_str().unwrap());
    let mut loaded=storage::load_review_checkpoint().unwrap();
    let chunks:Vec<[i32;3]>=serde_json::from_slice(&std::fs::read(root.join("chunks.json")).unwrap()).unwrap();
    for p in chunks {assert!(loaded.world.admit_edit_chunk(p.into()));}
    assert!(loaded.world.farming.jobs.valid());
    assert_eq!(identity(&loaded.world),expected);
    std::fs::write(root.join("old-reader-result.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"format":storage::SAVE_VERSION,"supply_counts":expected["supply_counts"],
        "exact_jobs_roster_chests":true,"sidecar_decoded":true})).unwrap()).unwrap();
}


#[test]
fn larger_supply_cargo_preserves_partial_store_reload_and_death() {
    let mut w=World::with_seed(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter,7);
    let y=engine_worldgen::terrain_height(&w.sample_config(),0,0)+3;
    let source=IVec3::new(2,y,2);let target=IVec3::new(5,y,2);
    w.set(source,B::Chest);w.set(target,B::Chest);
    let item=Item::Block(B::Planks);let cap=w.inventory.stack_limit(item);
    assert!(cap>=16);
    let mut supplied=ItemStack::new(item,16);
    block_data::store_hauled(&mut w,source,&mut supplied);assert_eq!(supplied.count,0);
    let taken=block_data::take_hauled(&mut w,source,item,16);assert_eq!(taken,16);
    let purpose=Cargo::Supply{source:source.to_array(),target:target.to_array(),project:10};
    w.farming.jobs=State{worker:Some(17),stockpile:Some(source.to_array()),
        carry:Some(ItemStack::new(item,taken)),cargo:Some(purpose),..State::default()};
    let mut slots=vec![Some(ItemStack::new(item,cap));w.block_data.chest(target).len()];
    slots[0]=Some(ItemStack::new(item,cap-3));w.block_data.set_chest(target,slots);
    let before=production::count(&w.block_data.chest(target),item);
    let mut cargo=w.farming.jobs.carry.take().unwrap();
    block_data::store_hauled(&mut w,target,&mut cargo);assert_eq!(cargo.count,13);
    block_data::store_hauled(&mut w,target,&mut cargo);assert_eq!(cargo.count,13);
    assert_eq!(production::count(&w.block_data.chest(target),item)+cargo.count,before+16);
    w.farming.jobs.carry=Some(cargo);
    let (loaded,_)=sidecar::decode(&sidecar::encode(&w)).unwrap();w.farming=loaded;
    assert_eq!(w.farming.jobs.carry,Some(cargo));assert_eq!(w.farming.jobs.cargo,Some(purpose));
    let feet=source.as_vec3();super::super::worker_fell(&mut w,17,feet);
    assert!(w.farming.jobs.carry.is_none());assert!(w.farming.jobs.cargo.is_none());
    assert_eq!(w.inventory.dropped.len(),1);assert_eq!(w.inventory.dropped[0].stack,cargo);
    assert_eq!(production::count(&w.block_data.chest(source),item),0);
    assert_eq!(production::count(&w.block_data.chest(target),item)+w.inventory.dropped[0].stack.count,before+16);
}


#[test]
#[ignore = "Synthetic bounds use a copied paid hub and real transport execute"]
fn native_pickup_boundaries() {
    let root=root();
    let chunks:Vec<[i32;3]>=serde_json::from_slice(&std::fs::read(root.join("chunks.json")).unwrap()).unwrap();
    for (demand,source_units,room_units,expected) in [(3,5,7,3),(13,5,7,5),(13,15,7,7)] {
        let mut w=storage::load_review_checkpoint().unwrap().world;
        for p in &chunks {assert!(w.admit_edit_chunk((*p).into()));}
        let mut j=std::mem::take(&mut w.farming.jobs);
        let plan=active(&j).unwrap();let item=Item::Block(B::Planks);
        let cap=w.inventory.stack_limit(item);let held=cap-room_units;
        let chosen:Vec<_>=j.quarter.as_ref().unwrap().frontier.as_ref().unwrap().projects[plan.project]
            .cells.iter().copied().filter(|o|o.material()==Some(item)&&known(&w,o.cell())!=Some(o.block()))
            .take((held+demand) as usize).collect();
        assert_eq!(chosen.len(),(held+demand) as usize);
        j.quarter.as_mut().unwrap().frontier.as_mut().unwrap().projects[plan.project].cells=chosen;
        j.worker=Some(0);j.helpers.clear();j.claims.clear();j.carry=None;j.cargo=None;
        j.shopping=None;j.sowing=None;j.building_material=false;j.reset_task();clear_frame(&mut j);
        let mut source=vec![None;w.block_data.chest(plan.primary).len()];
        source[0]=Some(ItemStack::new(item,source_units));w.block_data.set_chest(plan.primary,source);
        let mut target=vec![Some(ItemStack::new(Item::Block(B::Sand),cap));w.block_data.chest(plan.stockpile).len()];
        target[0]=Some(ItemStack::new(item,held));w.block_data.set_chest(plan.stockpile,target.clone());
        w.adventure.enemies.clear();let feet=plan.primary.as_vec3()+Vec3::new(1.5,0.0,0.5);
        vehicle::spawn_resident(&mut w,0,feet+Vec3::X*30.0);
        let task=Task::Supply(plan.primary,item);
        assert_eq!(transfer_needed(&w,&j,item),demand);assert!(task_valid(&w,&j,task));
        execute(&mut w,&mut j,task);
        assert!(j.carry.is_none());assert_eq!(production::count(&w.block_data.chest(plan.primary),item),source_units);
        w.adventure.enemies[0].pos=feet;assert!(reachable(&w,feet,task,w.voxel_scale.meters()));
        execute(&mut w,&mut j,task);
        assert_eq!(j.carry,Some(ItemStack::new(item,expected)));
        assert_eq!(production::count(&w.block_data.chest(plan.primary),item)+expected,source_units);
        assert_eq!(w.block_data.chest(plan.stockpile),target);
        let cargo=j.carry;execute(&mut w,&mut j,task);assert_eq!(j.carry,cargo);
        assert_eq!(production::count(&w.block_data.chest(plan.primary),item)+expected,source_units);
    }
}
