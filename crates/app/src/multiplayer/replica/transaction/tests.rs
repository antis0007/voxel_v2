use super::*;
use crate::world::factory::{self, Kind};
use crate::world::inventory::Item;
use std::sync::Arc;
const MACHINE: IVec3 = IVec3::new(-16,128,-16);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }
fn data(block: BlockId) -> Arc<world::ChunkData> {
    Arc::new(world::ChunkData { blocks:Box::new([block;32768]), light:Box::new([0;32768]),
        torches:HashSet::new(), modified_bounds:None, structural_bounds:None })
}
fn image(w: &mut World, sequence:u64, ack:u64, frame:&replication::terrain::Frame,
    mirror:&mut replication::Mirror, cache:&mut replication::FactoryImage, now:Instant) -> Vec<u8> {
    cache.refresh(w, now).unwrap();
    replication::capture_frame(w,&Player::new(),&DayNightCycle::default(),ack,Vec::new(),mirror,cache,sequence,frame).unwrap()
}
fn drain(q:&mut Queue,w:&mut World,now:Instant)->Option<replication::Applied> {
    q.drain(w,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new(),now).unwrap()
}
#[test]
fn fragmented_excavation_and_large_edits_publish_in_one_frame() {
    let (mut host,mut peer)=(world(),world());
    factory::fixture_place(&mut host,MACHINE,Kind::Quarry).unwrap();
    let coord=IVec3::new(0,4,0); let mut base=data(BlockId::Stone);
    Arc::make_mut(&mut base).blocks[0]=BlockId::CopperOre;
    host.chunks.insert(coord,base.clone()); peer.chunks.insert(coord,base);
    let now=Instant::now(); let mut cache=replication::FactoryImage::default();let mut mirror=replication::Mirror::default();
    let baseline=image(&mut host,0,0,&Default::default(),&mut mirror,&mut cache,now);
    replication::apply(&baseline,&mut peer,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new()).unwrap();
    let cells:Vec<_>=(0..12000).map(|i|(IVec3::new(i%32,128+i/1024,(i/32)%32),BlockId::Air)).collect();
    host.set_many(&cells); assert!(cells.iter().all(|(p,_)|host.get(*p)==BlockId::Air));
    let ore=crate::world::factory::blocks::Material::from_block(BlockId::CopperOre).unwrap().index();
    let n=host.factory.nodes.get_mut(&MACHINE).unwrap();n.excavation=1;n.blocks.output[ore]=1;
    let frame=replication::terrain::Frame {cells:cells.iter().map(|(p,b)|(p.to_array(),b.as_u32())).collect(),chunk:None};
    let bytes=image(&mut host,1,0,&frame,&mut mirror,&mut cache,now+Duration::from_millis(50));
    let mut sender=Sender::new(bytes).unwrap();let mut receiver=Receiver::default();let mut q=Queue::default();
    while let Some(packet)=sender.next() {
        if packet[0] == 1 { q.begin_receive(now).unwrap(); }
        sender.accepted();
        if let Some(bytes)=receiver.receive(&packet).unwrap() {q.stage(replication::prepare(&bytes,&peer).unwrap(),now,0).unwrap();}
        assert_eq!(peer.get(IVec3::new(0,128,0)),BlockId::CopperOre);
        assert_eq!(peer.factory.nodes[&MACHINE].excavation,0);
    }
    let mut polls=0;
    loop {
        polls+=1;
        if drain(&mut q,&mut peer,now).is_some(){break;}
        assert!(polls<100);
        assert_eq!(peer.get(IVec3::new(0,128,0)),BlockId::CopperOre);
        assert_eq!(peer.factory.nodes[&MACHINE].blocks.output[ore],0);
        assert!(cells.iter().skip(1).all(|(p,_)|peer.get(*p)==BlockId::Stone));
    }
    assert!(polls>1); assert_eq!(q.receipt(),Some(1));
    assert!(cells.iter().all(|(p,_)|peer.get(*p)==BlockId::Air));
    assert_eq!(peer.factory.nodes[&MACHINE].excavation,1);assert_eq!(peer.factory.nodes[&MACHINE].blocks.output[ore],1);
}
#[test]
fn frame_sequences_commands_and_timeout_fail_before_mutation() {
    let (mut host,mut peer)=(world(),world());let now=Instant::now();let mut cache=Default::default();let mut mirror=Default::default();
    host.inventory.try_insert(crate::world::inventory::ItemStack::new(Item::Resource(0),1));
    let bytes=image(&mut host,1,2,&Default::default(),&mut mirror,&mut cache,now);
    let mut q=Queue::default();assert!(q.stage(replication::prepare(&bytes,&peer).unwrap(),now,1).is_err());
    assert_eq!(peer.inventory.count_item(Item::Resource(0)),0);
    q.stage(replication::prepare(&bytes,&peer).unwrap(),now,2).unwrap();
    assert!(q.stage(replication::prepare(&bytes,&peer).unwrap(),now,2).is_err());
    assert!(q.drain(&mut peer,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new(),now+SYNC_TIMEOUT+Duration::from_secs(1)).is_err());
    assert_eq!(peer.inventory.count_item(Item::Resource(0)),0);
}
#[test]
fn deferred_chunk_and_held_water_edit_coalesce_before_publication() {
    let (mut host,mut peer)=(world(),world());let coord=IVec3::new(-1,4,0);let p=coord*32;
    peer.chunks.insert(coord,data(BlockId::Water));peer.water_levels.insert(p,3);
    host.water_levels.insert(p,3);
    let frame=replication::terrain::Frame {cells:vec![(p.to_array(),BlockId::Water.as_u32())],
        chunk:Some(Chunk{coord:coord.to_array(),runs:vec![(32768,BlockId::Air.as_u32())]})};
    let now=Instant::now();let bytes=image(&mut host,1,0,&frame,&mut Default::default(),&mut Default::default(),now);
    let mut q=Queue::default();q.set_missing(BTreeSet::from([coord.to_array()]));q.stage(replication::prepare(&bytes,&peer).unwrap(),now,0).unwrap();
    for _ in 0..200 {if let Some(applied)=drain(&mut q,&mut peer,now){assert!(!applied.water_mismatch);break;}}
    assert_eq!(q.receipt(),Some(1));assert!(!q.missing.contains(&coord.to_array()));
    assert_eq!(peer.get(p),BlockId::Water);assert_eq!(peer.water_levels.get(&p),Some(&3));assert_eq!(peer.get(p+IVec3::X),BlockId::Air);
}
#[test]
fn missing_sources_prepare_without_live_terrain_or_environment_changes() {
    let (mut host,mut peer)=(world(),world());let points:Vec<_>=(0..9).map(|x|IVec3::new(x*32,160,0)).collect();
    let frame=replication::terrain::Frame{cells:points.iter().map(|p|(p.to_array(),BlockId::Stone.as_u32())).collect(),chunk:None};
    let now=Instant::now();let bytes=image(&mut host,1,0,&frame,&mut Default::default(),&mut Default::default(),now);
    let mut q=Queue::default();q.stage(replication::prepare(&bytes,&peer).unwrap(),now,0).unwrap();
    let before=peer.chunks.len();let revision=peer.revision();
    loop {
        if drain(&mut q,&mut peer,Instant::now()).is_some(){break;}
        assert_eq!(peer.chunks.len(),before);assert_eq!(peer.revision(),revision);
        assert!(points.iter().all(|p|peer.get(*p)==BlockId::Air));
        assert!(now.elapsed()<Duration::from_secs(10));std::thread::sleep(Duration::from_millis(2));
    }
    assert!(points.iter().all(|p|peer.get(*p)==BlockId::Stone));
}

