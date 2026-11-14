use super::*;

#[test]
#[ignore = "wall-clock coherent frame costs; run without concurrent builds"]
fn coherent_frame_dense_sparse_cost() {
    for (label,count,sparse,machines) in [("dense4k",4096,false,32),("dense64k",65536,false,128),
        ("dense256k",262144,false,512),("sparse64",64,true,32),("sparse512",512,true,512)] {
        let mut rows=Vec::new();let mut wire_bytes=0;let mut peak=0;
        for _ in 0..5 {
            let (mut host,mut peer)=(world(),world());
            for i in 0..machines {factory::fixture_place(&mut host,MACHINE-IVec3::X*(i+2),Kind::Crusher).unwrap();}
            let points:Vec<_>=(0..count).map(|i|if sparse {IVec3::new(i*64+16,176,16)}else{
                let chunk=i/32768;let local=i%32768;IVec3::new(chunk*32+local%32,160+local/1024,(local/32)%32)
            }).collect();
            let mut bases=BTreeMap::new();
            for &p in &points {
                let coord=world::chunk_coord_from_world_block(p);
                let base=bases.entry(coord.to_array()).or_insert_with(||data(BlockId::Air));
                Arc::make_mut(base).blocks[world::ChunkData::local_index(p.x,p.y,p.z)]=BlockId::Stone;
                if sparse {let lamp=p+IVec3::X;let b=Arc::make_mut(base);b.blocks[world::ChunkData::local_index(lamp.x,lamp.y,lamp.z)]=BlockId::Torch;b.torches.insert(lamp);}
            }
            for (coord,base) in bases {host.chunks.insert(IVec3::from_array(coord),base.clone());peer.chunks.insert(IVec3::from_array(coord),base);}
            host.set_many(&points.iter().map(|&p|(p,BlockId::Air)).collect::<Vec<_>>());
            let frame=replication::terrain::Frame{cells:points.iter().map(|p|(p.to_array(),BlockId::Air.as_u32())).collect(),chunk:None};
            let now=Instant::now();let start=Instant::now();let bytes=image(&mut host,1,0,&frame,&mut Default::default(),&mut Default::default(),now);let encode=start.elapsed();wire_bytes=bytes.len();
            let start=Instant::now();let prepared=replication::prepare(&bytes,&peer).unwrap();let decode=start.elapsed();
            let mut q=Queue::default();let start=Instant::now();q.stage(prepared,now,0).unwrap();let grouping=start.elapsed();
            let mut stage=Duration::ZERO;let mut polls=0;let commit;
            loop {polls+=1;let start=Instant::now();let done=drain(&mut q,&mut peer,Instant::now());let elapsed=start.elapsed();
                peak=peak.max(q.staged_peak);if done.is_some(){commit=elapsed;break;}stage+=elapsed;assert!(polls<2000);
            }
            assert!(points.iter().all(|p|peer.get(*p)==BlockId::Air));
            rows.push([encode.as_secs_f64()*1e3,decode.as_secs_f64()*1e3,grouping.as_secs_f64()*1e3,stage.as_secs_f64()*1e3,commit.as_secs_f64()*1e3,polls as f64]);
        }
        for i in 0..6 {rows.sort_by(|a,b|a[i].total_cmp(&b[i]));eprintln!("[coherent-frame-cost] case={label} cells={count} machines={machines} metric={} median={:.3} max={:.3} unit={} bytes={wire_bytes} staged_peak={peak}",
            ["encode","decode","group","stage_total","final_poll_commit","poll_count"][i],rows[2][i],rows[4][i],if i==5 {"polls"} else {"ms"});}
    }
}
