//! Disposable two-client factory and terrain fixture. Never installs in a save.
use super::*;
use crate::world::factory::{self, Kind, assembler::review_fixture};

const FLOOR_M: i32 = 192;
const SEPARATION_M: i32 = 384;

fn origin(w: &World, index: i32) -> IVec3 {
    IVec3::new(index * SEPARATION_M, FLOOR_M, 0) * factory::step(w)
}

fn storage(w: &World, index: i32) -> IVec3 {
    origin(w, index) + IVec3::new(5, 0, 4) * factory::step(w)
}

pub(super) fn blast_cell(w: &World) -> IVec3 {
    origin(w, 1) + IVec3::new(0, 1, -4) * factory::step(w)
}

pub(super) fn setup(w: &mut World, player: &mut Player) {
    if review_fixture::network_enabled() {
        if let Err(error) = review_fixture::install_network(w, player) { eprintln!("[assembler-review] FAIL {error:#}"); }
        return;
    }
    let size = factory::step(w);
    for index in 0..2 {
        let p = origin(w, index);
        let mut edits = Vec::new();
        for x in -7 * size..8 * size {
            for z in -7 * size..8 * size {
                edits.push((p + IVec3::new(x, -1, z), BlockId::Stone));
                for y in 0..4 * size {
                    let b = if index == 1 && z == -4 * size && x.abs() <= 2 * size && y < 3 * size {
                        BlockId::Stone
                    } else { BlockId::Air };
                    edits.push((p + IVec3::new(x, y, z), b));
                }
            }
        }
        w.set_many(&edits);
        for (x, kind) in [(0, Kind::Generator), (1, Kind::Cable), (2, Kind::Furnace),
            (3, Kind::Pipe), (4, Kind::Pipe), (5, Kind::Storage)] {
            let n = factory::fixture_place(w, p + IVec3::new(x, 0, 4) * size, kind)
                .expect("review machine");
            if kind == Kind::Generator { n.input[3] = 1024; }
            if kind == Kind::Furnace {
                n.mode = 1; // Raw ore, rather than the default concentrate mode.
                n.input[2] = 1536;
            }
        }
    }
    player.pos = origin(w, 0).as_vec3() + Vec3::new(0.5, 0.1, 0.5);
    player.precise_pos = player.pos.as_dvec3();
    eprintln!("[base-review] fixture voxel_m={}", w.voxel_scale.meters());
}

pub(super) fn report(w: &World) {
    if review_fixture::network_enabled() { review_fixture::report(w); return; }
    let iron = |index| w.factory.nodes.get(&storage(w, index)).map_or(0, |n| n.input[8]);
    eprintln!("[base-review] home_iron={} outpost_iron={} blast={}",
        iron(0), iron(1), w.get(blast_cell(w)).as_u32());
}
