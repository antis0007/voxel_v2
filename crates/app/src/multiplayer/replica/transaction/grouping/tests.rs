use super::*;
use crate::multiplayer::{Chunk, MAX_EDITS};
use std::{mem::size_of, time::Instant};
mod reference;
mod perf;

type Frame = replication::terrain::Frame;
fn copy(frame: &Frame) -> Frame {
    Frame {
        cells: frame.cells.clone(),
        chunk: frame.chunk.as_ref().map(|c| Chunk {coord:c.coord, runs:c.runs.clone()}),
    }
}
fn dense(count: usize) -> Frame {
    let cells=(0..count).map(|i| {
        let j=(i*4051)%count;
        let local=j%32768;
        let p=[(j/32768) as i32*32-128+(local%32) as i32,160+(local/1024) as i32,(local/32%32) as i32-32];
        let b=if j%17==0 {BlockId::Water} else if j%7==0 {BlockId::Stone} else {BlockId::Air};
        (p,b.as_u32())
    }).collect();
    Frame {cells,chunk:None}
}
fn sparse() -> Frame {
    Frame {cells:(0..512).rev().map(|i|([i*64-16384,176,-1],BlockId::Stone.as_u32())).collect(),chunk:None}
}
fn full(alternating: bool) -> Frame {
    let coord=[-2,5,-1]; let base=IVec3::from_array(coord)*32;
    let runs=if alternating {(0..32768).map(|i|(1,(if i%2==0 {BlockId::Water} else {BlockId::Stone}).as_u32())).collect()}
        else {vec![(17,BlockId::Water.as_u32()),(1000,BlockId::Stone.as_u32()),(31751,BlockId::Air.as_u32())]};
    let cells=[(IVec3::ZERO,BlockId::Air),(IVec3::splat(31),BlockId::Torch),
        (IVec3::new(17,0,0),BlockId::Water),(IVec3::new(31,1,31),BlockId::Stone),
        (IVec3::new(-1,0,0),BlockId::Water),(IVec3::new(32,31,31),BlockId::Dirt)]
        .into_iter().rev().map(|(p,b)|((base+p).to_array(),b.as_u32())).collect();
    Frame {cells,chunk:Some(Chunk {coord,runs})}
}
fn missing(frame: &Frame) -> BTreeSet<[i32;3]> {
    frame.chunk.as_ref().map(|c|c.coord).into_iter().collect()
}
fn assert_groups(actual: &VecDeque<Group>, expected: &VecDeque<Group>) {
    assert_eq!(actual.len(),expected.len());
    assert_eq!(actual.capacity(),expected.capacity());
    for (a,b) in actual.iter().zip(expected) {
        assert_eq!(a.coord,b.coord);
        assert_eq!(a.edits.len(),b.edits.len());
        assert_eq!(a.edits.capacity(),b.edits.capacity(),"staging allocation at {:?}",a.coord);
        assert_eq!(a.next,0);assert!(a.staged.is_none());
        for (index,(a,b)) in a.edits.iter().zip(&b.edits).enumerate() {assert_eq!(a,b,"edit {index}");}
    }
}
fn prepared(frame: Frame) -> replication::Prepared {
    replication::Prepared {sequence:1,command_ack:0,terrain:frame,
        publish:Box::new(|_,_,_,_,_|panic!("staging must not publish"))}
}

