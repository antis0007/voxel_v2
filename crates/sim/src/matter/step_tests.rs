//! The coordinated tick: fine anchors, promotion on change, freezing, caps and demotion.
use super::fine_kernel::{fine_anchors, split_fine};
use crate::PHASE_OFFSETS;
use glam::IVec3;

#[test]
fn fine_anchors_cover_a_block_with_8_even_and_27_odd_blocks() {
    let b = IVec3::new(1, -2, 3);
    let even = fine_anchors(&[b], PHASE_OFFSETS[0]);
    assert_eq!(even.len(), 8);
    assert!(even.iter().all(|a| (*a - b * 4).cmpge(IVec3::ZERO).all() && (*a - b * 4).cmple(IVec3::splat(2)).all()));
    let odd = fine_anchors(&[b], PHASE_OFFSETS[1]);
    assert_eq!(odd.len(), 27);
    assert!(odd.iter().all(|a| (*a - b * 4).cmpge(IVec3::splat(-1)).all() && (*a - b * 4).cmple(IVec3::splat(3)).all()));
    let pair = fine_anchors(&[b, b + IVec3::X], PHASE_OFFSETS[1]);
    assert_eq!(pair.len(), 45, "nine shared face anchors listed once");
    assert!(pair.windows(2).all(|w| (w[0].z, w[0].y, w[0].x) < (w[1].z, w[1].y, w[1].x)));
}

#[test]
fn split_fine_floors_negative_coordinates() {
    assert_eq!(split_fine(IVec3::new(-1, 4, 7)), (IVec3::new(-1, 1, 1), IVec3::new(3, 0, 3)));
}
