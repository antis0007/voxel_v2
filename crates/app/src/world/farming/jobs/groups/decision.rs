use super::model::*;

impl State {
    /// One resident chooses on their existing scheduled work turn.
    pub fn decide(&mut self,id:u32,now:u64,opportunity:Option<Goal>,goal_live:bool) {
        let Some(p) = self.people.get(&id) else { return; };
        if now < p.due { return; }
        if let Some(p) = self.people.get_mut(&id) { p.due = now.saturating_add(PERIOD); }
        self.reviews = self.reviews.saturating_add(1);
        self.offers.retain(|o|o.expires > now);
        if let Some(index) = self.offers.iter().position(|o|o.to == id) {
            let offer = self.offers.remove(index);
            let trust = self.trust(id,offer.from,now);
            let willing = self.people.get(&id).is_some_and(|p|p.circle.is_none()
                && u16::from(p.belonging) + u16::from(trust) >= u16::from(p.autonomy) + 40 && p.craft >= 30);
            // Acceptance checks the recipient's own observed work opportunity.
            if willing && opportunity == Some(offer.goal) && self.accept(offer.clone(),now) {
                return;
            }
            self.refusal(offer.from,id,now);
            self.event(now,id,offer.from,offer.circle,Kind::Declined);
        }
        let Some(p) = self.people.get(&id).cloned() else { return; };
        let mut target = opportunity;
        if let Some(circle) = p.circle {
            let Some(g) = self.circles.iter().find(|g|g.id == circle).cloned() else { return; };
            if p.accepted_goal != Some(g.goal) {
                if opportunity == Some(g.goal) && u16::from(p.belonging)*2 >= u16::from(p.autonomy) {
                    if let Some(p) = self.people.get_mut(&id) { p.accepted_goal = Some(g.goal); }
                    self.event(now,id,id,Some(circle),Kind::Renewed);
                } else if opportunity.is_some() && (p.autonomy > p.belonging
                    || !goal_live && now.saturating_sub(g.last_progress) >= STALE) {
                    self.leave(id,now);
                }
                return;
            }
            let stalled = now.saturating_sub(g.last_progress) >= STALE;
            if stalled && opportunity.is_some_and(|goal|goal != g.goal)
                && p.autonomy > p.belonging.saturating_add(15) {
                self.leave(id,now);
            } else if !goal_live && opportunity.is_some() && stalled {
                if let Some(goal) = opportunity {
                    if let Some(g) = self.circles.iter_mut().find(|g|g.id == circle) {
                        g.goal = goal; g.last_progress = now;
                    }
                    if let Some(p) = self.people.get_mut(&id) { p.accepted_goal = Some(goal); }
                    self.event(now,id,id,Some(circle),Kind::GoalChanged);
                }
                return;
            } else {
                target = Some(g.goal);
            }
        }
        let Some(goal) = target else { return; };
        let Some(p) = self.people.get(&id) else { return; };
        if p.craft < 30 || u16::from(p.belonging)*2 < u16::from(p.autonomy)
            || self.offers.iter().any(|o|o.from == id) { return; }
        let circle = p.circle;
        if circle.is_some_and(|id|self.circles.iter().any(|g|g.id == id && g.members.len() >= MEMBERS))
            || circle.is_none() && self.circles.len() >= CIRCLES { return; }
        let candidate = self.people.keys().copied().filter(|&other|other != id
            && self.people.get(&other).is_some_and(|p|p.circle.is_none())
            && !self.offers.iter().any(|o|o.to == other)
            && self.trust(id,other,now) >= 40 && self.can_offer(id,other,now))
            .max_by_key(|&other|(self.trust(id,other,now),std::cmp::Reverse(other)));
        let Some(to) = candidate else { return; };
        self.offers.push(Offer {from:id,to,circle,goal,expires:now.saturating_add(4*PERIOD)});
        self.event(now,id,to,circle,Kind::Proposed);
    }

    fn accept(&mut self,offer:Offer,now:u64) -> bool {
        let Some(from) = self.people.get(&offer.from) else { return false; };
        if from.circle != offer.circle { return false; }
        let circle = if let Some(id) = offer.circle {
            let Some(g) = self.circles.iter_mut().find(|g|g.id == id && g.goal == offer.goal
                && g.members.len() < MEMBERS) else { return false; };
            g.members.push(offer.to); id
        } else {
            if self.circles.len() >= CIRCLES { return false; }
            let Some(id) = self.serial.checked_add(1) else { return false; };
            let parent = from.departed;
            self.serial = id;
            self.circles.push(Circle {id,name:format!("Work circle {id}"),members:vec![offer.from,offer.to],
                goal:offer.goal,parent,formed:now,last_progress:now,paid_blocks:0});
            if let Some(p) = self.people.get_mut(&offer.from) { p.circle = Some(id); p.accepted_goal = Some(offer.goal); }
            if parent.is_some() {
                self.splits = self.splits.saturating_add(1);
                self.event(now,offer.from,offer.to,Some(id),Kind::Splintered);
            }
            id
        };
        if let Some(p) = self.people.get_mut(&offer.to) { p.circle = Some(circle); p.accepted_goal = Some(offer.goal); }
        self.offers.retain(|o|o.to != offer.to && o.to != offer.from
            && o.from != offer.to && o.from != offer.from);
        self.accepted = self.accepted.saturating_add(1);
        self.event(now,offer.to,offer.from,Some(circle),Kind::Accepted);
        true
    }
}
