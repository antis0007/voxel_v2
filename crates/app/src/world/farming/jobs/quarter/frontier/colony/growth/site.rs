//! Bounded candidates reuse authored building geometry without terrain edits.
use super::*;
use std::collections::BTreeSet;
#[derive(Deserialize)]
struct Content { candidates: Vec<super::super::super::scene::Spec> }

pub(super) fn valid_plan(s: &Site, p: &Project) -> bool {
    let mut cells = BTreeSet::new();
    !p.name.is_empty() && p.name.len() <= 48 && (1..=256).contains(&p.cells.len())
        && p.food_reserve == 0 && p.meals_eaten == 0
        && p.home.as_ref().is_some_and(|h| {
            let a = IVec3::from(h.entry); let b = IVec3::from(h.destination);
            h.capacity == 4 && h.entry.iter().chain(&h.destination).all(|n|n.unsigned_abs() < 1 << 27)
                && a.y == b.y && (a-b).abs().element_sum() == 2 && (a.x == b.x || a.z == b.z)
                && [a-IVec3::Y,b-IVec3::Y].iter().all(|at|p.cells.iter().any(|o|o.cell() == *at && o.block() == B::Planks))
        })
        && p.cells.iter().all(|o|o.at.iter().all(|n|n.unsigned_abs() < 1 << 27)
            && matches!(o.block(), B::Planks | B::Chest) && cells.insert(o.at)
            && s.projects.iter().all(|old|old.cells.iter().all(|x|x.at != o.at)))
}

pub(super) fn select(w: &World, s: &Site, origin: IVec3, scans: &mut Scans) -> Result<Project, String> {
    let content: Content = ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../assets/city_growth.ron")))
        .map_err(|_| "Growth Content Invalid")?;
    if content.candidates.is_empty() || content.candidates.len() > 3 { return Err("Candidate Limit".into()); }
    let mut reason = "Site Unavailable".to_string();
    for spec in content.candidates {
        let p = super::super::super::scene::plan(spec, origin)?;
        match available(w, s, &p, scans) { Ok(()) => return Ok(p), Err(e) => reason = e }
    }
    Err(reason)
}

