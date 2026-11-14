use super::*;

pub(crate) fn hub_project(plan: &Plan) -> Result<Project, String> {
    let mut cells = BTreeMap::new();
    for &at in &plan.primary_access { put(&mut cells, at, B::Planks)?; }
    put(&mut cells, plan.hub, B::Chest)?;
    finish("Timber service hub", cells, None)
}
pub(crate) fn home_project(plan: &Plan) -> Result<Project, String> {
    let c = content::read()?;
    if plan.content_version != c.version || plan.provenance != content::provenance()
        || plan.complexity != c.complexity || plan.capacity != c.capacity
        || plan.turn != plan.parcel.turn || plan.parcel.complexity != plan.complexity
        || plan.parcel.origin != point(plan.origin) || !bounded(plan.origin) {
        return Err("Service Parcel Invalid".into());
    }
    let mut q = plan.parcel.clone();
    q.origin = [0, 0];
    let lot = q.lot(IVec3::from(plan.origin) - IVec3::Y, true);
    let mut cells = BTreeMap::new();
    for (local, block) in quarter_geometry::plan(&lot, &c.style) {
        if c.open_windows && block == B::WindowGlass { continue; }
        let material = if local.y == 0 { c.foundation }
            else if block == B::Cobblestone { c.frontage } else { block };
        put(&mut cells, quarter_geometry::at(&lot, local).to_array(), material)?;
    }
    // Paid footing permits a one-metre descent from open window ledges.
    let (width, depth) = (quarter_geometry::WIDTH, quarter_geometry::DEPTH);
    for x in -c.apron..=width { for z in -c.apron..=depth {
        if x != -c.apron && x != width && z != -c.apron && z != depth { continue; }
        let at = quarter_geometry::at(&lot, IVec3::new(x, 0, z)).to_array();
        if !cells.contains_key(&at) { put(&mut cells, at, c.foundation)?; }
    } }
    put(&mut cells, quarter_geometry::at(&lot, c.furniture.into()).to_array(), B::Chest)?;
    for &at in &plan.home_access { put(&mut cells, at, B::Planks)?; }
    let at = |x, y, z| quarter_geometry::at(&lot, IVec3::new(x, y, z)).to_array();
    finish("Unfinished timber house", cells, Some(Home {
        capacity: plan.capacity, entry: at(4, 1, -1), destination: at(4, 1, 1),
    }))
}
fn put(cells: &mut BTreeMap<[i32; 3], B>, at: [i32; 3], block: B) -> Result<(), String> {
    if !bounded(at) || !matches!(block, B::Log | B::Planks | B::Chest)
        || cells.insert(at, block).is_some() {
        return Err("Service Geometry Conflict".into());
    }
    Ok(())
}
fn finish(name: &str, cells: BTreeMap<[i32; 3], B>, home: Option<Home>) -> Result<Project, String> {
    if cells.is_empty() || cells.len() > 1024 { return Err("Service Project Capacity".into()); }
    let mut cells: Vec<_> = cells.into_iter().map(|(p, b)| Order::new(p.into(), b)).collect();
    cells.sort_by_key(|o| (o.at[1], o.at[2], o.at[0]));
    let mut layers = BTreeMap::new();
    for o in &cells {
        if o.material().is_none() { return Err("Service Material Invalid".into()); }
        if o.block() != B::Chest { *layers.entry(o.at[1]).or_insert(0usize) += 1; }
    }
    if layers.values().any(|n| *n > jobs::construction::LIMIT) { return Err("Service Layer Capacity".into()); }
    Ok(Project { name: name.into(), cells, food_reserve: 0, meals_eaten: 0, home })
}
