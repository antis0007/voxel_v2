//! Workbench page: tool assembly in a slot layout shaped like the tool, plus part, service and expedition tabs.
use super::*;
use crate::world::{tool_workshop::{self,Action},inventory::{Item,SurvivalInventory}};
use engine_tools::{PARTS,MATERIALS,ToolAssembly,PartRole};

mod service;
mod recipe_links;
#[cfg(test)]
mod recipe_link_tests;

const SLOT: f32 = 52.0;
const TAB_ID: &str = "tool_station_tab";
const TABS: [&str; 4] = ["Tool Station","Part Builder","Service","Expeditions"];

fn send(action:&mut UiAction,a:Action){*action=UiAction::Factory(crate::world::factory::Action::ToolWorkshop(a));}
fn icon(ui:&mut egui::Ui,icon:icons::Icon,size:f32)->egui::Response {
    let (rect,response)=ui.allocate_exact_size(egui::vec2(size,size),egui::Sense::click());
    icons::paint(ui.painter(),rect.shrink(3.0),icon);response
}
fn kind_index(kind:&engine_tools::PartKind)->usize {PARTS.part_kinds.iter().position(|k|k.id==*kind).unwrap_or(0)}

fn stats(ui:&mut egui::Ui,tool:&ToolAssembly){
    if let Ok(s)=tool.stats(){
        egui::Grid::new("tool_stats").min_col_width(40.0).show(ui,|ui|{
            for (name,value) in [("Durability",s.durability.to_string()),("Mining Speed",format!("{:.1}",s.mining_speed)),("Mining Tier",s.mining_level.to_string()),("Damage",format!("{:.1}",tool_workshop::attack_damage(tool))),("Modifier Slots",tool.modifier_slots().unwrap_or(0).to_string())]{ui.weak(name);ui.label(value);ui.end_row();}
        });
    }
}
fn traits(ui:&mut egui::Ui,tool:&ToolAssembly){
    for t in tool.traits().unwrap_or_default(){
        if let Some(r)=engine_tools::TRAITS.iter().find(|r|r.effect==t.effect){
            ui.label(RichText::new(format!("{} ×{}",r.display_name,t.stacks)).color(icons::GOLD));
            if let Some(line)=engine_content::descriptions::hint(&r.display_name){ui.label(RichText::new(line).size(12.0).weak());}
        }
    }
}
pub(super) fn compare(ui:&mut egui::Ui,old:&ToolAssembly,new:&ToolAssembly){
    if let (Ok(a),Ok(b))=(old.stats(),new.stats()) {
        for (name,a,b) in [("Durability",a.durability as f32,b.durability as f32),("Mining Speed",a.mining_speed,b.mining_speed),("Mining Tier",a.mining_level as f32,b.mining_level as f32),("Attack",a.attack,b.attack)] {
            if (a-b).abs()>0.001 {ui.label(RichText::new(format!("{name}: {a:.1} → {b:.1}")).color(if b>a {presentation::JADE} else {presentation::DANGER}));}
        }
    }
}

/// Grid cell per role on a 3×3 diagonal: working end top right, handle bottom left, like the item icon.
fn slot_cells(def:&engine_tools::ToolSchema)->Vec<(usize,usize)> {
    const SPARE: [(usize,usize);9] = [(2,0),(1,1),(0,2),(1,0),(2,1),(0,1),(1,2),(2,2),(0,0)];
    let mut used:Vec<(usize,usize)>=Vec::new();
    for p in def.parts.iter().take(8) {
        let want=match PARTS.part_kinds[kind_index(&p.kind)].role {PartRole::Head=>(2,0),PartRole::Binding|PartRole::Reinforcement=>(1,1),PartRole::Handle=>(0,2),PartRole::String=>(2,2)};
        let cell=std::iter::once(want).chain(SPARE).find(|c|!used.contains(c)).unwrap_or(want);
        used.push(cell);
    }
    used
}