pub(super) fn available(w: &World, s: &Site, p: &Project, scans: &mut Scans) -> Result<(), String> {
    scans.candidates = scans.candidates.saturating_add(1);
    if !valid_plan(s, p) { return Err("Proposal Invalid".into()); }
    let j = &w.farming.jobs;
    let pile = IVec3::from(j.stockpile.ok_or("Stockpile Unavailable")?);
    let prod = j.production.as_ref().ok_or("Production Unavailable")?;
    let crop_columns: BTreeSet<_> = w.farming.plots.keys().map(|&(x,_,z)|(x,z)).collect();
    let low = p.cells.iter().map(|o|o.cell()).reduce(IVec3::min).ok_or("Proposal Empty")?;
    let high = p.cells.iter().map(|o|o.cell()).reduce(IVec3::max).ok_or("Proposal Empty")?;
    if (high-low).max_element() > 12 { return Err("Site Bounds".into()); }
    for x in low.x..=high.x { for z in low.z..=high.z {
        let at = IVec3::new(x, low.y, z);
        if crop_columns.contains(&(x,z)) || prod.beds.iter().any(|b| {
            // Colony admission bounds the felling/haul corridor to 4.5 m.
            let margin = if b.crop == B::Sapling { (4.5/w.voxel_scale.meters()).ceil() as i32 } else { 0 };
            (b.at[0]-x).abs() <= margin && (b.at[2]-z).abs() <= margin
        }) || j.mine.iter().any(|(a,_)|a[0] == x && a[2] == z)
            || s.timber.iter().any(|a|a[0] == x && a[2] == z)
            || s.projects.iter().flat_map(|p|&p.cells).any(|o|o.at[0] == x && o.at[2] == z) {
            return Err("Reserved Ground".into());
        }
        if !known(w,at-IVec3::Y).is_some_and(B::is_solid) { return Err("Ground Unsupported".into()); }
        for y in low.y..=high.y+2 {
            scans.cells = scans.cells.saturating_add(1);
            let at = IVec3::new(x,y,z);
            if known(w,at) != Some(B::Air) { return Err("Site Obstructed".into()); }
            if crate::world::farming::base::overlaps(w,at,at) { return Err("Protected Ground".into()); }
        }
    }}
    for o in &p.cells {
        if (o.cell()-pile).as_vec3().length()*w.voxel_scale.meters() > jobs::RANGE {
            return Err("Stockpile Out of Range".into());
        }
        if j.build.iter().any(|old|old.at == o.at) { return Err("Build Conflict".into()); }
    }
    // The paid floor is not present yet. Validate its ground-level approach instead.
    let first = p.cells.iter().filter(|o|o.cell().y == low.y).find_map(|o| {
        [IVec3::X,IVec3::NEG_X,IVec3::Z,IVec3::NEG_Z].into_iter().map(|d|o.cell()+d)
            .find(|at|s.projects.iter().flat_map(|p|&p.cells).any(|old|old.cell() == *at)
                && known(w,*at).is_some_and(B::is_solid)
                && jobs::stand(w,*at+IVec3::Y,w.voxel_scale.meters()))
    }).ok_or("Street Disconnected")?;
    let start = jobs::metres(first+IVec3::Y,w.voxel_scale.meters());
    scans.path_searches = scans.path_searches.saturating_add(1);
    scans.path_node_budget = scans.path_node_budget.saturating_add(1100);
    let path = jobs::path_with_limit(w,start,jobs::Task::Store(pile),1100);
    if path.is_empty() && !jobs::reachable(w,start,jobs::Task::Store(pile),w.voxel_scale.meters()) {
        return Err("Stockpile Unreachable".into());
    }
    Ok(())
}


pub(crate) fn check(w: &mut World) -> anyhow::Result<()> {
    if w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).and_then(|s|s.growth.as_ref())
        .is_some_and(|g|g.version==3) {return super::repeatable_check::check(w);}
    if w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).and_then(|s|s.growth.as_ref())
        .is_some_and(|g|g.version==2) {return super::repeated_check::check(w);}
    use anyhow::{Context, ensure};
    let city = w.farming.jobs.quarter.as_ref().context("Missing colony")?;
    let s = city.frontier.as_ref().context("Missing site")?.clone();
    let origin = IVec3::from(city.origin);
    let first = select(w,&s,origin,&mut Scans::default()).map_err(anyhow::Error::msg)?;
    super::validation_check::check(&s,&first)?;
    let left = first.cells.iter().map(|o|o.cell()).min_by_key(|p|(p.x,p.z,p.y)).context("No candidate cell")?;
    let previous = known(w,left).context("Candidate unloaded")?;
    w.set(left,B::Stone);
    let fallback = select(w,&s,origin,&mut Scans::default());
    let preserved = known(w,left) == Some(B::Stone);
    w.set(left,previous);
    let fallback = fallback.map_err(anyhow::Error::msg)?;
    ensure!(preserved && fallback.home != first.home,"Blocked candidate failed to select another site");
    let block = origin+IVec3::new(5,1,18);
    let previous = known(w,block).context("Candidate unloaded")?;
    w.set(block,B::Stone);
    let blocked = select(w,&s,origin,&mut Scans::default()).is_err();
    let preserved = known(w,block) == Some(B::Stone);
    w.set(block,previous);
    ensure!(blocked && preserved,"Blocked candidates admitted or cleared a site");
    ensure!(w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).is_some_and(|site|
        site.projects.len() == s.projects.len()),"Site probes appended a project");
    println!("[city-growth] site-check passed fallback=1 all-blocked=1");
    Ok(())
}
