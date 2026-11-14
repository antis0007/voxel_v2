//! Disposable native onboarding checks and checkpoints for rendered reviews.
use anyhow::{Context, Result, ensure};
use crate::{fluid::VoxelScalePreset, player::{Input, Player}, world::{self, BlockId, DayNightCycle, World, WorldType}};
use std::{path::Path, time::Instant};

fn option(key: &str, default: &str) -> String {
    std::env::args().find_map(|a| a.strip_prefix(key).map(str::to_owned)).unwrap_or_else(|| default.into())
}

fn snapshot(w: &World) -> serde_json::Value {
    serde_json::json!({
        "city": world::farming::jobs::onboarding::view(w),
        "survival_complete": w.goals.completed,
        "next": w.goals.view().current.as_ref().map(|r| &r.title),
    })
}

fn check_roster(w: &mut World) -> Result<()> {
    let Some(housing) = world::farming::jobs::onboarding::view(w).and_then(|v| v.housing) else { return Ok(()) };
    ensure!(w.travel.following.is_none(), "Review fixture already has a follower");
    let mut housed_members = 0;
    for (id, _) in world::farming::jobs::crew::saved_workers(w) {
        w.travel.following = Some(id);
        let current = world::farming::jobs::onboarding::view(w).and_then(|v| v.housing).context("Housing disappeared")?;
        ensure!(current.residents + 1 == housing.residents, "Follower remained on city roster");
        ensure!(current.housed <= housing.housed && housing.housed - current.housed <= 1,
            "Removing one resident changed multiple housing assignments");
        housed_members += housing.housed - current.housed;
    }
    w.travel.following = None;
    ensure!(housed_members == housing.housed, "Housing counted residents outside the active roster");
    Ok(())
}

fn text(ctx: &egui::Context, w: &World, time: f64) -> String {
    ctx.begin_pass(egui::RawInput {
        time: Some(time), screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0,720.0))),
        ..Default::default()
    });
    super::publish_world(ctx, w);
    super::tracker(ctx);
    let out = ctx.end_pass();
    fn collect(shape: &egui::Shape, text: &mut String) {
        match shape {
            egui::Shape::Text(t) => { text.push_str(t.galley.text()); text.push('\n'); }
            egui::Shape::Vec(shapes) => for shape in shapes { collect(shape, text); },
            _ => {}
        }
    }
    let mut text = String::new();
    for shape in out.shapes { collect(&shape.shape, &mut text); }
    text
}

