//! Part Builder and Service tabs of the workbench page.
use super::*;

/// Part kinds as a pickable row, then every material that can make one, with its cost.
pub(super) fn parts(ui:&mut egui::Ui,inv:&SurvivalInventory,bench:bool,action:&mut UiAction){
    let id=egui::Id::new("tool_station_part");
    let mut kind=ui.ctx().data_mut(|d|d.get_temp::<usize>(id).unwrap_or(0)).min(PARTS.part_kinds.len()-1);
    ui.horizontal_wrapped(|ui|{
        for (i,p) in PARTS.part_kinds.iter().enumerate(){
            let response=icon(ui,icons::Icon::ToolPart(i as u8,0),40.0).on_hover_text(&p.display_name);
            if kind==i {ui.painter().rect_stroke(response.rect,0.0,egui::Stroke::new(1.0,presentation::AMBER),egui::StrokeKind::Inside);}
            if response.clicked(){kind=i;}
        }
    });
    let units=u32::from(PARTS.part_kinds[kind].material_cost)*64;
    ui.horizontal(|ui|{
        ui.strong(&PARTS.part_kinds[kind].display_name);
        ui.weak(format!("Material ×{}",units/64));
    });
    egui::Grid::new("part_materials").min_col_width(36.0).striped(true).show(ui,|ui|{
        for (m,row) in MATERIALS.iter().enumerate().filter(|(i,_)|tool_workshop::supply(*i).is_some()){
            let cast=tool_workshop::supply(m).is_some_and(|r|r.cast);
            let paid=tool_workshop::pay(&mut inv.clone(),m,units);
            let response=icon(ui,icons::Icon::ToolPart(kind as u8,m as u8),32.0);
            recipe_links::item(ui,&response,Item::Part{kind:kind as u8,material:m as u8});
            ui.label(&row.display_name);
            let cost=tool_workshop::material_cost(m,units,inv.voxel_m)
                .map_or_else(||"Unavailable".into(),|(item,n)|format!("{} ×{n:.0}",item.name()));
            let response=ui.label(RichText::new(cost).color(if paid {presentation::MUTED} else {presentation::DANGER}));
            if let Some((item,_))=tool_workshop::material_cost(m,units,inv.voxel_m) {recipe_links::item(ui,&response,item);}
            if cast {ui.weak("Casting Required");}
            else if ui.add_enabled(bench&&paid,egui::Button::new("Build")).clicked(){send(action,Action::Part(kind as u8,m as u8));}
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui|{
        let response=icon(ui,icons::Icon::ToolCast(kind as u8),36.0);
        recipe_links::item(ui,&response,Item::Cast(kind as u8));
        let clay=format!("Clay ×{}",tool_workshop::mold_cost(inv.voxel_m));
        let response=ui.add_enabled(bench,egui::Button::new("Clay Mold")).on_hover_text(&clay);
        let browsing=recipe_links::item(ui,&response,Item::Cast(kind as u8));
        if !browsing&&response.clicked(){send(action,Action::Mold(kind as u8));}
        let response=ui.add_enabled(bench,egui::Button::new("Ingot Mold")).on_hover_text(&clay);
        let browsing=recipe_links::item(ui,&response,Item::Cast(255));
        if !browsing&&response.clicked(){send(action,Action::Mold(255));}
    });
    ui.ctx().data_mut(|d|d.insert_temp(id,kind));
}

/// Owned tools as a row; the chosen one shows wear, repair, part swaps and modifiers.
pub(super) fn service(ui:&mut egui::Ui,view:&crate::world::adventure::View,inv:&SurvivalInventory,bench:bool,action:&mut UiAction){
    let owned:Vec<_>=view.tool_bank.iter().enumerate().filter(|(i,_)|tool_workshop::owns(inv,*i as u16)).collect();
    let selected_id=egui::Id::new("serviced_tool");
    let mut chosen=ui.ctx().data_mut(|d|d.get_temp::<usize>(selected_id).unwrap_or(0));
    if !owned.iter().any(|(i,_)|*i==chosen){chosen=owned.first().map_or(0,|(i,_)|*i);}
    ui.horizontal_wrapped(|ui|{for (i,t) in &owned {
        let response=icon(ui,held_art::tool_icon(t),40.0);
        let browsing=recipe_links::owned(ui,&response,inv,*i as u16);
        if response.clicked()&&!browsing {chosen=*i;}
        if chosen==*i {ui.painter().rect_stroke(response.rect,0.0,egui::Stroke::new(1.0,presentation::AMBER),egui::StrokeKind::Inside);}
    }});
    let Some((_,tool))=owned.iter().find(|(i,_)|*i==chosen) else {ui.weak("No Tools");return};
    ui.separator();
    ui.horizontal_top(|ui|{
        ui.vertical(|ui|{
            ui.strong(tool.schema_def().map_or("Tool",|s|s.display_name.as_str()));
            let response=icon(ui,held_art::tool_icon(tool),96.0);
            recipe_links::owned(ui,&response,inv,chosen as u16);
            stats(ui,tool);
            let full=tool.stats().map_or(1,|s|s.durability);
            ui.add(egui::ProgressBar::new(tool.durability_left as f32/full as f32).desired_width(180.0).text(format!("{} / {}",tool.durability_left,full)));
            ui.label(format!("Level {} · XP {} · Free Slots {}",tool.level,tool.xp,usize::from(tool.modifier_slots().unwrap_or(0)).saturating_sub(tool.modifiers.len())));
            let head=tool.head_material().ok();
            let paid=head.and_then(|h|MATERIALS.iter().position(|m|m.id==h.id)).is_some_and(|m|tool_workshop::pay(&mut inv.clone(),m,64));
            ui.horizontal(|ui|{
                if ui.add_enabled(bench&&paid&&tool.durability_left<full,egui::Button::new("Repair")).clicked(){send(action,Action::Repair(chosen as u16));}
                if let Some(head)=head {ui.weak(format!("{} ×1",head.display_name));}
            });
        });
        ui.separator();
        ui.vertical(|ui|{
            ui.label(RichText::new("Replace Part").color(presentation::AMBER));
            for (index,(part,old)) in tool.parts.iter().enumerate(){
                let k=kind_index(part);
                ui.horizontal_wrapped(|ui|{
                    ui.label(&PARTS.part_kinds[k].display_name);
                    for (m,row) in MATERIALS.iter().enumerate().filter(|(_,m)|m.id!=*old){
                        if inv.count_item(Item::Part{kind:k as u8,material:m as u8})==0 {continue;}
                        let mut next=(*tool).clone();
                        if next.replace_part(index,&row.id).is_err(){continue;}
                        let response=icon(ui,icons::Icon::ToolPart(k as u8,m as u8),32.0).on_hover_ui(|ui|{ui.strong(&row.display_name);compare(ui,tool,&next);ui.weak("Old Part Consumed");});
                        let browsing=recipe_links::item(ui,&response,Item::Part{kind:k as u8,material:m as u8});
                        if bench&&!browsing&&response.clicked(){send(action,Action::Replace(chosen as u16,index as u8,m as u8));}
                    }
                });
            }
            ui.add_space(6.0);
            ui.label(RichText::new("Modifiers").color(presentation::AMBER));
            egui::Grid::new("tool_modifiers").min_col_width(36.0).show(ui,|ui|{
                for(i,row)in engine_tools::modifiers::ROWS.iter().enumerate(){
                    let mut next=(*tool).clone();let result=engine_tools::modifiers::apply(&mut next,&row.id);
                    let paid=crate::world::inventory::resources::exchange(&mut inv.clone(),&[(row.resource,row.units)],&[]);
                    let hint=engine_content::descriptions::hint(&row.display_name);
                    if ui.add_enabled(bench&&paid&&result.is_ok(),egui::Button::new(&row.display_name)).on_hover_ui(|ui|{if let Some(line)=hint{ui.label(line);}if let Err(e)=&result {ui.label(e);}else{compare(ui,tool,&next);}}).clicked(){send(action,Action::Modify(chosen as u16,i as u8));}
                    ui.weak(format!("{} ×{:.2}",crate::world::expedition::NAMES[row.resource],row.units as f32/64.0));
                    if let Some(line)=hint{ui.label(RichText::new(line).size(12.0).weak());}
                    ui.end_row();
                }
            });
        });
    });
    ui.ctx().data_mut(|d|d.insert_temp(selected_id,chosen));
}
