//! One independent cold-source lane for player strokes; never waits on I/O.
use crate::world::{World,WorldSampleConfig,chunk_coord_from_world_block,regions::{DiskChunk,RegionView}};
use glam::IVec3;
use std::sync::{Arc,mpsc};
const MAX_BYTES:usize=4*1024*1024;
mod review;
pub(crate) use review::run as review_backlog;
struct Job { coord:IVec3, version:Option<u64>, generation:u64, cfg:WorldSampleConfig, view:RegionView }
struct Done { coord:IVec3, version:Option<u64>, generation:u64, data:Result<DiskChunk,String> }
pub(super) enum Admission { Ready, Waiting, Failed(String) }
pub(super) struct Source {
    tx:Option<mpsc::SyncSender<Job>>, rx:mpsc::Receiver<Done>,
    pending:Option<IVec3>, completed:Option<Done>, generation:u64, pub waiting:bool,
}
impl Default for Source {
    fn default()->Self {let (_,rx)=mpsc::sync_channel(1);Self{tx:None,rx,pending:None,completed:None,generation:0,waiting:false}}
}
impl Source {
    pub fn reset(&mut self) {self.generation=self.generation.wrapping_add(1);self.waiting=false;self.completed=None;}
    fn start(&mut self)->Result<(),String> {
        let (tx,jobs)=mpsc::sync_channel::<Job>(1);let (done,rx)=mpsc::sync_channel(1);
        std::thread::Builder::new().name("player-edit-source".into()).spawn(move|| {
            while let Ok(job)=jobs.recv() {
                let data=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    job.view.load(job.coord).map(|data|data.unwrap_or_else(||DiskChunk{
                        chunk:Arc::new(engine_worldgen::generate(&job.cfg,job.coord)),water:Arc::default(),
                    })).map_err(|e|format!("Player edit source: {e:#}"))
                })).unwrap_or_else(|_|Err("Player edit source panicked".into()));
                if done.send(Done{coord:job.coord,version:job.version,generation:job.generation,data}).is_err(){break;}
            }
        }).map_err(|e|format!("Player edit worker: {e}"))?;
        self.tx=Some(tx);self.rx=rx;self.pending=None;Ok(())
    }
    pub fn prepare(&mut self,world:&mut World,p:IVec3)->Admission {
        self.waiting=false;let coord=chunk_coord_from_world_block(p);
        let version=|w:&World,c:&IVec3|w.regions.view.records.get(c).map(|r|r.version);
        if self.completed.is_none() {
            match self.rx.try_recv() {
                Ok(done)=>{self.pending=None;self.completed=Some(done);}
                Err(mpsc::TryRecvError::Disconnected)=>{self.tx=None;self.pending=None;}
                Err(mpsc::TryRecvError::Empty)=>{}
            }
        }
        if let Some(done)=self.completed.take() {
            if done.generation==self.generation && !world.chunks.contains_key(&done.coord)
                && done.version==version(world,&done.coord) {
                match done.data {
                    Ok(data)=>{
                        let bytes=data.chunk.blocks.len()*2+data.chunk.torches.capacity()*32+data.water.capacity()*32;
                        if bytes>MAX_BYTES {return Admission::Failed("Player edit source exceeded reservation".into());}
                        if world.regions.backlog_full() {
                            self.completed=Some(Done{data:Ok(data),..done});
                        }else{world.install_simulation_chunk(done.coord,data);}
                    }
                    Err(error)=>return Admission::Failed(error),
                }
            }
        }
        if world.chunks.contains_key(&coord){return Admission::Ready;}
        self.waiting=true;
        world.request_simulation_writeback();
        if world.regions.backlog_full() || self.pending.is_some() || self.completed.is_some(){return Admission::Waiting;}
        if self.tx.is_none() {if let Err(error)=self.start(){self.waiting=false;return Admission::Failed(error);}}
        let job=Job{coord,version:version(world,&coord),generation:self.generation,cfg:world.sample_config(),view:world.regions.view.clone()};
        match self.tx.as_ref().unwrap().try_send(job) {
            Ok(())=>{self.pending=Some(coord);Admission::Waiting}
            Err(mpsc::TrySendError::Full(_))=>Admission::Waiting,
            Err(mpsc::TrySendError::Disconnected(_))=>{self.tx=None;self.waiting=false;Admission::Failed("Player edit worker stopped".into())}
        }
    }
}