#[test]
fn later_rewrite_waits_for_prior_publication_and_receipt() {
    let (mut host,mut peer)=(world(),world());let p=IVec3::new(0,160,0);let coord=world::chunk_coord_from_world_block(p);
    peer.chunks.insert(coord,data(BlockId::Air));let now=Instant::now();let mut mirror=Default::default();let mut cache=Default::default();
    let first=replication::terrain::Frame{cells:vec![(p.to_array(),BlockId::Stone.as_u32())],chunk:None};
    let a=image(&mut host,1,0,&first,&mut mirror,&mut cache,now);
    let second=replication::terrain::Frame{cells:vec![(p.to_array(),BlockId::Dirt.as_u32())],chunk:None};
    let b=image(&mut host,2,0,&second,&mut mirror,&mut cache,now);
    let mut q=Queue::default();q.stage(replication::prepare(&a,&peer).unwrap(),now,0).unwrap();
    assert!(q.stage(replication::prepare(&b,&peer).unwrap(),now,0).is_err());
    assert_eq!(peer.get(p),BlockId::Air);assert!(drain(&mut q,&mut peer,now).is_some());
    assert_eq!(peer.get(p),BlockId::Stone);assert!(q.stage(replication::prepare(&b,&peer).unwrap(),now,0).is_err());
    q.receipt_sent();q.stage(replication::prepare(&b,&peer).unwrap(),now,0).unwrap();
    assert!(drain(&mut q,&mut peer,now).is_some());assert_eq!(peer.get(p),BlockId::Dirt);assert_eq!(q.receipt(),Some(2));
}
#[test]
fn staging_overload_keeps_terrain_gameplay_and_ack_unpublished() {
    let (mut host,mut peer)=(world(),world());let p=IVec3::new(0,160,0);peer.chunks.insert(world::chunk_coord_from_world_block(p),data(BlockId::Air));
    host.inventory.try_insert(crate::world::inventory::ItemStack::new(Item::Resource(0),1));
    let frame=replication::terrain::Frame{cells:vec![(p.to_array(),BlockId::Stone.as_u32())],chunk:None};let now=Instant::now();
    let bytes=image(&mut host,1,0,&frame,&mut Default::default(),&mut Default::default(),now);
    let mut q=Queue::default();q.stage(replication::prepare(&bytes,&peer).unwrap(),now,0).unwrap();
    q.pending.as_mut().unwrap().bytes=replication::terrain::MAX_STAGED_BYTES;
    assert!(q.drain(&mut peer,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new(),now).is_err());
    assert_eq!(peer.get(p),BlockId::Air);assert_eq!(peer.inventory.count_item(Item::Resource(0)),0);assert_eq!(q.receipt(),None);
}

