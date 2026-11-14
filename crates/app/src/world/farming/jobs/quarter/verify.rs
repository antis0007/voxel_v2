//! Focused checks run even when unrelated app tests do not compile.
use super::*;
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, HashSet, VecDeque};

pub(super) fn floor_work_position(root: &std::path::Path) -> Result<()> {
    let mut w = World::with_seed_in(
        crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter,
        7,
        &root.join("work-position"),
    );
    let y = engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 3;
    let target = IVec3::new(7, y, 7);
    for x in 4..=10 {
        for z in 4..=10 {
            w.set(IVec3::new(x, y - 1, z), B::Stone);
            w.set(
                IVec3::new(x, y, z),
                if x == 7 && z == 7 { B::Air } else { B::Log },
            );
        }
    }
    let start = Vec3::new(4.5, y as f32 + 1.0, 7.5);
    let task = super::super::Task::Build(super::super::construction::Order::new(target, B::Log));
    let path = super::super::path(&w, start, task);
    ensure!(
        !path.is_empty(),
        "No work position beside the final floor cell"
    );
    ensure!(
        path.back()
            .is_some_and(|p| super::super::reachable(&w, *p, task, 1.0)),
        "Path ends outside construction reach"
    );
    if let Some(p) = path.back() {
        for x in [-0.08, 0.08] {
            for z in [-0.08, 0.08] {
                ensure!(
                    super::super::reachable(&w, *p + Vec3::new(x, 0.0, z), task, 1.0),
                    "Work position grazes a voxel corner"
                );
            }
        }
    }
    if let Some(first) = path.front() {
        let obstacle = first.floor().as_ivec3();
        w.set(obstacle, B::Stone);
        let rerouted = super::super::path(&w, start, task);
        ensure!(
            !rerouted.is_empty() && rerouted.front() != path.front(),
            "Blocked waypoint did not reroute"
        );
        w.set(obstacle, B::Air);
        ensure!(
            !super::super::path(&w, start, task).is_empty(),
            "Route did not recover after clearing"
        );
    }
    println!("[city-check] PASS floor-hole work position, blocked waypoint reroute and recovery");
    low_roof_descent(root)
}

fn low_roof_descent(root: &std::path::Path) -> Result<()> {
    let mut w = World::with_seed_in(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter, 7, &root.join("roof-descent"));
    let y = engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 1;
    for x in 3..=8 { for z in 4..=10 { w.set(IVec3::new(x,y,z),B::Stone); }}
    for x in [5,6] {
        for h in 1..=2 {w.set(IVec3::new(x,y+h,5),B::Stone);}
        w.set(IVec3::new(x,y+1,6),B::Stone);
    }
    w.set(IVec3::new(5,y+4,6),B::Planks);
    let chest=IVec3::new(5,y+1,9); w.set(chest,B::Chest);
    let start=Vec3::new(5.5,(y+3) as f32,5.5);
    let path=super::super::path(&w,start,super::super::Task::Store(chest));
    ensure!(path.front().is_some_and(|p|p.x>6.0),"Route enters an impassable roof lip");
    let mut player=crate::player::Player::new();
    player.pos=start; player.precise_pos=start.as_dvec3(); player.vel=Vec3::ZERO; player.flying=false;
    super::walk::route(&w,&mut player,path.into_iter().collect())?;
    ensure!(player.pos.y < start.y-1.5,"Worker route did not descend the roof-side steps");
    println!("[city-check] PASS grounded descent around a low roof lip");
    Ok(())
}

