use super::*;
use crate::world::inventory::Item;

pub(crate) fn fixture() -> (Plan, Site) {
    let c = content::read().unwrap();
    let spec = &c.candidates[0];
    let primary = [0, 18, 0];
    let primary_access = (18..=20).map(|x| [x, 18, 11]).collect();
    let home_access = route(spec.approach, &spec.route).unwrap().into_iter()
        .map(|[x, z]| [x, if x <= 24 { 18 } else { 17 }, z]).collect();
    let plan = Plan { content_version: c.version, provenance: content::provenance(), candidate: 0,
        base_projects: 6, primary, hub: [20, 18, 12], origin: [27, 17, 0], turn: false,
        complexity: c.complexity, capacity: c.capacity, parcel: parcel(c, spec, primary),
        primary_access, home_access, anchor_project: 0, anchor_order: 0 };
    let projects = (0..6).map(|i| Project { name: format!("Paid prefix {i}"),
        cells: vec![Order::new(if i == 0 { IVec3::new(17, 18, 11) } else { IVec3::new(0, 18, i) }, B::Planks)],
        food_reserve: 0, meals_eaten: 0, home: None }).collect();
    let site = Site { growth: None, version: 1, projects, timber: Vec::new(), timber_next: Some(0),
        phase: 6, layer: 0, project_layers: vec![20; 6], households: Vec::new(), housing_overrides: Vec::new() };
    (plan, site)
}

#[test]
fn service_geometry_reuses_full_timber_template() {
    let (plan, site) = fixture();
    assert!(valid(&plan, &site, 6));
    assert_eq!(candidate_count(), 4);
    let hub = hub_project(&plan).unwrap();
    let home = home_project(&plan).unwrap();
    assert_eq!(hub.cells.len(), 4);
    assert_eq!(plan.home_access.len(), 27);
    assert_eq!(home.cells.len(), 593);
    assert_eq!(home.home.as_ref().unwrap().capacity, 8);
    assert_eq!(home.cells.iter().filter(|o| o.block() == B::Chest).count(), 1);
    assert!(home.cells.iter().any(|o| o.block() == B::Log));
    assert_eq!(home.cells.iter().filter(|o| o.material() == Some(Item::Block(B::Log))).count(), 114);
    assert_eq!(home.cells.iter().filter(|o| o.material() == Some(Item::Block(B::Planks))).count(), 478);
    assert!(home.cells.iter().all(|o| matches!(o.material(), Some(Item::Block(B::Log | B::Planks | B::Chest)))));
    assert!(home.cells.iter().all(|o| near(o.at, plan.hub)));
    assert!(home.cells.iter().any(|o| !near(o.at, plan.primary)));
    let mut layers = BTreeMap::new();
    for o in &home.cells { *layers.entry(home.work_layer(*o)).or_insert(0) += 1; }
    assert!(layers.values().all(|n| *n <= 256));
    assert_eq!(layers.values().max(), Some(&217));
    let bounds = plan.parcel.bounds();
    assert!(home.cells.iter().filter(|o| !plan.home_access.contains(&o.at)).all(|o|
        o.at[0] >= bounds[0] && o.at[0] <= bounds[2] && o.at[2] >= bounds[1] && o.at[2] <= bounds[3]));
    let encoded = ron::to_string(&plan).unwrap();
    let restored: Plan = ron::from_str(&encoded).unwrap();
    assert_eq!(plan, restored);
    assert_eq!(home.cells, home_project(&restored).unwrap().cells);
}

#[test]
fn service_plan_rejects_provenance_and_paid_access_changes() {
    let (plan, site) = fixture();
    let reject = |change: fn(&mut Plan)| {
        let mut altered = plan.clone(); change(&mut altered); assert!(!valid(&altered, &site, 6));
    };
    reject(|p| p.provenance ^= 1);
    reject(|p| p.anchor_order += 1);
    reject(|p| p.primary_access[0][0] += 1);
    reject(|p| p.home_access[5][1] += 3);
    reject(|p| p.parcel.variant += 1);
    reject(|p| p.capacity = 9);
    reject(|p| p.origin[1] += 1);
    reject(|p| p.candidate = usize::MAX);
    reject(|p| p.primary = [i32::MAX; 3]);
    let mut occupied = site.clone();
    occupied.timber.push([25, 17, 8]);
    assert!(!valid(&plan, &occupied, 6));
    let mut unpaid = site;
    unpaid.project_layers[0] = 18;
    assert!(!valid(&plan, &unpaid, 6));
}

#[test]
fn service_plan_admission_is_canonical_and_does_not_reorder_prefix() {
    let (plan, mut site) = fixture();
    let old: Vec<_> = site.projects.iter().map(|p| p.cells.clone()).collect();
    site.projects.push(hub_project(&plan).unwrap());
    site.project_layers.push(18);
    assert!(valid(&plan, &site, 6));
    site.projects.push(home_project(&plan).unwrap());
    site.project_layers.push(17);
    assert!(valid(&plan, &site, 6));
    assert_eq!(old, site.projects[..6].iter().map(|p| p.cells.clone()).collect::<Vec<_>>());
    site.projects[7].cells[0] = Order::new(IVec3::new(50, 17, 0), B::Planks);
    assert!(!valid(&plan, &site, 6));
}

#[test]
fn service_routes_are_bounded_without_terrain_edits() {
    assert!(route([0, 0], &[[1, 1]]).is_err());
    assert!(route([0, 0], &[[65, 0]]).is_err());
    assert!(route([0, 0], &[[2, 0], [1, 0]]).is_err());
    assert!(primary_route([0, 18, 0], [9, 0]).is_err());
    assert!(step([24, 18, 11], [25, 17, 11]));
    assert!(!step([24, 18, 11], [25, 16, 11]));
}
