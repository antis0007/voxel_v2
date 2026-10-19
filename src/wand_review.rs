//! Opt-in, unsaved native acceptance scene. Replays the normal tool input path.
use crate::{
    creative::{self, AreaFootprintShape, BrushShape, CreativeState, RaycastTarget, ToolKind},
    player::{FlyMode, Input, Player},
    world::{BlockId as B, World},
};
use glam::{IVec3, Vec3};
use std::{collections::HashSet, time::Instant};

const CASES: &[&str] = &[
    "hand-place",
    "hand-break",
    "builder",
    "destructor",
    "cube",
    "sphere",
    "area-square",
    "area-circle",
    "explosive-4m",
    "explosive-8m",
    "spark",
    "frost",
    "burst",
    "bloom",
    "mend",
    "ward",
    "rift",
    "echo",
    "twin",
    "impact",
    "hand-single-cell",
    "builder-single-cell",
    "destructor-single-cell",
    "explosive-single-cell",
    "burst-close",
    "impact-close",
    "explosive-full-pack",
];
pub struct Review {
    pub automatic: bool,
    phase: usize,
    start: Instant,
    configured: bool,
    fired: bool,
    shots: u8,
    restore: Vec<(IVec3, B)>,
    expected: Vec<(IVec3, B)>,
    saw_projectile: bool,
    saw_sound: bool,
    done: bool,
    finished_edit: bool,
    min_mana: f32,
    peak_projectiles: usize,
    origin: Vec3,
    failures: usize,
    polish: bool,
}

