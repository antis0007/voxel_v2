use super::*;
use crate::world::inventory::{ItemStack, all_recipes};

pub(super) fn requirements(p: &Project) -> Result<Vec<(Item,u32)>,String> {
    let mut needs:Vec<(Item,u32)>=Vec::new();
    for order in &p.cells {
        let item=order.material().ok_or("Unsupported Construction Material")?;
        if let Some((_,count))=needs.iter_mut().find(|(old,_)|*old==item) {*count+=1;}
        else {needs.push((item,1));}
    }
    if needs.is_empty() || p.cells.len()>1024 {return Err("Service Material Limit".into());}
    Ok(needs)
}

pub(super) fn paid(needs: &[(Item,u32)], slots: &[Option<ItemStack>]) -> bool {
    needs.iter().all(|(item,count)|production::count(slots,*item)>=*count)
}

pub(super) fn status(needs: &[(Item,u32)], slots: &[Option<ItemStack>]) -> String {
    let missing=needs.iter().map(|(item,count)|count.saturating_sub(production::count(slots,*item))).sum::<u32>();
    format!("Service Materials: {missing} Blocks Missing")
}

fn scheduled(needs: &[(Item,u32)], have: impl Fn(Item)->u32) -> Result<(u32,u32,u32),String> {
    let logs=Item::Block(B::Log);let planks=Item::Block(B::Planks);let chest=Item::Block(B::Chest);
    if needs.iter().any(|(item,n)|!matches!(*item,Item::Block(B::Log|B::Planks|B::Chest)) && have(*item)<*n) {
        return Err("Material Recipe Unavailable".into());
    }
    let need=|item|needs.iter().find(|(i,_)|*i==item).map_or(0,|(_,n)|*n);
    let oak=all_recipes().iter().find(|r|r.name=="Oak planks").ok_or("Plank Recipe Missing")?;
    let furniture=all_recipes().iter().find(|r|r.name=="Chest").ok_or("Chest Recipe Missing")?;
    if oak.output.item!=planks || oak.output.count==0 || oak.ingredients.len()!=1 || oak.ingredients[0].0!=logs
        || furniture.output.item!=chest || furniture.output.count==0
        || furniture.ingredients.len()!=1 || furniture.ingredients[0].0!=planks {
        return Err("Service Recipe Inputs Unsupported".into());
    }
    let missing_chests=need(chest).saturating_sub(have(chest));
    let planks_target=need(planks).checked_add(missing_chests.div_ceil(furniture.output.count)
        .checked_mul(furniture.ingredients[0].1).ok_or("Service Recipe Limit")?).ok_or("Service Recipe Limit")?;
    let timber=planks_target.saturating_sub(have(planks)).div_ceil(oak.output.count)
        .checked_mul(oak.ingredients[0].1).ok_or("Service Recipe Limit")?;
    let logs_target=need(logs).checked_add(timber).ok_or("Service Recipe Limit")?;
    if planks_target>4096 || logs_target>4096 {return Err("Service Recipe Limit".into());}
    Ok((logs_target.min(256),planks_target,missing_chests))
}

pub(super) fn request(w: &mut World, service: &mut Services, primary: [i32;3], needs: &[(Item,u32)]) -> Result<(),String> {
    let slots=w.block_data.chest(primary.into());
    let have=|item|production::count(&slots,item).saturating_add(jobs::crew::cargo_item(&w.farming.jobs,item));
    let (logs,planks,chests)=scheduled(needs,have)?;
    let prod=w.farming.jobs.production.as_mut().ok_or("Production Unavailable")?;
    let chest_target=prod.made.get("Chest").copied().unwrap_or(0).checked_add(chests)
        .filter(|n|*n<=4096).ok_or("Chest Production Limit")?;
    service.targets.apply(prod,logs,planks,chest_target)
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    fn service_funding_accounts_for_every_order_without_mutating_stock() {
        let p=Project{name:"Paid".into(),cells:vec![jobs::construction::Order::new(IVec3::ZERO,B::Stone),
            jobs::construction::Order::new(IVec3::X,B::Planks)],food_reserve:0,meals_eaten:0,home:None};
        let needs=requirements(&p).unwrap();assert_eq!(needs.len(),2);
        let slots=vec![Some(ItemStack::new(Item::Block(B::Planks),1))];let before=slots.clone();
        assert!(!paid(&needs,&slots));assert!(scheduled(&needs,|i|production::count(&slots,i)).is_err());
        assert_eq!(slots,before);
    }
    #[test]
    fn service_funding_requests_physical_logs_for_house_and_recipes() {
        let needs=vec![(Item::Block(B::Log),122),(Item::Block(B::Planks),425),(Item::Block(B::Chest),1)];
        assert_eq!(scheduled(&needs,|_|0).unwrap(),(231,433,1));
        assert_eq!(scheduled(&needs,|item|if item==Item::Block(B::Planks){433}else{0}).unwrap(),(122,433,1));
        assert_eq!(scheduled(&needs,|item|if item==Item::Block(B::Chest){1}else{0}).unwrap(),(229,425,0));
        assert!(!paid(&needs,&[]));
    }
}
