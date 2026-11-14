use super::*;

pub(super) fn interruptions(w:&mut World)->Result<()> {
    let mut j=std::mem::take(&mut w.farming.jobs);
    let result=(|| {
        check_active(w,&mut j)?;
        let mut cargo_checked=false;
        for index in 0..j.helpers.len() {
            let mut helper=j.helpers.remove(index);helper.swap(&mut j);
            let candidate=j.carry.is_some()&&j.worker.is_some_and(|id|home(w,&j,id).is_some());
            let checked=if candidate {check_active(w,&mut j)} else {Ok(())};
            helper.swap(&mut j);j.helpers.insert(index,helper);checked?;
            if candidate {cargo_checked=true;break;}
        }
        ensure!(cargo_checked,"Loaded checkpoint has no housed carrier for interruption regression");
        Ok(())
    })();
    w.farming.jobs=j;result
}

fn check_active(w:&mut World,j:&mut State)->Result<()> {
        let id=j.worker.context("Missing lead resident")?;
        let (destination,entry,_)=home(w,j,id).context("Lead resident is not housed")?;
        let original=j.routine.clone();let original_follow=w.travel.following;
        let before_cargo=j.carry;let before_rests=j.routine.rests;
        let now=w.farming.clock;
        j.routine.enter(Phase::Rest,now);j.routine.target=Some(destination.to_array());
        j.task=Some(Task::Routine(destination));
        let previous=w.get(entry);w.set(entry,B::Stone);
        let pile=j.stockpile.context("Missing stockpile")?.into();
        service(w,metres(destination,w.voxel_scale.meters()),j,pile,768,RANGE*2.0+4.0);
        w.set(entry,previous);
        ensure!(j.routine.phase==Phase::Work&&j.routine.rests==before_rests,"Obstructed home accrued rest");
        ensure!(j.carry==before_cargo,"Obstruction discarded cargo");
        j.routine.enter(Phase::Rest,now);w.travel.following=Some(id);
        service(w,metres(destination,w.voxel_scale.meters()),j,pile,768,RANGE*2.0+4.0);
        w.travel.following=original_follow;
        ensure!(j.routine.phase==Phase::Work&&j.carry==before_cargo,"Following did not interrupt safely");
        ensure!(home(w,j,id).is_some(),"Home did not recover after obstruction");
        j.routine=original;j.reset_task();Ok(())
}
