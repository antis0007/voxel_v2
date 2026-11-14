use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(super) const PEOPLE: usize = 64;
pub(super) const CIRCLES: usize = 16;
pub(super) const MEMBERS: usize = 8;
pub(super) const PERIOD: u64 = 16 * super::super::PULSE;
pub(super) const STALE: u64 = 240 * super::super::PULSE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in super::super) struct Goal { pub area: [i32; 3] }
impl Goal {
    pub fn at(at: [i32; 3]) -> Self { Self { area: [at[0].div_euclid(8), at[1].div_euclid(16), at[2].div_euclid(8)] } }
    pub fn contains(self, at: [i32; 3]) -> bool { self == Self::at(at) }
    fn valid(self) -> bool { self.area.iter().all(|n| n.unsigned_abs() < 1 << 25) }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Person {
    pub craft: u8, pub belonging: u8, pub autonomy: u8,
    #[serde(default)] pub accepted_goal: Option<Goal>,
    pub circle: Option<u32>, pub departed: Option<u32>,
    pub due: u64, pub cursor: usize, pub contribution: u32,
}
impl Person {
    pub fn new(seed: u32, id: u32) -> Self {
        let h = engine_core::hash::hash_u32(seed ^ id.wrapping_mul(0x9e37_79b9));
        Self { craft: 30 + (h % 71) as u8, belonging: 30 + ((h >> 8) % 71) as u8,
            autonomy: 10 + ((h >> 16) % 81) as u8, accepted_goal: None, circle: None, departed: None,
            due: 0, cursor: 0, contribution: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Circle {
    pub id: u32, pub name: String, pub members: Vec<u32>, pub goal: Goal,
    pub parent: Option<u32>, pub formed: u64, pub last_progress: u64, pub paid_blocks: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Offer { pub from: u32, pub to: u32, pub circle: Option<u32>, pub goal: Goal, pub expires: u64 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Bond { pub pair: [u32; 2], pub trust: u8, pub at: u64, #[serde(default)] pub retry_after: [u64;2] }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Kind { Proposed, Accepted, Declined, Left, Splintered, GoalChanged, Renewed }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Event { pub at: u64, pub actor: u32, pub other: u32, pub circle: Option<u32>, pub kind: Kind }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct State {
    #[serde(default)] pub(super) enabled: bool,
    pub(super) people: BTreeMap<u32, Person>, pub(super) circles: Vec<Circle>,
    pub(super) offers: Vec<Offer>, pub(super) bonds: Vec<Bond>,
    pub(super) events: VecDeque<Event>, pub(super) serial: u32,
    pub(super) reviews: u64, pub(super) scanned_orders: u64,
    #[serde(default)] pub(super) goal_checks: u64,
    #[serde(default)] pub(super) selections: u64,
    pub(super) accepted: u32, pub(super) exits: u32, pub(super) splits: u32,
}
impl State {
    pub fn valid(&self) -> bool {
        let mut members = BTreeSet::new();
        let mut circle_ids = BTreeSet::new();
        let mut pairs = BTreeSet::new();
        let mut recipients = BTreeSet::new();
        self.people.len() <= PEOPLE && self.circles.len() <= CIRCLES
            && self.bonds.len() <= PEOPLE * 2 && self.offers.len() <= PEOPLE && self.events.len() <= 32
            && self.people.values().all(|p| p.craft <= 100 && p.belonging <= 100 && p.autonomy <= 100
                && p.cursor < 256 && p.accepted_goal.is_none_or(Goal::valid)
                && (p.circle.is_some() || p.accepted_goal.is_none()) && p.departed.is_none_or(|id| id > 0 && id <= self.serial))
            && self.circles.iter().all(|g| g.id > 0 && g.id <= self.serial && circle_ids.insert(g.id)
                && g.name == format!("Work circle {}",g.id) && g.goal.valid()
                && g.parent.is_none_or(|id| id > 0 && id < g.id)
                && !g.members.is_empty() && g.members.len() <= MEMBERS
                && g.members.iter().all(|id| members.insert(*id)
                    && self.people.get(id).is_some_and(|p|p.circle == Some(g.id))))
            && self.people.iter().all(|(id,p)| p.circle.is_none() || members.contains(id))
            && self.bonds.iter().all(|b| b.pair[0] < b.pair[1] && pairs.insert(b.pair)
                && b.trust <= 100 && b.pair.iter().all(|id|self.people.contains_key(id)))
            && self.offers.iter().all(|o| o.from != o.to && recipients.insert(o.to) && o.goal.valid()
                && self.people.contains_key(&o.from) && self.people.get(&o.to).is_some_and(|p|p.circle.is_none())
                && o.circle.is_none_or(|id| self.circles.iter().any(|g|g.id == id && g.members.contains(&o.from))))
    }
    pub fn is_empty(&self) -> bool { !self.enabled && self.people.is_empty() }
    pub fn person(&mut self, seed:u32, id:u32) {
        if self.people.len() < PEOPLE { self.people.entry(id).or_insert_with(||Person::new(seed,id)); }
    }
    pub fn proposed_goal(&self, id:u32) -> Option<Goal> {
        let circle = self.people.get(&id)?.circle?;
        self.circles.iter().find(|g|g.id == circle).map(|g|g.goal)
    }
    pub fn goal(&self, id:u32) -> Option<Goal> {
        let goal = self.proposed_goal(id)?;
        (self.people.get(&id)?.accepted_goal == Some(goal)).then_some(goal)
    }
    pub(super) fn event(&mut self, now:u64, actor:u32, other:u32, circle:Option<u32>, kind:Kind) {
        if self.events.len() == 32 { self.events.pop_front(); }
        self.events.push_back(Event {at:now,actor,other,circle,kind});
    }
    pub fn contact(&mut self, a:u32, b:u32, now:u64) {
        if a == b || !self.people.contains_key(&a) || !self.people.contains_key(&b) { return; }
        let pair = [a.min(b),a.max(b)];
        if let Some(bond) = self.bonds.iter_mut().find(|b|b.pair == pair) {
            if now.saturating_sub(bond.at) >= PERIOD {
                bond.trust = bond.trust.saturating_add(20).min(100); bond.at = now;
            }
        } else {
            if self.bonds.len() == PEOPLE * 2 {
                let stale = self.bonds.iter().enumerate()
                    .filter(|(_,b)|now.saturating_sub(b.at) >= STALE && self.trust(b.pair[0],b.pair[1],now) < 20)
                    .min_by_key(|(_,b)|(self.trust(b.pair[0],b.pair[1],now),b.at,b.pair)).map(|(i,_)|i);
                let Some(index) = stale else { return; };
                self.bonds.remove(index);
            }
            self.bonds.push(Bond {pair,trust:20,at:now,retry_after:[0;2]});
        }
    }
    pub fn trust(&self,a:u32,b:u32,now:u64) -> u8 {
        self.bonds.iter().find(|r|r.pair == [a.min(b),a.max(b)])
            .map_or(0,|r|r.trust.saturating_sub((now.saturating_sub(r.at)/STALE).min(100) as u8))
    }
    pub fn refusal(&mut self,from:u32,to:u32,now:u64) {
        if let Some(b) = self.bonds.iter_mut().find(|b|b.pair == [from.min(to),from.max(to)]) {
            b.retry_after[usize::from(from == b.pair[1])] = now.saturating_add(STALE);
        }
    }
    pub fn can_offer(&self,from:u32,to:u32,now:u64) -> bool {
        self.bonds.iter().find(|b|b.pair == [from.min(to),from.max(to)])
            .is_some_and(|b|now >= b.retry_after[usize::from(from == b.pair[1])])
    }
    pub fn retain(&mut self, ids:&[u32], now:u64) {
        let removed:Vec<_> = self.people.keys().filter(|id|!ids.contains(id)).copied().collect();
        for id in removed { self.leave(id,now); self.people.remove(&id); }
        self.bonds.retain(|b|b.pair.iter().all(|id|self.people.contains_key(id)));
        self.offers.retain(|o|o.expires > now && self.people.contains_key(&o.from)
            && self.people.get(&o.to).is_some_and(|p|p.circle.is_none())
            && o.circle.is_none_or(|id|self.circles.iter().any(|g|g.id == id && g.members.contains(&o.from))));
    }
    pub fn leave(&mut self,id:u32,now:u64) {
        let Some(circle) = self.people.get(&id).and_then(|p|p.circle) else { return; };
        if let Some(p) = self.people.get_mut(&id) { p.circle = None; p.accepted_goal = None; p.departed = Some(circle); }
        for g in &mut self.circles { g.members.retain(|&member|member != id); }
        self.circles.retain(|g|!g.members.is_empty());
        self.offers.retain(|o|o.from != id && o.to != id);
        self.exits = self.exits.saturating_add(1);
        self.event(now,id,id,Some(circle),Kind::Left);
    }
    pub fn committed(&mut self,id:u32,at:[i32;3],now:u64) {
        if !self.goal(id).is_some_and(|g|g.contains(at)) { return; }
        let Some(circle) = self.people.get(&id).and_then(|p|p.circle) else { return; };
        if let Some(g) = self.circles.iter_mut().find(|g|g.id == circle && g.goal.contains(at)) {
            g.paid_blocks = g.paid_blocks.saturating_add(1); g.last_progress = now;
            if let Some(p) = self.people.get_mut(&id) { p.contribution = p.contribution.saturating_add(1); }
        }
    }
}
