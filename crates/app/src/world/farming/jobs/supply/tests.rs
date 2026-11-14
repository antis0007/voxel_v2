use super::*;

fn stack() -> ItemStack { ItemStack::new(Item::Block(B::Planks),8) }
fn transfer() -> Cargo { Cargo::Supply { source:[0,1,0], target:[16,1,0], project:10 } }

#[test]
fn supply_cargo_survives_reset_reload_and_partial_store() {
    let mut j = State { worker:Some(17), stockpile:Some([0,1,0]), carry:Some(stack()),
        cargo:Some(transfer()), task:Some(Task::Store(IVec3::new(16,1,0))), ..State::default() };
    j.reset_task();
    let encoded = ron::to_string(&j).unwrap();
    let mut restored:State = ron::from_str(&encoded).unwrap();
    assert!(restored.valid());
    assert_eq!(restored.cargo,Some(transfer()));
    assert_eq!(destination(&restored,IVec3::ZERO),IVec3::new(16,1,0));
    restored.carry.as_mut().unwrap().count=3;
    restored.reconcile_claims();
    assert!(restored.claims.is_empty());
    assert_eq!(restored.cargo,Some(transfer()));
    assert!(restored.valid());
}

#[test]
fn interrupted_builder_keeps_original_return_chest() {
    let j = State { stockpile:Some([0,1,0]), carry:Some(stack()), building_material:false,
        cargo:Some(Cargo::Build { source:[16,1,0] }), ..State::default() };
    assert_eq!(destination(&j,IVec3::ZERO),IVec3::new(16,1,0));
    assert!(j.cargo.unwrap().valid(j.carry,false));
}

#[test]
fn unused_metadata_is_absent_and_invalid_purposes_are_rejected() {
    assert!(!ron::to_string(&State::default()).unwrap().contains("cargo:"));
    assert!(!transfer().valid(None,false));
    assert!(!transfer().valid(Some(stack()),true));
    assert!(!Cargo::Supply { source:[0;3],target:[0;3],project:10 }.valid(Some(stack()),false));
    assert!(!Cargo::Build { source:[1<<28,0,0] }.valid(Some(stack()),true));
}

#[test]
fn ordinary_claims_cannot_adopt_supply_or_wrong_source_cargo() {
    let order = Order::new(IVec3::new(2,1,0),B::Planks);
    let mut j = State { worker:Some(17), stockpile:Some([0,1,0]), carry:Some(stack()),
        cargo:Some(transfer()), build:vec![order], building_material:true, ..State::default() };
    j.reconcile_claims(); assert!(j.claims.is_empty());
    j.cargo=Some(Cargo::Build { source:[16,1,0] });
    j.reconcile_claims(); assert!(j.claims.is_empty());
    j.cargo=Some(Cargo::Build { source:[0,1,0] });
    j.reconcile_claims(); assert!(j.owns(order));
    assert!(j.claims_valid());
}