pub(super) fn geometry_checks() -> Result<()> {
    translated_reach()?;
    let rows = traditions().map_err(anyhow::Error::msg)?;
    let text = ron::to_string(&rows)?;
    let decoded: Vec<Tradition> = ron::from_str(&text)?;
    ensure!(decoded.len() == 3, "Architecture content round trip");
    for style in &rows {
        for complexity in 1..=3 {
            let lot = Lot {
                origin: [0; 3],
                tradition: 0,
                complexity,
                turn: false,
                layer: 0,
                occupied: false,
        market_cover: false,
                frontage: false,
        roof_access: false,
                variant: 0,
            };
            let blocks: BTreeMap<_, _> = geometry::plan(&lot, style)
                .into_iter()
                .map(|(p, b)| ((p.x, p.y, p.z), b))
                .collect();
            let solid = |p: IVec3| p.y == -1 || blocks.contains_key(&(p.x, p.y, p.z));
            let stand = |p: IVec3| solid(p - IVec3::Y) && !solid(p) && !solid(p + IVec3::Y);
            let start = IVec3::new(4, 0, -3);
            let mut seen = HashSet::from([start]);
            let mut queue = VecDeque::from([start]);
            while let Some(p) = queue.pop_front() {
                for d in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z] {
                    for dy in [0, 1, -1] {
                        let n = p + d + IVec3::Y * dy;
                        if n.x < -3 || n.x > 12 || n.z < -4 || n.z > 18 || n.y < 0 || n.y > 18 {
                            continue;
                        }
                        if stand(n) && seen.insert(n) {
                            queue.push_back(n);
                        }
                    }
                }
            }
            for floor in 0..complexity {
                ensure!(
                    seen.contains(&IVec3::new(2, i32::from(floor) * 4 + 1, 3)),
                    "Interior route: {} complexity={complexity} floor={floor}",
                    style.name
                );
            }
            for y in 0..=20 {
                ensure!(
                    blocks.keys().filter(|(_, by, _)| *by == y).count()
                        <= super::super::construction::LIMIT,
                    "Layer exceeds build queue"
                );
            }
            for scale in [
                crate::fluid::VoxelScalePreset::OneMeter,
                crate::fluid::VoxelScalePreset::HalfMeter,
                crate::fluid::VoxelScalePreset::QuarterMeter,
            ] {
                let fine = geometry::plan_for_scale(&lot, style, scale);
                let factor = scale.meters().recip() as usize;
                ensure!(
                    fine.len() == blocks.len() * factor.pow(3),
                    "Scaled material volume changed"
                );
                let lo = fine
                    .iter()
                    .map(|(p, _)| *p)
                    .fold(IVec3::splat(i32::MAX), IVec3::min);
                let hi = fine
                    .iter()
                    .map(|(p, _)| *p)
                    .fold(IVec3::splat(i32::MIN), IVec3::max);
                let coarse_lo = blocks
                    .keys()
                    .map(|(x, y, z)| IVec3::new(*x, *y, *z))
                    .fold(IVec3::splat(i32::MAX), IVec3::min);
                let coarse_hi = blocks
                    .keys()
                    .map(|(x, y, z)| IVec3::new(*x, *y, *z))
                    .fold(IVec3::splat(i32::MIN), IVec3::max);
                ensure!(
                    (hi - lo + IVec3::ONE).as_vec3() * scale.meters()
                        == (coarse_hi - coarse_lo + IVec3::ONE).as_vec3(),
                    "Scaled building dimensions changed"
                );
            }
        }
    }
    cover_checks(&rows)?;
    println!(
        "[city-check] PASS nine architecture circulation checks, 27 scale-volume checks, content round trip and layer bounds"
    );
    Ok(())
}

fn translated_reach() -> Result<()> {
    for v in [1.0f32, 0.5, 0.25] {
        for coordinate in [-96, -1, 0, 7, 8, 15, 16, 64, 1024] {
            let cell = IVec3::new(coordinate, 18, coordinate);
            let task = super::super::Task::Build(super::super::construction::Order::new(cell, B::Cobblestone));
            let offset = ((v * 0.5 + 0.4) / v).ceil() as i32;
            for side in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z] {
                let feet = ((cell + side * offset).as_vec3() + Vec3::new(0.5,0.0,0.5)) * v;
                for dx in [-0.1,0.1] { for dz in [-0.1,0.1] {
                    ensure!(super::super::in_reach(feet + Vec3::new(dx,0.0,dz), task.point(v), task, v),
                        "Translated work clearance rejected: cell={cell:?} scale={v} side={side:?}");
                }}
            }
        }
    }
    println!("[city-check] PASS 432 translated work-clearance samples across three voxel scales");
    Ok(())
}

fn cover_checks(rows: &[Tradition]) -> Result<()> {
    use crate::fluid::VoxelScalePreset as Scale;
    for style in rows { for complexity in 1..=3 {
        let lot = Lot { origin:[0;3], tradition:0, complexity, turn:false,
            layer:0, occupied:false, market_cover:true, frontage:false, roof_access:false, variant:0 };
        let mut bare = lot.clone(); bare.market_cover = false;
        let cover = geometry::market_cover();
        ensure!(cover.len() == 37, "Market cover bill changed");
        ensure!(cover.iter().all(|(p,_)| p.x < -1), "Market cover blocked the main street");
        for scale in [Scale::OneMeter, Scale::HalfMeter, Scale::QuarterMeter] {
            let full = geometry::plan_for_scale(&lot, style, scale);
            let house = geometry::plan_for_scale(&bare, style, scale);
            let factor = scale.meters().recip() as usize;
            ensure!(full.len() - house.len() == 37 * factor.pow(3), "Market cover scaled volume changed");
        }
    }}
    println!("[city-check] PASS paid market cover bill, clear street and 27 scaled cover plans");
    Ok(())
}
