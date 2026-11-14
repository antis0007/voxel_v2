//! All expensive scalar samples and stored reads remain on this one worker.
use super::*;
use crate::world::regions::{RegionView, Record};
pub(super) type SourceBlocks = [BlockId];
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RecordKey { version: u64, generation: u64, offset: u64, len: u32, checksum: u64 }
impl From<&Record> for RecordKey {
    fn from(r: &Record) -> Self { Self { version:r.version,generation:r.generation,offset:r.offset,len:r.len,checksum:r.checksum } }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub instance:u64, pub cfg:WorldSampleConfig, pub coord:IVec3, pub record:Option<RecordKey>, pub point:Option<IVec3>,
}
impl Key {
    pub fn at_position(world:&World,p:IVec3)->Self {
        let coord=chunk_coord_from_world_block(p);
        let record=world.regions.view.records.get(&coord).map(RecordKey::from);
        let point=record.is_none().then_some(p);
        Self{instance:world.regions.instance_id,cfg:world.sample_config(),coord,record,point}
    }
    #[cfg(test)]
    pub fn at(world:&World,c:IVec3)->Self{Self::at_position(world,c*super::super::CHUNK_SIZE)}
    pub fn address(&self)->(bool,IVec3){match self.point{Some(p)=>(true,p),None=>(false,self.coord)}}
    pub fn edge(&self)->i32{if self.point.is_some(){1}else{super::super::CHUNK_SIZE}}
    pub fn origin(&self)->IVec3{self.point.unwrap_or(self.coord*super::super::CHUNK_SIZE)}
    pub fn voxels(&self)->usize{let n=self.edge() as usize;n*n*n}
    pub fn local_index(&self,p:IVec3)->usize{if self.point.is_some(){0}else{super::super::ChunkData::local_index(p.x,p.y,p.z)}}
    pub fn is_current(&self,world:&World)->bool{*self==Self::at_position(world,self.origin())}
}
#[derive(Default,Debug,Clone,Copy)]
pub(super) struct Work { pub procedural_samples:u64, pub stored_reads:u64 }
#[cfg(test)]
pub(super) struct ReviewGate { pub entered:mpsc::SyncSender<()>,pub release:mpsc::Receiver<()> }
pub(super) struct Job {
    pub keys:Vec<Key>,pub generation:u64,pub view:RegionView,pub cancel:Arc<AtomicBool>,
    #[cfg(test)] pub review_gate:Option<ReviewGate>,
}
pub(super) struct Done {
    pub generation:u64,pub items:Vec<(Key,Result<Arc<SourceBlocks>,String>)>,pub work:Work,
}
pub(super) fn start()->Result<(mpsc::SyncSender<Job>,mpsc::Receiver<Done>),String>{
    let (tx,jobs)=mpsc::sync_channel::<Job>(1);let (done,rx)=mpsc::sync_channel(1);
    std::thread::Builder::new().name("readonly-picking-source".into()).spawn(move||{
        while let Ok(job)=jobs.recv(){
            #[cfg(test)]
            if let Some(gate)=&job.review_gate{
                let _=gate.entered.send(());let _=gate.release.recv_timeout(std::time::Duration::from_secs(15));
            }
            let mut items=Vec::new();let mut work=Work::default();
            let bounded=job.keys.len()<=MAX_BATCH && job.keys.iter().try_fold(0usize,|sum,key|
                sum.checked_add(key.voxels()*std::mem::size_of::<BlockId>())).is_some_and(|bytes|bytes<=CACHE_BYTES);
            for key in &job.keys {
                if job.cancel.load(Ordering::Relaxed){break;}
                let data=if !bounded {Err("Read-only source batch exceeds its bounded reservation".into())}
                else{std::panic::catch_unwind(std::panic::AssertUnwindSafe(||load(key,&job.view,&mut work)))
                    .unwrap_or_else(|_|Err("Read-only source sampling panicked".into()))};
                let failed=data.is_err();items.push((key.clone(),data));if failed{break;}
            }
            // Obsolete partial results are discarded, but real work remains counted.
            if job.cancel.load(Ordering::Relaxed){items.clear();}
            if done.send(Done{generation:job.generation,items,work}).is_err(){break;}
        }
    }).map_err(|e|format!("Read-only source worker: {e}"))?;Ok((tx,rx))
}
fn load(key:&Key,view:&RegionView,work:&mut Work)->Result<Arc<SourceBlocks>,String>{
    if let Some(p)=key.point {
        work.procedural_samples+=1;
        return Ok(Arc::from([engine_worldgen::sample_block(&key.cfg,p)]));
    }
    work.stored_reads+=1;
    let data=view.load(key.coord).map_err(|e|format!("Read-only stored source: {e:#}"))?
        .ok_or_else(||"Read-only stored source is missing".to_string())?;
    Ok(Arc::from(data.chunk.blocks.as_slice()))
}
