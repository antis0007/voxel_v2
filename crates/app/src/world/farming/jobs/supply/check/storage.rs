//! Synthetic slots exercise engine storage; no simulation runs before restoration.
use super::*;

pub(super) fn run(w: &mut World, at: IVec3) -> anyhow::Result<serde_json::Value> {
    let key=block_data::key(at);
    let original=w.block_data.entries.get(&key).cloned();
    let revision=w.block_data.storage_revision;
    let result=probe(w,at);
    if let Some(entry)=original { w.block_data.entries.insert(key,entry); }
    else { w.block_data.entries.remove(&key); }
    w.block_data.storage_revision=revision;
    result
}

fn probe(w: &mut World, at: IVec3) -> anyhow::Result<serde_json::Value> {
    ensure!(known(w,at).is_some_and(|b|b.chest_tier().is_some()),"Storage fixture needs a physical chest");
    let item=Item::Block(B::Planks);
    let limit=w.inventory.stack_limit(item);
    ensure!(limit>0,"Storage fixture item has zero stack limit");
    let len=w.block_data.chest(at).len();
    ensure!(matches!(len,27|36|45|54),"Storage fixture chest size is invalid");
    let mut slots=vec![Some(ItemStack::new(item,limit));len];
    slots[0]=(limit>1).then_some(ItemStack::new(item,limit-1));
    w.block_data.set_chest(at,slots);
    let before=production::count(&w.block_data.chest(at),item);
    let mut cargo=ItemStack::new(item,2);
    block_data::store_hauled(w,at,&mut cargo);
    let after_partial=production::count(&w.block_data.chest(at),item);
    ensure!(cargo.count==1 && after_partial==before+1 && after_partial+cargo.count==before+2,
        "Partial engine store lost or duplicated fixture cargo");
    let full=w.block_data.chest(at);
    block_data::store_hauled(w,at,&mut cargo);
    ensure!(cargo.count==1 && w.block_data.chest(at)==full,
        "Full engine store changed fixture cargo or stock");
    let mut slots=full;
    slots[0]=(limit>1).then_some(ItemStack::new(item,limit-1));
    w.block_data.set_chest(at,slots);
    let before_retry=production::count(&w.block_data.chest(at),item);
    block_data::store_hauled(w,at,&mut cargo);
    ensure!(cargo.count==0 && production::count(&w.block_data.chest(at),item)==before_retry+1,
        "Engine storage retry failed to deliver remaining fixture cargo");
    Ok(serde_json::json!({"synthetic_test_setup":true,"engine_store_hauled":true,
        "partial_store":true,"full_store":true,"retry_after_space":true,"fixture_cargo_units":2,
        "project_funding_used":false}))
}
