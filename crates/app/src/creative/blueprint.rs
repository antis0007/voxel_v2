//! Area Tool blueprints: two-corner selection, clipboard and paste preview.
//!
//! Finished pastes and cuts join the player's edit history as one undo step.
//!
//! Key presses queue here; `frame` runs them against the aimed cell and
//! advances the copy or write in progress within the frame's budget.
use super::{CreativeState, RaycastTarget, ToolKind};
use crate::keybinds::Action;
use crate::world::World;
use crate::world::blueprint::{self, Blueprint, Capture, Change, Job, Region, Sink, library};
use crate::world::edit_history::EditHistory;
use engine_core::Budget;
use glam::{IVec3, Vec3};
use std::sync::Arc;

mod overlay;

/// Solid cells above this draw only the paste box, not the silhouette.
const GHOST_CELLS: usize = 32_768;

enum Work {
    Copy { capture: Capture, cut: bool },
    Paste(Job),
    Cut(Job),
}

/// Panel-side state for the blueprint file list.
#[derive(Default)]
pub struct Shelf {
    pub entries: Vec<library::Entry>,
    pub name: String,
    pub renaming: Option<(String, String)>,
    pub confirm_delete: Option<String>,
    pub status: Option<String>,
    pub stale: bool,
}

pub struct BlueprintTool {
    corners: Option<(IVec3, IVec3)>,
    second_corner_next: bool,
    queued: Vec<Action>,
    clipboard: Option<Arc<Blueprint>>,
    ghost: Vec<[Vec3; 2]>,
    armed: bool,
    origin: Option<IVec3>,
    pub paste_air: bool,
    work: Option<Work>,
    pub shelf: Shelf,
}

impl Default for BlueprintTool {
    fn default() -> Self {
        Self {
            corners: None,
            second_corner_next: false,
            queued: Vec::new(),
            clipboard: None,
            ghost: Vec::new(),
            armed: false,
            origin: None,
            paste_air: true,
            work: None,
            shelf: Shelf { stale: true, ..Shelf::default() },
        }
    }
}

fn dims(size: IVec3) -> String {
    format!("{}×{}×{}", size.x, size.y, size.z)
}

impl BlueprintTool {
    /// Queues a blueprint key press for the next `frame`.
    pub fn press(&mut self, action: Action) {
        if self.queued.len() < 16 {
            self.queued.push(action);
        }
    }

    pub fn clipboard(&self) -> Option<&Blueprint> {
        self.clipboard.as_deref()
    }

    pub fn set_clipboard(&mut self, bp: Blueprint) {
        let solids: Vec<IVec3> = (0..bp.volume()).filter(|&i| bp.cells()[i] != crate::material::BlockId::Air).map(|i| bp.local(i)).collect();
        self.ghost = if solids.len() <= GHOST_CELLS { super::preview::contour(&solids) } else { Vec::new() };
        self.clipboard = Some(Arc::new(bp));
    }

    pub fn selection(&self) -> Option<Region> {
        self.corners.map(|(a, b)| Region::from_corners(a, b))
    }

    /// Done and total cells of the copy or write in progress.
    pub fn progress(&self) -> Option<(usize, usize)> {
        Some(match self.work.as_ref()? {
            Work::Copy { capture, .. } => capture.progress(),
            Work::Paste(job) | Work::Cut(job) => job.progress(),
        })
    }

    /// Drops an armed paste preview; true when one was armed.
    pub fn cancel_paste(&mut self) -> bool {
        std::mem::take(&mut self.armed)
    }

    /// Runs queued presses, then advances any copy or write within `budget`.
    /// `network` refuses writes: a client has no local authority over terrain.
    pub fn frame(&mut self, world: &mut World, creative: &mut CreativeState, target: RaycastTarget, network: bool, history: &mut EditHistory, sink: &mut Sink<'_>, budget: Budget) {
        if self.work.as_ref().is_some_and(|w| !work_world(w, world)) {
            self.work = None;
            self.corners = None;
        }
        if creative.locked() || network {
            self.armed = false;
        }
        for action in std::mem::take(&mut self.queued) {
            if let Some(message) = self.command(action, world, creative, target, network) {
                world.expedition.say(message);
            }
        }
        self.origin = match (&self.clipboard, target.place.or(target.hit)) {
            (Some(bp), Some(cell)) if self.armed => Some(blueprint::origin_at(cell, bp.size())),
            _ => None,
        };
        if let Some(message) = self.advance(world, history, sink, budget) {
            world.expedition.say(message);
        }
    }

