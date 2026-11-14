use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in super::super) enum Phase { #[default] Work, Meal, Rest, Social }

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in super::super) struct Resident {
    pub phase: Phase,
    pub due: u64,
    #[serde(default)] pub fatigue: u64,
    #[serde(default)] pub social_need: u64,
    #[serde(default)] pub needs_at: u64,
    pub since: u64,
    pub last: u64,
    pub dwell: u64,
    pub target: Option<[i32;3]>,
    pub rests: u32,
    #[serde(default)] pub rested_at: u64,
    #[serde(default)] pub social_partner: Option<u32>,
    #[serde(default)] pub social_at: u64,
    pub socials: u32,
    pub interrupted: u32,
    #[serde(default)] pub productive: u32,
    #[serde(default)] pub resumed: u32,
}
impl Resident {
    pub fn valid(&self)->bool { self.target.is_none_or(|p|p.iter().all(|n|n.unsigned_abs()<1<<28)) }
    pub fn enter(&mut self, phase:Phase, now:u64) {
        self.phase=phase; self.since=now; self.last=now; self.dwell=0; self.target=None;
    }
    pub fn request_meal(&mut self,now:u64,hungry:bool,threshold:u64) {
        if self.phase==Phase::Work&&now>=self.due&&self.fatigue>=threshold&&hungry {
            self.enter(Phase::Meal,now);
        }
    }
    pub fn meal_committed(&mut self,now:u64) {
        if self.phase==Phase::Meal {self.enter(Phase::Rest,now);}
    }
    pub fn observe(&mut self, now:u64, present:bool, cap:u64) {
        self.dwell=if present {self.dwell.saturating_add(now.saturating_sub(self.last).min(cap))} else {0};
        self.last=now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn due_meal_request_does_not_require_an_idle_job() {
        let mut r=Resident::default();r.due=10;r.fatigue=100;
        r.request_meal(20,false,60);assert_eq!(r.phase,Phase::Work);
        r.request_meal(20,true,60);assert_eq!(r.phase,Phase::Meal);
        assert_eq!(r.rests,0);assert_eq!(r.dwell,0);
    }
    #[test] fn only_a_committed_requested_meal_starts_rest() {
        let mut r=Resident::default();r.meal_committed(10);assert_eq!(r.phase,Phase::Work);
        r.enter(Phase::Meal,20);r.meal_committed(30);assert_eq!(r.phase,Phase::Rest);
        assert_eq!(r.since,30);assert_eq!(r.rests,0);
    }
    #[test] fn empty_destination_never_satisfies_rest() {
        let mut r=Resident::default(); r.enter(Phase::Rest,10);
        r.observe(1000,false,30); assert_eq!(r.dwell,0);
        r.observe(2000,true,30); assert_eq!(r.dwell,30);
        r.observe(2010,false,30); assert_eq!(r.dwell,0);
    }
    #[test] fn phase_transition_clears_destination_not_history() {
        let mut r=Resident::default(); r.rests=4; r.target=Some([1,2,3]);
        r.enter(Phase::Social,100); assert_eq!(r.rests,4); assert_eq!(r.target,None);
        assert_eq!(r.since,100); assert!(r.valid());
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in super::super) struct Meeting {
    pub ids: [u32;2],
    pub points: [[i32;3];2],
    pub started: u64,
    pub last: u64,
    pub dwell: u64,
    pub done: [bool;2],
}
impl Meeting {
    pub fn observe(&mut self,now:u64,present:bool,cap:u64,required:u64) {
        if self.dwell<required {
            self.dwell=if present {self.dwell.saturating_add(now.saturating_sub(self.last).min(cap))} else {0};
            self.last=now;
        }
    }
}
pub(in super::super) fn meetings_valid(rows:&[Meeting],capacity:usize)->bool {
    let mut ids=std::collections::BTreeSet::new();
    rows.len()<=capacity/2 && rows.iter().all(|m|
        m.ids[0]!=m.ids[1] && m.ids.iter().all(|id|ids.insert(*id))
        && m.points[0]!=m.points[1] && m.points.iter().flatten().all(|n|n.unsigned_abs()<1<<28))
}


#[cfg(test)]
mod meeting_tests {
    use super::*;
    fn meeting()->Meeting { Meeting {ids:[1,2],points:[[0,0,0],[1,0,0]],started:0,last:0,dwell:0,done:[false;2]} }
    #[test] fn reservations_require_distinct_people_and_positions() {
        let mut m=meeting();assert!(meetings_valid(&[m.clone()],64));
        m.ids=[1,1];assert!(!meetings_valid(&[m],64));
        let mut m=meeting();m.points=[[0,0,0];2];assert!(!meetings_valid(&[m],64));
    }
    #[test] fn empty_meeting_and_interrupted_dwell_do_not_count() {
        let mut m=meeting();m.observe(10000,false,30,120);assert_eq!(m.dwell,0);
        m.observe(20000,true,30,120);assert_eq!(m.dwell,30);
        m.observe(20030,false,30,120);assert_eq!(m.dwell,0);
    }
    #[test] fn paired_observations_do_not_double_elapsed_time() {
        let mut m=meeting();m.observe(15,true,30,120);m.observe(30,true,30,120);
        assert_eq!(m.dwell,30);
    }
    #[test] fn a_resident_cannot_reserve_two_visits() {
        let a=meeting();let mut b=meeting();b.ids=[2,3];assert!(!meetings_valid(&[a,b],64));
    }
}

pub(in super::super) fn waypoint_reached(flat_squared:f32,height:f32,scale:f32)->bool {
    flat_squared<0.01&&height<scale.max(0.25)*0.25
}
#[cfg(test)]
mod route_tests {
    #[test] fn doorway_turn_requires_cell_centre_precision() {
        assert!(!super::waypoint_reached(0.09*0.09+0.1*0.1,0.0,1.0));
        assert!(super::waypoint_reached(0.03*0.03+0.04*0.04,0.0,1.0));
        assert!(!super::waypoint_reached(0.0,0.3,1.0));
    }
}
