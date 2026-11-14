//! Native acceptance through the same fixed gameplay loop as window play.
use super::*;
use anyhow::{Context, Result, ensure};
use crate::{player::{Input, Player}, world::{WorldType, inventory::FoodKind}};
use std::path::Path;

pub(crate) fn preview(w: &mut World, p: &mut Player, steps: usize) {
    let mut input = Input::default();
    for _ in 0..steps.min(360_000) {
        crate::simulation::fixed::gameplay(w, p, &mut input, false, |_, _, _, _| crate::simulation::fixed::Actions::default());
        w.adventure.dirty_voxels.clear();
    }
    println!("[livelihood-preview] steps={} {}", w.farming.clock, summary(w).unwrap_or_default());
}

pub(crate) fn run(root: &Path) -> Result<()> {
    if woodlot::enabled() { return woodlot::check::run(root); }
    std::fs::create_dir_all(root)?;
    let mut w = World::with_seed_in(WorldType::Superflat, crate::fluid::VoxelScalePreset::OneMeter, 424242, root);
    let mut p = Player::new();
    scene::install(&mut w, &mut p).map_err(anyhow::Error::msg)?;
    let original_orders = w.farming.jobs.build.clone();
    let earth_source: Vec<_> = w.farming.jobs.mine.iter().filter(|(_, b)| *b == B::Dirt as u16).map(|(p,_)| *p).collect();
    let pile = IVec3::from(w.farming.jobs.stockpile.context("Missing stockpile")?);
    transaction_checks(&w)?;
    let mut input = Input::default();
    let started = std::time::Instant::now();
    let mut saved_seed = false;
    let mut saved_harvest = false;
    let mut positions = BTreeMap::new();
    for step in 0..360_000 {
        crate::simulation::fixed::gameplay(&mut w, &mut p, &mut input, false, |_, _, _, _| crate::simulation::fixed::Actions::default());
        w.adventure.dirty_voxels.clear();
        for (id, at) in super::super::crew::saved_workers(&w) { positions.entry(id).or_insert(at); }
        let seed = w.farming.jobs.sowing.is_some();
        let harvest = w.farming.jobs.carry.is_some_and(|s| matches!(s.item, Item::Block(B::CarrotCrop | B::BeetCrop))) && !seed;
        if (seed && !saved_seed) || (harvest && !saved_harvest) {
            let bytes = farming::sidecar::encode(&w);
            let before = ron::to_string(&w.farming.jobs)?;
            let (loaded, _) = farming::sidecar::decode(&bytes)?;
            ensure!(ron::to_string(&loaded.jobs)? == before, "Persistent cargo or production changed on reload");
            w.farming = loaded;
            let label = if seed { "carried-seed.vxf" } else { "carried-harvest.vxf" };
            std::fs::write(root.join(label), bytes)?;
            saved_seed |= seed; saved_harvest |= harvest;
        }
        if step % 12_000 == 0 {
            println!("[livelihood] step={step} {} {} helpers={:?}", summary(&w).unwrap_or_default(), super::super::status(&w),
                super::super::crew::saved_workers(&w));
        }
        let s = w.farming.jobs.production.as_ref().context("Lost production")?;
        if s.counts.eaten >= 3 && s.next_meal.len() == 3 && w.farming.jobs.build.is_empty() && saved_harvest { break; }
    }
    let j = &w.farming.jobs;
    let s = j.production.as_ref().context("Lost production")?;
    let slots = w.block_data.chest(pile);
    let feet = j.worker_pos.map(Vec3::from).unwrap_or(p.pos);
    let details = serde_json::json!({"jobs":j,"stock":slots,"enemies":w.adventure.enemies.iter().map(|e| format!("{:?}",e.pos)).collect::<Vec<_>>(),
        "remaining":j.build.iter().map(|o| serde_json::json!({"at":o.at,"block":format!("{:?}",o.block()),"ready":o.ready(&w),
            "available":j.available(*o),"unoccupied":super::super::construction::unoccupied(&w,*o,p.pos),
            "path":super::super::path(&w, feet, Job::Build(*o)).len()})).collect::<Vec<_>>()});
    std::fs::write(root.join("final-state.json"),serde_json::to_vec_pretty(&details)?)?;
    ensure!(j.valid() && saved_seed && saved_harvest, "Missing valid seed/harvest save checkpoint");
    ensure!(s.next_meal.len() == 3 && s.counts.eaten >= 3, "All three residents must physically eat: {:?}", s.counts);
    ensure!(s.counts.planted >= 12 && s.counts.harvested > 0 && s.counts.meals >= 4, "Production chain did not run");
    ensure!(j.build.is_empty(), "Shelter incomplete: {} {:?} {:?}", super::super::status(&w), j.task, j.carry);
    ensure!(original_orders.iter().all(|o| w.get(o.cell()) == o.block()), "Paid construction geometry missing");
    ensure!(count(&slots, s.meal) + super::super::crew::cargo_item(j, s.meal) + s.counts.eaten == s.counts.meals, "Meal conservation");
    let produce: u32 = [B::CarrotCrop, B::BeetCrop].into_iter()
        .map(|b| count(&slots, Item::Block(b)) + super::super::crew::cargo_item(j, Item::Block(b))).sum();
    ensure!(12 + s.counts.harvested == s.counts.planted + 2 * s.counts.meals + produce, "Crop/seed/recipe conservation");
    let plank = Item::Block(B::Planks);
    let chest = Item::Block(B::Chest);
    let log = Item::Block(B::Log);
    let raw = j.mine.iter().filter(|(at,_)| w.get((*at).into()) == B::Log).count() as u32
        + count(&slots, log) + super::super::crew::cargo_item(j, log);
    let built_planks = original_orders.iter().filter(|o| o.block() == B::Planks).count() as u32;
    let built_chests = original_orders.iter().filter(|o| o.block() == B::Chest).count() as u32;
    ensure!(12 * 4 == raw * 4 + built_planks + count(&slots, plank) + super::super::crew::cargo_item(j, plank)
        + 8 * (built_chests + count(&slots, chest) + super::super::crew::cargo_item(j, chest)), "Timber/plank/furniture conservation");
    let earth = Item::Block(B::Dirt);
    let built_earth = original_orders.iter().filter(|o| o.block() == B::Dirt).count() as u32;
    ensure!(earth_source.len() as u32 == built_earth + count(&slots, earth) + super::super::crew::cargo_item(j, earth)
        + earth_source.iter().filter(|p| w.get((**p).into()) == B::Dirt).count() as u32, "Earthwork conservation");
    ensure!(w.inventory.count_item(s.meal) == 0 && w.inventory.count_item(Item::Block(B::CarrotCrop)) == 0,
        "NPC production leaked into player inventory");
    let result = serde_json::json!({"passed":true,"steps":w.farming.clock,"wall_seconds":started.elapsed().as_secs_f64(),
        "residents":3,"plots":s.beds.len(),"counts":s.counts,"paid_placements":original_orders.len(),
        "seed_reload":saved_seed,"harvest_reload":saved_harvest,"food_conserved":true,"timber_conserved":true,
        "earthwork_placements":built_earth,"earthwork_conserved":true,
        "input":format!("12 seeds, 12 authored fallen log cells, and {} marked soil cells; no initial meals, planks, or furniture",earth_source.len()),
        "limits":"local scene; no autonomous forestry or regional economy; no multiplayer"});
    std::fs::write(root.join("livelihood-check.json"), serde_json::to_vec_pretty(&result)?)?;
    std::fs::write(root.join("finished-farming.vxf"), farming::sidecar::encode(&w))?;
    println!("[livelihood] PASS {}", result);
    Ok(())
}

pub(super) fn transaction_checks(w: &World) -> Result<()> {
    let planks = all_recipes().iter().find(|r| r.name == "Oak planks").context("Missing plank recipe")?;
    let mut slots = vec![Some(ItemStack::new(Item::Block(B::Stone), 64)); 27];
    slots[0] = Some(ItemStack::new(Item::Block(B::Log), 2));
    let original = slots.clone();
    ensure!(actions::transformed(w, &slots, planks).is_none() && slots == original, "Full output must not consume ingredients");
    slots[1] = None;
    let next = actions::transformed(w, &slots, planks).context("Valid recipe refused")?;
    ensure!(count(&next, Item::Block(B::Planks)) == 4 && count(&next, Item::Block(B::Log)) == 1, "Recipe amounts");
    slots[0] = None;
    ensure!(actions::transformed(w, &slots, planks).is_none(), "Missing input created goods");
    let old: Jobs = ron::from_str("(worker:None,worker_pos:None,stockpile:None,mine:[],carry:None)")?;
    ensure!(old.valid() && old.production.is_none() && old.sowing.is_none(), "Legacy job save defaults");
    println!("[livelihood] PASS recipe conservation, full output, missing input, legacy job defaults");
    Ok(())
}
