use super::*;

pub(in super::super) fn draw(ctx:&egui::Context,w:&mut World,p:&mut crate::player::Player) {
    let Some(city)=w.farming.jobs.quarter.as_ref() else{return;};
    let Some(site)=city.frontier.as_ref() else{return;};
    let colony=!site.project_layers.is_empty();
    let housed:usize=site.households.iter().map(|h|h.members.len()).sum();
    let capacity:usize=site.projects.iter().filter_map(|p|p.home.as_ref()).map(|h|usize::from(h.capacity)).sum();
    let rows:Vec<_>=site.projects.iter().enumerate().map(|(i,project)|
        (project.name.clone(),site.built(w,i),project.cells.len())).collect();
    let timber=site.timber.iter().filter(|at|known(w,IVec3::from(**at))==Some(B::Log)).count();
    let roster=super::super::super::crew::saved_workers(w);
    let workers=roster.len();
    let radius=super::worker_radius(w);
    let nearby=roster.iter().filter(|(_,at)|at.distance_squared(p.pos*w.voxel_scale.meters())<=radius*radius).count();
    let production_summary=production::summary(w);
    let forestry=production::woodlot_counts(w);
    let mut speed=w.farming.jobs.production.as_ref().map_or(1,|s|s.speed);
    let slots=w.farming.jobs.stockpile.map(|p|w.block_data.chest(p.into())).unwrap_or_default();
    let logs=production::count(&slots,Item::Block(B::Log));
    let planks=production::count(&slots,Item::Block(B::Planks));
    let meals=w.farming.jobs.production.as_ref().map_or(0,|s|production::count(&slots,s.meal));
    let base=IVec3::from(city.origin).as_vec3();
    let expansion=site.growth.as_ref().filter(|g|g.version==4).map(|g|g.waiting.clone());
    let depot=super::hub_plan(&w.farming.jobs).map(|hub| {
        let stock=w.block_data.chest(hub.stockpile);
        let count=|block|production::count(&stock,Item::Block(block));
        let couriers=super::super::super::crew::ids(&w.farming.jobs)
            .filter_map(|id|super::super::super::crew::view(&w.farming.jobs,id))
            .filter(|a|matches!(a.cargo,Some(super::super::super::supply::Cargo::Supply{..}))).count();
        format!("Depot | Logs {} | Planks {} | Chests {} | Courier {couriers}/1",count(B::Log),count(B::Planks),count(B::Chest))
    });
    let offer_service=site.growth.as_ref().is_some_and(|g|g.version==3
        && g.waiting=="No Space: Local Capacity Exhausted") && housed<workers;
    let mut expand_service=false;
    let mut paused = city.paused;
    let mut changed=false;
    egui::Window::new("Growing settlement")
        .anchor(egui::Align2::RIGHT_TOP,egui::vec2(-16.0,70.0)).default_width(300.0)
        .max_height((ctx.content_rect().height()-100.0).max(180.0)).vscroll(true).show(ctx,|ui| {
            if !colony && let Some((_, built, total)) = rows.first() {
                production::scene::first_objective(ui, w, *built, *total);
            }
            ui.label(format!("Nearby: {nearby}/{workers} · Fallen timber: {timber}"));
            ui.label(format!("Stockpile · Logs {logs} · Planks {planks} · Meals {meals}"));
            if colony {ui.label(format!("Housed {housed}/{workers} | Planned places {capacity} | Unhoused {}",workers.saturating_sub(housed)));}
            if offer_service && ui.button("Expand Services").clicked() {expand_service=true;}
            if let Some(status)=&expansion {ui.label(format!("Expansion | {status}"));}
            if let Some(stock)=&depot {ui.label(stock);}
            if let Some(summary)=production_summary {ui.label(summary);}
            if let Some((planted,felled))=forestry {ui.label(format!("Trees planted {planted} · Trees felled {felled}"));}
            for (name,built,total) in rows {
                ui.label(format!("{name}: {built}/{total}"));
                ui.add(egui::ProgressBar::new(built as f32/total as f32));
            }
            ui.label(&city.status);
            ui.checkbox(&mut paused,"Work Paused");
            ui.horizontal(|ui| {
                ui.label("World speed");
                for value in [1,2,4,8] {ui.selectable_value(&mut speed,value,format!("{value}x"));}
            });
            ui.horizontal(|ui| {
                if ui.button("Street View").clicked() {p.pos=base+if colony {Vec3::new(10.5,2.0,15.5)}else{Vec3::new(0.5,1.0,11.5)};p.yaw=0.15;p.pitch=0.0;p.flying=false;changed=true;}
                if ui.button("Site View").clicked() {p.pos=base+if colony {Vec3::new(25.0,23.0,28.0)}else{Vec3::new(0.5,14.0,15.5)};p.yaw=if colony {-0.72}else{0.0};p.pitch=-0.6;p.flying=true;changed=true;}
            });
        });
    if let Some(city) = w.farming.jobs.quarter.as_mut() {
        city.paused = paused;
    }
    if expand_service && let Err(error)=super::enable_service_growth(w)
        && let Some(city)=w.farming.jobs.quarter.as_mut() {city.status=error;}
    production::scene::set_speed(w,speed);
    // Camera buttons change the same position used by collision and activity tests.
    if changed {p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;}
}
