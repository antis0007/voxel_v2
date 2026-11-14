//! Nonblocking exact-point preparation for ordinary ray picking. No save/admission.
use super::{BlockId,World,WorldType,WorldSampleConfig,chunk_coord_from_world_block};
use glam::{DVec3,IVec3,Vec3};
use std::sync::{Arc,atomic::{AtomicBool,Ordering},mpsc};
mod source;
mod cache;
mod group;
use source::{Done,Job,Key,SourceBlocks};
use cache::Cache;
const CACHE_CHUNKS:usize=1024; // Source entries: exact points or whole stored chunks.
const CACHE_BYTES:usize=2*1024*1024; // Payload only; metadata also has bounded entry counts.
const MAX_BATCH:usize=512;
const MAX_REACH_BLOCKS:f32=256.0; // Original DDA performs at most447 reads at this reach.
#[derive(Debug,Clone,PartialEq)]
pub(crate) enum Readiness<T>{Ready(T),Pending,Failed(String)}
#[derive(Debug,Clone,Copy,PartialEq)]
pub(crate) struct PickHit{pub cell:IVec3,pub previous:IVec3,pub block:BlockId}
struct Pending{keys:Vec<Key>,generation:u64,cancel:Arc<AtomicBool>}
#[derive(Default,Debug)]
struct Stats{jobs:u64,requested_sources:u64,procedural_samples:u64,stored_reads:u64,completed_sources:u64}
pub(crate) struct ReadOnlyPicker{
    identity:Option<(u64,WorldSampleConfig)>,generation:u64,cache:Cache,failure:Option<(Key,String)>,
    tx:Option<mpsc::SyncSender<Job>>,rx:mpsc::Receiver<Done>,pending:Option<Pending>,stats:Stats,
}
impl Default for ReadOnlyPicker{
    fn default()->Self{let (_,rx)=mpsc::sync_channel(1);Self{identity:None,generation:0,cache:Cache::default(),
        failure:None,tx:None,rx,pending:None,stats:Stats::default()}}
}
impl Drop for ReadOnlyPicker{fn drop(&mut self){self.cancel();}}
impl ReadOnlyPicker{
    pub fn supports(world:&World)->bool{engine_worldgen::planetary::deep_time(world.generator_version) && world.world_type!=WorldType::Manyfold}
    fn cancel(&self){if let Some(job)=&self.pending{job.cancel.store(true,Ordering::Relaxed);}}
    pub fn reset(&mut self){self.cancel();self.generation=self.generation.wrapping_add(1);
        self.identity=None;self.cache.clear();self.failure=None;}
    fn prepare(&mut self,world:&World)->Result<(),String>{
        if !Self::supports(world){return Err("Read-only picking does not support this world".into());}
        let identity=(world.regions.instance_id,world.sample_config());
        if self.identity.as_ref()!=Some(&identity){self.reset();self.identity=Some(identity);}
        match self.rx.try_recv(){
            Ok(done)=>{
                self.pending=None;self.stats.procedural_samples+=done.work.procedural_samples;self.stats.stored_reads+=done.work.stored_reads;
                if done.generation==self.generation{for (key,result) in done.items{
                    if !key.is_current(world){continue;}
                    match result{
                        Ok(data)=>{
                            if data.len()!=key.voxels() || data.len()*std::mem::size_of::<BlockId>()>CACHE_BYTES{
                                self.failure=Some((key,"Read-only source has an invalid bounded footprint".into()));
                            }else{self.cache.insert(key,data);self.stats.completed_sources+=1;}
                        }
                        Err(error)=>self.failure=Some((key,error)),
                    }
                }}
            }
            Err(mpsc::TryRecvError::Disconnected)=>{
                if let Some(job)=self.pending.take(){if job.generation==self.generation{
                    if let Some(key)=job.keys.first().filter(|key|key.is_current(world)){
                        self.failure=Some((key.clone(),"Read-only source worker disconnected".into()));
                    }
                }}self.tx=None;
            }
            Err(mpsc::TryRecvError::Empty)=>{},
        }Ok(())
    }
    fn request(&mut self,world:&World,keys:Vec<Key>)->Readiness<()> {
        if keys.is_empty() || keys.len()>MAX_BATCH{return Readiness::Failed("Invalid bounded read-only source request".into());}
        // Different readers share the one bounded lane. Only identity/reset
        // invalidation cancels work; otherwise repeated consumers can starve it.
        if self.pending.is_some(){return Readiness::Pending;}
        if self.tx.is_none(){match source::start(){Ok((tx,rx))=>{self.tx=Some(tx);self.rx=rx;},Err(error)=>return Readiness::Failed(error)}}
        let cancel=Arc::new(AtomicBool::new(false));
        let job=Job{keys:keys.clone(),generation:self.generation,view:world.regions.view.clone(),cancel:cancel.clone(),
            #[cfg(test)] review_gate:None};
        let Some(tx)=self.tx.as_ref()else{return Readiness::Failed("Read-only source worker is unavailable".into());};
        match tx.try_send(job){
            Ok(())=>{self.stats.requested_sources+=keys.len() as u64;self.pending=Some(Pending{keys,generation:self.generation,cancel});self.stats.jobs+=1;Readiness::Pending}
            Err(mpsc::TrySendError::Full(_))=>Readiness::Pending,
            Err(mpsc::TrySendError::Disconnected(_))=>{self.tx=None;Readiness::Failed("Read-only source worker stopped".into())}
        }
    }
    // Lookup only. In particular the planning DDA cannot enqueue individual work.
    fn read(&mut self,world:&World,p:IVec3)->Readiness<BlockId>{
        if let Some(block)=world.resident_block(p){return Readiness::Ready(block);}
        let key=Key::at_position(world,p);
        if key.record.is_none() && (p.y<key.cfg.min_world_y() || p.y>=key.cfg.max_world_y()){return Readiness::Ready(BlockId::Air);}
        if let Some(block)=self.cache.get(&key,p){return Readiness::Ready(block);}
        if let Some((failed,error))=&self.failure{if *failed==key{return Readiness::Failed(error.clone());}}
        Readiness::Pending
    }
    pub fn read_ready(&mut self,world:&World,p:IVec3)->Readiness<BlockId>{
        if let Err(error)=self.prepare(world){return Readiness::Failed(error);}
        match self.read(world,p){Readiness::Pending=>match self.request(world,vec![Key::at_position(world,p)]){
            Readiness::Failed(error)=>Readiness::Failed(error),_=>Readiness::Pending},other=>other}
    }
    fn plan(&mut self,world:&World,origin:DVec3,dir:Vec3,reach:f32)->Result<Vec<Key>,String>{
        let mut keys=Vec::new();let mut seen=std::collections::HashSet::new();let mut overflow=false;
        // This pass is enumeration ONLY. Known exact geometry may stop it. Air
        // for unknowns never reaches an action, hit/miss result, or World cache.
        let _=super::collision::raycast_loaded_block_precise(world,origin,dir,reach,|p|match self.read(world,p){
            Readiness::Ready(block)=>Some(block),
            Readiness::Failed(_)=>None, // A deeper error is exposed only if the real current ray reaches it.
            Readiness::Pending=>{
                let key=Key::at_position(world,p);
                if seen.insert(key.address()){
                    if keys.len()==MAX_BATCH{overflow=true;return None;}keys.push(key);
                }Some(BlockId::Air)
            }
        });
        if overflow{Err("Read-only ray exceeded its bounded source plan".into())}else{Ok(keys)}
    }
    pub fn poll(&mut self,world:&World,origin:DVec3,dir:Vec3,reach:f32)->Readiness<Option<PickHit>>{
        if let Err(error)=self.prepare(world){return Readiness::Failed(error);}
        if reach.is_finite() && reach>MAX_REACH_BLOCKS{return Readiness::Failed("Read-only ray exceeds the bounded 256-block reach".into());}
        let mut unavailable:Option<Readiness<()>>=None;let mut missing=None;
        let hit=super::collision::raycast_loaded_block_precise(world,origin,dir,reach,|p|match self.read(world,p){
            Readiness::Ready(block)=>Some(block),Readiness::Pending=>{missing=Some(Key::at_position(world,p));unavailable=Some(Readiness::Pending);None},
            Readiness::Failed(error)=>{unavailable=Some(Readiness::Failed(error));None}
        });
        if let Some(result)=unavailable{
            // Repeated polls need no second DDA while the first missing source
            // is already in flight, or a cancelled job still owns the lane.
            if matches!(&result,Readiness::Pending) && self.pending.as_ref().is_some_and(|job|
                job.cancel.load(Ordering::Relaxed) || missing.as_ref().is_some_and(|key|job.keys.contains(key))){
                return Readiness::Pending;
            }
            return match result{
            Readiness::Failed(error)=>Readiness::Failed(error),
            _=>match self.plan(world,origin,dir,reach){
                Err(error)=>Readiness::Failed(error),Ok(keys)=>match self.request(world,keys){
                    Readiness::Failed(error)=>Readiness::Failed(error),_=>Readiness::Pending}
            }
        };}
        match hit{None=>Readiness::Ready(None),Some((cell,previous))=>match self.read(world,cell){
            Readiness::Ready(block)=>Readiness::Ready(Some(PickHit{cell,previous,block})),
            Readiness::Failed(error)=>Readiness::Failed(error),Readiness::Pending=>Readiness::Pending}}
    }
}
#[cfg(test)] mod tests;
#[cfg(test)] mod acceptance_tests;
#[cfg(test)] mod microbrick_tests;

#[cfg(test)] mod group_tests;


