//! Opt-in natural-world route. Only aim, ordinary movement/tool inputs and
//! production station actions are scripted. No terrain, loot, enemy or HP edits.
use crate::{creative::{CreativeState, ToolKind}, player::{Input, Player}, world::{self, World}};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
mod checkpoint;

pub struct Review {
    phase: usize,
    clock: f32,
    phase_start: f32,
    last: Vec3,
    stuck: f32,
    distance: f32,
    grounded: u32,
    airborne: u32,
    collision: bool,
    cast_seen: bool,
    mined: Vec<IVec3>,
    interacted: bool,
    capture: Option<String>,
    checkpoint: checkpoint::Checkpoint,
    pub done: bool,
    pub reloaded: bool,
    finished_since: Option<std::time::Instant>,
}
impl Review {
    pub fn from_env(benchmark: bool) -> Option<Self> {
        if !benchmark || std::env::var_os("VOXEL_WORKSHOP_REVIEW").is_none() { return None; }
        // Refuse to write a real player's checkpoint directory.
        let root = std::path::PathBuf::from(std::env::var_os("VOXEL_CAPTURE_DIR")?);
        if std::env::var_os("VOXEL_SAVE_DIR").map(std::path::PathBuf::from) != Some(root.join("disposable-saves")) { return None; }
        Some(Self { phase: 0, clock: 0.0, phase_start: 0.0, last: Vec3::NAN,
            stuck: 0.0, distance: 0.0, grounded: 0, airborne: 0, collision: false,
            cast_seen: false, mined: Vec::new(), interacted: false, capture: None,
            checkpoint: Default::default(), done: false, reloaded: false, finished_since: None })
    }
    pub fn finished(&mut self) -> bool {
        if self.done && self.finished_since.is_none() {
            self.capture = Some("stage-22-reloaded-world.png".into());
            println!("[workshop-loop] reloaded world rendered; audio_device={}", crate::audio::status());
        }
        self.done && self.finished_since.get_or_insert_with(std::time::Instant::now).elapsed().as_secs_f32() > 2.0
    }
    fn next(&mut self, label: &str, p: Vec3) {
        println!("[workshop-loop] stage={} label={label} time={:.2} feet={p:.2}", self.phase, self.clock);
        self.capture = Some(format!("stage-{:02}-{label}.png", self.phase));
        self.phase += 1;
        self.phase_start = self.clock;
        self.interacted = false;
    }
    pub fn capture(&mut self) -> Option<std::path::PathBuf> {
        Some(std::path::PathBuf::from(std::env::var_os("VOXEL_CAPTURE_DIR")?).join(self.capture.take()?))
    }
    pub fn input(&mut self, w: &World, p: &mut Player, i: &mut Input) {
        i.clear();
        if self.done { return; }
        if p.flying {
            eprintln!("[workshop-loop] FAIL unexpected flight; grounded traversal required");
            self.done = true;
            return;
        }
        self.clock += crate::simulation::STEP_SECONDS;
        let s = w.voxel_scale.meters();
        let feet = p.pos * s;
        if self.last.is_finite() {
            let d = feet.distance(self.last);
            self.distance += d;
            self.stuck = if d < 0.002 { self.stuck + crate::simulation::STEP_SECONDS } else { 0.0 };
        }
        self.last = feet;
        if p.on_ground { self.grounded += 1; } else { self.airborne += 1; }
        self.cast_seen |= !w.adventure.projectiles.is_empty() || w.adventure.mana < 95.0;
        let Some(heart) = world::sites::workshop_heart(&w.sample_config()) else { return; };
        let g = heart.y + 18.0;
        let target = match self.phase {
            0 => Vec3::new(0.0, g + 1.0, -31.0),
            1 => Vec3::new(0.0, g + 1.0, -39.5),
            2 | 20 => Vec3::new(-2.0, g + 1.0, -39.5),
            3 | 11 | 15 => Vec3::new(0.0, g, -45.0),
            18 => Vec3::new(0.0, g - 18.0, -88.0),
            19 => Vec3::new(0.0, g - 3.0, -50.5),
            4 | 10 | 16 | 17 => Vec3::new(0.0, g - 3.0, -50.5),
            5..=9 => Vec3::new(-3.5, g - 3.0, -42.0 - self.resource_depth()),
            12 => Vec3::new(2.5, g + 1.0, -39.5),
            13 | 14 => Vec3::new(0.0, g + 1.0, -39.5),
            21.. => return,
        };
        if self.phase == 17 && feet.z < -84.0 && !world::workshop::cleared(&w.adventure)
            && let Some(e) = w.adventure.enemies.iter().filter(|e| e.health > 0.0 && e.site == IVec3::ZERO)
                .min_by(|a,b| a.pos.distance_squared(feet).total_cmp(&b.pos.distance_squared(feet))) {
                aim(p, e.center(), s);
                let d = e.pos.distance(feet);
                i.cast_spell = true;
                i.back = d < 4.0;
                i.forward = d > 9.0;
                i.right = (self.clock * 0.4).sin() > 0.0;
                i.left = !i.right;
                i.jump = p.on_ground && self.stuck > 0.15;
                if i.jump { self.collision = true; }
                if (self.clock * 2.0) as u32 != ((self.clock - crate::simulation::STEP_SECONDS) * 2.0) as u32 {
                    println!("[workshop-combat] kind={} enemy={:.2} hp={:.1} feet={feet:.2} mana={:.1}",e.kind,e.pos,e.health,w.adventure.mana);
                }
                return;
            }
        let target = if self.phase == 17 { Vec3::new(0.0, heart.y, -88.0) } else { target };
        let d = (target - feet) * Vec3::new(1.0, 0.0, 1.0);
        aim(p, target + Vec3::Y * 1.2, s);
        if d.length() > 0.28 {
            i.forward = true;
            i.sneak = d.length() < 1.2;
            // One-metre stairs require the native jump at the coarse scale.
            i.jump = p.on_ground && self.stuck > 0.12;
            if i.jump { self.collision = true; }
        } else if self.phase <= 1 || matches!(self.phase, 3 | 4 | 10 | 11 | 13 | 15 | 16 | 18 | 19) {
            self.next("walk", feet);
        } else if (5..=9).contains(&self.phase) {
            let (resource, needed) = self.resource();
            if w.expedition.resources[resource] >= needed {
                self.next("mined", feet);
            } else {
                let depth = self.resource_depth();
                let floor = (-depth * 0.45 / s).floor() * s;
                aim(p, Vec3::new(-5.5, g + floor + s + 0.5, -42.0 - depth), s);
                i.break_block = true;
            }
        } else if self.phase == 14
            && w.adventure.best_wand_tier() >= 2 { self.next("wand-upgraded", feet); }
        if self.clock - self.phase_start > 55.0 && self.phase != 21 {
            eprintln!("[workshop-loop] stalled phase={} feet={feet:.2} target={target:.2}", self.phase);
            self.done = true;
        }
    }
    fn resource_depth(&self) -> f32 { match self.phase { 5 => 8.5, 6 => 9.5, 7 => 10.5, 8 => 11.5, _ => 12.5 } }
    fn resource(&self) -> (usize, u32) { match self.phase { 5 => (0,64), 6 => (3,16), 7 => (4,64), 8 => (4,128), _ => (5,64) } }
    pub fn after(&mut self, w: &mut World, p: &mut Player, c: &mut CreativeState,
        day: &mut world::DayNightCycle, saves: &mut world::storage::SaveService,
        blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
        if self.done { return; }
        c.tool = ToolKind::Hand;
        let s = w.voxel_scale.meters();
        let feet = p.pos * s;
        let Some(heart) = world::sites::workshop_heart(&w.sample_config()) else { return; };
        let g = heart.y + 18.0;
        if (5..=9).contains(&self.phase)
            && let Some((cell,_)) = world::raycast_block(w, p.eye(w.voxel_scale), p.forward_dir(), 6.0 / s)
                && world::expedition::resource(w.get(cell)).is_some() && !self.mined.contains(&cell) {
                self.mined.push(cell);
            }
        if matches!(self.phase, 2 | 12 | 20) && !self.interacted {
            let at = Vec3::new(if self.phase == 12 { 4.5 } else { -3.5 }, g + 1.5, -39.5);
            if feet.distance(at) < 4.0 {
                aim(p, at, s);
                if let Some((cell,_)) = world::raycast_block(w, p.eye(w.voxel_scale), p.forward_dir(), 6.0 / s)
                    && w.get(cell) == if self.phase == 12 { world::BlockId::Smelter } else { world::BlockId::Workbench } {
                    world::expedition::interact(w, cell, p.pos, blocks, chunks);
                    self.interacted = true;
                    self.next(if self.phase == 12 { "processed" } else { "workbench" }, feet);
                }
            }
        }
        if self.phase == 14 && self.clock - self.phase_start > 0.5 {
            world::factory::action(w, p.pos, world::factory::Action::Wand);
            if w.adventure.wand >= 2 {
                for slot in 0..world::adventure::wands::capacity(w.adventure.wand) {
                    world::factory::action(w, p.pos, world::factory::Action::SetWandSlot(slot as u32,
                        if slot == 0 { 0 } else { world::adventure::wands::EMPTY }));
                }
                world::factory::action(w, p.pos, world::factory::Action::SetWandMode(0));
            }
        }
        if self.phase == 17 && world::workshop::cleared(&w.adventure) {
            self.next("dungeon-cleared", feet);
        }
        if self.phase == 21 {
            self.done = self.checkpoint.tick(w,p,day,saves,&self.mined, self.distance,
                self.grounded, self.airborne, self.collision, self.cast_seen);
            if self.done && self.checkpoint.reloaded {
                self.reloaded = true;
                self.capture = Some("stage-21-reloaded.png".into());
            }
        }
        if (self.clock * 2.0) as u32 != ((self.clock - crate::simulation::STEP_SECONDS) * 2.0) as u32 {
            println!("[workshop-player] phase={} t={:.2} feet={feet:.2} ground={} hp={:.1} mana={:.1} resources={:?}",
                self.phase, self.clock, p.on_ground, w.adventure.health, w.adventure.mana, w.expedition.resources);
        }
    }
}
fn aim(p: &mut Player, target: Vec3, s: f32) {
    let d = (target - (p.pos * s + Vec3::Y * (1.62 * p.scale))).normalize_or_zero();
    p.yaw = d.x.atan2(-d.z);
    p.pitch = d.y.asin();
}
