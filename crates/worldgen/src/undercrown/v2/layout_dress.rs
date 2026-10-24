//! Furnishing and enlarging the authored rooms: colonnades, the kitchen's
//! table and hearth, the chapel's pews and altar, alcoves and lanterns in
//! the Lantern Gardens, the frozen organ and ice alcoves of the Choir, the
//! borough's lamps, courthouse dais and baths, and the foundry's furnace
//! bank, lava fall and east gallery. Nothing here adds a room or a prop:
//! extra spaces belong to the room they open off.
use super::Shape;
use super::feature::Kind as F;
use super::furnish::{Back, Fit};
use super::layout::{Builder, hall};
use crate::BlockId as B;
use crate::undercrown::plan::{District as D, Style as S};
use glam::Vec3;

pub(super) fn rootbound(b: &mut Builder, l: f32) {
    // The first landing: four columns clear of the well, the doors and the
    // corridor mouths.
    for (x, z) in [(-4.5, -7.5), (-4.5, 0.5), (6.5, -8.5), (6.5, 0.5)] {
        b.column(x, z, l, l + 8.5, S::Rootbound);
    }
    // Pilgrims' kitchen: a long table, a hearth on the west wall, a pantry.
    b.fit((-3.5, l, -24.0), (4.5, l + 2.0, -20.0), Fit::Tables { floor: l });
    b.fit((-6.0, l, -27.6), (-4.6, l + 4.5, -24.4), Fit::Hearth { floor: l, back: Back::LoX });
    b.fit((-3.0, l, -28.0), (1.5, l + 2.4, -27.2), Fit::Cabinets { floor: l, block: B::RedCap });
    // The chapel: two colonnades, pews either side of an aisle, an altar on
    // a dais at the south end between two braziers.
    let f = l - 3.0;
    for z in [-27.5, -23.5, -19.5] {
        b.column(-25.5, z, f, l + 6.5, S::Rootbound);
        b.column(-14.5, z, f, l + 6.5, S::Rootbound);
    }
    b.fit((-24.2, f, -27.0), (-15.8, f + 1.5, -18.5), Fit::Pews { floor: f });
    b.fit((-22.5, f, -31.6), (-17.5, f + 3.0, -28.6), Fit::Altar { floor: f, style: S::Rootbound });
    for x in [-23.5, -16.5] {
        b.fixture(x, -30.5, f, f + 2.0, Fit::Brazier { floor: f, block: B::StoneBricks });
    }
}

pub(super) fn gardens(b: &mut Builder, l: f32, seed: u32) {
    // Two alcoves off the orchard, and glow-cap lanterns hung from its roof.
    if let Some(orchard) = b.room_of(D::LanternGardens, 1) {
        for (c, rad) in [(Vec3::new(3.0, l - 0.5, 40.0), Vec3::new(5.0, 4.0, 5.0)), (Vec3::new(18.0, l - 0.5, 52.0), Vec3::new(6.0, 4.0, 4.5))] {
            b.space(orchard, Shape::Cave { c, rad, floor: l - 2.0 }, S::Fungal, None, false);
        }
    }
    b.fit((8.0, l + 3.0, 30.0), (28.0, l + 10.0, 46.0), Fit::Lanterns { low: l + 3.5, seed: seed.wrapping_add(61) });
    // Seed bank: cabinets of glowing jars along both long walls.
    b.fit((33.0, l - 1.0, 31.0), (41.0, l + 1.4, 31.8), Fit::Cabinets { floor: l - 1.0, block: B::GlowCap });
    b.fit((33.0, l - 1.0, 40.2), (41.0, l + 1.4, 41.0), Fit::Cabinets { floor: l - 1.0, block: B::GlowCap });
}

pub(super) fn chasm(b: &mut Builder, c: f32) {
    // The Frozen Choir: an organ alcove east, a frozen cascade south.
    if let Some(choir) = b.room_of(D::FrozenChoir, 1) {
        b.space(choir, Shape::Cave { c: Vec3::new(139.0, c + 5.0, -18.0), rad: Vec3::new(4.5, 4.0, 6.0), floor: c + 4.0 }, S::Ice, None, false);
        b.space(choir, Shape::Cave { c: Vec3::new(127.0, c + 5.0, -29.0), rad: Vec3::new(6.0, 4.0, 3.5), floor: c + 4.0 }, S::Ice, None, false);
    }
    b.fit((140.5, c + 3.0, -23.0), (142.5, c + 12.0, -13.0), Fit::Organ { floor: c + 3.0, seed: 71 });
    b.fit((122.0, c + 3.0, -33.0), (132.0, c + 10.0, -31.0), Fit::IceSheet { back: Back::LoZ });
}

pub(super) fn borough(b: &mut Builder, bo: f32) {
    // Street lamps along both sides, clear of the passages and the stair.
    for (x, z) in [(47.5, -74.5), (54.5, -74.5), (75.5, -74.5), (82.5, -74.5), (47.5, -62.5), (65.5, -62.5), (72.5, -62.5)] {
        b.fixture(x, z, bo, bo + 4.0, Fit::Lamp { floor: bo, h: 3.0 });
    }
    b.fixture(44.5, -98.5, bo + 1.0, bo + 5.0, Fit::Lamp { floor: bo + 1.0, h: 3.0 });
    // The courthouse: the judges' dais at the south end, benches facing it.
    let f = bo - 2.0;
    b.fit((91.0, f, -80.0), (103.0, f + 3.5, -76.5), Fit::Dais { floor: f, style: S::Canal, back: Back::LoZ });
    b.fit((92.0, f, -74.0), (102.0, f + 1.5, -68.0), Fit::Pews { floor: f });
    // The bathhouse: a sunken pool with steps, columns, two braziers.
    let f = bo + 1.0;
    b.fit((112.5, f - 1.5, -76.0), (117.5, f + 1.0, -70.5), Fit::Bath { floor: f, depth: 1.5 });
    for (x, z) in [(111.5, -67.5), (120.5, -67.5), (111.5, -76.5)] {
        b.column(x, z, f, f + 7.0, S::Canal);
    }
    for z in [-69.5, -72.5] {
        b.fixture(119.5, z, f, f + 2.0, Fit::Brazier { floor: f, block: B::Limestone });
    }
}

pub(super) fn foundry(b: &mut Builder, c: f32) {
    let f = c + 2.0;
    // A bank of furnaces on the north wall, west of the archive door.
    b.fit((108.0, f, 9.5), (116.0, f + 11.0, 12.0), Fit::Furnaces { floor: f, back: Back::HiZ });
    // A lava fall down the east wall into the channel; it crusts with it.
    b.fit((127.0, c + 1.0, 3.5), (128.4, f + 11.0, 6.5), Fit::Lavafall { needs: super::Mech::COOLING });
    for x in [115.5, 119.5] {
        b.column(x, -3.5, f, f + 11.0, S::Foundry);
    }
    // The east gallery, three metres up a ramp, with a brazier at its end.
    if let Some(furnace) = b.room_of(D::AshenFoundry, 1) {
        b.space(furnace, hall(128.0, 135.0, -5.0, 1.0, f + 3.0, 3.5, 1.0), S::Foundry, None, false);
    }
    b.feature((121.0, f - 1.0, -5.0), (128.0, f + 3.0, -2.5), F::Ramp { along_x: true, y0: f, y1: f + 3.0, style: S::Foundry, needs: None });
    b.fixture(133.5, -3.5, f + 3.0, f + 5.0, Fit::Brazier { floor: f + 3.0, block: B::BlastBrick });
}
