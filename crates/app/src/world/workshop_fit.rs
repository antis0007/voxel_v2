//! Fittings for the starter workshop of a new world: a lectern with a letter, a stocked
//! chest, a bed and torches over the machines. Text and stock live in
//! `assets/workshop_fittings.ron`. Runs once from world creation, never on load, so
//! existing worlds keep their own interior. Cells are found by scanning the hall that
//! worldgen built, so the hall itself and every generator gate stay untouched.
use super::inventory::{Item, ItemStack};
use super::{BlockId, World, block_data, workshop};
use glam::IVec3;

#[derive(serde::Deserialize)]
struct Fittings {
    letter: Vec<String>,
    chest: Vec<(Item, u32)>,
    torch_over: Vec<BlockId>,
}

const RON: &str = include_str!("../../../../assets/workshop_fittings.ron");

static FITTINGS: std::sync::LazyLock<Option<Fittings>> = std::sync::LazyLock::new(|| {
    ron::from_str(RON).map_err(|error| eprintln!("[workshop] assets/workshop_fittings.ron: {error}")).ok()
});

/// Half extents of the hall footprint around its bench, in metres.
const HALL_X: i32 = 9;
const HALL_Z: i32 = 8;
/// Cells kept clear around the bench, and the spacing between fitted pieces, in metres.
const BENCH_CLEARANCE: i32 = 3;
const PIECE_SPACING: i32 = 4;
/// Floor-to-ceiling search height above a candidate cell.
const CEILING_RISE: i32 = 7;

/// Adds the fittings; returns how many blocks were placed. Only the default 1 m grid is fitted.
pub fn fit(world: &mut World) -> usize {
    let cfg = world.sample_config();
    if world.voxel_scale.meters() != 1.0 { return 0; }
    let Some(home) = workshop::home(&cfg) else { return 0 };
    let Some(fittings) = FITTINGS.as_ref() else { return 0 };
    let bench = IVec3::new(home.x.floor() as i32, home.y.floor() as i32, home.z.floor() as i32);
    if world.get(bench) != BlockId::Workbench { return 0; }
    let mut placed = 0;
    // Torches over the machines light the room around the bench.
    for dz in -HALL_Z..=HALL_Z {
        for dx in -HALL_X..=HALL_X {
            let at = bench + IVec3::new(dx, 0, dz);
            if fittings.torch_over.contains(&world.get(at)) && world.get(at + IVec3::Y) == BlockId::Air {
                world.set(at + IVec3::Y, BlockId::Torch);
                placed += 1;
            }
        }
    }
    // Furniture sits against a wall of the same room, nearest the bench first.
    let mut cells = Vec::new();
    for dz in -HALL_Z..=HALL_Z {
        for dx in -HALL_X..=HALL_X {
            let at = bench + IVec3::new(dx, 0, dz);
            // Not on the bench pedestal: the bench keeps its own bay.
            if dx.abs().max(dz.abs()) >= BENCH_CLEARANCE && free_against_wall(world, at) { cells.push(at); }
        }
    }
    cells.sort_by_key(|c| (c.x - bench.x).pow(2) + (c.z - bench.z).pow(2));
    let mut taken: Vec<IVec3> = Vec::new();
    for block in [BlockId::Chest, BlockId::Lectern, BlockId::Bed] {
        let Some(&at) = cells.iter().find(|c| taken.iter().all(|t| (t.x - c.x).abs() + (t.z - c.z).abs() >= PIECE_SPACING)) else { continue };
        world.set(at, block);
        println!("[workshop] fitted {block:?} at {} {} {}", at.x, at.y, at.z);
        taken.push(at);
        placed += 1;
        match block {
            BlockId::Chest => stock(world, at, fittings),
            BlockId::Lectern => { block_data::set_text(world, at, &fittings.letter); }
            _ => {}
        }
    }
    placed
}

/// Air with two cells of headroom, a floor below, a roof overhead and a wall beside it.
fn free_against_wall(world: &World, at: IVec3) -> bool {
    let air = |p| world.get(p) == BlockId::Air;
    if !air(at) || !air(at + IVec3::Y) || !world.get(at - IVec3::Y).is_solid() { return false; }
    if !(2..=CEILING_RISE).any(|rise| world.get(at + IVec3::Y * rise).is_solid()) { return false; }
    [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].into_iter().any(|d| world.get(at + d).is_solid())
}

fn stock(world: &mut World, at: IVec3, fittings: &Fittings) {
    let mut slots = vec![None; world.get(at).chest_slots().max(block_data::CHEST_SLOTS)];
    for (slot, &(item, count)) in slots.iter_mut().zip(&fittings.chest) {
        *slot = Some(ItemStack::new(item, count));
    }
    world.block_data.set_chest(at, slots);
}
