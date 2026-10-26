//! The coordinated tick: fine anchors, promotion on change, freezing, caps and demotion.
use super::fine_kernel::{fine_anchors, split_fine};
use crate::test_support::*;
use crate::{BlockView, Cell, MatterStore, PHASE_OFFSETS, VOLUME_FULL};
use glam::IVec3;

fn stone_floor(m: &mut MatterStore, lo: IVec3, hi: IVec3) {
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                m.replace_block(IVec3::new(x, y, z), Cell::filled(STONE)).unwrap();
            }
        }
    }
}

/// Volume of `material` over a box in fine units: a uniform block counts as 64 cells of itself.
fn fine_units(m: &MatterStore, lo: IVec3, hi: IVec3, material: u8) -> u64 {
    let mut sum = 0;
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                match m.block(IVec3::new(x, y, z)) {
                    BlockView::Uniform(c) if c.material == material => sum += 64 * u64::from(c.volume),
                    BlockView::Uniform(_) => {}
                    BlockView::Fine(cells) => {
                        sum += cells.iter().filter(|c| c.material == material).map(|c| u64::from(c.volume)).sum::<u64>()
                    }
                }
            }
        }
    }
    sum
}

#[test]
fn an_odd_phase_block_spills_fine_water_into_the_uniform_air_beside_it() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    stone_floor(&mut m, IVec3::new(-4, 0, -4), IVec3::new(4, 1, 4));
    let b = IVec3::new(0, 1, 0);
    m.replace_block(b, water(VOLUME_FULL)).unwrap();
    m.promote(b, 0).unwrap();
    let (lo, hi) = (IVec3::new(-4, 0, -4), IVec3::new(4, 4, 4));
    let before = fine_units(&m, lo, hi, WATER);
    let mut woken = Vec::new();
    // Fine x = 3 is the last column of block b, x = 4 the first of the block beside it.
    let anchor = IVec3::new(3, 4, 1);
    let out = m.run_fine_block(&t, 7, 1, 0, anchor, &mut woken);
    assert!(out.changed && !out.stalled && out.promoted == 1, "{out:?}");
    assert!(m.is_fine(b + IVec3::X) && !m.is_fine(b - IVec3::Y));
    assert_eq!(fine_units(&m, lo, hi, WATER), before, "water is conserved");
    assert_eq!(woken, vec![b, b + IVec3::X]);
}

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
