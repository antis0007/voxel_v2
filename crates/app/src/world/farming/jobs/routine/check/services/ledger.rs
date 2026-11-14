use super::*;
use crate::world::inventory::Item;
pub(super) fn stock(w:&World,at:IVec3)->u32 {
    let slots=w.block_data.chest(at);
    production::count(&slots,Item::Block(B::Log))*4+production::count(&slots,Item::Block(B::Planks))
        +production::count(&slots,Item::Block(B::Chest))*8
}
pub(super) fn timber(w:&World)->u32 {
    let mut total=timber_balance(w);
    if let Some(h)=frontier::hub_plan(&w.farming.jobs) {total+=stock(w,h.stockpile);}
    let base=w.farming.jobs.stockpile.map(IVec3::from).unwrap_or(IVec3::ZERO)-IVec3::Y;
    if let Ok(s)=site(w) {for order in s.projects.iter().flat_map(|p|&p.cells) {
        let d=order.cell()-base;
        if order.block()==B::Log&&known(w,order.cell())==Some(B::Log)
            &&!(d.x.abs()<=24&&d.z.abs()<=24&&(0..=10).contains(&d.y)){total+=4;}
    }}
    total
}