#[test]
fn chunk_buckets_match_tree_order_and_capacity_for_shuffled_dense_and_sparse_frames() {
    for frame in [dense(0),dense(1),dense(3),dense(5),dense(4097),dense(512),dense(65536),dense(MAX_EDITS),sparse()] {
        frame.validate().unwrap();let missing=missing(&frame);
        let expected=reference::group(copy(&frame),&missing).unwrap();
        let actual=prepare(frame,&missing).unwrap();
        assert_eq!(actual.0,expected.0);assert_groups(&actual.1,&expected.1);
    }
}
#[test]
fn full_chunk_overrides_match_tree_without_intermediate_cell_writes() {
    for frame in [full(false),full(true),{let mut f=full(false);f.cells.clear();f}] {
        frame.validate().unwrap();let missing=missing(&frame);
        let expected=reference::group(copy(&frame),&missing).unwrap();
        let actual=prepare(frame,&missing).unwrap();
        assert_eq!(actual.0,expected.0);assert_groups(&actual.1,&expected.1);
        let coord=actual.0.unwrap();let group=actual.1.iter().find(|g|g.coord.to_array()==coord).unwrap();
        assert_eq!(group.edits.len(),32768);
        let base=IVec3::from_array(coord)*32;
        assert_eq!(group.edits.first().unwrap().0,base);
        assert_eq!(group.edits.last().unwrap().0,base+IVec3::splat(31));
    }
}
#[test]
fn stage_keeps_atomic_state_and_byte_accounting_for_bucketed_groups() {
    for frame in [dense(MAX_EDITS),sparse(),full(true)] {
        frame.validate().unwrap();let missing=missing(&frame);
        let expected=reference::group(copy(&frame),&missing).unwrap();
        let bytes=size_of::<super::super::Pending>()+expected.1.capacity()*size_of::<Group>()
            +expected.1.iter().map(|g|g.edits.capacity()*size_of::<(IVec3,BlockId)>()).sum::<usize>();
        let now=Instant::now();let mut q=super::super::Queue::default();
        q.set_missing(missing);q.begin_receive(now).unwrap();q.stage(prepared(frame),now,0).unwrap();
        assert_eq!(q.receiving,None);assert_eq!(q.receipt(),None);assert_eq!(q.committed,0);
        let pending=q.pending.as_ref().unwrap();
        assert_eq!(pending.bytes,bytes);assert_eq!(pending.terrain,expected.0);
        assert_groups(&pending.groups,&expected.1);
    }
}
#[test]
fn invalid_frames_and_missing_terrain_leave_receiving_state_unpublished() {
    let cell=([0,160,0],BlockId::Stone.as_u32());
    let mut invalid=vec![Frame{cells:vec![cell,cell],chunk:None},
        Frame{cells:vec![([0,160,0],BlockId::COUNT as u32)],chunk:None},
        Frame{cells:vec![cell;MAX_EDITS+1],chunk:None},
        Frame{cells:Vec::new(),chunk:Some(Chunk{coord:[0,5,0],runs:vec![(32767,BlockId::Air.as_u32())]})}];
    let mut too_many_chunks=sparse();
    too_many_chunks.chunk=Some(Chunk{coord:[20,5,20],runs:vec![(32768,BlockId::Air.as_u32())]});
    invalid.push(too_many_chunks);
    for frame in invalid {
        let now=Instant::now();let mut q=super::super::Queue::default();
        q.set_missing(missing(&frame));q.begin_receive(now).unwrap();
        assert!(q.stage(prepared(frame),now,0).is_err());
        assert!(q.pending.is_none());assert_eq!(q.receiving,Some(now));assert_eq!(q.receipt(),None);assert_eq!(q.committed,0);
    }
    for (frame,missing) in [(full(false),BTreeSet::new()),(Frame{cells:vec![cell],chunk:None},BTreeSet::from([[0,5,0]]))] {
        frame.validate().unwrap();
        let old_error=reference::group(copy(&frame),&missing).err().unwrap().to_string();
        let error=prepare(copy(&frame),&missing).err().unwrap().to_string();
        assert_eq!(error,old_error);
        let now=Instant::now();let mut q=super::super::Queue::default();q.set_missing(missing);q.begin_receive(now).unwrap();
        assert_eq!(q.stage(prepared(frame),now,0).err().unwrap().to_string(),error);
        assert!(q.pending.is_none());assert_eq!(q.receiving,Some(now));assert_eq!(q.receipt(),None);
    }
}


fn assert_equivalent(frame: Frame) {
    frame.validate().unwrap();let missing=missing(&frame);
    let expected=reference::group(copy(&frame),&missing).unwrap();
    let actual=prepare(frame,&missing).unwrap();
    assert_eq!(actual.0,expected.0);assert_groups(&actual.1,&expected.1);
}
#[test]
fn maximum_cells_and_full_chunk_keep_the_512_chunk_boundary() {
    let chunk=||Chunk{coord:[20,5,20],runs:vec![(32768,BlockId::Water.as_u32())]};
    let mut maximum=dense(MAX_EDITS);maximum.chunk=Some(chunk());assert_equivalent(maximum);
    let mut boundary=sparse();boundary.cells.pop();boundary.chunk=Some(chunk());assert_equivalent(boundary);
}
fn seeded(seed: u32) -> Frame {
    fn next(s: &mut u32) -> u32 {*s=s.wrapping_mul(1664525).wrapping_add(1013904223);*s}
    let blocks=[BlockId::Air,BlockId::Stone,BlockId::Water,BlockId::Torch];
    let coord=[-((seed%7) as i32)-5,5,(seed%3) as i32-4];let base=IVec3::from_array(coord)*32;
    let mut state=seed;let mut remaining=32768;let mut runs=Vec::new();
    while remaining>0 {
        let count=(next(&mut state) as usize%1024+1).min(remaining);
        runs.push((count as u16,blocks[next(&mut state) as usize%blocks.len()].as_u32()));remaining-=count;
    }
    let mut cells=Vec::new();
    for i in 0..257 {
        let index=(i*101+seed as usize)&32767;
        let p=base+IVec3::new((index%32) as i32,(index/1024) as i32,(index/32%32) as i32);
        cells.push((p.to_array(),blocks[next(&mut state) as usize%blocks.len()].as_u32()));
    }
    let count=cells.len();cells.rotate_left(seed as usize%count);
    cells.push(((base-IVec3::X).to_array(),BlockId::Water.as_u32()));
    Frame{cells,chunk:Some(Chunk{coord,runs})}
}
#[test]
fn seeded_rle_and_overrides_match_the_tree_reference() {
    for seed in [1,7,31,0xA5A5] {assert_equivalent(seeded(seed));}
}
