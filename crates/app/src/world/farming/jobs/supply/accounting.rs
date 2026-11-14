pub(super) fn remaining(required: u32, stock: u32, in_transit: u32) -> u32 {
    required.saturating_sub(stock).saturating_sub(in_transit)
}

pub(super) fn unique<T: PartialEq>(items: impl IntoIterator<Item=T>) -> Vec<T> {
    let mut result = Vec::new();
    for item in items { if !result.contains(&item) { result.push(item); } }
    result
}

pub(super) fn accepts(source: Option<[i32; 3]>, transfer: bool, expected: Option<[i32; 3]>, remote: bool) -> bool {
    // Unmarked legacy cargo predates the remote project.
    !transfer && source.map_or(!remote, |from|expected == Some(from))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn real_goods_are_credited_once() {
        assert_eq!(remaining(20,5,8),7);
        assert_eq!(remaining(20,8,5),7);
        assert_eq!(remaining(20,13,0),7);
    }
    #[test] fn completed_or_overfunded_projects_do_not_reserve() {
        assert_eq!(remaining(0,0,8),0);
        assert_eq!(remaining(7,12,0),0);
        assert_eq!(remaining(7,0,12),0);
    }
    #[test] fn pickup_partial_store_and_reload_keep_the_same_liability() {
        let required=64;
        assert_eq!(remaining(required,0,0),64);
        assert_eq!(remaining(required,0,8),56);
        assert_eq!(remaining(required,3,5),56);
        assert_eq!(remaining(required,8,0),56);
        assert_eq!(remaining(required-4,8-4,0),56);
    }
    #[test] fn primary_or_legacy_cargo_never_pays_remote_orders() {
        let primary = [0,1,0]; let hub = [16,1,0];
        assert!(!accepts(Some(primary),false,Some(hub),true));
        assert!(!accepts(None,false,Some(hub),true));
        assert!(accepts(Some(hub),false,Some(hub),true));
        assert!(accepts(None,false,Some(primary),false));
        assert!(!accepts(Some(hub),false,Some(primary),false));
    }
    #[test] fn transfer_cargo_has_no_construction_claims() {
        assert!(!accepts(Some([16,1,0]),true,Some([16,1,0]),true));
        assert!(!accepts(None,true,Some([0,1,0]),false));
    }
    #[test] fn interrupted_builder_return_does_not_request_duplicate_delivery() {
        let outstanding = 40u32;
        let building_cargo = 8;
        let hub = 20;
        let transfer = 8;
        assert_eq!(remaining(outstanding-building_cargo,hub,transfer),4);
        assert_eq!(remaining(outstanding,hub+building_cargo,transfer),4);
    }
    #[test] fn satisfied_and_full_hubs_check_each_material_only_once() {
        let orders = (0..548).map(|i|i%4);
        for all_satisfied in [false,true] {
            let mut checked = 0;
            let wanted:Vec<_> = unique(orders.clone()).into_iter().filter(|_| {
                checked += 1;
                if all_satisfied { remaining(32,32,0) > 0 } else { false }
            }).collect();
            assert!(wanted.is_empty());
            assert_eq!(checked,4);
        }
    }
}
