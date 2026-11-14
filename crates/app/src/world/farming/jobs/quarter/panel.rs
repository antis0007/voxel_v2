use super::{IVec3, Vec3, World, traditions};
use crate::player::Player;

pub(crate) fn draw(ctx: &egui::Context, w: &mut World, p: &mut Player) {
    if w.farming.jobs.quarter.as_ref().is_some_and(|q|q.frontier.is_some()) {
        super::frontier::draw(ctx,w,p);return;
    }
    let roster = super::super::crew::saved_workers(w);
    let builders = roster.len();
    let suppliers = usize::from(w.farming.jobs.market.as_ref()
        .is_some_and(|market| roster.iter().any(|(id, _)| *id == market.resident)));
    let market = super::super::market::summary(w);
    let cover = w.farming.jobs.quarter.as_ref().and_then(|q| q.lots.iter().find(|l| l.market_cover))
        .map(|lot| super::geometry::market_cover().iter().filter(|(p,b)| w.get(super::geometry::at(lot,*p)) == *b).count());
    let Some(city) = w.farming.jobs.quarter.as_mut() else {
        return;
    };
    egui::Window::new("City Quarter")
        .default_pos([16.0, 70.0])
        .default_width(300.0)
        .show(ctx, |ui| {
            if city.layout.as_ref().is_some_and(|p| p.design.is_some()) {
                ui.label(format!("Physical roster: {}", builders));
                ui.label(format!("Builders: {} | Timber supplier: {}", builders - suppliers, suppliers));
                ui.label(format!("Paid homes completed: {}", city.completed));
            } else {
                ui.label(format!(
                    "Residents: {} ┬╖ Housing: {}",
                    city.residents, city.housing_demand
                ));
                ui.label(format!("Homes Completed: {}", city.completed));
                ui.label(format!("Workers: {builders}"));
            }
            if let Some(market) = &market { ui.label(market); }
            if let Some(cover) = cover { ui.label(format!("Market Cover: {cover}/37")); }
            if let Some(plan) = &city.layout {
                if let Ok(rows) = super::layout::profiles() { ui.label(&rows[plan.profile as usize].name); }
                ui.label(format!("Layout Seed: {} · Parcels: {}", plan.seed, plan.parcels.len()));
                if plan.design.is_some() {
                    ui.label("Boulevard 9m | Streets 5m | Alleys 3m");
                    ui.label("Public plaza: 105 square metres");
                    ui.label("5 buildings supplied for design review");
                    ui.label("Workshop / storage: planned uses");
                    if plan.parcels[0].tradition == 1 && !plan.parcels[0].roof_access {
                        ui.label("Courtyard roof: known build-access block");
                    }
                }
            }
            ui.label(&city.status);
            ui.checkbox(&mut city.paused, "Projects Paused");
            ui.horizontal(|ui| {
                if ui.button("Street View").clicked() {
                    p.pos = IVec3::from(city.origin).as_vec3() + Vec3::new(0.5, 1.0, 20.5);
                    p.precise_pos = p.pos.as_dvec3();
                    p.vel = Vec3::ZERO;
                    p.flying = false;
                    p.yaw = 0.0;
                    p.pitch = 0.02;
                }
                if ui.button("Overview").clicked() {
                    p.pos = IVec3::from(city.origin).as_vec3() + Vec3::new(0.5, 20.0, 24.5);
                    p.precise_pos = p.pos.as_dvec3();
                    p.vel = Vec3::ZERO;
                    p.flying = true;
                    p.yaw = 0.0;
                    p.pitch = -0.63;
                }
            });
            if let Ok(rows) = traditions() {
                for lot in &city.lots {
                    ui.separator();
                    ui.label(&rows[lot.tradition].name);
                    ui.label(format!(
                        "Complexity: {} ┬╖ Layer: {}",
                        lot.complexity, lot.layer
                    ));
                }
            }
        });
}