/// Draws the whole workbench page with its tab bar.
pub(super) fn page(ui:&mut egui::Ui,view:&crate::world::adventure::View,inv:&SurvivalInventory,bench:bool,action:&mut UiAction){
    let id=egui::Id::new(TAB_ID);
    let mut tab=ui.ctx().data_mut(|d|d.get_temp::<usize>(id).unwrap_or(0)).min(TABS.len()-1);
    ui.horizontal_wrapped(|ui|{
        for (i,name) in TABS.iter().enumerate(){ui.selectable_value(&mut tab,i,*name);}
        if !bench {ui.label(RichText::new("Workbench Required").color(presentation::AMBER));}
    });
    ui.separator();
    match tab {
        0=>build(ui,inv,bench,action),
        1=>service::parts(ui,inv,bench,action),
        2=>service::service(ui,view,inv,bench,action),
        _=>super::resonance::draw(ui,&view.resonance,bench,action),
    }
    ui.ctx().data_mut(|d|d.insert_temp(id,tab));
}

fn build(ui:&mut egui::Ui,inv:&SurvivalInventory,bench:bool,action:&mut UiAction){
    let id=egui::Id::new("tool_station_choices");
    let (mut schema,mut selected,mut slot)=ui.ctx().data_mut(|d|d.get_temp::<(usize,[u8;8],usize)>(id).unwrap_or((0,[0;8],0)));
    schema=schema.min(PARTS.schemas.len()-1);
    let wide=ui.available_width()>=620.0;
    let mut body=|ui:&mut egui::Ui|{
        // Tool list.
        egui::ScrollArea::vertical().id_salt("schemas").max_height(SLOT*6.0).show(ui,|ui|{
            ui.set_width(44.0);
            ui.vertical(|ui|for (i,def) in PARTS.schemas.iter().enumerate().filter(|(_,s)|s.parts.len()<=8) {
                let response=icon(ui,icons::Icon::BuiltTool(i as u16,selected,0),40.0).on_hover_text(&def.display_name);
                if schema==i {ui.painter().rect_stroke(response.rect,0.0,egui::Stroke::new(1.0,presentation::AMBER),egui::StrokeKind::Inside);}
                if response.clicked()&&schema!=i {schema=i;slot=0;}
            });
        });
        let def=&PARTS.schemas[schema];
        slot=slot.min(def.parts.len().saturating_sub(1));
        let mut available=inv.clone();
        let mut have=[false;8];
        for (i,p) in def.parts.iter().enumerate().take(8){
            selected[i]=selected[i].min((MATERIALS.len()-1) as u8);
            have[i]=available.remove_item(Item::Part{kind:kind_index(&p.kind) as u8,material:selected[i]},1);
        }
        let materials=|picks:&[u8;8]|->Vec<_>{picks.iter().take(def.parts.len()).map(|&i|MATERIALS[i as usize].id.clone()).collect()};
        let current=ToolAssembly::new(&def.id,&materials(&selected)).ok();
        // Slots in the tool's shape, then the choices for the active slot.
        ui.vertical(|ui|{
            ui.strong(&def.display_name);
            let (area,_)=ui.allocate_exact_size(egui::vec2(SLOT*3.0+16.0,SLOT*3.0+16.0),egui::Sense::hover());
            for (i,(p,(cx,cy))) in def.parts.iter().zip(slot_cells(def)).enumerate(){
                let k=kind_index(&p.kind);
                let rect=egui::Rect::from_min_size(area.min+egui::vec2(cx as f32*(SLOT+8.0),cy as f32*(SLOT+8.0)),egui::vec2(SLOT,SLOT));
                let response=ui.interact(rect,ui.id().with(("slot",i)),egui::Sense::click()).on_hover_text(&PARTS.part_kinds[k].display_name);
                let browsing=recipe_links::item(ui,&response,Item::Part{kind:k as u8,material:selected[i]});
                icons::frame(ui.painter(),rect,slot==i);
                let mut painter=ui.painter().clone();
                if !have[i] {painter.set_opacity(0.35);}
                icons::paint(&painter,rect.shrink(4.0),icons::Icon::ToolPart(k as u8,selected[i]));
                let edge=if slot==i {presentation::AMBER} else if have[i] {presentation::EDGE} else {presentation::DANGER};
                ui.painter().rect_stroke(rect,0.0,egui::Stroke::new(1.0,edge),egui::StrokeKind::Inside);
                let owned=inv.count_item(Item::Part{kind:k as u8,material:selected[i]});
                ui.painter().text(rect.right_bottom()-egui::vec2(3.0,2.0),egui::Align2::RIGHT_BOTTOM,format!("×{owned}"),egui::FontId::monospace(11.0),if owned>0 {presentation::PAPER} else {presentation::DANGER});
                if response.clicked()&&!browsing {slot=i;}
            }
            if let Some(p)=def.parts.get(slot){
                let k=kind_index(&p.kind);
                ui.label(RichText::new(&PARTS.part_kinds[k].display_name).color(presentation::PAPER));
                ui.horizontal_wrapped(|ui|{
                    let owned:Vec<_>=MATERIALS.iter().enumerate().map(|(m,row)|(m,row,inv.count_item(Item::Part{kind:k as u8,material:m as u8}))).filter(|(_,_,n)|*n>0).collect();
                    if owned.is_empty(){ui.label(RichText::new(format!("No {}",PARTS.part_kinds[k].display_name)).color(presentation::DANGER));}
                    for (m,row,n) in owned {
                        let mut picks=selected;picks[slot]=m as u8;
                        let next=ToolAssembly::new(&def.id,&materials(&picks)).ok();
                        let response=icon(ui,icons::Icon::ToolPart(k as u8,m as u8),36.0).on_hover_ui(|ui|{
                            ui.strong(format!("{} ×{n}",row.display_name));
                            if let (Some(a),Some(b))=(&current,&next) {compare(ui,a,b);}
                        });
                        let browsing=recipe_links::item(ui,&response,Item::Part{kind:k as u8,material:m as u8});
                        if selected[slot]==m as u8 {ui.painter().rect_stroke(response.rect,0.0,egui::Stroke::new(1.0,presentation::AMBER),egui::StrokeKind::Inside);}
                        if response.clicked()&&!browsing {selected[slot]=m as u8;}
                    }
                });
            }
        });
        // Result and stats.
        let complete=have.iter().take(def.parts.len()).all(|h|*h);
        ui.vertical(|ui|{
            if let Some(tool)=&current{
                icon(ui,held_art::tool_icon(tool),96.0);
                stats(ui,tool);
                traits(ui,tool);
            }
            ui.add_space(6.0);
            if ui.add_enabled(bench&&complete,egui::Button::new("Assemble").min_size(egui::vec2(120.0,28.0))).clicked(){send(action,Action::Assemble(schema as u16,selected));}
            let missing=have.iter().take(def.parts.len()).filter(|h|!**h).count();
            let state=if !bench {Some("Workbench Required".to_owned())} else if missing>0 {Some(format!("Missing Parts: {missing}"))} else {None};
            if let Some(state)=state {ui.label(RichText::new(state).color(presentation::DANGER));}
        });
    };
    if wide {ui.horizontal_top(|ui|body(ui));} else {ui.vertical(|ui|body(ui));}
    ui.ctx().data_mut(|d|d.insert_temp(id,(schema,selected,slot)));
}

