use super::super::model::*;
use anyhow::{Context,Result,ensure};

pub(super) fn formed(goal:Goal) -> Result<State> {
    let mut s=State {enabled:true,..State::default()};
    for id in 0..4 {s.person(1,id);}
    for p in s.people.values_mut() {p.belonging=90;p.autonomy=30;}
    s.contact(0,1,0);s.contact(0,1,PERIOD);
    s.decide(0,PERIOD,Some(goal),false);
    ensure!(s.circles.is_empty() && s.offers.len() == 1,"Proposal assigned membership without recipient");
    s.decide(1,PERIOD+1,Some(goal),false);
    ensure!(s.circles.len() == 1 && s.accepted == 1 && s.valid(),"Recipient failed voluntary acceptance");
    Ok(s)
}
pub(super) fn run() -> Result<()> {
    let goal=Goal::at([1,10,1]);let other=Goal::at([20,10,1]);
    let mut s=formed(goal)?;
    let bytes=ron::to_string(&s)?;
    let mut replay:State=ron::from_str(&bytes)?;
    ensure!(s == replay,"Policy save/reload changed state");
    for state in [&mut s,&mut replay] {
        state.people.get_mut(&0).context("Missing person 0")?.autonomy=100;
        state.people.get_mut(&0).context("Missing person 0")?.belonging=60;
        state.contact(0,2,STALE);state.contact(0,2,STALE+PERIOD);
        state.decide(0,STALE+2*PERIOD,Some(other),true);
        state.decide(2,STALE+2*PERIOD+1,Some(other),false);
    }
    ensure!(s == replay && s.valid(),"Replayed choices diverged");
    ensure!(s.exits == 1 && s.splits == 1 && s.circles.len() == 2,"Independent resident failed to splinter");
    ensure!(s.circles[1].parent == Some(1) && s.circles[0].members == vec![1],"Splinter lost origin or moved unwilling member");
    let mut unwilling=State::default();
    unwilling.person(2,0);unwilling.person(2,1);
    unwilling.people.get_mut(&0).context("Missing person 0")?.belonging=100;
    unwilling.people.get_mut(&0).context("Missing person 0")?.autonomy=10;
    unwilling.people.get_mut(&1).context("Missing person 1")?.belonging=0;
    unwilling.people.get_mut(&1).context("Missing person 1")?.autonomy=100;
    unwilling.contact(0,1,0);unwilling.contact(0,1,PERIOD);
    unwilling.decide(0,PERIOD,Some(goal),false);unwilling.decide(1,PERIOD+1,Some(goal),false);
    ensure!(unwilling.circles.is_empty(),"Unwilling resident joined");
    let before=unwilling.clone();unwilling.decide(1,PERIOD+1,Some(goal),false);
    ensure!(unwilling == before,"Same-tick repeat changed decisions");
    let mut no_work=formed(goal)?;
    no_work.leave(1,3*PERIOD);no_work.offers.clear();
    no_work.decide(1,4*PERIOD,None,false);
    ensure!(no_work.offers.is_empty(),"No work still proposed project");
    let mut invalid=s.clone();invalid.circles[0].members.push(2);
    ensure!(!invalid.valid(),"Duplicate group membership accepted");
    s.retain(&[1,2,3],4*STALE);
    ensure!(s.valid() && !s.people.contains_key(&0),"Removed resident retained organization authority");
    edge_cases(goal,other)?;
    println!("[group-check] PASS proposal/consent, refusal, motives, splinter origin, membership bounds and deterministic reload");
    Ok(())
}

fn edge_cases(goal:Goal,other:Goal) -> Result<()> {
    let mut s=State::default();for id in 0..3 {s.person(1,id);}
    for p in s.people.values_mut() {p.belonging=90;p.autonomy=30;}
    s.people.get_mut(&1).context("Missing refuser")?.belonging=0;
    s.people.get_mut(&1).context("Missing refuser")?.autonomy=100;
    for turn in 0..5 {s.contact(0,1,turn*PERIOD);}
    for turn in 0..4 {s.contact(0,2,turn*PERIOD);}
    s.decide(0,5*PERIOD,Some(goal),false);s.decide(1,5*PERIOD+1,Some(goal),false);
    s.decide(0,6*PERIOD,Some(goal),false);s.decide(2,6*PERIOD+1,Some(goal),false);
    ensure!(s.people[&0].circle == s.people[&2].circle && s.people[&0].circle.is_some(),"Refusal starved willing contact");
    ensure!(s.people[&1].circle.is_none(),"Refusal was overridden");
    let mut s=formed(goal)?;
    s.decide(0,2*STALE,Some(other),false);
    ensure!(s.goal(0) == Some(other) && s.goal(1).is_none(),"Goal change coerced another member");
    s.decide(1,2*STALE+1,Some(other),true);
    ensure!(s.goal(1) == Some(other),"Member did not independently renew project consent");
    let mut abandoned=formed(goal)?;
    for p in abandoned.people.values_mut() {p.accepted_goal=None;p.autonomy=10;}
    abandoned.decide(0,2*STALE,Some(other),false);
    abandoned.decide(1,2*STALE+1,Some(other),false);
    ensure!(abandoned.circles.is_empty() && abandoned.valid(),"Expired goal stranded unconsenting members");
    let mut s=State::default();for id in 0..64 {s.person(1,id);}
    'fill: for a in 0..64 {for b in a+1..64 {
        s.contact(a,b,0);if s.bonds.len() == 128 {break 'fill;}
    }}
    s.contact(62,63,200*STALE);
    ensure!(s.bonds.len() == 128 && s.trust(62,63,200*STALE) == 20 && s.valid(),"Stale bond cap froze new relationships");
    println!("[group-check] PASS refusal cooldown, willing alternate, per-member goal renewal and stale bond replacement");
    Ok(())
}