mod perf;

#[test]
fn incomplete_image_transfer_is_bounded_and_cannot_restart_its_timer() {
    let mut q=Queue::default();let mut peer=world();let now=Instant::now();q.begin_receive(now).unwrap();
    assert!(q.begin_receive(now+Duration::from_secs(1)).is_err());
    assert!(q.drain(&mut peer,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new(),now+SYNC_TIMEOUT+Duration::from_secs(1)).is_err());
    assert_eq!(q.receipt(),None);assert_eq!(peer.revision(),0);
}

#[test]
fn final_fragment_after_receive_deadline_cannot_reset_timeout() {
    let (mut host, peer) = (world(), world());
    let now = Instant::now();
    host.inventory.try_insert(crate::world::inventory::ItemStack::new(Item::Resource(0), 1));
    let bytes = image(&mut host, 1, 0, &Default::default(), &mut Default::default(), &mut Default::default(), now);
    let mut q = Queue::default();
    q.begin_receive(now).unwrap();
    assert!(q.stage(replication::prepare(&bytes, &peer).unwrap(), now + SYNC_TIMEOUT + Duration::from_millis(1), 0).is_err());
    assert!(q.pending.is_none());
    assert_eq!(q.receiving, Some(now));
    assert_eq!(q.receipt(), None);
    assert_eq!(peer.inventory.count_item(Item::Resource(0)), 0);
    assert_eq!(peer.revision(), 0);
}
