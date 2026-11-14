//! Read-only assays of the live terrain. A transect reports bands of evidence,
//! so additional faces help narrow a deposit without revealing ore coordinates.
use super::{BlockId as B, World, inventory::Item, travel::panning};
use crate::player::Player;
use glam::{IVec3, Vec3};

const ORES: [(B, &str); 11] = [
    (B::CopperOre, "Copper"),
    (B::TinOre, "Tin"),
    (B::IronOre, "Iron"),
    (B::CoalOre, "Coal"),
    (B::GoldQuartz, "Gold quartz"),
    (B::Crystal, "Crystal"),
    (B::Chalcopyrite, "Chalcopyrite"),
    (B::Malachite, "Malachite"),
    (B::Hematite, "Hematite"),
    (B::Magnetite, "Magnetite"),
    (B::Cassiterite, "Cassiterite"),
];

enum Target {
    Rock(IVec3, IVec3),
    Sediment(panning::Bed),
}

fn target(w: &World, player: &Player, slot: usize) -> Option<Target> {
    let item = w.inventory.hotbar.get(slot)?.as_ref()?.item;
    let reach = super::interaction::REACH / w.voxel_scale.meters();
    let eye = player.eye(w.voxel_scale);
    let direction = player.forward_dir();
    match item {
        panning::PAN => {
            let (cell, _) = super::raycast_block(w, eye, direction, reach)?;
            let bed = panning::wet_bed(w, cell, w.get(cell))?;
            Some(Target::Sediment(bed))
        }
        Item::Tool(7 | 8) => {
            let (cell, outside) = super::raycast_block(w, eye, direction, reach)?;
            let block = w.get(cell);
            let rock = matches!(block, B::Stone | B::Basalt | B::Limestone | B::Sandstone)
                || ORES.iter().any(|&(ore, _)| ore == block);
            (rock && (cell - outside).abs().element_sum() == 1)
                .then_some(Target::Rock(cell, cell - outside))
        }
        _ => None,
    }
}

pub fn prompt(w: &World, player: &Player, slot: usize) -> Option<super::interaction::Prompt> {
    let title = match target(w, player, slot)? {
        Target::Rock(..) => "Rock face",
        Target::Sediment(_) => "Wet sediment",
    };
    Some(super::interaction::Prompt {
        title,
        action: "[Shift+F] Prospect",
    })
}

pub fn inspect(w: &mut World, player: &Player, slot: usize) -> bool {
    let Some(target) = target(w, player, slot) else {
        return false;
    };
    let s = super::factory::step(w);
    let (cell, title, detail) = match target {
        Target::Sediment(bed) => {
            let grade = panning::feed(w, bed).map_or(0, |(_, feed)| feed.grade());
            let signal = match grade {
                0 => "Barren",
                1..=255 => "Trace gold",
                256..=511 => "Gold-bearing",
                _ => "Rich gold",
            };
            (
                bed.cell,
                format!("Sediment · {signal}"),
                "Compare shallow beds upstream and downstream.".to_owned(),
            )
        }
        Target::Rock(cell, inward) => {
            let side = if inward.x == 0 { IVec3::X } else { IVec3::Z };
            let up = inward.cross(side);
            let mut hits = [[0u8; 3]; ORES.len()];
            for depth in 0..9 {
                let band = depth / 3;
                for a in -1..=1 {
                    for b in -1..=1 {
                        let p = cell + (inward * depth as i32 + side * a + up * b) * s;
                        if let Some(i) = ORES.iter().position(|&(ore, _)| ore == w.get(p)) {
                            hits[i][band] += 1;
                        }
                    }
                }
            }
            let mut finds: Vec<_> = ORES
                .iter()
                .zip(hits)
                .filter_map(|(&(_, name), counts)| {
                    let band = counts.iter().position(|&count| count > 0)?;
                    let strength = match counts[band] {
                        1 => "trace",
                        2..=5 => "some",
                        _ => "strong",
                    };
                    Some((
                        band,
                        format!("{name} · {strength} · {}–{} m", band * 3, band * 3 + 3),
                    ))
                })
                .collect();
            finds.sort_by_key(|(band, _)| *band);
            let heading = match inward.to_array() {
                [1, 0, 0] => "East",
                [-1, 0, 0] => "West",
                [0, 0, 1] => "South",
                [0, 0, -1] => "North",
                [0, 1, 0] => "Up",
                _ => "Down",
            };
            let title = finds.first().map_or_else(
                || format!("{heading} · No ore in sample"),
                |(_, first)| format!("{heading} · {first}"),
            );
            let detail = finds
                .into_iter()
                .map(|(_, line)| line)
                .collect::<Vec<_>>()
                .join("\n");
            (cell, title, detail)
        }
    };
    w.expedition.say_at(
        title,
        detail,
        (cell.as_vec3() + Vec3::splat(0.5)) / s as f32,
    );
    true
}
