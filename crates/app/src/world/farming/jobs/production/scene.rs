use super::*;
use crate::world::{adventure::Enemy, block_data};

pub(crate) fn install(w: &mut World, p: &mut crate::player::Player) -> Result<(), String> {
    if !super::super::enabled(w) || w.world_type != crate::world::WorldType::Superflat
        || w.voxel_scale.meters() != 1.0 || w.farming.jobs.worker.is_some() {
        return Err("Livelihood scene requires a fresh local one-metre Superflat world".into());
    }
    let mut s: State = ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/city_livelihood.ron")))
        .map_err(|e| e.to_string())?;
    if !s.valid() { return Err("Invalid livelihood content".into()); }
    let y = engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 1;
    let base = IVec3::new(0, y, 0);
    let mut edits = Vec::new();
    for z in -12..=12 { for x in -12..=12 {
        edits.push((base + IVec3::new(x, 0, z), B::Dirt));
        for h in 1..=7 { edits.push((base + IVec3::new(x, h, z), B::Air)); }
    }}
    let pile = base + IVec3::new(0, 1, 0);
    edits.extend([(pile, B::Chest), (pile + IVec3::Z, B::Workbench)]);
    for b in &mut s.beds { b.at[1] += y; }
    for b in &mut s.bills { b.station[1] += y; }
    for z in [-6, -4, -2, 0, 2] { edits.push((base + IVec3::new(-9, 0, z), B::Water)); }
    // An explicit finite fallen-timber input; forestry is a separate work order.
    let logs: Vec<_> = (2..=7).flat_map(|x| [8, 9].map(move |z| base + IVec3::new(x, 1, z))).collect();
    edits.extend(logs.iter().map(|&p| (p, B::Log)));
    w.set_many(&edits);
    for id in 0..3 {
        let feet = base.as_vec3() + Vec3::new(-0.5 + id as f32, 1.0, 4.5);
        let mut e = Enemy::new(IVec3::new(id, 0, 8), 0, 40, feet, 0);
        e.home = feet; w.adventure.enemies.push(e); w.travel.homes.insert(id as u32, feet);
        if id == 0 { super::super::assign(w, 0); }
        else if !super::super::crew::add(w, id as u32) { return Err("Resident assignment failed".into()); }
    }
    super::super::designate(w, pile);
    super::super::set_priorities(w, 0, [1, 1, 2]);
    for crop in [B::CarrotCrop, B::BeetCrop] {
        let mut seed = ItemStack::new(Item::Block(crop), 6);
        block_data::store_hauled(w, pile, &mut seed);
        if seed.count != 0 { return Err("Seed stockpile full".into()); }
    }
    let j = &mut w.farming.jobs;
    j.mine.extend(logs.iter().map(|p| (p.to_array(), B::Log as u16)));
    let platforms: Vec<_> = (-6..=-1).flat_map(|z| (2..=7).filter_map(move |x| {
        ((x == 2 || x == 7 || z == -6 || z == -1) && !(x == 4 && z == -1)
            && !((x == 2 || x == 7) && (z == -5 || z == -2)))
            .then_some(base + IVec3::new(x,1,z))
    })).collect();
    let borrow: Vec<_> = (4..=8).flat_map(|z| (-11..=-8).map(move |x| base + IVec3::new(x,0,z)))
        .take(platforms.len()).collect();
    j.mine.extend(borrow.iter().map(|p| (p.to_array(), B::Dirt as u16)));
    // Roof edges need a work position above the wall's occluding top edge.
    // These platforms consume exactly the earth dug from the marked borrow pit.
    // Corner gaps retain ground-level reach if adjacent walls are built first.
    j.build.extend(platforms.into_iter().map(|p| super::super::construction::Order::new(p, B::Dirt)));
    // The existing paid constructor builds a modest roofed work shelter.
    j.build.extend(shelter_cells(base).map(|cell| {
        super::super::construction::Order::new(cell, B::Planks)
    }));
    j.build.push(super::super::construction::Order::new(base + IVec3::new(-1,1,2), B::Chest));
    j.production = Some(s);
    if super::woodlot::enabled() { super::woodlot::install(w)?; }
    p.pos = base.as_vec3() + Vec3::new(0.5, 1.0, 11.5);
    p.precise_pos = p.pos.as_dvec3(); p.vel = Vec3::ZERO; p.flying = false; p.yaw = 0.0; p.pitch = 0.0;
    Ok(())
}

pub(in crate::world::farming::jobs) fn shelter_cells(base: IVec3) -> impl Iterator<Item = IVec3> {
    (-5..=-2).flat_map(move |z| {
        (3..=6).flat_map(move |x| {
            (1..=3).filter_map(move |h| {
                let wall = x == 3 || x == 6 || z == -5 || z == -2;
                let door = x == 4 && z == -2 && h < 3;
                ((wall || h == 3) && !door).then_some(base + IVec3::new(x, h, z))
            })
        })
    })
}

pub(in crate::world::farming::jobs) fn first_objective(
    ui: &mut egui::Ui, _w: &World, _built: usize, _total: usize,
) {
    crate::egui_ui::goals_hud::controls(ui);
}

pub(crate) fn speed(w: &World) -> f32 {
    if !super::super::enabled(w) { return 1.0; }
    w.farming.jobs.production.as_ref().map_or(1.0, |s| f32::from(s.speed))
}

pub(crate) fn set_speed(w: &mut World, speed: u8) {
    if matches!(speed, 1 | 2 | 4 | 8) && let Some(s) = w.farming.jobs.production.as_mut() {
        s.speed = speed;
    }
}

pub(crate) fn panel(ctx: &egui::Context, w: &mut World) {
    let Some(summary) = super::summary(w) else { return };
    egui::Window::new("Settlement production").default_pos(egui::pos2(16.0, 80.0)).show(ctx, |ui| {
        if w.farming.jobs.quarter.is_none() && let Some(pile) = w.farming.jobs.stockpile {
            let base = IVec3::from(pile) - IVec3::Y;
            let cells = shelter_cells(base);
            let total = cells.count();
            let built = shelter_cells(base)
                .filter(|&cell| super::super::known(w, cell) == Some(B::Planks))
                .count();
            first_objective(ui, w, built, total);
        }
        ui.label(summary); ui.label(super::super::status(w));
        if let Some(s)=w.farming.jobs.production.as_ref() {
            ui.label(format!("Residents {} · Meals per resident {}–{}",s.workers.len(),
                s.workers.iter().map(|id|s.served.get(id).copied().unwrap_or(0)).min().unwrap_or(0),
                s.served.values().copied().max().unwrap_or(0)));
        }
        if let Some(f) = w.farming.jobs.production.as_ref().and_then(|s| s.woodlot.as_ref()) {
            ui.label(format!("Trees planted {} · Trees felled {} · Log target {}", f.planted, f.felled, f.target));
        }
        if let Some(s) = w.farming.jobs.production.as_mut() {
            ui.horizontal(|ui| {
                ui.label("World speed");
                for speed in [1, 2, 4, 8] {
                    ui.selectable_value(&mut s.speed, speed, format!("{speed}×"));
                }
            });
        }
        if let Some(p) = w.farming.jobs.stockpile {
            for stack in w.block_data.chest(p.into()).iter().flatten() {
                ui.label(format!("{} × {}", stack.item.name(), stack.count));
            }
        }
    });
}