/// Opt-in production renderer review; the fixture never opens or mutates a save.
pub(super) fn preview(ui:&mut egui::Ui,service:bool) {
    let mut inv=SurvivalInventory::default();
    let mut view=crate::world::adventure::View::default();
    let find=|id:&str|MATERIALS.iter().position(|m|m.id.0==id).unwrap_or(0) as u8;
    let pick=|j:usize|find(if j==0{"iron"}else if j==1{"bone"}else{"wood"});
    for (i,def) in PARTS.schemas.iter().enumerate().take(3){
        let mats:Vec<_>=def.parts.iter().enumerate().map(|(j,_)|MATERIALS[pick(j) as usize].id.clone()).collect();
        let Ok(mut t)=ToolAssembly::new(&def.id,&mats) else {continue};
        t.wear(35);t.gain_xp(80);
        view.tool_bank.push(t);
        inv.try_insert(crate::world::inventory::ItemStack::new(Item::Assembled{id:i as u16,schema:i as u16},1));
        for (j,p) in def.parts.iter().enumerate(){inv.try_insert(crate::world::inventory::ItemStack::new(Item::Part{kind:kind_index(&p.kind) as u8,material:pick(j)},3));}
    }
    inv.try_insert(crate::world::inventory::ItemStack::new(Item::Part{kind:0,material:find("copper")},2));
    held_art::sync_tools(&view.tool_bank);
    ui.ctx().data_mut(|d|{
        d.insert_temp(egui::Id::new(TAB_ID),if service {2usize} else {0});
        d.insert_temp(egui::Id::new("tool_station_choices"),(0usize,[find("iron"),find("bone"),find("wood"),0,0,0,0,0],0usize));
    });
    let mut action=UiAction::None;
    page(ui,&view,&inv,true,&mut action);
}