    fn command(&mut self, action: Action, world: &World, creative: &mut CreativeState, target: RaycastTarget, network: bool) -> Option<String> {
        let busy = self.work.is_some();
        let writes_refused = || {
            if creative.locked() {
                Some("Blueprints: Creative Only".to_owned())
            } else if network {
                Some("Blueprints: Single Player Only".to_owned())
            } else {
                None
            }
        };
        match action {
            Action::BlueprintCorner => {
                if !creative.choose_tool(ToolKind::AreaTool) {
                    return Some("Blueprints: Creative Only".into());
                }
                let cell = target.hit.or(target.place)?;
                let corners = match self.corners {
                    Some((a, _)) if self.second_corner_next => (a, cell),
                    _ => (cell, cell),
                };
                self.second_corner_next = !self.second_corner_next;
                self.corners = Some(corners);
                let region = Region::from_corners(corners.0, corners.1);
                Some(format!("Selection: {} · {} Cells", dims(region.size()), region.volume()))
            }
            Action::BlueprintCopy | Action::BlueprintCut => {
                let cut = action == Action::BlueprintCut;
                if cut && let Some(refused) = writes_refused() {
                    return Some(refused);
                }
                let Some(region) = self.selection() else { return Some("No Selection".into()) };
                if busy {
                    return Some("Blueprint Busy".into());
                }
                let Some(capture) = Capture::new(world, region) else {
                    return Some(format!("Selection Too Large: {} / {} Cells", region.volume(), blueprint::MAX_VOLUME));
                };
                self.work = Some(Work::Copy { capture, cut });
                None
            }
            Action::BlueprintPaste => {
                if let Some(refused) = writes_refused() {
                    return Some(refused);
                }
                let Some(bp) = self.clipboard.clone() else { return Some("Clipboard Empty".into()) };
                if !self.armed {
                    self.armed = true;
                    return None;
                }
                if busy {
                    return Some("Blueprint Busy".into());
                }
                let origin = self.origin?;
                self.armed = false;
                self.work = Some(Work::Paste(Job::paste(world, bp, origin, !self.paste_air)));
                None
            }
            Action::BlueprintRotate | Action::BlueprintMirror => {
                let bp = self.clipboard.as_deref()?;
                let turned = if action == Action::BlueprintRotate { bp.rotated() } else { bp.mirrored() };
                let size = turned.size();
                self.set_clipboard(turned);
                Some(format!("Clipboard: {}", dims(size)))
            }
            _ => None,
        }
    }

    fn advance(&mut self, world: &mut World, history: &mut EditHistory, sink: &mut Sink<'_>, budget: Budget) -> Option<String> {
        let finished = match self.work.as_mut()? {
            Work::Copy { capture, .. } => capture.step(world, budget, usize::MAX),
            Work::Paste(job) | Work::Cut(job) => job.step(world, sink, budget, usize::MAX),
        };
        if !finished {
            return None;
        }
        Some(match self.work.take()? {
            Work::Copy { capture, cut } => {
                let (region, skipped) = (capture.region(), capture.skipped);
                let bp = capture.finish();
                let mut text = format!("Copied: {} · {} Blocks", dims(bp.size()), bp.solid_count());
                if skipped > 0 {
                    text += &format!(" · Skipped: {skipped}");
                }
                if cut {
                    let ops: Vec<Change> = (0..bp.volume())
                        .filter(|&i| bp.cells()[i] != crate::material::BlockId::Air)
                        .map(|i| Change { p: region.min + bp.local(i), old: bp.cells()[i], new: crate::material::BlockId::Air })
                        .collect();
                    self.work = Some(Work::Cut(Job::changes(world, ops)));
                }
                self.set_clipboard(bp);
                text
            }
            Work::Paste(job) => record(world, history, job, "Pasted"),
            Work::Cut(job) => record(world, history, job, "Cut"),
        })
    }
}

fn record(world: &World, history: &mut EditHistory, job: Job, verb: &str) -> String {
    let skipped = job.skipped;
    let changes = job.into_record();
    let count = changes.len();
    let mut text = format!("{verb}: {count} Blocks");
    if skipped > 0 {
        text += &format!(" · Skipped: {skipped}");
    }
    if !history.push(world, changes) {
        text += " · Undo: Too Large";
    }
    text
}

fn work_world(work: &Work, world: &World) -> bool {
    match work {
        Work::Copy { capture, .. } => capture.same_world(world),
        Work::Paste(job) | Work::Cut(job) => job.same_world(world),
    }
}
