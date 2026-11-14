//! Authoritative part construction and persistent tool ownership.
use super::{World, BlockId, inventory::{Item, ItemStack, SurvivalInventory, resources}};
use engine_tools::{ToolAssembly, PARTS, MATERIALS};
use std::sync::LazyLock;

pub const MAX_TOOLS: usize = 4096;
/// Tool attack ratings share the world's 100-health combat scale.
pub fn attack_damage(tool:&ToolAssembly)->f32 {
    if tool.is_broken(){1.0}else{tool.stats().map_or(1.0,|s|s.attack.max(0.0)*4.0)}
}
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Part(u8,u8), Mold(u8), Assemble(u16,[u8;8]), Repair(u16), Modify(u16,u8),
    Replace(u16,u8,u8),
}
#[derive(serde::Deserialize)]
pub struct Supply { pub material: String, pub block: Option<BlockId>, pub resource: Option<usize>, pub cast: bool }
pub static SUPPLIES: LazyLock<Vec<Supply>> = LazyLock::new(|| {
    let mut rows:Vec<Supply>=ron::from_str(include_str!("../../../../assets/tools/workshop.ron")).expect("tool workshop supply rows");
    for m in MATERIALS.iter() {
        if rows.iter().any(|r|r.material==m.id.0) {continue;}
        if let Some(name)=&m.block {if let Ok(block)=ron::from_str::<BlockId>(name) {
            rows.push(Supply{material:m.id.0.clone(),block:Some(block),resource:None,cast:block.material().sim.melts_into.is_some()});
        }}
    }
    for r in &rows {assert!(engine_tools::material(&r.material).is_some());assert!(r.resource.is_none_or(|i|i<super::factory::RESOURCE_COUNT));}
    rows
});
pub fn supply(material: usize) -> Option<&'static Supply> {
    let id=&MATERIALS.get(material)?.id.0;
    SUPPLIES.iter().find(|r| &r.material==id)
}
/// The same physical block-volume conversion used by payment, for cost displays.
pub fn material_cost(material:usize,units:u32,voxel_m:f32)->Option<(Item,f64)> {
    let row=supply(material)?;
    if let Some(resource)=row.resource {Some((Item::Resource(resource as u8),f64::from(units)/64.0))}
    else {row.block.map(|b|(Item::Block(b),(f64::from(units)/(64.0*f64::from(voxel_m).powi(3))).ceil()))}
}
pub fn mold_cost(voxel_m:f32)->u32 {(1.0/voxel_m.powi(3)).round() as u32}
pub fn pay(inv:&mut SurvivalInventory, material:usize, units:u32)->bool {
    let Some(row)=supply(material) else {return false};
    if let Some(resource)=row.resource { resources::exchange(inv,&[(resource,units)],&[]) }
    else if let Some((item,count))=material_cost(material,units,inv.voxel_m) { inv.remove_item(item,count as u32) } else {false}
}
fn insert(inv:&mut SurvivalInventory,item:Item)->bool { inv.try_insert(ItemStack::new(item,1)).1==0 }
pub fn action(w:&mut World,action:Action) {
    let mut inv=w.inventory.clone();
    let result:Result<(),String>=(|| {
        match action {
            Action::Part(kind,material) => {
                let part=PARTS.part_kinds.get(kind as usize).ok_or("Unknown Part")?;
                let row=supply(material as usize).ok_or("Material Unavailable")?;
                if row.cast {return Err("Casting Required".into())}
                if !pay(&mut inv,material as usize,u32::from(part.material_cost)*64) {return Err("Missing Material".into())}
                if !insert(&mut inv,Item::Part{kind,material}) {return Err("Inventory Full".into())}
            }
            Action::Mold(kind) => {
                if kind != 255 && PARTS.part_kinds.get(kind as usize).is_none() {return Err("Unknown Part".into())}
                if !inv.remove_item(Item::Block(BlockId::Clay),mold_cost(inv.voxel_m)) {return Err("Clay Required".into())}
                if !insert(&mut inv,Item::Cast(kind)) {return Err("Inventory Full".into())}
            }
            Action::Assemble(schema,materials) => {
                if w.adventure.tool_bank.len()>=MAX_TOOLS {return Err("Tool Storage Full".into())}
                let def=PARTS.schemas.get(schema as usize).ok_or("Unknown Tool")?;
                if def.parts.len()>materials.len() {return Err("Too Many Parts".into())}
                let mut selected=Vec::new();
                for (part,&material) in def.parts.iter().zip(&materials) {
                    let kind=PARTS.part_kinds.iter().position(|p|p.id==part.kind).ok_or("Unknown Part")? as u8;
                    let mat=MATERIALS.get(material as usize).ok_or("Unknown Material")?;
                    if !inv.remove_item(Item::Part{kind,material},1) {return Err("Missing Parts".into())}
                    selected.push(mat.id.clone());
                }
                let tool=ToolAssembly::new(&def.id,&selected).map_err(|e|e.to_string())?;
                let id=w.adventure.tool_bank.len() as u16;
                if !insert(&mut inv,Item::Assembled{id,schema}) {return Err("Inventory Full".into())}
                w.adventure.tool_bank.push(tool);
            }
            Action::Replace(id,index,material) => {
                if !owns(&inv,id) {return Err("Tool Required".into())}
                let mut next=w.adventure.tool_bank.get(id as usize).ok_or("Unknown Tool")?.clone();
                let (part,old)=next.parts.get(index as usize).ok_or("Unknown Part")?.clone();
                let row=MATERIALS.get(material as usize).ok_or("Unknown Material")?;
                if old==row.id {return Err("Material Unchanged".into())}
                let kind=PARTS.part_kinds.iter().position(|p|p.id==part).ok_or("Unknown Part")? as u8;
                next.replace_part(index as usize,&row.id).map_err(|e|e.to_string())?;
                if !inv.remove_item(Item::Part{kind,material},1) {return Err("Part Required".into())}
                // The removed fitting is spent; replacement cannot duplicate material.
                w.adventure.tool_bank[id as usize]=next;
            }
            Action::Repair(id) => {
                if !owns(&inv,id) {return Err("Tool Required".into())}
                let tool=w.adventure.tool_bank.get(id as usize).ok_or("Unknown Tool")?;
                let head=tool.head_material().map_err(|e|e.to_string())?;
                let material=MATERIALS.iter().position(|m|m.id==head.id).ok_or("Unknown Material")?;
                let mut next=tool.clone();
                if next.repair(&head.id,1).map_err(|e|e.to_string())?==0 {return Err("Already Repaired".into())}
                if !pay(&mut inv,material,64) {return Err("Head Material Required".into())}
                w.adventure.tool_bank[id as usize]=next;
            }
            Action::Modify(id,index) => {
                if !owns(&inv,id) {return Err("Tool Required".into())}
                let row=engine_tools::modifiers::ROWS.get(index as usize).ok_or("Unknown Modifier")?;
                let mut next=w.adventure.tool_bank.get(id as usize).ok_or("Unknown Tool")?.clone();
                engine_tools::modifiers::apply(&mut next,&row.id).map_err(|e|e.to_string())?;
                if !resources::exchange(&mut inv,&[(row.resource,row.units)],&[]) {return Err("Missing Material".into())}
                w.adventure.tool_bank[id as usize]=next;
            }
        }
        Ok(())
    })();
    match result {Ok(())=>{w.inventory=inv;w.sync_from_inventory();},Err(e)=>w.expedition.say(&e)}
}
pub fn owns(inv:&SurvivalInventory,id:u16)->bool {
    inv.hotbar.iter().chain(&inv.pack).chain(std::iter::once(&inv.offhand)).chain(std::iter::once(&inv.cursor)).flatten().any(|s|matches!(s.item,Item::Assembled{id:i,..} if i==id))
}
pub fn encode(bank:&[ToolAssembly],bytes:&mut Vec<u8>)->anyhow::Result<()> {
    bytes.extend_from_slice(&(bank.len() as u32).to_le_bytes());
    for tool in bank {let mut data=Vec::new();tool.encode(&mut data)?;bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());for b in data {bytes.extend_from_slice(&u32::from(b).to_le_bytes());}}
    Ok(())
}
pub fn decode(read:&mut impl FnMut()->anyhow::Result<u32>)->anyhow::Result<Vec<ToolAssembly>> {
    let count=read()? as usize;anyhow::ensure!(count<=MAX_TOOLS,"Too many tools");
    let mut out=Vec::with_capacity(count);
    for _ in 0..count {
        let len=read()? as usize;anyhow::ensure!(len<=16384,"Tool record too large");
        let mut data=Vec::with_capacity(len);for _ in 0..len {let b=read()?;anyhow::ensure!(b<=255,"Invalid tool byte");data.push(b as u8);}
        let (mut tool,used)=ToolAssembly::decode(&data)?;anyhow::ensure!(used==len,"Trailing tool data");
        tool.reconcile_durability()?;tool.traits()?;
        anyhow::ensure!(tool.level == engine_tools::progression::level_for_xp(&PARTS.progression,tool.xp),"Invalid tool level");

        engine_tools::modifiers::validate_tool(&tool).map_err(anyhow::Error::msg)?;
        out.push(tool);
    }
    Ok(out)
}

static PART_NAMES: LazyLock<Vec<Vec<String>>> = LazyLock::new(|| PARTS.part_kinds.iter().map(|p|MATERIALS.iter().map(|m|format!("{} {}",m.display_name,p.display_name)).collect()).collect());
static MOLD_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| PARTS.part_kinds.iter().map(|p|format!("{} Mold",p.display_name)).collect());
pub fn part_name(kind:u8,material:u8)->&'static str {PART_NAMES.get(kind as usize).and_then(|v|v.get(material as usize)).map_or("Part",String::as_str)}
pub fn mold_name(kind:u8)->&'static str {if kind==255 {"Ingot Mold"} else {MOLD_NAMES.get(kind as usize).map_or("Mold",String::as_str)}}
