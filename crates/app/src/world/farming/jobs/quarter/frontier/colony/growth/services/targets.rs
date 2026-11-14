use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Target { pub original: u32, pub managed: Option<u32> }

impl Target {
    fn new(value: u32) -> Self {Self {original:value, managed:Some(value)}}
    fn valid(&self, max: u32) -> bool {
        (1..=max).contains(&self.original)
            && self.managed.is_none_or(|n|(self.original..=max).contains(&n))
    }
    fn set(&mut self, current: &mut u32, desired: u32) {
        if self.managed!=Some(*current) {self.managed=None;return;}
        *current=desired.max(self.original);self.managed=Some(*current);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillTarget {
    pub index: usize, pub recipe: String, pub worker: u32,
    pub station: [i32;3], pub once: bool, pub target: Target,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Targets { pub woodlot: Target, pub bills: Vec<BillTarget> }

impl Targets {
    pub(super) fn capture(prod: &production::State) -> Result<Self,String> {
        let woodlot=Target::new(production::woodlot_target(prod).ok_or("Woodlot Unavailable")?);
        let bills=prod.bills.iter().enumerate().filter(|(_,b)|matches!(b.recipe.as_str(),"Oak planks"|"Chest"))
            .map(|(index,b)|BillTarget {index,recipe:b.recipe.clone(),worker:b.worker,
                station:b.station,once:b.once,target:Target::new(b.target)}).collect();
        let targets=Self {woodlot,bills};
        if !targets.valid() {return Err("Service Production Bills Required".into());}
        Ok(targets)
    }
    pub(super) fn valid(&self) -> bool {
        self.woodlot.valid(256) && self.bills.len()<=64
            && self.bills.iter().any(|b|b.recipe=="Oak planks")
            && self.bills.iter().any(|b|b.recipe=="Chest")
            && self.bills.windows(2).all(|b|b[0].index<b[1].index)
            && self.bills.iter().all(|b|b.index<64 && b.target.valid(4096)
                && matches!((b.recipe.as_str(),b.once),("Oak planks",false)|("Chest",true))
                && b.station.iter().all(|n|n.unsigned_abs()<1<<28))
    }
    pub(super) fn apply(&mut self, prod: &mut production::State, logs: u32, planks: u32, chests: u32) -> Result<(),String> {
        let mut woodlot=production::woodlot_target(prod).ok_or("Woodlot Unavailable")?;
        self.woodlot.set(&mut woodlot,logs.clamp(1,256));
        production::set_woodlot_target(prod,woodlot)?;
        for saved in &mut self.bills {
            let Some(bill)=prod.bills.get_mut(saved.index).filter(|b|b.recipe==saved.recipe
                && b.worker==saved.worker && b.station==saved.station && b.once==saved.once) else {
                saved.target.managed=None;continue;
            };
            let desired=if saved.recipe=="Chest" {chests}else{planks};
            saved.target.set(&mut bill.target,desired.clamp(1,4096));
        }
        Ok(())
    }
    pub(super) fn restore(&mut self, prod: &mut production::State) -> Result<(),String> {
        self.apply(prod,1,1,1)
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    fn service_target_restoration_preserves_manual_changes() {
        let mut owned=Target::new(48);let mut current=48;
        owned.set(&mut current,230);assert_eq!(current,230);
        owned.set(&mut current,1);assert_eq!(current,48);
        owned.set(&mut current,192);current=77;
        owned.set(&mut current,256);assert_eq!(current,77);assert!(owned.managed.is_none());
        owned.set(&mut current,1);assert_eq!(current,77);
    }
    #[test]
    fn service_saved_targets_reject_out_of_bound_ownership() {
        assert!(!Target{original:0,managed:Some(1)}.valid(256));
        assert!(!Target{original:48,managed:Some(47)}.valid(256));
        assert!(!Target{original:48,managed:Some(257)}.valid(256));
        assert!(Target{original:48,managed:None}.valid(256));
    }
}
