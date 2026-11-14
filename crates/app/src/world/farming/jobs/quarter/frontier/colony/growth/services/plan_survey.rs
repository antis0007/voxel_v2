use super::*;

pub(crate) fn select(w: &mut World, s: &Site, primary: [i32; 3], candidate: usize) -> Result<Option<Plan>, String> {
    if w.voxel_scale.meters() != 1.0 { return Err("Service Survey Requires One Metre Cells".into()); }
    if !bounded(primary) || w.farming.jobs.stockpile != Some(primary) { return Err("Stockpile Changed".into()); }
    if !(6..=16).contains(&s.projects.len()) || s.projects.iter().map(|p| p.cells.len()).sum::<usize>() > 4096 {
        return Err("Service Project Budget".into());
    }
    let c = content::read()?;
    let spec = c.candidates.get(candidate).ok_or("Service Candidates Exhausted")?;
    let q = parcel(c, spec, primary);
    let hub = shifted(primary, spec.hub);
    let approach = shifted(primary, spec.approach);
    let stops: Vec<_> = spec.route.iter().map(|p| shifted(primary, *p)).collect();
    let home_points = route(approach, &stops)?;
    let columns = terrain::columns(&q, hub, home_points.iter().copied().chain([approach]));
    terrain::admit(w, &columns, primary[1])?;
    if terrain::reserved(w, s, s.projects.len(), &columns)? { return Ok(None); }
    let Some(origin) = terrain::floor(w, q.origin, primary[1]) else { return Ok(None); };
    let Some(hub) = terrain::floor(w, hub, primary[1]) else { return Ok(None); };
    let mut anchors = Vec::new();
    for (i, p) in s.projects.iter().enumerate() {
        let high = p.cells.iter().map(|o| p.work_layer(*o)).max().unwrap_or(0);
        if !s.project_layers.get(i).is_some_and(|y| *y > high + 1) { continue; }
        for (k, o) in p.cells.iter().enumerate() {
            let distance = (o.at[0] - approach[0]).abs() + (o.at[2] - approach[1]).abs();
            if o.block() == B::Planks && (1..=8).contains(&distance) && near(o.at, primary)
                && known(w, o.cell()) == Some(B::Planks) && jobs::stand(w, o.cell() + IVec3::Y, 1.0)
                && (-1..=2).all(|dy| !terrain::protected(w, o.cell() + IVec3::Y * dy)) {
                anchors.push((distance, i, k, o.at));
            }
        }
    }
    anchors.sort_unstable();
    let Some(&(_, anchor_project, anchor_order, a)) = anchors.first() else { return Ok(None); };
    let primary_points = primary_route(a, approach)?;
    let heights = |points: Vec<[i32; 2]>| points.into_iter().map(|p| terrain::floor(w, p, primary[1])).collect::<Option<Vec<_>>>();
    let Some(primary_access) = heights(primary_points) else { return Ok(None); };
    let Some(home_access) = heights(home_points) else { return Ok(None); };
    let plan = Plan { content_version: c.version, provenance: content::provenance(), candidate,
        base_projects: s.projects.len(), primary, hub, origin, turn: q.turn, complexity: c.complexity,
        capacity: c.capacity, parcel: q, primary_access, home_access, anchor_project, anchor_order };
    if !valid(&plan, s, plan.base_projects) { return Ok(None); }
    if !available(w, s, &plan)? { return Ok(None); }
    Ok(Some(plan))
}

pub(crate) fn available(w: &mut World, s: &Site, plan: &Plan) -> Result<bool, String> {
    if w.voxel_scale.meters() != 1.0 || w.farming.jobs.stockpile != Some(plan.primary)
        || !valid(plan, s, plan.base_projects) { return Ok(false); }
    let columns = terrain::columns(&plan.parcel, point(plan.hub),
        plan.primary_access.iter().chain(&plan.home_access).copied().map(point));
    terrain::admit(w, &columns, plan.primary[1])?;
    if terrain::reserved(w, s, plan.base_projects, &columns)?
        || !terrain::paid_floor(w, s, plan.anchor_project, plan.anchor_order)
        || known(w, plan.primary.into()).is_none_or(|b| b.chest_tier().is_none()) { return Ok(false); }
    let b = plan.parcel.bounds();
    for x in b[0]..=b[2] { for z in b[1]..=b[3] {
        if !terrain::clear(w, [x, plan.origin[1], z], 10) { return Ok(false); }
    } }
    if plan.home_access.iter().any(|p| !terrain::clear(w, *p, 2)) { return Ok(false); }
    let Some(a) = anchor(s, plan) else { return Ok(false); };
    let start = jobs::metres(a.cell() + IVec3::Y, 1.0);
    let task = jobs::Task::Store(plan.primary.into());
    if jobs::path_with_limit(w, start, task, 1100).is_empty() && !jobs::reachable(w, start, task, 1.0) {
        return Ok(false);
    }
    if s.projects.len() > plan.base_projects {
        return Ok(hub_available(w, s, plan, plan.base_projects));
    }
    Ok(terrain::clear(w, plan.hub, 2) && plan.primary_access.iter().all(|p| terrain::clear(w, *p, 2)))
}

pub(crate) fn hub_available(w: &World, s: &Site, plan: &Plan, hub_index: usize) -> bool {
    if hub_index != plan.base_projects || w.voxel_scale.meters() != 1.0
        || plan.primary_access.is_empty()
        || plan.primary_access.len() > 8 || !bounded(plan.primary) || !bounded(plan.hub)
        || plan.provenance != content::provenance()
        || !terrain::paid_floor(w, s, plan.anchor_project, plan.anchor_order) { return false; }
    let Ok(c) = content::read() else { return false; };
    let Some(spec) = c.candidates.get(plan.candidate) else { return false; };
    let Some(a) = anchor(s, plan) else { return false; };
    let Ok(path) = primary_route(a.at, shifted(plan.primary, spec.approach)) else { return false; };
    if plan.content_version != c.version || point(plan.hub) != shifted(plan.primary, spec.hub)
        || plan.primary_access.iter().copied().map(point).collect::<Vec<_>>() != path
        || !step(a.at, plan.primary_access[0]) || plan.primary_access.windows(2).any(|p| !step(p[0], p[1]))
        || plan.primary_access.last().is_none_or(|p| !step(*p, plan.hub) || p[1] != plan.hub[1]) {
        return false;
    }
    let Ok(expected) = hub_project(plan) else { return false; };
    let Some(p) = s.projects.get(hub_index) else { return false; };
    if !same(p, &expected) || !s.home_complete(w, hub_index)
        || known(w, plan.primary.into()).is_none_or(|b| b.chest_tier().is_none()) { return false; }
    p.cells.iter().all(|o| near(o.at, plan.primary) && jobs::resident(w, o.cell())
        && !terrain::protected(w, o.cell()) && known(w, o.cell()) == Some(o.block()))
        && known(w, IVec3::from(plan.hub) - IVec3::Y).is_some_and(B::is_solid)
        && (-1..=2).all(|dy| !terrain::protected(w, IVec3::from(plan.hub) + IVec3::Y * dy))
        && plan.primary_access.iter().all(|p| jobs::stand(w, IVec3::from(*p) + IVec3::Y, 1.0)
            && (-1..=2).all(|dy| !terrain::protected(w, IVec3::from(*p) + IVec3::Y * dy)))
}
