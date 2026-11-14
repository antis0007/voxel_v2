//! Ordered player requests. No request carries authoritative resources or damage.
use super::*;
use crate::{creative::{self,CreativeState,ToolKind},egui_ui::UiAction,world::inventory::{Item,ItemStack,SlotAddr}};
#[derive(Clone,Serialize,Deserialize)]
pub enum Command {
    Scanner(IVec3, world::scanner::commands::Action),
    Inventory(inventory::Command), Factory(world::factory::Action), Chip(world::factory::chips::Action),
    Radio(world::orbital::radio::Action), Receiver(world::orbital::receiver::Action),
    Request(glam::IVec2,world::orbital::Channel), Drop(usize), Lamp,
    Give(Item,u32), Tools(Tools),
    SelectCreativeItem(Item,u32,usize),
    SelectCreativeRow(Vec<Item>),
    /// Leave the death screen; the caller moves the body, which `apply` cannot.
    Respawn,
    /// Reverse the sender's newest creative edit step, or reapply the newest undone one.
    EditHistory(world::edit_history::Direction),
    Resident(u32, world::travel::ResidentAction),
    Study(world::progression::study::Lesson),
    StudyMaterials(world::progression::study::Lesson, bool),
}
#[derive(Clone,Serialize,Deserialize)]
pub struct Request {pub sequence:u64,pub command:Command}

