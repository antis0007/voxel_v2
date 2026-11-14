use super::*;

pub(crate) fn valid(plan: &Plan, s: &Site, base_projects: usize) -> bool {
    if plan.base_projects != base_projects || !(6..=16).contains(&base_projects)
        || !(base_projects..=base_projects + 2).contains(&s.projects.len())
        || s.projects.iter().any(|p| p.cells.len() > 1024)
        || s.projects.iter().map(|p| p.cells.len()).sum::<usize>() > 4096
        || s.project_layers.len() != s.projects.len() || plan.anchor_project >= base_projects
        || !bounded(plan.primary) || !bounded(plan.hub) || !bounded(plan.origin)
        || plan.primary_access.is_empty() || plan.primary_access.len() > 8
        || plan.home_access.is_empty() || plan.home_access.len() > 64
        || plan.primary_access.iter().chain(&plan.home_access).any(|p| !bounded(*p)
            || (p[1] - plan.primary[1]).abs() > 3)
        || (plan.hub[1] - plan.primary[1]).abs() > 3
        || (plan.origin[1] - plan.primary[1]).abs() > 3 { return false; }
    let Ok(c) = content::read() else { return false; };
    let Some(spec) = c.candidates.get(plan.candidate) else { return false; };
    let q = parcel(c, spec, plan.primary);
    if plan.content_version != c.version || plan.provenance != content::provenance()
        || plan.parcel != q || plan.turn != q.turn || plan.complexity != c.complexity
        || plan.capacity != c.capacity || point(plan.origin) != q.origin
        || point(plan.hub) != shifted(plan.primary, spec.hub) { return false; }
    let Some(a) = anchor(s, plan) else { return false; };
    let old = &s.projects[plan.anchor_project];
    let high = old.cells.iter().map(|o| old.work_layer(*o)).max().unwrap_or(0);
    if a.block() != B::Planks || !bounded(a.at) || !near(a.at, plan.primary)
        || s.project_layers[plan.anchor_project] <= high + 1 { return false; }
    let approach = shifted(plan.primary, spec.approach);
    let Ok(primary) = primary_route(a.at, approach) else { return false; };
    let stops: Vec<_> = spec.route.iter().map(|p| shifted(plan.primary, *p)).collect();
    let Ok(home) = route(approach, &stops) else { return false; };
    if plan.primary_access.iter().copied().map(point).collect::<Vec<_>>() != primary
        || plan.home_access.iter().copied().map(point).collect::<Vec<_>>() != home
        || !step(a.at, plan.primary_access[0])
        || plan.primary_access.windows(2).any(|p| !step(p[0], p[1]))
        || plan.home_access.windows(2).any(|p| !step(p[0], p[1])) { return false; }
    let Some(&last) = plan.primary_access.last() else { return false; };
    if !step(last, plan.hub) || last[1] != plan.hub[1] || !step(last, plan.home_access[0])
        || plan.home_access.last().is_none_or(|p| p[1] != plan.origin[1]) { return false; }
    let Ok(hub) = hub_project(plan) else { return false; };
    let Ok(house) = home_project(plan) else { return false; };
    if hub.cells.iter().any(|o| !near(o.at, plan.primary))
        || house.cells.iter().any(|o| !near(o.at, plan.hub))
        || !house.cells.iter().any(|o| !near(o.at, plan.primary)) { return false; }
    let mut cells: BTreeSet<_> = s.projects.iter().take(base_projects)
        .flat_map(|p| p.cells.iter().map(|o| o.at)).collect();
    if hub.cells.iter().chain(&house.cells).any(|o| !cells.insert(o.at)) || cells.len() > 4096 {
        return false;
    }
    let bounds = q.bounds();
    let access: BTreeSet<_> = plan.primary_access.iter().chain(&plan.home_access)
        .chain([&plan.hub]).copied().map(point).collect();
    if s.projects.iter().take(base_projects).flat_map(|p| &p.cells).map(|o| point(o.at))
        .chain(s.timber.iter().copied().map(point)).any(|p| access.contains(&p)
            || (p[0] >= bounds[0] && p[0] <= bounds[2] && p[1] >= bounds[1] && p[1] <= bounds[3])) {
        return false;
    }
    s.projects.get(base_projects).is_none_or(|p| same(p, &hub))
        && s.projects.get(base_projects + 1).is_none_or(|p| same(p, &house))
}
