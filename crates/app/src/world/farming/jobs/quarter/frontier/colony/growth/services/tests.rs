use super::*;

fn fixture() -> (Site,State,Services) {
    let (plan,site)=plan::fixture();
    let mut prefix=infill::InfillPlan::new(IVec3::from(plan.primary)-IVec3::Y,plan.primary,17,0);
    prefix.schema=2;prefix.surveyed=true;prefix.attempts=1;
    #[derive(Deserialize)] struct Content {production:production::State}
    let content:Content=ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../assets/city_colony.ron"))).unwrap();
    let mut service=Services::new(site.projects.len(),targets::Targets::capture(&content.production).unwrap());
    service.cursor=plan.candidate;service.attempts=1;service.stage=Stage::HubMaterials;service.plan=Some(plan.clone());
    service.reserved_budget.reserve();
    let mut g=State::new(plan.primary);g.version=4;g.infill=Some(prefix);g.proposal=Some(plan::hub_project(&plan).unwrap());
    (site,g,service)
}

fn save(site: &mut Site, g: &State, service: &Services) {
    let mut saved=g.clone();saved.services=Some(service.clone());site.growth=Some(saved);
    assert!(site.valid(),"rejected {:?}",service.stage);
    let bytes=ron::to_string(site).unwrap();let loaded:Site=ron::from_str(&bytes).unwrap();
    assert!(loaded.valid());assert_eq!(bytes,ron::to_string(&loaded).unwrap());
}

fn terminal(site: &mut Site, i: usize) {
    site.project_layers[i]=site.projects[i].cells.iter().map(|o|site.projects[i].work_layer(*o)).max().unwrap()+2;
    site.phase+=1;
}

#[test]
fn service_stages_keep_paid_prefix_and_ordered_projects() {
    let (mut site,mut g,mut service)=fixture();
    let prefix=ron::to_string(&site.projects).unwrap();
    save(&mut site,&g,&service);
    state::admit(&mut site,&mut g,&mut service).unwrap();
    assert_eq!(service.stage,Stage::HubBuilding);save(&mut site,&g,&service);
    assert!(state::finished(&site,&mut g,&mut service).is_err());
    terminal(&mut site,6);state::finished(&site,&mut g,&mut service).unwrap();
    assert_eq!(service.stage,Stage::HomeMaterials);save(&mut site,&g,&service);
    state::admit(&mut site,&mut g,&mut service).unwrap();
    assert_eq!(service.stage,Stage::HomeBuilding);save(&mut site,&g,&service);
    terminal(&mut site,7);state::finished(&site,&mut g,&mut service).unwrap();
    assert!(g.complete && g.project.is_none());save(&mut site,&g,&service);
    assert_eq!(ron::to_string(&site.projects[..6]).unwrap(),prefix);
    assert_eq!(site.projects[7].home.as_ref().unwrap().capacity,8);
    assert!(g.households_live(&site));
    assert_eq!(jobs::construction::LIMIT,256);
}

#[test]
fn service_saved_state_rejects_stage_project_and_prefix_corruption() {
    let (mut site,g,service)=fixture();save(&mut site,&g,&service);
    for mode in 0..11 {
        let mut bad=site.clone();let state=bad.growth.as_mut().unwrap();let s=state.services.as_mut().unwrap();
        match mode {
            0=>state.version=3,1=>state.complete=true,2=>s.base_project_count=7,
            3=>s.plan.as_mut().unwrap().provenance^=1,4=>s.hub_project=Some(6),
            5=>state.proposal.as_mut().unwrap().cells.reverse(),6=>s.stage=Stage::HomeMaterials,
            7=>s.cursor=plan::candidate_count()+1,8=>s.targets.woodlot.original=257,
            9=>state.infill.as_mut().unwrap().schema=1,_=>s.retry_at=1,
        }
        assert!(!bad.valid(),"accepted mutation {mode}");
    }
}

#[test]
fn service_upgrade_requires_settled_exhausted_growth_three_and_demand() {
    let (mut site,mut g,_)=fixture();g.version=3;g.proposal=None;
    g.waiting="No Space: Local Capacity Exhausted".into();site.growth=Some(g.clone());
    assert!(state::eligible(&g,&site,1));assert!(!state::eligible(&g,&site,0));
    let mut bad=g.clone();bad.waiting="Street Disconnected: No Paid Frontage".into();
    assert!(!state::eligible(&bad,&site,1));
    bad=g.clone();bad.version=2;assert!(!state::eligible(&bad,&site,1));
    bad=g.clone();bad.infill.as_mut().unwrap().restart();assert!(!state::eligible(&bad,&site,1));
    bad=g.clone();bad.project=Some(5);assert!(!state::eligible(&bad,&site,1));
    site.project_layers[0]-=1;assert!(!state::eligible(&g,&site,1));
}
