//! Field records: a physical reading, a useful spell and a permanent discovery.
use super::inscriptions::INSCRIPTIONS;
use crate::{
    player::Player,
    world::{BlockId, World, interaction::Prompt},
};
use glam::{IVec3, Vec3};

pub const MASK: u32 = ((1 << 24) - 1) & !((1 << 11) - 1);

pub fn history(id: usize) -> Option<&'static str> {
    Some(match id {
        11 => {
            "The Watch sent lamp cards below when the braid went dark. The last issue slip lists six lamps and seven names."
        }
        12 => {
            "The Founders banked sparks behind the black arch. Beside the warning, a porter wrote: keep one ward for the walk home."
        }
        13 => {
            "Outward: one body. Homeward: the same weight. The Ballast clerk left a second line blank."
        }
        14 => {
            "The freight tally balances when the weight rests across the room. Someone has written 'floor' on both sides of the diagram."
        }
        15 => "The grave watch issued light cards, never torches. Smoke gathered beneath the ossuary vault; something answered every extinguished lamp.",
        16 => "Sluice crews froze a spill before opening the next gate. The thaw schedule is crossed out beside the drowned ward's last ration tally.",
        17 => "The Founders carried furnace current through wet copper. A scorch across the quench bay marks where the apprentice stood.",
        18 => "The gardeners fed the root beds from the infirmary drains. A later hand calls the crop a medicine; the kitchen ledger calls it supper.",
        19 => "Etching cards cut contaminated mortar from the sealed ward. The disposal ledger stops before the doors were bricked shut.",
        20 => "The star keepers drew falling bodies around a lodestone. Their final chart labels the braid a road, and marks a second inhabited horizon.",
        21 => "The Surveyors measured down from both ends of the shaft. Their freight diagram matches the Manyfold counterweight; neither end is marked home.",
        22 => "Pilgrims banked shots around stone corners to clear the return aisle. The porter warned that a ward mattered more beyond the black arch.",
        23 => "The engine crews used cutting cards to reopen collapsed service routes. The last work order joins a buried gate to a freight tube, then ends at an unnamed sky terminal.",
        _ => return None,
    })
}

pub fn hint(id: usize) -> Option<&'static str> {
    Some(match id {
        11 => "Read the lamp tally in a buried scriptorium.",
        12 => "Read the porter's record beside a buried gate.",
        13 => "Return home through a buried gate.",
        14 => "Dock the counterweight in the Manyfold tube.",
        15..=23 => "Read the working record in this ruin's scriptorium.",
        _ => return None,
    })
}

/// Grant once. XP and ownership share the same checksummed progression save.
pub fn grant(world: &mut World, id: usize) -> bool {
    if !(11..INSCRIPTIONS.len()).contains(&id) || world.progression.lore & (1 << id) != 0 {
        return false;
    }
    world.progression.lore |= 1 << id;
    world.adventure.discoveries |= 1 << id;
    world.adventure.spell_log.add(INSCRIPTIONS[id].spell);
    crate::world::progression::grant_xp(world, if id == 13 { 40 } else { 25 });
    world.expedition.say(format!(
        "{} · {}",
        INSCRIPTIONS[id].name, INSCRIPTIONS[id].benefit
    ));
    true
}

pub(crate) fn bookcase(world: &World, player: &Player) -> Option<IVec3> {
    if !world.rules.mode.interacts() || world.adventure.health <= 0.0 {
        return None;
    }
    let scale = world.voxel_scale.meters();
    let eye = player.eye(world.voxel_scale);
    // A real first-hit bookcase is required: no reading through walls or after mining it.
    let (cell, _) = crate::world::raycast_block(world, eye, player.forward_dir(), 4.5 / scale)?;
    (world.get(cell) == BlockId::Bookcase).then_some(cell)
}

fn target(world: &World, player: &Player) -> Option<usize> {
    let cell = bookcase(world, player)?;
    let scale = world.voxel_scale.meters();
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
    let cfg = world.sample_config();
    let matches = |q: Vec3| {
        (p.x - q.x).abs() <= 0.6 && (p.z - q.z).abs() <= 0.6 && (0.5..1.8).contains(&(p.y - q.y))
    };
    if engine_worldgen::hell_gate::record_near(&cfg, p).is_some_and(matches) {
        return Some(12);
    }
    if engine_worldgen::dungeon::record_near(&cfg, p).is_some_and(matches) {
        return Some(engine_worldgen::dungeon::record_curriculum(&cfg, p).unwrap_or(11));
    }
    None
}

pub fn prompt(world: &World, player: &Player) -> Option<Prompt> {
    let id = target(world, player)?;
    Some(Prompt {
        title: INSCRIPTIONS[id].name,
        action: "[F] Read",
    })
}

pub fn interact(world: &mut World, player: &Player) -> bool {
    let Some(id) = target(world, player) else {
        return false;
    };
    if id >= 15 { grant(world, 11); }
    grant(world, id);
    world.expedition.say(history(id).unwrap());
    true
}
