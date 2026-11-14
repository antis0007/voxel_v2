use super::*;
#[test]
fn deferred_chunk_freezes_held_edits_and_keeps_later_generation_separate() {
    let mut w=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);let p=IVec3::new(-1,128,0);w.set(p,BlockId::Stone);
    let coord=world::chunk_coord_from_world_block(p).to_array();let mut terrain=Terrain::new([coord].into_iter().collect());
    let mut cells=BTreeMap::from([(p.to_array(),BlockId::Dirt.as_u32())]);
    terrain.start(&mut w,p.as_vec3(),&mut cells).unwrap();assert!(cells.is_empty());
    w.set(p,BlockId::Air);assert!(terrain.defer_edit(p.to_array(),BlockId::Air.as_u32()));
    let began=Instant::now();while terrain.prepared.is_none(){terrain.poll().unwrap();assert!(began.elapsed()<Duration::from_secs(5));std::thread::sleep(Duration::from_millis(1));}
    let chunk=terrain.take(&mut cells).unwrap();assert!(terrain.awaiting_ack);assert_eq!(cells[&p.to_array()],BlockId::Air.as_u32());
    let index=world::ChunkData::local_index(p.x,p.y,p.z);let mut at=0;let mut captured=None;
    for (count,block) in chunk.runs {if index>=at&&index<at+usize::from(count){captured=Some(block);}at+=usize::from(count);}
    assert_eq!(captured,Some(BlockId::Stone.as_u32()));
    let frozen=std::mem::take(&mut cells);w.set(p,BlockId::Dirt);assert!(!terrain.defer_edit(p.to_array(),BlockId::Dirt.as_u32()));cells.insert(p.to_array(),BlockId::Dirt.as_u32());
    assert_eq!(frozen[&p.to_array()],BlockId::Air.as_u32());assert_eq!(cells[&p.to_array()],BlockId::Dirt.as_u32());
    terrain.acknowledge().unwrap();assert!(!terrain.awaiting_ack);assert!(terrain.acknowledge().is_err());
}
