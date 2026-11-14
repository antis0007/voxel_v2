//! Held wand from the authored families in `assets/wands.ron`, hung on the hand's grip socket.
use super::{Model, wand::WandView};
use crate::wand_style::WandStyle;
use glam::Vec3;

pub(super) fn enabled() -> bool { crate::viewmodel_review::selected().is_none() }
pub(super) fn muzzle(style: WandStyle) -> Vec3 { super::wand_kit::muzzle(style) }
pub(super) fn draw(m: &mut Model, v: &WandView, style: WandStyle) {
    let pose = super::wand_pose::pose(v);
    super::wand_kit::draw_cast(m, super::wayfarer::attachment(v), style,
        style.clamped().glow_colour(v.spell), pose.cast, pose.cast_left);
}
