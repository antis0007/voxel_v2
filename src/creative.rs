use glam::{IVec3, Vec3};
use std::collections::{HashSet, VecDeque};

use crate::fluid::VoxelScalePreset;
use crate::material::BlockId;
use crate::player::Player;
use crate::world::{World, WorldSampleConfig, player_aabb};
use std::cell::RefCell;

pub const HOTBAR_SLOTS: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Hand,
    Brush,
    BuildersWand,
    DestructorWand,
    AreaTool,
    ExplosiveWand,
}

impl ToolKind {
    pub const ALL: [Self; 6] = [
        Self::Hand,
        Self::Brush,
        Self::BuildersWand,
        Self::DestructorWand,
        Self::AreaTool,
        Self::ExplosiveWand,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hand => "Hand",
            Self::Brush => "Brush",
            Self::BuildersWand => "Builder Wand",
            Self::DestructorWand => "Destructor Wand",
            Self::AreaTool => "Area Tool",
            Self::ExplosiveWand => "Explosive Wand",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushMode {
    Place,
    Erase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushShape {
    Cube,
    Sphere,
}

impl BrushShape {
    pub const ALL: [Self; 2] = [Self::Cube, Self::Sphere];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cube => "Cube",
            Self::Sphere => "Sphere",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaFootprintShape {
    Circle,
    Square,
}

impl AreaFootprintShape {
    pub const ALL: [Self; 2] = [Self::Circle, Self::Square];

    pub fn label(self) -> &'static str {
        match self {
            Self::Circle => "Circle",
            Self::Square => "Square",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AreaToolSettings {
    pub radius: i32,
    pub thickness: i32,
    pub shape: AreaFootprintShape,
}

impl Default for AreaToolSettings {
    fn default() -> Self {
        Self {
            radius: 0,
            thickness: 1,
            shape: AreaFootprintShape::Circle,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CreativeState {
    pub size_in_metres: bool,
    pub hand_size_m: f32,
    pub active_voxel_m: f32,
    pending: Option<EditStroke>,
    blast_preview: Option<(PreviewResult, f32)>,
    preview_cache: RefCell<Option<(PreviewKey, PreviewResult)>>,
    pub show_panel: bool,
    pub show_palette: bool,
    pub show_material_editor: bool,
    pub show_quick_select: bool,
    pub tool: ToolKind,
    pub brush_shape: BrushShape,
    pub brush_radius: i32,
    pub area_tool: AreaToolSettings,
    pub explosion_radius: i32,
    pub wand_max_blocks: usize,
    pub placement_distance: f32,
    pub preview_cap: usize,
    pub selected_slot: usize,
    pub palette_search: String,
    hotbar: [BlockId; HOTBAR_SLOTS],
}

impl Default for CreativeState {
    fn default() -> Self {
        let defaults = BlockId::hotbar_defaults();
        let fallback = defaults.first().copied().unwrap_or(BlockId::Grass);
        let mut hotbar = [fallback; HOTBAR_SLOTS];
        for (slot, block) in hotbar.iter_mut().zip(defaults.iter().copied()) {
            *slot = block;
        }
        Self {
            size_in_metres: true,
            hand_size_m: 1.0,
            active_voxel_m: 1.0,
            pending: None,
            blast_preview: None,
            preview_cache: RefCell::new(None),
            show_panel: false,
            show_palette: false,
            show_material_editor: false,
            show_quick_select: false,
            tool: ToolKind::Hand,
            brush_shape: BrushShape::Cube,
            brush_radius: 0,
            area_tool: AreaToolSettings::default(),
            explosion_radius: 4,
            wand_max_blocks: 4,
            placement_distance: 5.0,
            preview_cap: 512,
            selected_slot: 0,
            palette_search: String::new(),
            hotbar,
        }
    }
}

// Largest supported 8 m brush at 0.25 m: 65^3 cells. Never truncate a stroke.
const MAX_TOOL_CELLS: usize = 524288;
mod preview;
#[derive(Clone, Debug)]
struct EditStroke {
    cfg: WorldSampleConfig,
    mode: BrushMode,
    block: BlockId,
    cells: VecDeque<IVec3>,
    explosive: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviewKey {
    cfg: WorldSampleConfig,
    revision: u64,
    target: RaycastTarget,
    settings: [u64; 12],
}
impl CreativeState {
    pub fn sync_world_scale(&mut self, metres: f32) {
        if self.active_voxel_m == metres {
            return;
        }
        self.active_voxel_m = metres;
        self.hand_size_m = ((self.hand_size_m / metres).round().max(1.0) * metres)
            .min(self.hand_width_limit());
        self.brush_radius = self.brush_radius.min(self.radius_limit());
        self.area_tool.radius = self.area_tool.radius.min(self.radius_limit());
        self.explosion_radius = self.explosion_radius.min(self.radius_limit());
        self.pending = None;
        self.blast_preview = None;
        *self.preview_cache.borrow_mut() = None;
    }
    pub fn radius_cells(&self, units: i32, scale: VoxelScalePreset) -> i32 {
        let factor = if self.size_in_metres {
            scale.meters().recip()
        } else {
            1.0
        };
        (units.max(0) as f32 * factor).round().clamp(0.0, 32.0) as i32
    }
    pub fn radius_limit(&self) -> i32 {
        8
    }
    pub fn size_unit(&self) -> &'static str {
        if self.size_in_metres { "m" } else { "voxels" }
    }
    pub fn thickness_cells(&self, units: i32, scale: VoxelScalePreset) -> i32 {
        let factor = if self.size_in_metres {
            scale.meters().recip()
        } else {
            1.0
        };
        (units.max(1) as f32 * factor).round().clamp(1.0, 32.0) as i32
    }
    pub fn hand_side(&self, scale: VoxelScalePreset) -> i32 {
        if self.size_in_metres {
            (self.hand_size_m / scale.meters()).round().clamp(1.0, 8.0) as i32
        } else {
            1
        }
    }
    pub fn hand_width_limit(&self) -> f32 {
        (8.0 * self.active_voxel_m).min(2.0)
    }
    pub fn pending_cells(&self) -> usize {
        self.pending.as_ref().map_or(0, |e| e.cells.len())
    }
    pub fn pending_mode(&self) -> Option<BrushMode> { self.pending.as_ref().map(|e|e.mode) }
    pub fn pending_explosive(&self) -> bool {self.pending.as_ref().is_some_and(|e|e.explosive)}
    pub fn blast_ready(&self)->bool {self.pending_cells()==0 && self.blast_preview.is_none()}
    pub fn update_feedback(&mut self, dt:f32) {
        if let Some((_,time))=&mut self.blast_preview {*time-=dt;}
        if self.pending_cells()==0 && self.blast_preview.as_ref().is_some_and(|(_,t)|*t<=0.0) {self.blast_preview=None;}
    }
    pub fn queue_blast(&mut self, world:&World, cells:Vec<IVec3>, preview:Option<PreviewResult>) {
        if self.pending_cells()>0 {return;}
        self.queue_edit(world,BrushMode::Erase,BlockId::Air,cells);
        if let Some(e)=&mut self.pending {e.explosive=true;}
        self.blast_preview=preview.map(|p|(p,0.45));
    }
    pub fn queue_edit(
        &mut self,
        world: &World,
        mode: BrushMode,
        block: BlockId,
        cells: Vec<IVec3>,
    ) {
        if self.pending_cells() == 0 {
            self.pending = Some(EditStroke {
                cfg: world.sample_config(),
                mode,
                block,
                cells: cells.into(),
                explosive: false,
            });
        }
    }
    pub fn next_edit_position(&mut self, cfg: WorldSampleConfig) -> Option<IVec3> {
        if self.pending.as_ref().is_some_and(|e| e.cfg != cfg) {
            self.pending = None;
        }
        self.pending.as_ref()?.cells.front().copied()
    }
    pub fn next_edit(&mut self, cfg: WorldSampleConfig) -> Option<(BrushMode, IVec3, BlockId)> {
        let edit = self.pending.as_mut()?;
        if edit.cfg != cfg {
            self.pending = None;
            return None;
        }
        if let Some(cell) = edit.cells.pop_front() {
            Some((edit.mode, cell, edit.block))
        } else {
            self.pending = None;
            None
        }
    }
    pub fn close_panels(&mut self) {
        *self.preview_cache.borrow_mut() = None;
        self.show_panel = false;
        self.show_palette = false;
        self.show_material_editor = false;
        self.show_quick_select = false;
    }
    pub fn cancel_edits(&mut self) {self.pending=None;self.blast_preview=None;*self.preview_cache.borrow_mut()=None;}

    pub fn pick_block(&mut self, block: BlockId) -> BlockId {
        let block = if block.is_torch() {
            BlockId::Torch
        } else {
            block
        };
        if let Some(slot) = self.hotbar.iter().position(|&item| item == block) {
            self.select_slot(slot)
        } else {
            self.assign_hotbar_slot(self.selected_slot, block);
            block
        }
    }
    pub fn hotbar(&self) -> &[BlockId; HOTBAR_SLOTS] {
        &self.hotbar
    }

    pub fn selected_block(&self) -> BlockId {
        self.hotbar[self.selected_slot.min(HOTBAR_SLOTS - 1)]
    }

    pub fn select_slot(&mut self, slot: usize) -> BlockId {
        self.selected_slot = slot.min(HOTBAR_SLOTS - 1);
        self.selected_block()
    }

    pub fn assign_hotbar_slot(&mut self, slot: usize, block: BlockId) {
        if slot >= HOTBAR_SLOTS {
            return;
        }
        self.hotbar[slot] = block;
        self.selected_slot = slot;
    }

    pub fn swap_hotbar_slots(&mut self, a: usize, b: usize) {
        if a >= HOTBAR_SLOTS || b >= HOTBAR_SLOTS || a == b {
            return;
        }
        self.hotbar.swap(a, b);
        if self.selected_slot == a {
            self.selected_slot = b;
        } else if self.selected_slot == b {
            self.selected_slot = a;
        }
    }

    pub fn cycle_hotbar_slot(&mut self, dir: i32) -> BlockId {
        let next = (self.selected_slot as i32 + dir).rem_euclid(HOTBAR_SLOTS as i32) as usize;
        self.selected_slot = next;
        self.selected_block()
    }

    pub fn any_ui_open(&self) -> bool {
        self.show_panel || self.show_palette || self.show_quick_select || self.show_material_editor
    }

    pub fn reach_distance_blocks(&self, voxel_scale: VoxelScalePreset) -> f32 {
        self.placement_distance / voxel_scale.meters().max(1e-6)
    }

    pub fn preview_mode(
        &self,
        place_pressed: bool,
        break_pressed: bool,
        preview_pressed: bool,
    ) -> BrushMode {
        match self.tool {
            ToolKind::BuildersWand => BrushMode::Place,
            ToolKind::DestructorWand | ToolKind::ExplosiveWand => BrushMode::Erase,
            _ if place_pressed => BrushMode::Place,
            _ if break_pressed => BrushMode::Erase,
            _ if preview_pressed => BrushMode::Place,
            _ => BrushMode::Erase,
        }
    }

    pub fn adjust_cursor_size(&mut self, delta: i32) {
        match self.tool {
            ToolKind::Hand => {
                self.size_in_metres = true;
                self.hand_size_m = (self.hand_size_m + delta as f32 * self.active_voxel_m)
                    .clamp(self.active_voxel_m, self.hand_width_limit());
            }
            ToolKind::Brush => {
                self.brush_radius = (self.brush_radius + delta).clamp(0, self.radius_limit());
            }
            ToolKind::AreaTool => {
                self.area_tool.radius =
                    (self.area_tool.radius + delta).clamp(0, self.radius_limit());
            }
            ToolKind::BuildersWand | ToolKind::DestructorWand => {
                self.wand_max_blocks =
                    (self.wand_max_blocks as i32 + delta * if self.wand_max_blocks <= 16 { 1 } else { 8 }).clamp(1, 512) as usize;
            }
            ToolKind::ExplosiveWand => {
                self.explosion_radius =
                    (self.explosion_radius + delta).clamp(1, self.radius_limit());
            }
        }
    }

    pub fn adjust_range(&mut self, delta: i32) {
        self.placement_distance = (self.placement_distance + delta as f32 * 0.5).clamp(2.0, 24.0);
    }

    pub fn edit_repeat_interval_seconds(&self) -> f32 {
        match self.tool {
            ToolKind::Hand => 0.22,
            ToolKind::Brush => 0.16,
            ToolKind::AreaTool => 0.14,
            ToolKind::BuildersWand | ToolKind::DestructorWand => 0.12,
            // Explosions are deliberate clicks, never an automatic held-button stream.
            ToolKind::ExplosiveWand => 3600.0,
        }
    }

    pub fn cursor_summary(&self) -> String {
        match self.tool {
            ToolKind::Hand => format!(
                "Build {:.2}m | reach {:.1}m",
                if self.size_in_metres {
                    self.hand_size_m
                } else {
                    self.active_voxel_m
                },
                self.placement_distance
            ),
            ToolKind::Brush => {
                format!(
                    "Brush radius {} {} | reach {:.1}m",
                    self.brush_radius,
                    self.size_unit(),
                    self.placement_distance
                )
            }
            ToolKind::AreaTool => format!(
                "Area radius {} {}, depth {} {} | reach {:.1}m",
                self.area_tool.radius,
                self.size_unit(),
                self.area_tool.thickness,
                self.size_unit(),
                self.placement_distance
            ),
            ToolKind::BuildersWand | ToolKind::DestructorWand => {
                format!(
                    "Wand area {} {} | reach {:.1}m",
                    self.wand_max_blocks,
                    if self.size_in_metres { "m²" } else { "faces" },
                    self.placement_distance
                )
            }
            ToolKind::ExplosiveWand => format!(
                "Blast radius {} {} | reach {:.1}m",
                self.explosion_radius,
                self.size_unit(),
                self.placement_distance
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RaycastTarget {
    pub hit: Option<IVec3>,
    pub place: Option<IVec3>,
}

impl From<Option<(IVec3, IVec3)>> for RaycastTarget {
    fn from(value: Option<(IVec3, IVec3)>) -> Self {
        match value {
            Some((hit, place)) => Self {
                hit: Some(hit),
                place: Some(place),
            },
            None => Self::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreviewResult {
    pub blocks: Vec<IVec3>,
    pub segments: Vec<[Vec3;2]>,
    pub total_cells: usize,
    pub color: [f32; 4],
    pub invert_blend: bool,
    pub center_block: Option<IVec3>,
}

pub fn get_preview(
    creative: &CreativeState,
    world: &World,
    raycast: RaycastTarget,
    place_pressed: bool,
    break_pressed: bool,
    preview_pressed: bool,
    is_executing: bool,
) -> PreviewResult {
    if creative.tool==ToolKind::ExplosiveWand {
        if let Some((preview,_))=&creative.blast_preview {return preview.clone();}
    }
    let key = PreviewKey {
        cfg: world.sample_config(),
        revision: if matches!(creative.tool,ToolKind::BuildersWand|ToolKind::DestructorWand|ToolKind::Hand) {world.revision()} else {0},
        target: raycast,
        settings: [
            creative.tool as u64,
            creative.brush_shape as u64,
            creative.brush_radius as u64,
            creative.area_tool.radius as u64,
            creative.area_tool.thickness as u64,
            creative.area_tool.shape as u64,
            creative.explosion_radius as u64,
            creative.wand_max_blocks as u64,
            creative.hand_size_m.to_bits() as u64,
            creative.preview_cap as u64,
            creative.selected_block().as_u32() as u64,
            (creative.size_in_metres as u64)
                | ((place_pressed as u64) << 1)
                | ((break_pressed as u64) << 2)
                | ((preview_pressed as u64) << 3)
                | ((is_executing as u64) << 4),
        ],
    };
    if let Some((cached, result)) = creative.preview_cache.borrow().as_ref() {
        if *cached == key {
            return result.clone();
        }
    }
    let result = generate_preview(
        creative,
        world,
        raycast,
        place_pressed,
        break_pressed,
        preview_pressed,
        is_executing,
        true,
    );
    *creative.preview_cache.borrow_mut() = Some((key, result.clone()));
    result
}
fn generate_preview(
    creative: &CreativeState,
    world: &World,
    raycast: RaycastTarget,
    place_pressed: bool,
    break_pressed: bool,
    preview_pressed: bool,
    _is_executing: bool,
    limit_preview: bool,
) -> PreviewResult {
    let mode = creative.preview_mode(place_pressed, break_pressed, preview_pressed);
    let show_placement = mode == BrushMode::Place;
    if limit_preview && matches!(creative.tool,ToolKind::Brush|ToolKind::AreaTool|ToolKind::ExplosiveWand) {
        return preview::volume(creative,world.voxel_scale,raycast,mode,show_placement);
    }

    let (mut blocks, color, invert_blend, center_block) = match creative.tool {
        ToolKind::Hand => {
            let target = match mode {
                BrushMode::Place if show_placement => {
                    if preview_pressed {
                        raycast.place.or(raycast.hit)
                    } else {
                        raycast.hit.and(raycast.place)
                    }
                }
                BrushMode::Erase => raycast.hit,
                _ => raycast.hit,
            };
            let mut blocks = Vec::new();
            if let Some(target) = target {
                let object = world.get(target).is_model()
                    || world.get(target).is_torch()
                    || (mode == BrushMode::Place
                        && (creative.selected_block().is_model()
                            || creative.selected_block().is_torch()));
                let side = if object {
                    1
                } else {
                    creative.hand_side(world.voxel_scale)
                };
                let mut origin = IVec3::new(
                    target.x.div_euclid(side),
                    target.y.div_euclid(side),
                    target.z.div_euclid(side),
                ) * side;
                if mode == BrushMode::Place && side > 1 {
                    if let (Some(hit), Some(place)) = (raycast.hit, raycast.place) {
                        let normal = place - hit;
                        for axis in 0..3 {
                            if normal[axis] > 0 {
                                origin[axis] = hit[axis] + 1;
                            } else if normal[axis] < 0 {
                                origin[axis] = hit[axis] - side;
                            }
                        }
                    }
                }
                for z in 0..side {
                    for y in 0..side {
                        for x in 0..side {
                            blocks.push(origin + IVec3::new(x, y, z));
                        }
                    }
                }
            }
            let color = if mode == BrushMode::Place {
                [0.3, 0.85, 1.0, 1.0]
            } else {
                [1.0, 0.42, 0.32, 1.0]
            };
            (blocks, color, false, None)
        }
        ToolKind::Brush => {
            let center = match mode {
                BrushMode::Place if show_placement => raycast.place.or(raycast.hit),
                _ => raycast.hit.or(raycast.place),
            };
            let mut blocks = Vec::new();
            let mut actual_center = None;
            if let Some(center) = center {
                actual_center = Some(center);
                let radius = creative.radius_cells(creative.brush_radius, world.voxel_scale);
                for dz in -radius..=radius {
                    for dy in -radius..=radius {
                        for dx in -radius..=radius {
                            let include = match creative.brush_shape {
                                BrushShape::Cube => true,
                                BrushShape::Sphere => {
                                    dx * dx + dy * dy + dz * dz <= radius * radius
                                }
                            };
                            if include {
                                blocks.push(center + IVec3::new(dx, dy, dz));
                            }
                        }
                    }
                }
            }
            (blocks, [1.0, 1.0, 1.0, 1.0], true, actual_center)
        }
        ToolKind::AreaTool => {
            let mut blocks = Vec::new();
            let mut actual_center = None;
            if let (Some(hit), Some(place)) = (raycast.hit, raycast.place) {
                let center = match mode {
                    BrushMode::Place if show_placement => place,
                    _ => hit,
                };
                actual_center = Some(center);
                let normal = place - hit;
                if normal != IVec3::ZERO {
                    let (axis_u, axis_v) = if normal.y != 0 {
                        (IVec3::X, IVec3::Z)
                    } else if normal.x != 0 {
                        (IVec3::Y, IVec3::Z)
                    } else {
                        (IVec3::X, IVec3::Y)
                    };
                    let radius =
                        creative.radius_cells(creative.area_tool.radius, world.voxel_scale);
                    let thickness =
                        creative.thickness_cells(creative.area_tool.thickness, world.voxel_scale);
                    for dv in -radius..=radius {
                        for du in -radius..=radius {
                            let include = match creative.area_tool.shape {
                                AreaFootprintShape::Circle => du * du + dv * dv <= radius * radius,
                                AreaFootprintShape::Square => true,
                            };
                            if include {
                                for depth in 0..thickness {
                                    blocks.push(
                                        center
                                            + axis_u * du
                                            + axis_v * dv
                                            + normal
                                                * depth
                                                * if mode == BrushMode::Erase { -1 } else { 1 },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            (blocks, [1.0, 1.0, 1.0, 1.0], true, actual_center)
        }
        ToolKind::BuildersWand => {
            let blocks = wand_targets(creative, world, raycast, true);
            let center = raycast.hit;
            (blocks, [0.1, 0.5, 1.0, 1.0], false, center)
        }
        ToolKind::DestructorWand => {
            let blocks = wand_targets(creative, world, raycast, false);
            let center = raycast.hit;
            (blocks, [1.0, 0.05, 0.05, 1.0], false, center)
        }
        ToolKind::ExplosiveWand => {
            let center = raycast.hit;
            let mut blocks = Vec::new();
            if let Some(center) = center {
                let radius = creative
                    .radius_cells(creative.explosion_radius, world.voxel_scale)
                    .max(1);
                for z in -radius..=radius {
                    for y in -radius..=radius {
                        for x in -radius..=radius {
                            if x * x + y * y + z * z <= radius * radius {
                                let p = center + IVec3::new(x, y, z);
                                blocks.push(p);
                            }
                        }
                    }
                }
            }
            (blocks, [1.0, 0.38, 0.08, 1.0], false, center)
        }
    };

    let segments = if limit_preview { preview::boundary(creative,world.voxel_scale,raycast,&blocks) } else {Vec::new()};
    let total_cells = blocks.len();
    if limit_preview {
        blocks = cap_blocks(
            blocks,
            creative.preview_cap,
        );
    }
    PreviewResult {
        blocks,
        segments,
        total_cells,
        color,
        invert_blend,
        center_block,
    }
}

pub fn blast_targets(c: &CreativeState, w: &World, ray: RaycastTarget) -> Vec<IVec3> {
    let mut cells=generate_preview(c, w, ray, false, true, false, true, false).blocks;
    if let Some(center)=ray.hit {cells.sort_unstable_by_key(|p|(*p-center).length_squared());}
    cells
}

pub fn spell_blast_targets(scale:f32,point:Vec3,radius:f32)->Vec<IVec3> {
    let center=(point/scale).floor().as_ivec3();let radius=(radius/scale).ceil().clamp(1.0,32.0) as i32;
    let mut cells=Vec::new();
    for z in -radius..=radius {for y in -radius..=radius {for x in -radius..=radius {
        if x*x+y*y+z*z<=radius*radius {cells.push(center+IVec3::new(x,y,z));}
    }}}
    cells.sort_unstable_by_key(|p|(*p-center).length_squared());cells
}

pub fn edit_blocks(
    creative: &CreativeState,
    world: &World,
    player: &Player,
    raycast: RaycastTarget,
    place_pressed: bool,
    break_pressed: bool,
    preview_pressed: bool,
) -> Option<(BrushMode, Vec<IVec3>)> {
    if creative.pending_cells() > 0 {
        return None;
    }
    let mode = match creative.tool {
        ToolKind::Hand if place_pressed => Some(BrushMode::Place),
        ToolKind::Hand if break_pressed => Some(BrushMode::Erase),
        ToolKind::BuildersWand if place_pressed => Some(BrushMode::Place),
        ToolKind::DestructorWand if break_pressed => Some(BrushMode::Erase),
        ToolKind::Brush | ToolKind::AreaTool if place_pressed => Some(BrushMode::Place),
        ToolKind::Brush | ToolKind::AreaTool if break_pressed => Some(BrushMode::Erase),
        _ => None,
    }?;

    let preview = generate_preview(
        creative,
        world,
        raycast,
        place_pressed,
        break_pressed,
        preview_pressed,
        true,
        false,
    );
    let mut targets = preview.blocks;
    if mode == BrushMode::Place {
        targets.retain(|&cell| {
            let b = world.get(cell);
            b.is_empty() || b.is_liquid()
        });
        let (player_min, player_max) =
            player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
        targets.retain(|cell| !block_overlaps_aabb(*cell, player_min, player_max));
    }
    Some((mode, targets))
}

fn cap_blocks(mut blocks: Vec<IVec3>, cap: usize) -> Vec<IVec3> {
    if blocks.len() > cap {
        let length = blocks.len();
        blocks = (0..cap).map(|i| blocks[i * length / cap]).collect();
    }
    blocks
}

fn wand_targets(c: &CreativeState, world: &World, ray: RaycastTarget, build: bool) -> Vec<IVec3> {
    let thickness = c.thickness_cells(1, world.voxel_scale);
    let area_scale = if c.size_in_metres {
        world.voxel_scale.meters().recip().powi(2)
    } else {
        1.0
    };
    let count = ((c.wand_max_blocks as f32 * area_scale) as usize)
        .min(MAX_TOOL_CELLS / thickness as usize);
    let normal = match (ray.hit, ray.place) {
        (Some(h), Some(p)) => p - h,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for p in preview_wand_blocks(world, ray, build, count) {
        for depth in 0..thickness {
            out.push(p + normal * depth * if build { 1 } else { -1 });
        }
    }
    out
}
fn preview_wand_blocks(
    world: &World,
    raycast: RaycastTarget,
    build_mode: bool,
    max_blocks: usize,
) -> Vec<IVec3> {
    if max_blocks == 0 {
        return Vec::new();
    }
    let Some(hit) = raycast.hit else {
        return Vec::new();
    };
    let Some(place) = raycast.place else {
        return Vec::new();
    };
    let normal = place - hit;
    if normal == IVec3::ZERO {
        return Vec::new();
    }

    let source_mat = world.get(hit);
    if source_mat == BlockId::Air {
        return Vec::new();
    }

    let axis = if normal.x != 0 {
        0
    } else if normal.y != 0 {
        1
    } else {
        2
    };

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut out = Vec::new();
    // Cap traversal separately from output count so blocked faces cannot flood-fill
    // across an effectively unbounded support plane.
    let search_budget = max_blocks.saturating_mul(16).max(max_blocks);
    queue.push_back(hit);
    visited.insert(hit);

    while let Some(p) = queue.pop_front() {
        if out.len() >= max_blocks || visited.len() > search_budget {
            break;
        }

        let target = if build_mode { p + normal } else { p };
        let target_block = world.get(target);
        let exposed = world.get(p+normal).is_empty() || world.get(p+normal).is_liquid();
        if exposed && ((build_mode && (target_block.is_empty() || target_block.is_liquid())) || !build_mode) {
            out.push(target);
        }

        for dir in [
            IVec3::X,
            -IVec3::X,
            IVec3::Y,
            -IVec3::Y,
            IVec3::Z,
            -IVec3::Z,
        ] {
            if dir[axis] != 0 {
                continue;
            }
            let next = p + dir;
            if visited.contains(&next) || world.get(next) != source_mat {
                continue;
            }
            visited.insert(next);
            queue.push_back(next);
        }
    }

    out
}

pub(crate) fn block_overlaps_aabb(block: IVec3, min: Vec3, max: Vec3) -> bool {
    let block_min = block.as_vec3();
    let block_max = block_min + Vec3::ONE;
    min.x < block_max.x
        && max.x > block_min.x
        && min.y < block_max.y
        && max.y > block_min.y
        && min.z < block_max.z
        && max.z > block_min.z
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_targets_single_place_block() {
        let creative = CreativeState::default();
        let preview = generate_preview(
            &creative,
            &World::new(
                crate::world::WorldType::Superflat,
                crate::fluid::VoxelScalePreset::OneMeter,
            ),
            RaycastTarget {
                hit: Some(IVec3::new(4, 5, 6)),
                place: Some(IVec3::new(4, 6, 6)),
            },
            true,
            false,
            false,
            false,
            false,
        );
        assert_eq!(preview.blocks, vec![IVec3::new(4, 6, 6)]);
    }

    #[test]
    fn hotbar_selection_cycles_within_slot_count() {
        let mut creative = CreativeState::default();
        creative.selected_slot = HOTBAR_SLOTS - 1;
        creative.cycle_hotbar_slot(1);
        assert_eq!(creative.selected_slot, 0);
    }

    #[test]
    fn builder_wand_limits_search_on_blocked_superflat_surface() {
        let world = World::new(
            crate::world::WorldType::Superflat,
            crate::fluid::VoxelScalePreset::OneMeter,
        );
        let stone_y = 0;
        let blocks = preview_wand_blocks(
            &world,
            RaycastTarget {
                hit: Some(IVec3::new(0, stone_y, 0)),
                place: Some(IVec3::new(1, stone_y, 0)),
            },
            true,
            64,
        );
        assert!(blocks.is_empty());
    }

    #[test]
    fn assigning_hotbar_slot_updates_selected_block() {
        let mut creative = CreativeState::default();
        creative.assign_hotbar_slot(2, BlockId::Torch);
        assert_eq!(creative.selected_slot, 2);
        assert_eq!(creative.selected_block(), BlockId::Torch);
    }
}
