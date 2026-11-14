//! Opt-in inspection and fresh-click selection, without deferred actions.
use crate::world::{BlockId, PickHit, Readiness};

pub fn enabled(opt_in: bool, supported: bool, hud: bool, click: bool, other_action: bool) -> bool {
    opt_in && supported && (hud || click) && !other_action
}

#[derive(Debug, PartialEq)]
pub enum ClickDecision { Idle, Pick(BlockId), Miss, Pending, Failed }

/// Consumes one click against the current ray result.
pub fn consume_click(click: &mut bool, result: &Readiness<Option<PickHit>>) -> ClickDecision {
    if !std::mem::take(click) { return ClickDecision::Idle; }
    match result {
        Readiness::Ready(Some(hit)) => ClickDecision::Pick(hit.block),
        Readiness::Ready(None) => ClickDecision::Miss,
        Readiness::Pending => ClickDecision::Pending,
        Readiness::Failed(_) => ClickDecision::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ready(block: BlockId) -> Readiness<Option<PickHit>> {
        Readiness::Ready(Some(PickHit { cell: glam::IVec3::ZERO,
            previous: glam::IVec3::Y, block }))
    }
    #[test]
    fn opt_in_click_or_hud_excludes_other_actions() {
        for bits in 0..32 {
            let opt=bits&1!=0; let supported=bits&2!=0; let hud=bits&4!=0;
            let click=bits&8!=0; let other=bits&16!=0;
            assert_eq!(enabled(opt,supported,hud,click,other),
                opt && supported && (hud || click) && !other);
        }
    }
    #[test]
    fn readiness_never_replays_a_consumed_click() {
        let cases=[(ready(BlockId::Stone),ClickDecision::Pick(BlockId::Stone)),
            (Readiness::Ready(None),ClickDecision::Miss),
            (Readiness::Pending,ClickDecision::Pending),
            (Readiness::Failed("unavailable".into()),ClickDecision::Failed)];
        for (result,expected) in cases {
            let mut click=true;
            assert_eq!(consume_click(&mut click,&result),expected);
            assert!(!click);
            assert_eq!(consume_click(&mut click,&ready(BlockId::Dirt)),ClickDecision::Idle);
            click=true;
            assert_eq!(consume_click(&mut click,&ready(BlockId::Dirt)),ClickDecision::Pick(BlockId::Dirt));
        }
    }
}
