use super::*;

fn paid() -> (World,jobs::State,IVec3,jobs::construction::Order) {
    let (mut w,pile,order)=assigned_world();
    let mut j=std::mem::take(&mut w.farming.jobs);
    paid_cargo(&mut w,&mut j,pile);
    assert!(j.owns(order));
    (w,j,pile,order)
}

#[test]
fn rejected_city_commits_keep_cargo_and_player_edits() {
    for case in 0..6 {
        let (mut w,mut j,pile,order)=paid();
        let mut player=Vec3::splat(-100.0);
        match case {
            0 => j.claims[0].worker=11,
            1 => j.cargo=Some(jobs::supply::Cargo::Build {source:(pile+IVec3::X).to_array()}),
            2 => player=order.cell().as_vec3(),
            3 => w.set(order.cell()-IVec3::Y,B::Air),
            4 => w.set(order.cell(),B::DressedStone),
            5 => {j.build.clear();j.reconcile_claims();assert!(j.claims.is_empty());},
            _ => unreachable!(),
        }
        let block=w.get(order.cell());
        let cargo=j.carry;
        jobs::construction::place(&mut w,&mut j,order,player);
        assert_eq!(w.get(order.cell()),block,"case {case}: rejected edit");
        assert_eq!(j.carry,cargo,"case {case}: rejected debit");
        assert_eq!(material_total(&w,&j,pile,order),2,"case {case}: conservation");
    }
}

#[test]
fn paid_city_commit_survives_reload_and_repeated_attempts() {
    let (mut w,j,pile,order)=paid();
    let before=ron::to_string(&j.checkpoint(&w.adventure.enemies, w.farming.clock)).unwrap();
    w.farming.jobs=j;
    let (loaded,_)=crate::world::farming::sidecar::decode(&crate::world::farming::sidecar::encode(&w)).unwrap();
    assert_eq!(ron::to_string(&loaded.jobs).unwrap(),before);
    w.farming=loaded;
    let mut j=std::mem::take(&mut w.farming.jobs);
    assert!(j.valid() && j.owns(order));
    for _ in 0..3 {
        jobs::construction::place(&mut w,&mut j,order,Vec3::splat(-100.0));
        assert_eq!(w.get(order.cell()),B::Planks);
        assert_eq!(j.carry.unwrap().count,1);
        assert_eq!(material_total(&w,&j,pile,order),2);
        assert!(j.claims.is_empty() && j.build.is_empty() && j.valid());
    }
}

#[test]
fn city_work_needs_a_live_nearby_actor_and_physical_arrival() {
    for case in 0..4 {
        let (mut w,mut j,pile,order)=paid();
        j.task=Some(jobs::Task::Build(order));
        j.work=2;
        w.farming.jobs=j;
        match case {
            0 => w.adventure.enemies.clear(),
            1 => w.adventure.enemies[0].health=0.0,
            2 => w.adventure.enemies[0].pos+=Vec3::X*1000.0,
            3 => {}, // Assigned worker is nearby, but has not walked to the target.
            _ => unreachable!(),
        }
        w.farming.clock=0;
        jobs::service(&mut w,Vec3::new(0.5,pile.y as f32,0.5));
        assert_eq!(w.get(order.cell()),B::Air,"case {case}: no remote commit");
        assert_eq!(w.farming.jobs.carry.unwrap().count,2);
        assert_eq!(material_total(&w,&w.farming.jobs,pile,order),2);
        assert!(w.farming.jobs.valid());
        if case==3 {
            // Route requests are budgeted. Finish the queue without moving the
            // actor or advancing simulation time; arrival must still be needed.
            for _ in 0..64 {
                if w.farming.jobs.status != "Finding route" { break; }
                jobs::service(&mut w, Vec3::new(0.5,pile.y as f32,0.5));
            }
            assert_eq!(w.get(order.cell()), B::Air);
            assert_eq!(w.farming.jobs.carry.unwrap().count, 2);
            assert_eq!(material_total(&w,&w.farming.jobs,pile,order), 2);
            assert_eq!(w.farming.jobs.status,"Walking to build");
            assert_eq!(w.farming.jobs.task,Some(jobs::Task::Build(order)));
            assert!(!w.farming.jobs.path.is_empty());
        }
    }
}

#[test]
fn city_quarter_without_a_saved_worker_waits_without_orders() {
    let (mut w,_,order)=assigned_world();
    w.farming.jobs.build.clear();
    w.farming.jobs.worker_pos=None;
    let mut parcel=lot(1);parcel.origin=order.cell().to_array();
    let mut city=State {frontier:None,layout:None,lots:vec![parcel],housing_demand:4,
        residents:20,paused:false,completed:0,origin:[0;3],status:String::new()};
    advance(&mut w,&mut city);
    assert_eq!(city.status,"No Builder");
    assert_eq!(city.lots[0].layer,0);
    assert_eq!(city.completed,0);
    assert_eq!(city.housing_demand,4);
    assert!(w.farming.jobs.build.is_empty());
}

#[test]
fn city_authority_live_roster_keeps_paid_claims_and_reload_conservation() {
    let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root=std::env::temp_dir().join(format!("city-authority-live-{}-{nonce}",std::process::id()));
    assert!(!root.exists(),"fresh isolated test root");
    std::fs::create_dir_all(&root).unwrap();
    super::super::crew_check::run(&root).unwrap();
}