fn checkpoint(root: &Path, w: &mut World, p: &Player) -> Result<()> {
    let chunks: Vec<_> = w.chunks.keys().map(|c| c.to_array()).collect();
    std::fs::write(root.join("chunks.json"), serde_json::to_vec(&chunks)?)?;
    let mut saves = world::storage::SaveService::new();
    world::storage::set_autosave_enabled(false);
    saves.request(true);
    let started = Instant::now();
    let day = DayNightCycle::new(0.4, 1200.0);
    while saves.busy() {
        saves.tick(Some(w), p, &day);
        ensure!(!saves.failed, "Save failed: {:?}", saves.status);
        ensure!(started.elapsed().as_secs() < 30, "Save timed out");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(())
}

pub(crate) fn run(root: &Path) -> Result<()> {
    ensure!(world::storage::save_directory() == root.join("disposable-saves"), "Explicit disposable save root required");
    std::fs::create_dir_all(root)?;
    std::fs::create_dir_all(world::storage::save_directory())?;
    let resume = std::env::args().any(|a| a == "--onboarding-resume");
    if resume {
        ensure!(!super::enabled(), "Hidden preference did not survive process restart");
        let mut saved = world::storage::load_review_checkpoint()?;
        let chunks: Vec<[i32;3]> = serde_json::from_slice(&std::fs::read(root.join("chunks.json"))?)?;
        for c in chunks { ensure!(saved.world.admit_edit_chunk(c.into()), "Working chunk did not reload"); }
        check_roster(&mut saved.world)?;
        world::goals::update(&mut saved.world, 1.0, false);
        let expected: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("expected.json"))?)?;
        ensure!(snapshot(&saved.world) == expected, "Objective changed on full process reload");
        let ctx = egui::Context::default();
        ensure!(text(&ctx, &saved.world, 0.0).is_empty(), "Hidden tracker rendered after restart");
        super::set_enabled(true);
        let _ = text(&ctx, &saved.world, 0.1);
        ensure!(!text(&ctx, &saved.world, 0.2).is_empty(), "Opt-in did not restore tracker");
        std::fs::write(root.join("reload-passed.json"), serde_json::to_vec_pretty(&expected)?)?;
        println!("[onboarding] PASS full process reload, completion and persistent opt-out/opt-in");
        return Ok(());
    }
    ensure!(!world::storage::quicksave_path().exists(), "Fresh review root required");
    ensure!(super::enabled(), "Fresh profile did not show guidance");
    let scenario = option("--onboarding-scenario=", "city");
    let stage = option("--onboarding-stage=", "start");
    let mut w = World::with_seed_in(WorldType::Superflat, VoxelScalePreset::OneMeter, 424242,
        &world::storage::save_directory());
    let mut p = Player::new();
    let ctx = egui::Context::default();
    if scenario == "survival" {
        use world::inventory::{Item, ItemStack, SurvivalInventory};
        w.set_rules(world::rules::Rules::new_world(world::rules::GameMode::Survival));
        w.inventory = SurvivalInventory::empty();
        p.pos = w.suggested_spawn(); p.precise_pos = p.pos.as_dvec3();
        world::goals::update(&mut w, 1.0, false);
        let _ = text(&ctx, &w, 0.0);
        let first = text(&ctx, &w, 0.1);
        ensure!(first.contains("Logs") && !first.contains("[Break]"), "Fresh Survival guidance or binding missing: {first}");
        if stage != "start" {
            w.inventory.try_insert(ItemStack::new(Item::Block(BlockId::Log), 2));
            world::goals::update(&mut w, 1.0, false);
            let next = text(&ctx, &w, 1.0);
            ensure!(next.contains("Done") && next.contains("Planks"), "Completion feedback missing: {next}");
            if stage == "complete" {
                for name in ["Oak planks", "Oak planks", "Artificer workbench"] {
                    let recipe = world::inventory::all_recipes().iter().find(|r| r.name == name).context("Missing onboarding recipe")?;
                    ensure!(w.inventory.stage_recipe_into_grid(recipe, false), "Recipe inputs missing: {name}");
                    ensure!(w.inventory.craft_from_grid(true, false).is_some(), "Recipe failed: {name}");
                    world::goals::update(&mut w, 1.0, false);
                }
                ensure!(w.goals.is_done("logs") && w.goals.is_done("planks") && w.goals.is_done("workbench"), "Survival progression failed");
            }
        }
    } else {
        if scenario == "colony" {
            ensure!(std::env::var("VOXEL_CITY_COLONY").as_deref() == Ok("1"), "Colony review requires its scene flag");
            world::farming::jobs::quarter::install(&mut w, &mut p).map_err(anyhow::Error::msg)?;
        } else {
            world::farming::jobs::production::scene::install(&mut w, &mut p).map_err(anyhow::Error::msg)?;
        }
        let initial = world::farming::jobs::onboarding::view(&w).context("City objective missing")?;
        ensure!(initial.total > 0 && initial.built == 0 && !initial.complete(), "Fresh objective already completed");
        let before = initial.clone();
        w.inventory.try_insert(world::inventory::ItemStack::new(world::inventory::Item::Block(BlockId::Planks), 64));
        ensure!(world::farming::jobs::onboarding::view(&w) == Some(before), "Player inventory advanced city objective");
        w.inventory = world::inventory::SurvivalInventory::empty();
        if stage != "start" {
            let mut input = Input::default();
            let mut sim = crate::simulation::WorldSimulation::new();
            sim.reset_for_world(w.voxel_scale);
            let mut day = DayNightCycle::new(0.4, 1200.0);
            let (mut blocks, mut chunks) = (Default::default(), Default::default());
            for step in 0..360_000 {
                ensure!(sim.prepare_step(), "World simulation cannot advance");
                // The window driver settles loose drops outside fixed::gameplay.
                let mut drops = std::mem::take(&mut w.inventory.dropped);
                for drop in &mut drops {
                    let next = drop.pos - glam::Vec3::Y * (crate::simulation::STEP_SECONDS * 2.5).min(0.1);
                    if w.get(((next - glam::Vec3::Y * 0.12) / w.voxel_scale.meters()).floor().as_ivec3()).is_empty() {
                        drop.pos = next;
                    }
                }
                w.inventory.dropped = drops;
                crate::simulation::fixed::gameplay(&mut w, &mut p, &mut input, false, |_,_,_,_| crate::simulation::fixed::Actions::default());
                crate::simulation::fixed::finish(&mut sim, &mut w, &p, &mut day,
                    &mut blocks, &mut chunks, crate::simulation::Budget::UNBOUNDED, true, true);
                blocks.clear(); chunks.clear();
                w.adventure.dirty_voxels.clear();
                if step % 120 == 0 {
                    let current = world::farming::jobs::onboarding::view(&w).context("Lost city objective")?;
                    if (stage == "projects" && current.built == current.total)
                        || (stage == "partial" && current.built > 0 && current.built < current.total)
                        || (stage == "complete" && current.complete()) { break; }
                }
            }
            let current = world::farming::jobs::onboarding::view(&w).context("Lost city objective")?;
            let reached = match stage.as_str() {
                "projects" => current.built == current.total,
                "complete" => current.complete(),
                _ => current.built > 0 && current.built < current.total,
            };
            ensure!(reached, "City did not reach requested stage: {current:?}");
            if current.housing.as_ref().is_some_and(|h| h.demand > 0) {
                ensure!(!current.complete(), "Unhoused residents reported housing ready");
            }
        }
        let _ = text(&ctx, &w, 0.0);
        let shown = text(&ctx, &w, 0.1);
        let title = if scenario == "colony" {"Housing Demand"}
            else if stage == "complete" {"Settlement Established"} else {"Settlement Start"};
        ensure!(shown.contains(title), "City UI state missing: {shown}");
        ensure!(!shown.contains("Hold on a log"), "Survival instructions leaked into city");
        if scenario == "colony" {
            let housing = world::farming::jobs::onboarding::view(&w).and_then(|v| v.housing).context("Colony housing missing")?;
            ensure!(housing.residents == world::farming::jobs::crew::saved_workers(&w).len()
                && housing.demand == housing.residents.saturating_sub(housing.housed), "Housing did not use actual residents");
            check_roster(&mut w)?;
        }
    }
    world::goals::update(&mut w, 1.0, false);
    std::fs::write(root.join("ui-text.txt"), text(&ctx, &w, 2.0))?;
    checkpoint(root, &mut w, &p)?;
    if scenario != "survival" {
        let chunks = std::mem::take(&mut w.chunks);
        let unloaded = world::farming::jobs::onboarding::view(&w).context("Unloaded site lost objective")?;
        ensure!(unloaded.unloaded > 0 && !unloaded.complete(), "Unloaded site reported completion");
        w.chunks = chunks;
    }
    let expected = snapshot(&w);
    std::fs::write(root.join("expected.json"), serde_json::to_vec_pretty(&expected)?)?;
    super::set_enabled(false);
    ensure!(text(&ctx, &w, 3.0).is_empty(), "Opt-out still draws guidance");
    println!("[onboarding] PASS {scenario}/{stage} {expected}");
    Ok(())
}
