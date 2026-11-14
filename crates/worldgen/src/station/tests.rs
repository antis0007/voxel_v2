use super::*;
use crate::BlockId as B;
use std::collections::{HashSet, VecDeque};

const SEEDS: std::ops::Range<u32> = 0..200;

fn cells(plan: &Plan) -> impl Iterator<Item = IVec3> + '_ {
    (plan.min.y..=plan.max.y).flat_map(move |y| {
        (plan.min.z..=plan.max.z).flat_map(move |z| (plan.min.x..=plan.max.x).map(move |x| IVec3::new(x, y, z)))
    })
}

fn inside(b: IVec3, a: IVec3, c: IVec3) -> bool {
    b.cmpge(a).all() && b.cmple(c).all()
}

/// Floor cells a standing player reaches from just inside the outer hatch.
fn reachable(plan: &Plan) -> HashSet<IVec3> {
    let stand = |p: IVec3| {
        block(plan, p) == Some(B::Air)
            && block(plan, p + IVec3::Y) == Some(B::Air)
            && block(plan, p - IVec3::Y).is_some_and(|b| b != B::Air)
    };
    let start = IVec3::new(1, 0, -1);
    assert!(stand(start), "no standing room inside the hatch");
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        for o in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            let q = p + o;
            if inside(q, plan.min, plan.max) && !seen.contains(&q) && stand(q) {
                seen.insert(q);
                queue.push_back(q);
            }
        }
    }
    seen
}

#[test]
fn plans_are_deterministic_and_vary_with_the_seed() {
    let a = plan(11);
    assert_eq!(a.spaces, plan(11).spaces);
    let layouts: HashSet<Vec<(i32, i32)>> = SEEDS.map(|s| plan(s).spaces.iter().map(|r| (r.min.x, r.max.z)).collect()).collect();
    assert!(layouts.len() > 20, "only {} layouts", layouts.len());
}

#[test]
fn every_station_holds_the_required_rooms_at_spec_size() {
    for seed in SEEDS {
        let p = plan(seed);
        let clear = |s: &Space| s.max - s.min + IVec3::ONE;
        for kind in [Kind::Airlock, Kind::Corridor, Kind::Trade, Kind::Crew, Kind::Quarters, Kind::Freight, Kind::Damaged] {
            assert!(p.spaces.iter().any(|s| s.kind == kind), "seed {seed} lacks {kind:?}");
        }
        for s in &p.spaces {
            let c = clear(s);
            assert!(c.y >= 3, "seed {seed} {:?} is {} m high", s.kind, c.y);
            match s.kind {
                Kind::Corridor => assert_eq!(c.z, 3),
                Kind::Airlock => assert_eq!((c.x, c.z), (6, 4)),
                Kind::Crew | Kind::Quarters | Kind::Galley | Kind::Engineering | Kind::Damaged => assert!(c.x >= 4 && c.z >= 4, "seed {seed} {:?} {c}", s.kind),
                Kind::Freight => assert!(c.x >= 12 && c.z >= 8 && c.y >= 4, "seed {seed} freight {c}"),
                Kind::Trade => assert!(c.x >= 8 && c.z >= 6),
            }
        }
        for (a, b) in &p.openings {
            let size = *b - *a + IVec3::ONE;
            let span = size.x.max(size.z);
            assert!(span == 2 && (size.y == 3 || (*a, *b) == p.breach), "seed {seed} opening {size}");
        }
    }
}

#[test]
fn the_hull_is_sealed_except_at_the_hatch_and_the_breach() {
    for seed in SEEDS.step_by(7) {
        let p = plan(seed);
        for c in cells(&p).filter(|&c| block(&p, c) == Some(B::Air)) {
            for o in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                if block(&p, c + o).is_none() {
                    let vent = inside(c, p.hatch.0, p.hatch.1) || inside(c, p.breach.0, p.breach.1);
                    assert!(vent, "seed {seed}: air at {c} leaks toward {}", c + o);
                }
            }
        }
    }
}

#[test]
fn every_room_inhabitant_and_locker_is_reachable_on_foot() {
    for seed in SEEDS.step_by(3) {
        let p = plan(seed);
        let walk = reachable(&p);
        for s in &p.spaces {
            assert!(walk.iter().any(|&c| s.contains(c)), "seed {seed}: {:?} unreachable", s.kind);
        }
        for m in &p.markers {
            assert!(walk.contains(&m.at), "seed {seed}: {:?} at {} unreachable", m.role, m.at);
        }
        for c in cells(&p).filter(|&c| matches!(block(&p, c), Some(B::Chest | B::ChestSteel | B::ChestCopper))) {
            let near = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y + IVec3::X, IVec3::NEG_Y - IVec3::X, IVec3::NEG_Y + IVec3::Z, IVec3::NEG_Y - IVec3::Z]
                .iter()
                .any(|&o| walk.contains(&(c + o)) || walk.contains(&(c + o - IVec3::Y)));
            assert!(near, "seed {seed}: locker at {c} has no standing cell beside it");
        }
    }
}

