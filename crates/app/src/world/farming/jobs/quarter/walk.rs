//! Grounded player movement checks the completed street entrance.
use super::*;
use anyhow::{Result, ensure};

pub(super) fn check(w: &World, p: &mut crate::player::Player) -> Result<()> {
    let city = w
        .farming
        .jobs
        .quarter
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing quarter"))?;
    let lot = &city.lots[0];
    let base = IVec3::from(city.origin).as_vec3();
    let home = IVec3::from(lot.origin).as_vec3();
    let mut points = vec![
        base + Vec3::new(0.5, 1.0, 2.5),
        home + Vec3::new(4.5, 0.0, -2.5),
        home + Vec3::new(4.5, 1.0, 3.5),
    ];
    if lot.complexity > 1 {
        points.extend([
            home + Vec3::new(7.5, 1.0, 13.5),
            home + Vec3::new(7.5, 5.0, 9.5),
            home + Vec3::new(2.5, 5.0, 3.5),
        ]);
    }
    let travelled = route(w, p, points)?;
    ensure!(!p.flying, "Walk check unexpectedly used flight");
    ensure!(
        p.pos.y >= home.y + if lot.complexity > 1 { 4.9 } else { 0.9 },
        "Interior floor not reached"
    );
    println!(
        "[city-check] PASS grounded street-to-interior route distance_m={travelled:.2} feet={:?}",
        p.pos
    );
    if lot.market_cover { market(w, p)?; }
    Ok(())
}

pub(super) fn route(w: &World, p: &mut crate::player::Player, points: Vec<Vec3>) -> Result<f32> {
    let mut travelled = 0.0;
    for target in points {
        let mut arrived = false;
        for _ in 0..2400 {
            let d = (target - p.pos) * Vec3::new(1.0, 0.0, 1.0);
            if d.length() < 0.25 {
                arrived = true;
                break;
            }
            p.yaw = d.x.atan2(-d.z);
            p.pitch = 0.0;
            let before = p.pos;
            let mut input = crate::player::Input::default();
            input.forward = true;
            crate::player::update_player(p, &mut input, w, crate::simulation::STEP_SECONDS);
            travelled += p.pos.distance(before);
        }
        ensure!(
            arrived,
            "Grounded route blocked: feet={:?} target={target:?}",
            p.pos
        );
    }
    Ok(travelled)
}

fn market(w: &World, p: &mut crate::player::Player) -> Result<()> {
    let q = w.farming.jobs.quarter.as_ref().ok_or_else(|| anyhow::anyhow!("Missing quarter"))?;
    let base = IVec3::from(q.origin).as_vec3();
    // Independent grounded route from the same public street used by the home check.
    p.pos = base + Vec3::new(0.5, 1.0, 2.5);
    p.precise_pos = p.pos.as_dvec3(); p.vel = Vec3::ZERO; p.flying = false;
    let points = vec![base + Vec3::new(0.5, 1.0, 11.5),
        base + Vec3::new(-3.5, 1.0, 11.5), base + Vec3::new(-3.5, 1.0, 12.5),
        base + Vec3::new(-4.5, 1.0, 12.5)];
    let distance = route(w, p, points)?;
    ensure!(!p.flying && (p.pos.y - base.y - 1.0).abs() < 0.15, "Market walk lost grounded clearance");
    println!("[city-check] PASS grounded street-to-covered-market route distance_m={distance:.2} feet={:?}", p.pos);
    Ok(())
}
