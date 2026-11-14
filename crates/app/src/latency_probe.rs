//! Input-to-pixels latency probe for player block edits (benchmark only).
//!
//! `VOXEL_LATENCY_PROBE=<events>` with `--benchmark-*`: builds a small fixture
//! in front of a fixed camera, then alternates hand breaks and places through
//! the real input path (`Input::break_block` / `place_block`, the same edge
//! detection, raycast and edit drain a player uses). Each event records:
//!
//! - `applied`: the frame the world cell changed (edit drain);
//! - `published`: the frame the edited chunk's mesh at or past the edit's
//!   revision was installed for drawing (`ChunkCache::published_revision`),
//!   even while later sim edits to the same chunk are still pending;
//! - `presented`: the end of the render call of that frame (the frame the
//!   change is first on screen);
//! - `reaction` (breaks only): the frame the sand resting on the broken block
//!   first moved (cellular sim).
//!
//! Times are wall clock from the frame the input was raised; frames count
//! rendered frames. OS input queueing before the frame starts is not included.
//! Results print as `[latency]` lines. Run:
//! `VOXEL_LATENCY_PROBE=40 <exe> --benchmark-physics --benchmark-duration=90`
//! (the default 8 s benchmark ends before 40 events). `VOXEL_LATENCY_DEBUG=1`
//! adds a `[player-mesh-wait]` streamer trace every fourth frame of a stall
//! and `VOXEL_TOOL_REVIEW=1` prints the target cells at each input.
use crate::{
    creative::{CreativeState, ToolKind},
    player::{FlyMode, Input, Player},
    world::{BlockId as B, World},
};
use glam::{IVec3, Vec3};
use std::{collections::HashSet, time::Instant};

/// Frames to wait after a fixture rebuild before acting, so its own remesh
/// is published and the next measurement starts from a quiet streamer.
const SETTLE_FRAMES: u32 = 30;
/// Give up on a stage after this many frames and record it as missed.
const TIMEOUT_FRAMES: u32 = 240;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Break,
    Place,
}

#[derive(Clone, Copy, Debug, Default)]
struct Stamp {
    frames: u32,
    ms: f32,
}

#[derive(Clone, Copy, Debug)]
struct Event {
    kind: Kind,
    input_at: Instant,
    input_frame: u64,
    applied: Option<Stamp>,
    published: Option<Stamp>,
    presented: Option<Stamp>,
    reaction: Option<Stamp>,
    /// World revision right after the edit was applied.
    revision: u64,
}

enum Phase {
    Setup,
    Settle { frames: u32 },
    Act,
    Measure(Event),
    Done,
}

pub struct Probe {
    events_wanted: usize,
    done: Vec<Event>,
    phase: Phase,
    frame: u64,
    target: IVec3,
    camera_m: Vec3,
    backstop: IVec3,
    sand_above: IVec3,
    publish_seen_this_frame: bool,
    debug: bool,
}

impl Probe {
    pub fn from_env(benchmark: bool) -> Option<Self> {
        if !benchmark {
            return None;
        }
        let events = std::env::var("VOXEL_LATENCY_PROBE").ok()?.parse::<usize>().ok()?;
        Some(Self {
            events_wanted: events.clamp(2, 1000),
            done: Vec::new(),
            phase: Phase::Setup,
            frame: 0,
            target: IVec3::ZERO,
            camera_m: Vec3::new(0.5, 102.4, 8.0),
            backstop: IVec3::ZERO,
            sand_above: IVec3::ZERO,
            publish_seen_this_frame: false,
            debug: std::env::var_os("VOXEL_LATENCY_DEBUG").is_some(),
        })
    }

