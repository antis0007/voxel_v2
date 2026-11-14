//! Version 54 massing. Rebuild wings so rooms and stairs follow the silhouette.
use super::*;

pub(super) fn castle(mut b: Building, kind: u32) -> Building {
    if kind != 3 { return b; }
    for (i, w) in b.wings.iter_mut().enumerate() {
        let mut s = WingSpec::new(w.x0, w.z0, w.nx, w.nz, w.seed);
        s.cell = w.cell;
        s.h = w.h;
        s.storeys = w.storeys;
        s.windows = w.windows;
        s.doors = w.doors.clone();
        s.keep = w.keep.clone();
        s.age = w.age * 0.45;
        s.pal = w.pal;
        s.roof = w.roof;
        s.chimney = w.chimney;
        s.min_room = 4;
        if i < 4 {
            // Two capped watch towers and two open fighting terraces.
            s.roof = if i == 0 || i == 3 { Roof::SteepHip } else { Roof::Flat };
            s.pal.wall = B::Limestone;
            s.pal.frame = B::StoneBricks;
            s.pal.plinth = B::StoneBricks;
            s.pal.roof = B::Planks;
        } else if i == 4 {
            // The long keep has one dominant gable, not a stack of flat slabs.
            s.roof = Roof::Gable;
            s.pal.wall = B::Limestone;
            s.pal.frame = B::StoneBricks;
            s.pal.roof = B::Planks;
            s.windows = Windows::Panes;
        } else {
            s.open = true;
            if i == 5 { s.roof = Roof::Hip; }
        }
        *w = Wing::new(s);
    }
    Building::new(b.wings, b.curtains, b.plot)
}

pub(super) fn industrial(mut b: Building, kind: u32, seed: u32) -> Building {
    for (i, w) in b.wings.iter_mut().enumerate() {
        match kind {
            2 => {
                // Pump galleries use pitched steel; the regulating hall has a
                // flat maintenance roof with the original chimney retained.
                w.pal.frame = B::Concrete;
                w.pal.roof = B::RustedSteel;
                w.pal.glass = B::WindowGlass;
                w.roof = if i == 2 { Roof::Flat } else { Roof::Lean };
            }
            3 => {
                // Keep the old masonry shell around the newer furnace plant.
                if i < 4 {
                    w.pal.roof = B::RustedSteel;
                } else if i == 4 {
                    w.pal.frame = B::RustedSteel;
                    w.pal.glass = B::WindowGlass;
                    w.pal.roof = B::SteelPlate;
                    w.roof = if hash2(0, 17, seed) < 0.5 { Roof::Lean } else { Roof::Gable };
                }
            }
            4 => {
                w.pal.frame = B::RustedSteel;
                w.pal.roof = B::SteelPlate;
                w.pal.glass = B::WindowGlass;
                w.age = w.age.max(0.21);
            }
            _ => return b,
        }
    }
    Building::new(b.wings, b.curtains, b.plot)
}
