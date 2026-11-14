//! Exercise eviction and legacy decoding without retaining fixture changes.
use super::*;
use anyhow::{Context, ensure};
use crate::world::BlockId as B;

pub(super) fn check(w: &mut World, s: &mut State) -> anyhow::Result<()> {
    legacy()?;
    let Some(household) = s.households.iter().find(|h| usable(w, s, h.project)).cloned() else {
        return Ok(());
    };
    let home = s.projects[household.project].home.as_ref().context("Missing home")?;
    let door = (IVec3::from(home.entry) + IVec3::from(home.destination)) / 2;
    let target = jobs::metres(home.destination.into(), w.voxel_scale.meters());
    let roof = s.projects[household.project].cells.iter().filter(|o| o.block() == B::Planks)
        .max_by_key(|o| o.cell().y).context("Missing house roof")?.cell();
    let saved_households = s.households.clone();
    let saved_homes = w.travel.homes.clone();
    let saved_residents = residents(w);
    let actor_count = w.adventure.enemies.len();
    for (cell, replacement) in [(door, B::Planks), (roof, B::Air)] {
        let before = known(w, cell).context("Housing test cell is unavailable")?;
        w.set(cell, replacement);
        let outcome = (|| {
            ensure!(!usable(w, s, household.project), "Damaged home still usable");
            service(w, s);
            ensure!(!s.households.iter().any(|h| h.project == household.project),
                "Damaged house retained occupants");
            ensure!(household.members.iter().all(|id| w.travel.homes.get(id) != Some(&target)),
                "Evicted resident retained unsafe destination");
            ensure!(valid(s), "Eviction broke household ledger");
            ensure!(residents(w) == saved_residents && w.adventure.enemies.len() == actor_count,
                "Eviction changed resident roster or actor count");
            Ok::<(), anyhow::Error>(())
        })();
        // Restore even when a negative assertion fails.
        w.set(cell, before);
        s.households = saved_households.clone();
        w.travel.homes = saved_homes.clone();
        let restored = (|| {
            ensure!(usable(w, s, household.project), "Restored house unavailable");
            service(w, s);
            ensure!(s.households == saved_households && w.travel.homes == saved_homes,
                "Restored household allocation changed");
            ensure!(residents(w) == saved_residents && w.adventure.enemies.len() == actor_count,
                "Housing check changed resident roster or actor count");
            Ok::<(), anyhow::Error>(())
        })();
        s.households = saved_households.clone();
        w.travel.homes = saved_homes.clone();
        outcome?;
        restored?;
    }
    overrides(w, s, &household)?;
    println!("[city-colony-check] PASS housing door obstruction, roof loss, manual overrides, restored occupancy and legacy decoding");
    Ok(())
}

fn legacy() -> anyhow::Result<()> {
    let project = format!("(name:\"Legacy\",cells:[(at:(0,0,0),block:{})],food_reserve:0,meals_eaten:0)", B::Planks as u16);
    let p: super::super::super::Project = ron::from_str(&project)?;
    ensure!(p.home.is_none(), "Legacy project acquired housing metadata");
    let fixture = format!("(version:1,projects:[{project}],timber:[],phase:0,layer:0)");
    let old: State = ron::from_str(&fixture)?;
    ensure!(old.households.is_empty() && old.housing_overrides.is_empty()
        && old.projects[0].home.is_none() && old.valid(),
        "Legacy colony data no longer decodes with empty housing");
    Ok(())
}

fn overrides(w: &mut World, s: &mut State, household: &Household) -> anyhow::Result<()> {
    let id = *household.members.first().context("Empty occupied household")?;
    let home = s.projects[household.project].home.as_ref().context("Missing home")?;
    let explicit = jobs::metres(home.entry.into(), w.voxel_scale.meters());
    let saved_households = s.households.clone();
    let saved_overrides = s.housing_overrides.clone();
    let saved_homes = w.travel.homes.clone();
    let following = w.travel.following;
    let guards = w.travel.guard_posts.clone();
    let roster = residents(w);
    let actors = w.adventure.enemies.len();
    for mode in 0..3 {
        if mode == 0 { w.travel.homes.insert(id, explicit); }
        if mode == 1 { w.travel.following = Some(id); }
        if mode == 2 { w.travel.guard_posts.insert(id, explicit); }
        let destination = w.travel.homes.get(&id).copied();
        let outcome = (|| {
            service(w, s);
            ensure!(s.housing_overrides.contains(&id)
                && s.households.iter().all(|h| !h.members.contains(&id)), "Manual resident command ignored");
            ensure!(w.travel.homes.get(&id).copied() == destination, "Manual destination overwritten");
            w.travel.following = following;
            w.travel.guard_posts = guards.clone();
            service(w, s);
            ensure!(s.households.iter().all(|h| !h.members.contains(&id))
                && w.travel.homes.get(&id).copied() == destination, "Manual resident re-adopted");
            let restored: State = ron::from_str(&ron::to_string(s)?)?;
            ensure!(valid(&restored) && restored.housing_overrides.contains(&id), "Manual override lost on reload");
            ensure!(residents(w) == roster && w.adventure.enemies.len() == actors, "Manual override changed actors");
            Ok::<(), anyhow::Error>(())
        })();
        s.households = saved_households.clone();
        s.housing_overrides = saved_overrides.clone();
        w.travel.homes = saved_homes.clone();
        w.travel.following = following;
        w.travel.guard_posts = guards.clone();
        outcome?;
    }
    service(w, s);
    ensure!(s.households == saved_households && s.housing_overrides == saved_overrides
        && w.travel.homes == saved_homes, "Override test changed restored occupancy");
    Ok(())
}