    pub fn finished(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    fn camera(&self, w: &World, p: &mut Player) {
        let scale = w.voxel_scale.meters();
        p.pos = self.camera_m / scale;
        p.yaw = 0.0;
        p.pitch = 0.0;
        p.vel = Vec3::ZERO;
        p.flying = true;
        p.fly_mode = FlyMode::Noclip;
    }

    /// Clears an arena around the fixed camera; called once when the world loads.
    pub fn install(&mut self, w: &mut World, p: &mut Player, c: &mut CreativeState) {
        let n = (1.0 / w.voxel_scale.meters()).max(1.0) as i32;
        let saved=std::env::var_os("VOXEL_LATENCY_SAVED").is_some();
        if saved { self.camera_m=p.pos*w.voxel_scale.meters(); }
        let mut edits = Vec::new();
        if !saved {
        for y in 96 * n..110 * n {
            for z in -6 * n..12 * n {
                for x in -6 * n..6 * n {
                    edits.push((IVec3::new(x, y, z), if y < 97 * n { B::Stone } else { B::Air }));
                }
            }
        }
        }
        w.set_many(&edits);
        c.close_panels();
        c.sync_world_scale(w.voxel_scale.meters());
        c.placement_distance = 16.0;
        c.assign_hotbar_slot(0, B::StoneBricks);
        c.tool = ToolKind::Hand;
        self.camera(w, p);
        let eye = p.eye(w.voxel_scale);
        let forward = p.forward_dir();
        self.target = (eye + forward * 4.5).floor().as_ivec3();
        self.backstop = (eye + forward * 5.5).floor().as_ivec3();
        self.sand_above = self.target + IVec3::Y;
        if saved {
            let eye=p.eye(w.voxel_scale);
            let mut clear=Vec::new();
            for step in 0..=(5*n) {
                let at=(eye+forward*step as f32).floor().as_ivec3();
                for dy in -1..=1 {for dx in -1..=1 {clear.push((at+IVec3::new(dx,dy,0),B::Air));}}
            }
            w.set_many(&clear);
        }
        println!(
            "[latency] installed events={} target={:?} scale={}",
            self.events_wanted,
            self.target,
            w.voxel_scale.meters()
        );
    }

    fn next_kind(&self) -> Kind {
        if self.done.len().is_multiple_of(2) { Kind::Break } else { Kind::Place }
    }

    fn build_fixture(&self, w: &mut World, dirty: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
        let mut edits = vec![(self.backstop, B::Stone)];
        // Clear the sand that fell out of earlier cycles under the target column
        // and in the sightline: a spilled grain in front took the first click.
        let toward_camera = (self.camera_m.z / w.voxel_scale.meters()).floor() as i32 - self.target.z;
        for dz in 0..=toward_camera.max(0) {
            for dy in -8..=if dz == 0 { 4 } else { 3 } {
                for dx in -1..=1 {
                    if dz == 0 && dx != 0 { continue; }
                    edits.push((self.target + IVec3::new(dx, dy, dz), B::Air));
                }
            }
        }
        match self.next_kind() {
            Kind::Break => {
                edits.push((self.target, B::Dirt));
                for dy in 1..=3 {
                    edits.push((self.target + IVec3::new(0, dy, 0), B::Sand));
                }
                // A cap keeps the column from being one free-standing sand stack
                // that the sim might settle before the break.
                edits.push((self.target + IVec3::new(0, 4, 0), B::Stone));
            }
            Kind::Place => {
                // Survival places draw from the world inventory, not the creative hotbar;
                // an emptied stack placed nothing and read as a missed event.
                use crate::world::inventory::{Item, ItemStack};
                w.inventory.hotbar[0] = Some(ItemStack::new(Item::Block(B::StoneBricks), 64));
            }
        }
        let foreground=w.environment.player_edit;
        w.environment.make_player_room(self.target);
        w.environment.player_edit=true;
        w.set_many(&edits);
        w.environment.player_edit=foreground;
        for (cell, _) in edits {
            dirty.insert(cell);
            crate::world::mark_chunk_and_neighbors_dirty(chunks, cell);
        }
    }

    /// Frame start, after other review inputs: drives the fixture and raises
    /// the tool input for exactly one frame.
    pub fn input(
        &mut self,
        w: &mut World,
        p: &mut Player,
        c: &mut CreativeState,
        input: &mut Input,
        dirty: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
        ready: bool,
    ) {
        // Tool input is ignored while the world is still spawning; wait.
        if !ready {
            return;
        }
        self.frame += 1;
        self.publish_seen_this_frame = false;
        self.camera(w, p);
        c.tool = ToolKind::Hand;
        c.selected_slot = 0;
        input.break_block = false;
        input.place_block = false;
        match &mut self.phase {
            Phase::Setup => {
                self.build_fixture(w, dirty, chunks);
                self.phase = Phase::Settle { frames: 0 };
            }
            Phase::Settle { frames } => {
                *frames += 1;
                if *frames >= SETTLE_FRAMES {
                    self.phase = Phase::Act;
                }
            }
            Phase::Act => {
                let kind = self.next_kind();
                if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() {
                    eprintln!("[latency-act] event={} kind={kind:?} target={:?} above={:?}", self.done.len(), w.get(self.target), w.get(self.sand_above));
                }
                match kind {
                    Kind::Break => input.break_block = true,
                    Kind::Place => input.place_block = true,
                }
                self.phase = Phase::Measure(Event {
                    kind,
                    input_at: Instant::now(),
                    input_frame: self.frame,
                    applied: None,
                    published: None,
                    presented: None,
                    reaction: None,
                    revision: u64::MAX,
                });
            }
            Phase::Measure(e) if e.applied.is_none() => {
                // Current mining has material-dependent hold time. Keep the
                // real input held until authority commits, rather than pulsing
                // the old instant-break path for a single frame.
                match e.kind {
                    Kind::Break => input.break_block = true,
                    Kind::Place => input.place_block = true,
                }
            }
            Phase::Measure(_) | Phase::Done => {}
        }
    }

    fn stamp(&self, e: &Event) -> Stamp {
        Stamp {
            frames: (self.frame - e.input_frame) as u32,
            ms: e.input_at.elapsed().as_secs_f32() * 1000.0,
        }
    }

    /// Called only for a tool commit, never for unrelated environmental changes.
    pub fn after_edit(&mut self,w:&World,cell:IVec3,before:B) {
        if cell!=self.target {return;}
        let stamp=self.stamp_for_active();
        if let (Some(stamp),Phase::Measure(e))=(stamp,&mut self.phase) {
            let matches=match e.kind {Kind::Break=>before==B::Dirt && w.get(cell)==B::Air,
                Kind::Place=>before==B::Air && w.get(cell)!=B::Air};
            if e.applied.is_none() && matches {e.applied=Some(stamp);e.revision=w.revision();}
        }
    }
    fn stamp_for_active(&self)->Option<Stamp> {
        if let Phase::Measure(e)=&self.phase {Some(self.stamp(e))}else{None}
    }

    /// After the cellular simulation of this frame.
    pub fn after_sim(&mut self, w: &World) {
        let above = self.sand_above;
        if self.debug
            && let Phase::Measure(e) = &self.phase
            && e.kind == Kind::Break
        {
            eprintln!(
                "[latency-debug] frame+{} target={:?} above={:?} +2={:?}",
                self.frame - e.input_frame,
                w.get(self.target),
                w.get(above),
                w.get(above + IVec3::Y)
            );
        }
        let stamp = match &self.phase {
            Phase::Measure(e) if e.kind == Kind::Break && e.applied.is_some() && e.reaction.is_none() => {
                (w.get(above) != B::Sand).then(|| self.stamp(e))
            }
            _ => None,
        };
        if let (Some(s), Phase::Measure(e)) = (stamp, &mut self.phase) {
            e.reaction = Some(s);
        }
    }

    pub fn stalled_chunk(&self)->Option<IVec3> {
        if let Phase::Measure(e)=&self.phase {
            let age=self.frame-e.input_frame;
            // `VOXEL_LATENCY_DEBUG` traces every fourth frame of a stall once the
            // edit is older than the usual mesh round trip.
            let period=if self.debug {4} else {120};
            if e.applied.is_some() && e.published.is_none() && age>=period && age.is_multiple_of(period) {
                return Some(crate::world::chunk_coord_from_world_block(self.target));
            }
        }
        None
    }

    /// After chunk uploads; `published` gives the world revision of the mesh
    /// installed for a chunk. The edit is on the GPU once that is at or past
    /// the revision the edit produced, even if later sim edits are pending.
    pub fn after_upload(&mut self, published: impl Fn(IVec3) -> Option<u64>) {
        let chunk = crate::world::chunk_coord_from_world_block(self.target);
        let stamp = match &self.phase {
            Phase::Measure(e)
                if e.applied.is_some()
                    && e.published.is_none()
                    && published(chunk).is_some_and(|r| r >= e.revision) =>
            {
                Some(self.stamp(e))
            }
            _ => None,
        };
        if let (Some(s), Phase::Measure(e)) = (stamp, &mut self.phase) {
            e.published = Some(s);
            self.publish_seen_this_frame = true;
        }
    }

    /// After the frame's render call returned successfully.
    pub fn after_present(&mut self) {
        let mut finish = None;
        if let Phase::Measure(e) = &self.phase {
            let mut e = *e;
            if self.publish_seen_this_frame && e.presented.is_none() {
                e.presented = Some(self.stamp(&e));
            }
            let age = (self.frame - e.input_frame) as u32;
            let reaction_done = e.kind == Kind::Place || e.reaction.is_some();
            if (e.presented.is_some() && reaction_done) || age >= TIMEOUT_FRAMES {
                finish = Some(e);
            } else {
                self.phase = Phase::Measure(e);
            }
        }
        if let Some(e) = finish {
            println!("[latency-progress] event={} kind={:?} applied={} published={} presented={} reaction={} age_ms={:.1}",
                self.done.len(),e.kind,e.applied.is_some(),e.published.is_some(),e.presented.is_some(),e.reaction.is_some(),e.input_at.elapsed().as_secs_f64()*1000.0);
            self.done.push(e);
            if self.done.len() >= self.events_wanted {
                self.report();
                self.phase = Phase::Done;
            } else {
                self.phase = Phase::Setup;
            }
        }
    }

    fn report(&self) {
        println!("[latency] events={} (frames and ms from the frame the input was raised)", self.done.len());
        for kind in [Kind::Break, Kind::Place] {
            let of = |f: fn(&Event) -> Option<Stamp>| -> Vec<Stamp> {
                self.done.iter().filter(|e| e.kind == kind).filter_map(f).collect()
            };
            let total = self.done.iter().filter(|e| e.kind == kind).count();
            let stages: [(&str, Vec<Stamp>); 4] = [
                ("applied", of(|e| e.applied)),
                ("published", of(|e| e.published)),
                ("presented", of(|e| e.presented)),
                ("reaction", of(|e| e.reaction)),
            ];
            for (name, mut v) in stages {
                if kind == Kind::Place && name == "reaction" {
                    continue;
                }
                println!("{}", summary_line(kind, name, total, &mut v));
            }
        }
        for (i, e) in self.done.iter().enumerate() {
            let f = |s: Option<Stamp>| s.map_or("-".to_string(), |s| format!("{}f/{:.1}ms", s.frames, s.ms));
            println!(
                "[latency-event] {i} {:?} applied={} published={} presented={} reaction={}",
                e.kind,
                f(e.applied),
                f(e.published),
                f(e.presented),
                f(e.reaction)
            );
        }
    }
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return f32::NAN;
    }
    let i = ((sorted.len() - 1) as f32 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn summary_line(kind: Kind, stage: &str, total: usize, v: &mut [Stamp]) -> String {
    let mut ms: Vec<f32> = v.iter().map(|s| s.ms).collect();
    ms.sort_by(f32::total_cmp);
    let mut frames: Vec<f32> = v.iter().map(|s| s.frames as f32).collect();
    frames.sort_by(f32::total_cmp);
    format!(
        "[latency] {kind:?} {stage:<9} n={}/{total} ms p50={:.1} p95={:.1} max={:.1} frames p50={} p95={} max={}",
        v.len(),
        percentile(&ms, 0.5),
        percentile(&ms, 0.95),
        ms.last().copied().unwrap_or(f32::NAN),
        percentile(&frames, 0.5),
        percentile(&frames, 0.95),
        frames.last().copied().unwrap_or(f32::NAN),
    )
}
