use crate::ToolAssembly;
use engine_core::modifiers::{Effect,evaluate};
#[derive(serde::Deserialize)]
pub struct Modifier {pub id:String,pub display_name:String,pub resource:usize,pub units:u32,pub max_rank:usize,pub min_level:u8,pub effects:Vec<Effect>}
pub static ROWS:std::sync::LazyLock<Vec<Modifier>>=std::sync::LazyLock::new(|| {
    let rows:Vec<Modifier>=ron::from_str(include_str!("../../../assets/tools/modifiers.ron")).expect("tool modifiers");
    for (i,r) in rows.iter().enumerate() {assert!(r.max_rank>0 && r.units>0 && !rows[..i].iter().any(|p|p.id==r.id));for e in &r.effects {assert!(e.add.is_finite() && e.multiply.is_finite() && e.multiply>0.0);}}
    rows
});
pub fn stat(tool:&ToolAssembly,name:&str,base:f32)->f32 {
    evaluate(name,base,tool.modifiers.iter().filter_map(|id|ROWS.iter().find(|r|&r.id==id)).flat_map(|r|&r.effects))
}
pub fn validate_tool(tool:&ToolAssembly)->Result<(),String> {
    if tool.modifiers.len()>tool.modifier_slots().map_err(|e|e.to_string())? as usize {return Err("Modifier Slots Full".into())}
    for id in &tool.modifiers {
        let row=ROWS.iter().find(|r|&r.id==id).ok_or("Unknown Modifier")?;
        if tool.modifiers.iter().filter(|m|*m==id).count()>row.max_rank || tool.level<row.min_level {return Err("Modifier Limit".into())}
    }
    Ok(())
}
pub fn apply(tool:&mut ToolAssembly,id:&str)->Result<(),String> {
    let mut next=tool.clone();next.modifiers.push(id.to_owned());validate_tool(&next)?;
    // Added capacity starts empty; sharpening cannot leave durability above its new cap.
    next.durability_left=next.durability_left.min(next.stats().map_err(|e|e.to_string())?.durability);
    *tool=next;Ok(())
}