/// Gas moves through every cell that is not a full solid, as the habitat
/// pressure grid treats it.
#[test]
fn the_bulkhead_keeps_the_breach_from_venting_the_station() {
    for seed in SEEDS {
        let p = plan(seed);
        let open = |c: IVec3| sealed_block(&p, c).is_some_and(|b| !b.is_solid());
        let start = IVec3::new(1, 1, -1);
        assert!(p.pressurised(start) && open(start));
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(c) = queue.pop_front() {
            for o in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                let q = c + o;
                if sealed_block(&p, q).is_none() {
                    assert!(inside(c, p.hatch.0, p.hatch.1), "seed {seed}: gas at {c} escapes toward {q}");
                    continue;
                }
                if open(q) && seen.insert(q) {
                    queue.push_back(q);
                }
            }
        }
        for c in &seen {
            assert!(p.pressurised(*c), "seed {seed}: gas reaches unpressurised cell {c}");
        }
        for c in cells(&p).filter(|&c| p.pressurised(c) && open(c)) {
            assert!(seen.contains(&c), "seed {seed}: pressurised cell {c} is cut off");
        }
        for s in p.spaces.iter().filter(|s| s.kind != Kind::Damaged) {
            assert!(seen.contains(&s.min.with_y(s.max.y)), "seed {seed}: {:?} has no air", s.kind);
        }
        assert!(p.spaces.iter().any(|s| s.kind == Kind::Damaged && !p.pressurised(s.min)));
    }
}

#[test]
fn rooms_are_lit_except_the_damaged_section() {
    for seed in SEEDS.step_by(5) {
        let p = plan(seed);
        for s in &p.spaces {
            let lamps = (s.min.x..=s.max.x)
                .flat_map(|x| (s.min.z..=s.max.z).map(move |z| IVec3::new(x, s.max.y + 1, z)))
                .filter(|&c| block(&p, c) == Some(B::GlowCap))
                .count();
            if s.kind == Kind::Damaged {
                assert_eq!(lamps, 0, "seed {seed}");
            } else {
                assert!(lamps > 0, "seed {seed}: dark {:?}", s.kind);
            }
        }
        assert!(p.markers.iter().any(|m| m.role == Role::Trader) && p.markers.iter().filter(|m| m.role == Role::Hostile).count() == 2);
    }
}

/// Floor plan of one station at standing height, 12 px per metre:
/// `STATION_SEED=n cargo test --release -p engine_worldgen --lib station_plan_png -- --ignored`.
#[test]
#[ignore]
fn station_plan_png() {
    let seed = std::env::var("STATION_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
    let p = plan(seed);
    const PX: i32 = 12;
    let size = p.max - p.min + IVec3::ONE;
    let (w, h) = ((size.x * PX) as usize, (size.z * PX) as usize);
    let mut rgb = vec![0u8; w * h * 3];
    let colour = |b: Option<B>| -> [u8; 3] {
        match b {
            None => [12, 14, 20],
            Some(B::Air) => [196, 198, 204],
            Some(B::SteelBlock | B::SteelPlate) => [70, 74, 84],
            Some(B::RustedSteel) => [128, 70, 40],
            Some(B::CopperBlock | B::HazardPaint) => [214, 150, 40],
            Some(B::WindowGlass) => [120, 190, 230],
            Some(B::Bed) => [170, 40, 50],
            Some(B::Chest | B::ChestSteel | B::ChestCopper) => [240, 200, 60],
            Some(B::StorageCrate | B::SalvageCrate) => [140, 100, 60],
            Some(B::Gravel) => [110, 104, 96],
            _ => [60, 150, 90],
        }
    };
    for z in 0..size.z {
        for x in 0..size.x {
            let c = IVec3::new(p.min.x + x, 0, p.min.z + z);
            let mut rgbc = colour(block(&p, c));
            if p.markers.iter().any(|m| m.at == c) {
                rgbc = [200, 40, 200];
            }
            for dz in 0..PX {
                for dx in 0..PX {
                    let edge = dx == 0 || dz == 0;
                    let k = (((z * PX + dz) as usize) * w + (x * PX + dx) as usize) * 3;
                    let v = if edge { rgbc.map(|v| v.saturating_sub(18)) } else { rgbc };
                    rgb[k..k + 3].copy_from_slice(&v);
                }
            }
        }
    }
    let dir = format!("{}/../../target/maps", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).expect("map dir");
    let out = format!("{dir}/station-{seed}.png");
    std::fs::write(&out, crate::map_probe::png(w as u32, h as u32, &rgb)).expect("write plan");
    eprintln!("{out}");
}