/// Transport retries keep their original sequence and must never debit twice.
pub fn admit_sequence(last: u64, incoming: u64) -> Result<bool> {
    if incoming <= last { return Ok(false); }
    ensure!(last.checked_add(1) == Some(incoming), "command gap");
    Ok(true)
}
#[derive(Clone,Serialize,Deserialize)]
pub struct Tools {
    pub tool:u8,pub brush:u8,pub radius:i32,pub area_radius:i32,pub thickness:i32,pub square:bool,
    pub explosion:i32,pub limit:usize,pub hand:f32,pub metres:bool,
}
impl Default for Tools {fn default()->Self{Self::capture(&CreativeState::default())}}
impl Tools {
    pub fn capture(c:&CreativeState)->Self {Self{tool:c.tool as u8,brush:c.brush_shape as u8,radius:c.brush_radius,
        area_radius:c.area_tool.radius,thickness:c.area_tool.thickness,square:c.area_tool.shape==creative::AreaFootprintShape::Square,
        explosion:c.explosion_radius,limit:c.wand_max_blocks,hand:c.hand_size_m,metres:c.size_in_metres}}
    pub fn validate(&self)->Result<()> {ensure!(self.tool<6&&self.brush<2&&(0..=16).contains(&self.radius)
        &&(0..=16).contains(&self.area_radius)&&(1..=16).contains(&self.thickness)&&(0..=16).contains(&self.explosion)
        &&self.limit<=4096&&self.hand.is_finite()&&(0.25..=8.0).contains(&self.hand),"invalid tool settings");Ok(())}
    pub fn creative(&self,slot:usize,block:BlockId,scale:VoxelScalePreset)->CreativeState {
        let mut c=CreativeState::default();c.tool=ToolKind::ALL[self.tool as usize];
        c.brush_shape=if self.brush==0 {creative::BrushShape::Cube}else{creative::BrushShape::Sphere};
        c.brush_radius=self.radius;c.area_tool.radius=self.area_radius;c.area_tool.thickness=self.thickness;
        c.area_tool.shape=if self.square {creative::AreaFootprintShape::Square}else{creative::AreaFootprintShape::Circle};
        c.explosion_radius=self.explosion;c.wand_max_blocks=self.limit;c.hand_size_m=self.hand;
        c.size_in_metres=self.metres;c.active_voxel_m=scale.meters();c.selected_slot=slot;c.placement_distance=6.0;
        c.assign_hotbar_slot(slot,block);c
    }
}
pub fn from_ui(action:UiAction)->Result<Command,UiAction> {Ok(match action {
    UiAction::Factory(a)=>Command::Factory(a),UiAction::Chip(a)=>Command::Chip(a),
    UiAction::OrbitalRadio(a)=>Command::Radio(a),UiAction::OrbitalReceiver(a)=>Command::Receiver(a),
    UiAction::RequestOrbital(p,c)=>Command::Request(p,c),UiAction::DropInventory(i)=>Command::Drop(i),
    UiAction::GiveItem(i,n)=>Command::Give(i,n),
    UiAction::SelectCreativeItem(i,n,s)=>Command::SelectCreativeItem(i,n,s),
    UiAction::SelectCreativeRow(items)=>Command::SelectCreativeRow(items),
    UiAction::Study(lesson)=>Command::Study(lesson),
    UiAction::StudyMaterials(lesson,research)=>Command::StudyMaterials(lesson,research),
    UiAction::Respawn=>Command::Respawn,other=>return Err(other),
})}
pub fn apply(command:Command,w:&mut World,p:&Player,day:&DayNightCycle,actor:PlayerId,slot:usize,creative:bool,
    tools:&mut Tools,history:&mut world::edit_history::EditHistory,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>)->Result<()> {
    ensure!(slot<world::inventory::HOTBAR_SLOTS,"invalid hand slot");
    match command {
        Command::Scanner(anchor, action) => {
            world::scanner::commands::apply(w, anchor, p.eye(w.voxel_scale) * w.voxel_scale.meters(), action)?;
            return Ok(());
        }
        Command::Inventory(c)=>{let bench=world::factory::at_bench(w,p.pos);c.apply_world(w,p,bench)?;w.sync_from_inventory();return Ok(());}
        Command::Factory(a)=>{if let Some(b)=world::factory::action(w,p.pos,a){select(w,slot,b);}}
        Command::Chip(a)=>world::factory::chips::chip_action(w,p.pos,a),
        Command::Radio(a)=>{for cell in world::orbital::radio::action(w,day,p,a){blocks.insert(cell);world::mark_chunk_and_neighbors_dirty(chunks,cell);}}
        Command::Receiver(a)=>{for cell in world::orbital::receiver::action(w,day,p,a){blocks.insert(cell);world::mark_chunk_and_neighbors_dirty(chunks,cell);}}
        Command::Request(cell,channel)=>{let message=world::orbital::radio::request(w,day,p,cell,channel);w.expedition.say(message);}
        Command::Lamp=>w.expedition.toggle_lamp(),
        Command::Drop(i)=>{ensure!(i<world::inventory::HOTBAR_SLOTS,"invalid drop slot");
            ensure!(w.inventory.dropped.len()<1024,"dropped item limit");
            if let Some(stack)=w.inventory.cursor.take().or_else(||w.inventory.hotbar[i].take()) {
                let at=(p.eye(w.voxel_scale)+p.forward_dir()*0.5/w.voxel_scale.meters())*w.voxel_scale.meters();
                w.inventory.dropped.push(world::inventory::DroppedStack{pos:at,stack});
                w.inventory.pickup_cooldown=0.6;
            }w.sync_from_inventory();return Ok(());}
        Command::Give(item,count)=>{ensure!(creative,"creative grants are disabled by the host");
            ensure!(!matches!(item,Item::Bucket(ul) if !world::water_tools::valid_bucket_payload(ul)),"invalid bucket volume");
            ensure!(count>0&&count<=w.inventory.stack_limit(item),"invalid grant quantity");
            ensure!(!matches!(item,Item::Wand{id,..} if id as usize>=w.adventure.wand_bank.len()),"unknown wand");
            w.inventory.try_insert_pack_first(ItemStack::new(item,count));w.sync_from_inventory();return Ok(());}
        Command::SelectCreativeItem(item,count,target)=>{
            ensure!(creative,"creative grants are disabled by the host");
            creative::catalogue::select(w,item,count,target)?;
            return Ok(());
        }
        Command::SelectCreativeRow(items)=>{
            ensure!(creative,"creative grants are disabled by the host");
            creative::catalogue::select_row(w,&items)?;
            return Ok(());
        }
        Command::Tools(config)=>{config.validate()?;*tools=config;}
        Command::Respawn=>anyhow::bail!("respawn moves the body; the session handles it"),
        Command::Study(lesson) => {
            world::progression::study::buy(w, p, lesson);
            return Ok(());
        }
        Command::StudyMaterials(lesson, research) => {
            world::progression::study::buy_materials(w, p, lesson, research);
            return Ok(());
        }
        Command::Resident(villager, action) => {
            world::travel::resident_action(w, villager, p.eye(w.voxel_scale) * w.voxel_scale.meters(), action, Some(actor));
            return Ok(());
        }
        Command::EditHistory(direction)=>{
            ensure!(creative,"Undo: Creative Only");
            ensure!(history.request(direction),"Edit History Busy");
            return Ok(());
        }
    }
    w.reconcile_legacy_to_inventory();w.sync_from_inventory();Ok(())
}
fn select(w:&mut World,slot:usize,b:BlockId) {
    if let Some(i)=w.inventory.hotbar.iter().position(|s|s.is_some_and(|s|s.item.placeable_block()==Some(b))) {
        w.inventory.swap_slot_with_hotbar(SlotAddr::Hotbar(i),slot);
    } else if let Some(i)=w.inventory.pack.iter().position(|s|s.is_some_and(|s|s.item.placeable_block()==Some(b))) {
        w.inventory.swap_slot_with_hotbar(SlotAddr::Pack(i),slot);
    }
}
