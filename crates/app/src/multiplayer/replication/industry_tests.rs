use super::*;
use crate::world::industry::{State, STEAM};
use crate::world::factory::{self, Kind};
use std::time::{Duration, Instant};
const POS: IVec3 = IVec3::new(-16,64,-16);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }
fn bytes(s:&State)->Vec<u8>{let mut b=Vec::new();s.encode(&mut b);b}
fn send(w:&mut World,m:&mut Mirror,image:&mut FactoryImage,now:Instant)->Vec<u8>{
    image.refresh(w,now).unwrap();capture_shared(w,&Player::new(),&DayNightCycle::default(),0,Vec::new(),m,image).unwrap()
}
fn receive(w:&mut World,b:&[u8]){apply(b,w,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new()).unwrap();}
#[test]
fn ambient_cached_factory_release_reaches_two_peers_and_reconnect() {
    let mut host=world();let n=factory::fixture_place(&mut host,POS,Kind::Chimney).unwrap();
    n.exhaust[STEAM]=77;
    let mut peers=[(world(),Mirror::default()),(world(),Mirror::default())];let mut image=FactoryImage::default();let now=Instant::now();
    for (w,m) in &mut peers {receive(w,&send(&mut host,m,&mut image,now));assert_eq!(w.factory.nodes[&POS].exhaust[STEAM],77);}
    let mut state=std::mem::take(&mut host.industry);factory::industry::route(&mut host,&mut state);host.industry=state;
    assert_eq!(host.factory.nodes[&POS].exhaust[STEAM],0);
    assert_eq!(host.industry.air.values().map(|m|m[STEAM]).sum::<u64>(),77);
    for (w,m) in &mut peers {
        receive(w,&send(&mut host,m,&mut image,now+Duration::from_millis(100)));
        assert!(w.industry.air.is_empty());assert_eq!(w.factory.nodes[&POS].exhaust[STEAM],77);
    }
    for (w,m) in &mut peers {
        receive(w,&send(&mut host,m,&mut image,now+Duration::from_millis(250)));
        assert_eq!(bytes(&w.industry),bytes(&host.industry));assert_eq!(w.factory.nodes[&POS].exhaust[STEAM],0);
    }
    peers[1]=(world(),Mirror::default());let (w,m)=&mut peers[1];receive(w,&send(&mut host,m,&mut image,now+Duration::from_millis(251)));
    assert_eq!(bytes(&peers[1].0.industry),bytes(&host.industry));
    host.industry=State::default();image.invalidate();
    for (w,m) in &mut peers {m.refresh();receive(w,&send(&mut host,m,&mut image,now+Duration::from_millis(252)));assert!(w.industry.air.is_empty());}
}
#[test]
fn ambient_in_flight_delta_is_immutable_and_different_peer_baselines_converge() {
    let mut host=world();let mut a=world();let mut b=world();let mut ma=Mirror::default();let mut mb=Mirror::default();let mut image=FactoryImage::default();let now=Instant::now();
    host.industry.air.insert((-1,64,0,1),[1;6]);let first=send(&mut host,&mut ma,&mut image,now);
    host.industry.air.insert((-1,64,0,1),[9;6]);host.industry.deposits.insert((-1,63,0),[7;6]);image.invalidate();
    receive(&mut b,&send(&mut host,&mut mb,&mut image,now));receive(&mut a,&first);
    assert_eq!(a.industry.air[&(-1,64,0,1)],[1;6]);assert_eq!(b.industry.air[&(-1,64,0,1)],[9;6]);
    receive(&mut a,&send(&mut host,&mut ma,&mut image,now));assert_eq!(bytes(&a.industry),bytes(&b.industry));
    host.industry.air.clear();host.industry.deposits.clear();image.invalidate();
    receive(&mut a,&send(&mut host,&mut ma,&mut image,now));receive(&mut b,&send(&mut host,&mut mb,&mut image,now));
    assert_eq!(bytes(&a.industry),bytes(&host.industry));assert_eq!(bytes(&b.industry),bytes(&host.industry));
}
#[test]
fn ambient_malformed_section_keeps_every_component_unpublished() {
    let mut host=world();let mut peer=world();host.industry.air.insert((0,64,0,1),[7;6]);
    host.inventory.try_insert(world::inventory::ItemStack::new(world::inventory::Item::Resource(0),3));
    let image=send(&mut host,&mut Mirror::default(),&mut FactoryImage::default(),Instant::now());
    let mut raw=Vec::new();flate2::read::ZlibDecoder::new(&image[..]).read_to_end(&mut raw).unwrap();
    // INDUSTRY is last; corrupt its final deletion count without touching prior sections.
    *raw.last_mut().unwrap()=1;
    let mut encoder=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::fast());encoder.write_all(&raw).unwrap();let corrupt=encoder.finish().unwrap();
    assert!(apply(&corrupt,&mut peer,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new()).is_err());
    assert!(peer.industry.air.is_empty());assert_eq!(peer.inventory.count_item(world::inventory::Item::Resource(0)),0);
    raw[0]=11;let mut e=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::fast());e.write_all(&raw).unwrap();
    assert!(prepare(&e.finish().unwrap(),&peer).err().unwrap().to_string().contains("version mismatch"));
}
#[test]
fn ambient_terrain_transaction_publishes_only_after_staging_and_rejects_replay() {
    let (mut host,mut peer)=(world(),world());let coord=IVec3::new(0,4,0);
    let base=std::sync::Arc::new(world::ChunkData{blocks:Box::new([BlockId::Stone;32768]),light:Box::new([0;32768]),torches:HashSet::new(),modified_bounds:None,structural_bounds:None});
    host.chunks.insert(coord,base.clone());peer.chunks.insert(coord,base);
    let cells:Vec<_>=(0..12000).map(|i|([i%32,128+i/1024,(i/32)%32],BlockId::Air.as_u32())).collect();
    host.industry.air.insert((0,128,0,1),[13;6]);let now=Instant::now();let mut image=FactoryImage::default();image.refresh(&mut host,now).unwrap();
    let frame=terrain::Frame{cells,chunk:None};let packet=capture_frame(&host,&Player::new(),&DayNightCycle::default(),0,Vec::new(),&mut Mirror::default(),&image,1,&frame).unwrap();
    let mut queue=super::super::replica::Queue::default();
    queue.stage(prepare(&packet,&peer).unwrap(),now,0).unwrap();assert!(peer.industry.air.is_empty());
    let mut polls=0;loop {polls+=1;let result=queue.drain(&mut peer,&mut Player::new(),&mut DayNightCycle::default(),&mut HashSet::new(),&mut HashSet::new(),now).unwrap();
        if result.is_some(){break;}assert!(peer.industry.air.is_empty());assert_eq!(peer.get(IVec3::new(0,128,0)),BlockId::Stone);assert!(polls<200);}
    assert!(polls>1);assert_eq!(bytes(&peer.industry),bytes(&host.industry));assert_eq!(peer.get(IVec3::new(0,128,0)),BlockId::Air);
    queue.receipt_sent();assert!(queue.stage(prepare(&packet,&peer).unwrap(),now,0).is_err());
}
