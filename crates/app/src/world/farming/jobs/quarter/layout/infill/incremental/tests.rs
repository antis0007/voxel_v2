use super::*;

fn plan() -> InfillPlan {
    let mut p=InfillPlan::new(IVec3::ZERO,[0,1,0],71,0);
    p.schema=2;p.surveyed=true;p.attempts=1;p.search=Some(Search::default());p
}

#[test]
fn repeatable_cursor_enumerates_each_geometry_once() {
    let p=plan();let templates=templates(IVec3::ZERO).unwrap();
    let mut seen=BTreeSet::new();
    for index in 0..CANDIDATES {
        let parcel=parcel(index,&p,&templates).unwrap();
        assert!(seen.insert((parcel.origin,parcel.turn,parcel.stairs_right)));
        assert!(bounded(parcel.origin));
    }
    assert_eq!(seen.len(),9604);
    assert_eq!(CANDIDATES_PER_POLL,64);
    assert_eq!(ROUTES_PER_POLL,2);
}

#[test]
fn repeatable_saved_shortlist_rejects_corrupt_progress_and_rank() {
    let mut p=plan();let templates=templates(IVec3::ZERO).unwrap();
    let parcel=parcel(0,&p,&templates).unwrap();
    let candidate=Candidate {frontage:1,centre:centre(&parcel,&p),provenance:parcel.provenance,index:0};
    p.search=Some(Search {cursor:64,shortlist:vec![candidate],..Search::default()});
    assert!(valid(&p));
    let bytes=ron::to_string(&p).unwrap();let loaded:InfillPlan=ron::from_str(&bytes).unwrap();
    assert!(valid(&loaded));assert_eq!(ron::to_string(&loaded).unwrap(),bytes);
    for mode in 0..9 {
        let mut bad=p.clone();let search=bad.search.as_mut().unwrap();
        match mode {
            0=>search.cursor=CANDIDATES+1,
            1=>search.cursor=0,
            2=>search.routing=true,
            3=>search.shortlist[0].provenance^=1,
            4=>search.shortlist[0].centre+=1,
            5=>search.shortlist[0].frontage=97,
            6=>search.shortlist.push(search.shortlist[0].clone()),
            7=>search.paused=true,
            _=>search.routes=ROUTES+1,
        }
        assert!(!valid(&bad),"accepted mutation {mode}");
    }
    let search=p.search.as_mut().unwrap();search.shortlist.clear();search.routing=true;search.cursor=CANDIDATES;
    assert!(!valid(&p));
}

#[test]
fn repeatable_rank_prefers_near_frontage_and_preserves_small_saved_state() {
    let mut p=plan();let templates=templates(IVec3::ZERO).unwrap();
    let mut list=Vec::new();
    for index in 0..16 {
        let parcel=parcel(index,&p,&templates).unwrap();
        list.push(Candidate {frontage:1,centre:centre(&parcel,&p),provenance:parcel.provenance,index});
    }
    list.sort();
    p.search=Some(Search {cursor:CANDIDATES,routing:true,shortlist:list,..Search::default()});
    assert!(valid(&p));assert!(ron::to_string(&p).unwrap().len()<2048);
    let mut far=p.search.as_ref().unwrap().shortlist[0].clone();far.frontage+=1;far.centre=0;
    assert!(far>p.search.as_ref().unwrap().shortlist[0]);
    p.restart();assert!(!p.searching() && !p.surveyed && p.retry_at==0);
}
