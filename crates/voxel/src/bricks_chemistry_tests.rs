//! The kernel's phase changes and reactions on the real tables: `assets/materials.ron` and
//! `assets/reactions.ron` are checked together with the rules that read them.
use engine_content::{BlockId as B, SIM_RULES};
use engine_sim::{BrickStore, Cell, block_rule, split_voxel, step};
use glam::IVec3;

fn dk(kelvin: f32) -> u16 {
    (kelvin * 10.0).round() as u16
}

fn cell(b: B, kelvin: f32) -> Cell {
    Cell { temperature: dk(kelvin), ..Cell::filled(b as u8) }
}

/// One heat phase on a single block: `subject` in cell 0 among stone at its temperature, so only the
/// reaction pass acts.
fn in_block(subject: Cell, neighbour: Option<Cell>) -> [Cell; 8] {
    let mut cells = [Cell { temperature: subject.temperature, ..Cell::filled(B::Stone as u8) }; 8];
    cells[0] = subject;
    if let Some(n) = neighbour {
        cells[1] = n;
    }
    block_rule(&SIM_RULES, &mut cells, 1, 0, IVec3::ZERO);
    cells
}

#[test]
fn ice_melts_water_boils_and_steam_condenses_at_the_tabled_points() {
    for (b, kelvin, becomes) in [
        (B::Ice, 280.0, B::Water),
        (B::Ice, 270.0, B::Ice),
        (B::Water, 374.0, B::Steam),
        (B::Water, 300.0, B::Water),
        (B::Water, 273.0, B::Ice),
        (B::Steam, 347.0, B::Water),
        (B::Steam, 360.0, B::Steam),
    ] {
        assert_eq!(in_block(cell(b, kelvin), None)[0].material, becomes as u8, "{b:?} at {kelvin} K");
    }
}

#[test]
fn lava_and_water_make_basalt_and_steam() {
    let cells = in_block(cell(B::Lava, 1400.0), Some(Cell::filled(B::Water as u8)));
    assert_eq!((cells[0].material, cells[1].material), (B::Basalt as u8, B::Steam as u8));
}

#[test]
fn a_hot_log_lights_the_air_and_burns_to_ash() {
    let mut s = BrickStore::new(500);
    let put = |s: &mut BrickStore, p: IVec3, c: Cell| {
        let (chunk, local) = split_voxel(p);
        s.set(chunk, local, c).unwrap();
    };
    for x in -4..4 {
        for z in -4..4 {
            put(&mut s, IVec3::new(x, 0, z), Cell::filled(B::Stone as u8));
        }
    }
    let log = IVec3::new(0, 1, 0);
    put(&mut s, log, cell(B::Log, 700.0));
    let at = |s: &BrickStore, p: IVec3| {
        let (chunk, local) = split_voxel(p);
        s.get(chunk, local).material
    };
    let mut saw_fire = false;
    for tick in 0..8000 {
        step(&mut s, &SIM_RULES, 1, tick);
        saw_fire |= (-1..2).any(|x| (1..3).any(|y| (-1..2).any(|z| at(&s, IVec3::new(x, y, z)) == B::Fire as u8)));
        if at(&s, log) == B::Ash as u8 {
            break;
        }
    }
    assert!(saw_fire, "the log set the air beside it alight");
    assert_eq!(at(&s, log), B::Ash as u8, "and burned down to ash");
}

#[test]
fn logs_at_room_temperature_cost_nothing() {
    let mut s = BrickStore::new(500);
    for x in -4..4 {
        for z in -4..4 {
            let (chunk, local) = split_voxel(IVec3::new(x, 0, z));
            s.set(chunk, local, Cell::filled(if x.abs() < 2 { B::Log } else { B::Stone } as u8)).unwrap();
        }
    }
    for tick in 0..8 {
        step(&mut s, &SIM_RULES, 1, tick);
    }
    assert_eq!(s.active_bricks(), 0);
}
