use glam::{IVec3, Vec3};
use std::collections::{HashSet, VecDeque};

use crate::fluid::VoxelScalePreset;
use crate::material::BlockId;
use crate::player::Player;
use crate::world::{World, player_aabb};

pub const HOTBAR_SLOTS: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Hand,
    Brush,
    BuildersWand,
    DestructorWand,
    AreaTool,
}

impl ToolKind {
    pub const ALL: [Self; 5] = [
        Self::Hand,
        Self::Brush,
        Self::BuildersWand,
        Self::DestructorWand,
        Self::AreaTool,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hand => "Hand",
            Self::Brush => "Brush",
            Self::BuildersWand => "Builder Wand",
            Self::DestructorWand => "Destructor Wand",
            Self::AreaTool => "Area Tool",
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
    pub show_panel: bool,
    pub show_palette: bool,
    pub show_quick_select: bool,
    pub tool: ToolKind,
    pub brush_shape: BrushShape,
    pub brush_radius: i32,
    pub area_tool: AreaToolSettings,
    pub wand_max_blocks: usize,
    pub placement_distance: f32,
    pub preview_cap: usize,
    pub selected_slot: usize,
    pub shift_lock: bool,
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
            show_panel: false,
            show_palette: false,
            show_quick_select: false,
            tool: ToolKind::Hand,
            brush_shape: BrushShape::Cube,
            brush_radius: 0,
            area_tool: AreaToolSettings::default(),
            wand_max_blocks: 64,
            placement_distance: 5.0,
            preview_cap: 512,
            selected_slot: 0,
            shift_lock: false,
            hotbar,
        }
    }
}

impl CreativeState {
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
        self.show_panel || self.show_palette || self.show_quick_select
    }

    pub fn reach_distance_blocks(&self, voxel_scale: VoxelScalePreset) -> f32 {
        self.placement_distance / voxel_scale.meters().max(1e-6)
    }

    pub fn preview_mode(
        &self,
        place_pressed: bool,
        break_pressed: bool,
        sneak_pressed: bool,
    ) -> BrushMode {
        match self.tool {
            ToolKind::BuildersWand => BrushMode::Place,
            ToolKind::DestructorWand => BrushMode::Erase,
            _ if place_pressed => BrushMode::Place,
            _ if break_pressed => BrushMode::Erase,
            ToolKind::Hand if sneak_pressed => BrushMode::Place,
            ToolKind::Hand => BrushMode::Erase,
            _ => BrushMode::Place,
        }
    }

    pub fn adjust_cursor_size(&mut self, delta: i32) {
        match self.tool {
            ToolKind::Hand => {}
            ToolKind::Brush => {
                self.brush_radius = (self.brush_radius + delta).clamp(0, 8);
            }
            ToolKind::AreaTool => {
                self.area_tool.radius = (self.area_tool.radius + delta).clamp(0, 12);
            }
            ToolKind::BuildersWand | ToolKind::DestructorWand => {
                self.wand_max_blocks =
                    (self.wand_max_blocks as i32 + delta * 8).clamp(8, 512) as usize;
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
        }
    }

    pub fn cursor_summary(&self) -> String {
        match self.tool {
            ToolKind::Hand => format!("Hand range {:.1}m", self.placement_distance),
            ToolKind::Brush => {
                format!(
                    "Brush radius {} @ {:.1}m",
                    self.brush_radius, self.placement_distance
                )
            }
            ToolKind::AreaTool => format!(
                "Area radius {} x thickness {} @ {:.1}m",
                self.area_tool.radius, self.area_tool.thickness, self.placement_distance
            ),
            ToolKind::BuildersWand | ToolKind::DestructorWand => {
                format!(
                    "Wand cap {} @ {:.1}m",
                    self.wand_max_blocks, self.placement_distance
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
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

pub struct PreviewResult {
    pub blocks: Vec<IVec3>,
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
    sneak_pressed: bool,
    is_executing: bool,
) -> PreviewResult {
    let mode = creative.preview_mode(place_pressed, break_pressed, sneak_pressed);
    let show_placement = sneak_pressed || is_executing;

    let (mut blocks, color, invert_blend, center_block) = match creative.tool {
        ToolKind::Hand => {
            let target = match mode {
                BrushMode::Place if show_placement => {
                    if sneak_pressed {
                        raycast.place.or(raycast.hit)
                    } else {
                        raycast.hit.and(raycast.place)
                    }
                }
                BrushMode::Erase => raycast.hit,
                _ => raycast.hit,
            };
            let blocks = target.into_iter().collect();
            (blocks, [1.0, 1.0, 1.0, 1.0], true, None)
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
                let radius = creative.brush_radius.max(0);
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
                    let radius = creative.area_tool.radius.max(0);
                    let thickness = creative.area_tool.thickness.max(1);
                    for dv in -radius..=radius {
                        for du in -radius..=radius {
                            let include = match creative.area_tool.shape {
                                AreaFootprintShape::Circle => du * du + dv * dv <= radius * radius,
                                AreaFootprintShape::Square => true,
                            };
                            if include {
                                for depth in 0..thickness {
                                    blocks
                                        .push(center + axis_u * du + axis_v * dv + normal * depth);
                                }
                            }
                        }
                    }
                }
            }
            (blocks, [1.0, 1.0, 1.0, 1.0], true, actual_center)
        }
        ToolKind::BuildersWand => {
            let blocks = preview_wand_blocks(world, raycast, true, creative.wand_max_blocks);
            let center = raycast.hit;
            (blocks, [0.1, 0.5, 1.0, 1.0], false, center)
        }
        ToolKind::DestructorWand => {
            let blocks = preview_wand_blocks(world, raycast, false, creative.wand_max_blocks);
            let center = raycast.hit;
            (blocks, [1.0, 0.05, 0.05, 1.0], false, center)
        }
    };

    blocks = cap_blocks(blocks, creative.preview_cap);

    PreviewResult {
        blocks,
        color,
        invert_blend,
        center_block,
    }
}

pub fn edit_blocks(
    creative: &CreativeState,
    world: &World,
    player: &Player,
    raycast: RaycastTarget,
    place_pressed: bool,
    break_pressed: bool,
    sneak_pressed: bool,
) -> Option<(BrushMode, Vec<IVec3>)> {
    let mode = match creative.tool {
        ToolKind::Hand if place_pressed => Some(BrushMode::Place),
        ToolKind::Hand if break_pressed => Some(BrushMode::Erase),
        ToolKind::BuildersWand if place_pressed => Some(BrushMode::Place),
        ToolKind::DestructorWand if break_pressed => Some(BrushMode::Erase),
        ToolKind::Brush | ToolKind::AreaTool if place_pressed => Some(BrushMode::Place),
        ToolKind::Brush | ToolKind::AreaTool if break_pressed => Some(BrushMode::Erase),
        _ => None,
    }?;

    let preview = get_preview(
        creative,
        world,
        raycast,
        place_pressed,
        break_pressed,
        sneak_pressed,
        true,
    );
    let mut targets = preview.blocks;
    if mode == BrushMode::Place {
        let (player_min, player_max) =
            player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
        targets.retain(|cell| !block_overlaps_aabb(*cell, player_min, player_max));
    }
    Some((mode, targets))
}

fn cap_blocks(mut blocks: Vec<IVec3>, cap: usize) -> Vec<IVec3> {
    if blocks.len() > cap {
        blocks.truncate(cap);
    }
    blocks
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
        if (build_mode && target_block == BlockId::Air) || !build_mode {
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

fn block_overlaps_aabb(block: IVec3, min: Vec3, max: Vec3) -> bool {
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
        let preview = get_preview(
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
