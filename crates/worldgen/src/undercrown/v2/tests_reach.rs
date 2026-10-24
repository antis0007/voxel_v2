//! Reachability at more than one voxel scale, every mechanism state against
//! the archive, and the lights.
use super::tests::{ALWAYS, BOROUGH};
use super::tests_grid::{Grid, plan_scaled, start_cell};
use super::*;
use engine_core::VoxelScalePreset as Scale;

#[test]
fn every_scale_walks_the_same_rooms() {
    for (seed, scale) in [(424242u32, Scale::OneMeter), (7, Scale::OneMeter), (424242, Scale::HalfMeter), (99, Scale::HalfMeter)] {
        let (c, plan) = plan_scaled(seed, scale);
        let l = plan.v2.as_ref().unwrap();
        let grid = Grid::new(&c, l, Mech::default());
        let got = grid.rooms(&plan, &grid.reach_list(start_cell(&plan, &grid)));
        for name in ALWAYS {
            assert!(got.contains(&name), "seed {seed} at {scale:?}: {name} unreachable; reached {got:?}");
        }
        for name in BOROUGH.iter().chain(&["Glass Archive"]) {
            assert!(!got.contains(name), "seed {seed} at {scale:?}: {name} reachable before its gate");
        }
        let mut wound = grid.clone();
        wound.apply(&c, l, Mech::WINCH, &l.edits(&c, Mech::default(), Mech::WINCH));
        let got = wound.rooms(&plan, &wound.reach_list(start_cell(&plan, &wound)));
        for name in BOROUGH {
            assert!(got.contains(&name), "seed {seed} at {scale:?}: {name} unreachable with the winch wound");
        }
    }
}

/// Whatever has been rung, repaired, opened or closed, the levers the
/// archive needs stay within reach, from the well and from the chasm floor,
/// and pulling them opens it.
#[test]
fn no_mechanism_state_soft_locks_the_archive() {
    let (c, plan) = plan_scaled(424242, Scale::OneMeter);
    let l = plan.v2.as_ref().unwrap();
    let base = Grid::new(&c, l, Mech::default());
    let prop = |label: &str| plan.props.iter().find(|p| p.label == label).map(|p| p.pos).unwrap();
    let (lens, gate, platform) = (prop("Observatory lens"), prop("Cooling gate"), prop("Receiving platform"));
    let archive = plan.rooms.iter().find(|r| r.name == "Glass Archive").unwrap();
    let chasm = plan.rooms.iter().find(|r| r.name == "Crown Chasm").unwrap();
    let floor = base.cell(Vec3::new((chasm.x0 + chasm.x1) * 0.5, chasm.floor + 1.0, (chasm.z0 + chasm.z1) * 0.5));
    let state = |m: Mech| {
        let mut g = base.clone();
        g.apply(&c, l, m, &l.edits(&c, Mech::default(), m));
        g
    };
    for bits in 0..=255u16 {
        let m = Mech(bits);
        let g = state(m);
        for from in [start_cell(&plan, &g), floor] {
            let seen = g.reach_list(from);
            assert!(m.has(Mech::LENS) || g.touches(&seen, lens), "{m:?}: the lens is out of reach from {from:?}");
            let crossing = m.has(Mech::COOLING) || m.has(Mech::FREIGHT) || g.touches(&seen, gate) || g.touches(&seen, platform);
            assert!(crossing, "{m:?}: neither the cooling gate nor the receiving platform is in reach from {from:?}");
        }
        // Only the bits the archive does not need vary the finished state.
        if bits & (Mech::LENS.0 | Mech::COOLING.0) == 0 {
            let done = state(m.with(Mech::LENS).with(Mech::COOLING));
            let seen = done.reach_list(start_cell(&plan, &done));
            assert!(done.in_room(&seen, archive), "{m:?}: the archive stays shut with the lens and the cooling gate");
        }
    }
}

#[test]
fn lights_hang_on_solid_walls_and_never_block_the_way() {
    for scale in [Scale::OneMeter, Scale::HalfMeter] {
        let (c, plan) = plan_scaled(424242, scale);
        let l = plan.v2.as_ref().unwrap();
        let grid = Grid::new(&c, l, Mech::ALL);
        let mut torches = Vec::new();
        for y in 0..grid.size.y {
            for z in 0..grid.size.z {
                for x in 0..grid.size.x {
                    let p = grid.min + IVec3::new(x, y, z);
                    let b = grid.get(p);
                    if let Some(off) = b.torch_support_offset() {
                        assert!(!b.is_solid(), "{b:?} is solid");
                        assert!(grid.get(p + off).is_solid(), "{scale:?}: {b:?} at {p:?} hangs on {:?}", grid.get(p + off));
                        torches.push(grid.centre(p));
                    }
                }
            }
        }
        // Every hall, domed room and long passage carries a light.
        for s in &l.spaces {
            if matches!(s.shape, Shape::Hall { .. } | Shape::Round { .. }) {
                let (lo, hi) = s.bounds();
                assert!(torches.iter().any(|t| t.cmpge(lo).all() && t.cmplt(hi).all()), "{scale:?}: a {:?} space of room {} is dark", s.style, s.room);
            }
        }
        for t in l.tunnels.iter().filter(|t| t.length() >= 12.0 && t.gate.is_none() && !t.sealed) {
            let (lo, hi) = t.bounds();
            assert!(torches.iter().any(|p| p.cmpge(lo).all() && p.cmplt(hi).all()), "{scale:?}: passage {:?}->{:?} is dark", t.a, t.b);
        }
        // Nothing that glows stands where a walker's body goes.
        let body = (2.0 / grid.v).round() as i32;
        for p in grid.reach_list(start_cell(&plan, &grid)) {
            for k in 0..body {
                let b = grid.get(p + IVec3::Y * k);
                assert!(!matches!(b, B::GlowCap | B::Crystal | B::StarGlass | B::Lava), "{scale:?}: {b:?} in the walk at {p:?}");
            }
        }
    }
}
