use super::*;
use jobs::construction::Order;

fn fixture() -> Site {
    let projects = (0..6).map(|i|Project{name:format!("Home {i}"),
        cells:vec![Order::new(IVec3::new(i*2,0,-3),B::Planks)],food_reserve:0,meals_eaten:0,home:None}).collect();
    Site{growth:Some(State::new([0,1,0])),version:1,projects,timber:vec![],timber_next:Some(0),
        phase:6,layer:0,project_layers:vec![2;6],households:vec![],housing_overrides:vec![]}
}
fn proposal() -> Project {
    let spec = ron::from_str(r#"(name:"New house",origin:(3,1,16),size:(4,4),food_reserve:0,meals_eaten:0,path:[(8,1,15),(9,1,15),(10,1,15)])"#).unwrap();
    super::super::super::scene::plan(spec,IVec3::ZERO).unwrap()
}

#[test]
fn growth_saved_proposal_checks_home_geometry_and_paid_floor() {
    let mut s = fixture();
    let p = proposal();
    assert_eq!(p.cells.len(),91);
    assert_eq!(p.cells.iter().filter(|o|o.block()==B::Chest).count(),1);
    s.growth.as_mut().unwrap().proposal=Some(p.clone());
    assert!(s.valid());
    for at in [[1<<28,2,15],[5,3,15],[4,2,15],[50,2,50]] {
        s.growth.as_mut().unwrap().proposal.as_mut().unwrap().home.as_mut().unwrap().entry=at;
        assert!(!s.valid());
    }
    s.growth.as_mut().unwrap().proposal=Some(p);
    let bytes=ron::to_string(&s).unwrap();
    let loaded:Site=ron::from_str(&bytes).unwrap();
    assert!(loaded.valid() && loaded.growth.unwrap().proposal.is_some());
}

#[test]
fn growth_saved_completion_requires_terminal_parallel_ledger() {
    let mut s=fixture();let p=proposal();
    let first=p.cells.iter().map(|o|o.cell().y).min().unwrap();
    let last=p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap()+2;
    s.projects.push(p);s.project_layers.push(first);
    let g=s.growth.as_mut().unwrap();g.project=Some(6);g.complete=true;
    assert!(!s.valid());
    s.growth.as_mut().unwrap().complete=false;
    assert!(s.valid());
    s.project_layers[6]=last;s.phase=7;s.growth.as_mut().unwrap().complete=true;
    assert!(s.valid());
    s.project_layers.clear();
    assert!(!s.valid());
}

#[test]
fn growth_saved_retry_cannot_stall_past_one_poll_window() {
    let mut g=State::new([0,1,0]);g.next_poll=u64::MAX;
    assert!(!g.due(100));
    assert_eq!(g.next_poll,100+4*jobs::PULSE);
    assert!(g.due(100+4*jobs::PULSE));
    let mut s=fixture();s.growth=None;
    let legacy=ron::to_string(&s).unwrap();
    assert!(!legacy.contains("growth:"));
    let loaded:Site=ron::from_str(&legacy).unwrap();
    assert!(loaded.valid() && loaded.growth.is_none());
}

#[test]
fn growth_repeatable_version_requires_its_own_plan_schema() {
    let mut s=fixture();
    let mut plan=infill::InfillPlan::new(IVec3::ZERO,[0,1,0],17,0);
    plan.schema=2;
    let g=s.growth.as_mut().unwrap();g.version=3;g.infill=Some(plan);
    assert!(s.valid());
    let bytes=ron::to_string(&s).unwrap();let restored:Site=ron::from_str(&bytes).unwrap();
    assert!(restored.valid());assert_eq!(bytes,ron::to_string(&restored).unwrap());
    s.growth.as_mut().unwrap().version=2;assert!(!s.valid());
    s.growth.as_mut().unwrap().infill.as_mut().unwrap().schema=1;assert!(s.valid());
    s.growth.as_mut().unwrap().version=3;assert!(!s.valid());
}