#[cfg(test)]
mod acceptance {
    use super::*;
    use crate::{
        fluid::VoxelScalePreset,
        world::{
            WorldType,
            inventory::{Item, ItemStack},
        },
    };
    #[test]
    fn wand_blast_keeps_full_radius_and_preview_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut c = CreativeState::default();
            c.sync_world_scale(scale.meters());
            c.tool = ToolKind::ExplosiveWand;
            c.explosion_radius = 8;
            let hit = IVec3::new(0, (150.0 / scale.meters()) as i32, 0);
            let ray = RaycastTarget {
                hit: Some(hit),
                place: Some(hit + IVec3::Z),
            };
            let preview = creative::get_preview(&c, &w, ray, false, false, false, false);
            let cells = creative::blast_targets(&c, &w, ray);
            assert_eq!(preview.total_cells, cells.len());
            assert_eq!(
                cells.iter().map(|p| p.x).max().unwrap(),
                (8.0 / scale.meters()) as i32
            );
            c.queue_blast(&w, cells, Some(preview.clone()));
            c.update_feedback(0.1);
            let moved = creative::get_preview(
                &c,
                &w,
                RaycastTarget {
                    hit: Some(hit - IVec3::Z * 20),
                    place: Some(hit - IVec3::Z * 19),
                },
                false,
                false,
                false,
                false,
            );
            assert_eq!(preview.segments, moved.segments);
            c.cancel_edits();
            assert_eq!(c.pending_cells(), 0);
        }
    }
    #[test]
    fn wand_idle_previews_match_primary_action_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut c = CreativeState::default();
            c.sync_world_scale(scale.meters());
            let hit = IVec3::new(0, (150.0 / scale.meters()) as i32, 0);
            let ray = RaycastTarget {
                hit: Some(hit),
                place: Some(hit + IVec3::Z),
            };
            for tool in [
                ToolKind::Hand,
                ToolKind::Brush,
                ToolKind::AreaTool,
                ToolKind::ExplosiveWand,
            ] {
                c.tool = tool;
                let idle = creative::get_preview(&c, &w, ray, false, false, false, false);
                let pressed = creative::get_preview(&c, &w, ray, false, true, false, false);
                assert_eq!(idle.segments, pressed.segments, "{tool:?} {scale:?}");
                if tool != ToolKind::ExplosiveWand {
                    let alt = creative::get_preview(&c, &w, ray, false, false, true, false);
                    let right = creative::get_preview(&c, &w, ray, true, false, false, false);
                    assert_eq!(alt.segments, right.segments, "{tool:?} {scale:?}");
                }
            }
        }
    }
    #[test]
    fn wand_blast_drops_loot_when_pack_is_full_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let item = Item::Block(B::Sand);
            let stack = Some(ItemStack::new(item, w.inventory.stack_limit(item)));
            w.inventory.hotbar.fill(stack);
            w.inventory.pack.fill(stack);
            w.sync_from_inventory();
            let cell = IVec3::new(0, (150.0 / scale.meters()) as i32, 0);
            w.set(cell, B::StoneBricks);
            let mut dirty = HashSet::new();
            let mut chunks = HashSet::new();
            assert!(crate::world::expedition::blast_harvest(
                &mut w,
                cell,
                &mut dirty,
                &mut chunks
            ));
            assert_eq!(w.get(cell), B::Air);
            assert!(
                w.inventory
                    .dropped
                    .iter()
                    .any(|d| d.stack.item == Item::Block(B::StoneBricks) && d.stack.count == 1)
            );
            let limit = w.inventory.stack_limit(Item::Block(B::StoneBricks));
            w.inventory.dropped[0].stack.count = limit;
            w.set(cell, B::StoneBricks);
            assert!(crate::world::expedition::blast_harvest(
                &mut w,
                cell,
                &mut dirty,
                &mut chunks
            ));
            assert_eq!(w.inventory.dropped.len(), 2);
            assert!(
                w.inventory
                    .dropped
                    .iter()
                    .all(|d| d.stack.count <= w.inventory.stack_limit(d.stack.item))
            );
            for _ in 0..(1.0 / scale.meters().powi(3)) as usize {
                w.set(cell, B::IronOre);
                assert!(crate::world::expedition::blast_harvest(
                    &mut w,
                    cell,
                    &mut dirty,
                    &mut chunks
                ));
            }
            assert_eq!(
                w.inventory
                    .dropped
                    .iter()
                    .filter(|d| d.stack.item == Item::Resource(2))
                    .map(|d| d.stack.count)
                    .sum::<u32>(),
                1
            );
        }
    }
}
impl Review {
    pub fn from_env(benchmark: bool) -> Option<Self> {
        if !benchmark {
            return None;
        }
        let mode = std::env::var("VOXEL_WAND_REVIEW").ok()?;
        Some(Self {
            automatic: mode != "manual",
            phase: if mode == "polish" { 4 } else { 0 },
            start: Instant::now(),
            configured: false,
            fired: false,
            shots: 0,
            restore: Vec::new(),
            expected: Vec::new(),
            saw_projectile: false,
            saw_sound: false,
            done: false,
            finished_edit: false,
            min_mana: 100.0,
            peak_projectiles: 0,
            origin: Vec3::ZERO,
            failures: 0,
            polish: mode == "polish",
        })
    }
    fn camera(w: &World, p: &mut Player) {
        let scale = w.voxel_scale.meters();
        p.pos = Vec3::new(0.5, 102.4, 8.0) / scale;
        p.yaw = 0.0;
        p.pitch = -0.23;
        p.vel = Vec3::ZERO;
        p.flying = true;
        p.fly_mode = FlyMode::Noclip;
    }
    fn tool_case(&self) -> bool {
        self.phase < 10 || matches!(self.phase, 20..=23 | 26)
    }
    fn place_case(&self) -> bool {
        matches!(self.phase, 0 | 2 | 20 | 21)
    }
    fn blast_case(&self) -> bool {
        matches!(self.phase, 8 | 9 | 23 | 26)
    }
    pub fn install(&mut self, w: &mut World, p: &mut Player, c: &mut CreativeState) {
        let n = (1.0 / w.voxel_scale.meters()) as i32;
        let mut edits = Vec::new();
        for y in 96 * n..108 * n {
            for z in -5 * n..12 * n {
                for x in -7 * n..7 * n {
                    let b = if z < 0 {
                        B::StoneBricks
                    } else if y < 97 * n {
                        B::Stone
                    } else {
                        B::Air
                    };
                    edits.push((IVec3::new(x, y, z), b));
                }
            }
        }
        w.set_many(&edits);
        w.adventure.wand = 3;
        w.adventure.rare_spells = (1 << 6) | (1 << 7);
        c.close_panels();
        c.sync_world_scale(w.voxel_scale.meters());
        c.placement_distance = 16.0;
        c.assign_hotbar_slot(0, B::StoneBricks);
        Self::camera(w, p);
        self.start = Instant::now();
        println!(
            "[wand-review] installed scale={} automatic={}",
            w.voxel_scale.meters(),
            self.automatic
        );
    }
    pub fn input(
        &mut self,
        w: &mut World,
        p: &mut Player,
        c: &mut CreativeState,
        input: &mut Input,
        dirty: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
    ) {
        if !self.automatic || self.done {
            return;
        }
        input.clear();
        if !self.configured {
            Self::camera(w, p);
            if matches!(self.phase, 24 | 25) {
                p.pos.z = 0.08 / w.voxel_scale.meters();
                p.pitch = 0.0;
            }
            self.origin = p.pos * w.voxel_scale.meters();
            c.cancel_edits();
            c.close_panels();
            w.set_many(&self.restore);
            for (cell, _) in self.restore.drain(..) {
                dirty.insert(cell);
                crate::world::mark_chunk_and_neighbors_dirty(chunks, cell);
            }
            w.adventure.projectiles.clear();
            w.adventure.mana = 100.0;
            w.adventure.health = 60.0;
            w.adventure.loadout = Default::default();
            c.brush_radius = 2;
            c.area_tool.radius = 2;
            c.area_tool.thickness = 1;
            c.wand_max_blocks = 16;
            c.size_in_metres = !matches!(self.phase, 20..=23);
            if !c.size_in_metres {
                c.wand_max_blocks = 1;
            }
            c.explosion_radius = if self.phase == 9 { 8 } else { 4 };
            if self.phase == 23 {
                c.explosion_radius = 1;
            }
            if self.phase == 26 {
                use crate::world::inventory::{Item, ItemStack};
                let item = Item::Block(B::Sand);
                let stack = Some(ItemStack {
                    item,
                    count: w.inventory.stack_limit(item),
                });
                w.inventory.hotbar.fill(stack);
                w.inventory.pack.fill(stack);
                w.sync_from_inventory();
            }
            c.brush_shape = if self.phase == 5 {
                BrushShape::Sphere
            } else {
                BrushShape::Cube
            };
            c.area_tool.shape = if self.phase == 7 {
                AreaFootprintShape::Circle
            } else {
                AreaFootprintShape::Square
            };
            c.tool = match self.phase {
                0 | 1 => ToolKind::Hand,
                2 | 21 => ToolKind::BuildersWand,
                3 | 22 => ToolKind::DestructorWand,
                4 | 5 => ToolKind::Brush,
                6 | 7 => ToolKind::AreaTool,
                8 | 9 | 23 | 26 => ToolKind::ExplosiveWand,
                _ => ToolKind::Hand,
            };
            if !self.tool_case() {
                let spell = match self.phase {
                    10 | 18 | 19 | 25 => 0,
                    11 => 2,
                    12 | 24 => 3,
                    13 => 5,
                    14 => 1,
                    15 => 4,
                    16 => 6,
                    _ => 7,
                };
                w.adventure.loadout.slots = [spell, 8, 8, 8];
                w.adventure.loadout.cursor = 0;
                if self.phase == 18 {
                    w.adventure.loadout.mode = 1;
                }
                if matches!(self.phase, 19 | 25) {
                    w.adventure.loadout.mode = 2;
                    w.adventure.loadout.slots[1] = 3;
                }
            }
            self.expected.clear();
            self.fired = false;
            self.shots = 0;
            self.saw_projectile = false;
            self.saw_sound = false;
            self.finished_edit = false;
            self.min_mana = 100.0;
            self.peak_projectiles = 0;
            self.start = Instant::now();
            self.configured = true;
        }
        // Show the same placement mode the upcoming right-click uses.
        input.alt_down = matches!(self.phase, 0 | 20);
        if self.start.elapsed().as_secs_f32() >= 1.5 && !self.fired {
            let hit = crate::world::raycast_block_precise(
                w,
                p.precise_eye(w.voxel_scale),
                p.forward_dir(),
                c.reach_distance_blocks(w.voxel_scale),
            );
            let ray = RaycastTarget {
                hit: hit.map(|h| h.0),
                place: hit.map(|h| h.1),
            };
            let cells = if self.blast_case() {
                creative::blast_targets(c, w, ray)
            } else if self.tool_case() {
                creative::edit_blocks(
                    c,
                    w,
                    p,
                    ray,
                    self.place_case(),
                    !self.place_case(),
                    input.alt_down,
                )
                .map(|v| v.1)
                .unwrap_or_default()
            } else if matches!(self.phase, 12 | 19 | 24 | 25) {
                hit.map(|h| {
                    creative::spell_blast_targets(
                        w.voxel_scale.meters(),
                        (h.1.as_vec3() + Vec3::splat(0.5)) * w.voxel_scale.meters(),
                        2.5,
                    )
                })
                .unwrap_or_default()
            } else {
                Vec::new()
            };
            self.restore = cells.iter().map(|&cell| (cell, w.get(cell))).collect();
            self.expected = self
                .restore
                .iter()
                .filter_map(|&(cell, old)| {
                    if !self.tool_case() {
                        return None;
                    }
                    let place = self.place_case();
                    if place && !old.is_empty() && !old.is_liquid() {
                        None
                    } else {
                        Some((cell, if place { B::StoneBricks } else { B::Air }))
                    }
                })
                .collect();
            input.place_block = self.place_case();
            input.break_block = self.tool_case() && !self.place_case();
            input.cast_spell = !self.tool_case();
            self.fired = true;
            println!(
                "[wand-review] fire {} hit={:?} targets={}",
                CASES[self.phase],
                hit,
                self.expected.len()
            );
        }
    }
    pub fn observe(
        &mut self,
        w: &World,
        p: &Player,
        c: &CreativeState,
        preview: &creative::PreviewResult,
    ) -> Option<String> {
        if !self.automatic || self.done {
            return None;
        }
        self.saw_projectile |= !w.adventure.projectiles.is_empty();
        self.saw_sound |= !w.adventure.sounds.is_empty();
        self.min_mana = self.min_mana.min(w.adventure.mana);
        self.peak_projectiles = self.peak_projectiles.max(w.adventure.projectiles.len());
        let elapsed = self.start.elapsed().as_secs_f32();
        let name = CASES[self.phase];
        if self.fired && self.tool_case() && c.pending_cells() == 0 && !self.finished_edit {
            self.finished_edit = true;
            println!(
                "[wand-review] edit-complete {name} scale={} after_ms={:.1}",
                w.voxel_scale.meters(),
                (elapsed - 1.5) * 1000.0
            );
        }
        let shot = if self.shots == 0 && elapsed > 0.8 {
            self.shots = 1;
            Some("preview")
        } else if self.shots == 1 && elapsed > 1.58 {
            self.shots = 2;
            Some("action")
        } else if self.shots == 2 && elapsed > 3.5 && c.pending_cells() == 0 {
            self.shots = 3;
            Some("result")
        } else {
            None
        };
        let result = shot.map(|s| format!("{}-{name}-{s}.png", w.voxel_scale.meters()));
        if shot == Some("preview") {
            println!(
                "[wand-review] preview {name} cells={} lines={} finite={}",
                preview.total_cells,
                preview.segments.len(),
                preview.segments.iter().flatten().all(|v| v.is_finite())
            );
        }
        if elapsed > 5.0 && c.pending_cells() == 0 || elapsed > 20.0 {
            let wrong = self
                .expected
                .iter()
                .filter(|&&(cell, b)| w.get(cell) != b)
                .count();
            let changed = self
                .restore
                .iter()
                .filter(|&&(cell, b)| w.get(cell) != b)
                .count();
            let travelled = (p.pos * w.voxel_scale.meters()).distance(self.origin);
            let passed = wrong == 0
                && c.pending_cells() == 0
                && if self.tool_case() {
                    changed > 0 && (self.phase != 26 || !w.inventory.dropped.is_empty())
                } else {
                    self.min_mana < 99.0
                        && self.saw_sound
                        && match self.phase {
                            12 | 19 | 24 | 25 => changed > 0,
                            14 => w.adventure.health > 85.0,
                            15 => w.adventure.ward_hits > 0,
                            17 => travelled > 0.75,
                            18 => self.peak_projectiles >= 2,
                            _ => self.saw_projectile,
                        }
                };
            if !passed {
                self.failures += 1;
            }
            if self.blast_case() {println!("[wand-review] blast audio={} drops={} bounded={}",crate::audio::status(),w.inventory.dropped.len(),w.inventory.dropped.iter().all(|d|d.stack.count<=w.inventory.stack_limit(d.stack.item)));}
            println!(
                "[wand-review] acceptance {name} {} min_mana={:.1} peak_shots={} travelled={travelled:.2}",
                if passed { "PASS" } else { "FAIL" },
                self.min_mana,
                self.peak_projectiles
            );
            println!(
                "[wand-review] result {name} scale={} mismatch={wrong} changed={changed} pending={} projectile={} sound={} health={} mana={} elapsed={elapsed:.2}",
                w.voxel_scale.meters(),
                c.pending_cells(),
                self.saw_projectile,
                self.saw_sound,
                w.adventure.health,
                w.adventure.mana
            );
            self.phase += 1;
            if self.polish && self.phase == 10 {
                self.phase = 20;
            }
            self.configured = false;
            if self.phase == CASES.len() {
                self.done = true;
                println!(
                    "[wand-review] COMPLETE scale={} failures={}",
                    w.voxel_scale.meters(),
                    self.failures
                );
            }
        }
        result
    }
}
