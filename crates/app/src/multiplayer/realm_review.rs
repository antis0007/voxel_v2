//! Disposable saves for the native cross-realm review.
use super::*;
use world::{BlockId, hell_gate, portals::gates::Aperture, storage};

/// Exercise the ordinary local chart travel and checkpoint paths.
pub(super) fn sky_and_forest() -> Result<()> {
    use engine_voxel::RealmKind;
    use world::{hall_gate, lightless_gate, lightless_life};

    ensure!(std::env::var_os("VOXEL_SAVE_DIR").is_some(), "Set a disposable VOXEL_SAVE_DIR");
    let mut surface = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut player = Player::new();
    let mut day = DayNightCycle::default();
    let floor = surface.suggested_spawn().floor().as_ivec3() + IVec3::Y * 4;
    let place = |world: &mut World, p, block| -> Result<()> {
        ensure!(world.admit_edit_chunk(world::chunk_coord_from_world_block(p)), "Fixture chunk unavailable");
        world.set(p, block);
        Ok(())
    };
    let stand = |player: &mut Player, p: IVec3| {
        player.pos = p.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
        player.precise_pos = player.pos.as_dvec3();
    };
    let cross_chart = |source: &mut World, player: &mut Player, day: &DayNightCycle| -> Result<World> {
        ensure!(hall_gate::usable(source, player), "Gate unusable in {:?} at {:?}", source.world_type, player.pos);
        let mut saves = storage::SaveService::resident();
        let mut travel = hall_gate::Travel::default();
        travel.request(&mut saves);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if !travel.saving_destination() { saves.tick(Some(source), player, day); }
            if let Some(world) = travel.poll(source, player, day, &saves, |_, _, _| Ok(()))? {
                return Ok(world);
            }
            ensure!(Instant::now() < deadline, "Realm checkpoint timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    for (kind, gate, at) in [
        (RealmKind::Highfold, BlockId::HighfoldGate, floor),
        (RealmKind::Gloaming, BlockId::GloamingGate, floor + IVec3::X * 20),
    ] {
        place(&mut surface, at, gate)?;
        stand(&mut player, at);
        if kind == RealmKind::Gloaming {
            ensure!(!hall_gate::usable(&surface, &player), "Gloaming gate opened without woods");
            for x in [-4, 4] {
                for y in 1..=6 {
                    place(&mut surface, at + IVec3::new(x, y, 0), BlockId::Log)?;
                }
            }
        } else {
            place(&mut surface, at + IVec3::Y * 8, BlockId::Stone)?;
            ensure!(!hall_gate::usable(&surface, &player), "Highfold gate opened under a roof");
            place(&mut surface, at + IVec3::Y * 8, BlockId::Air)?;
        }
        let mut realm = cross_chart(&mut surface, &mut player, &day)?;
        ensure!(realm.world_type.realm() == Some(kind), "Arrived in the wrong realm");
        eprintln!("[realm-review] entered {kind:?} at {:?}", player.pos);
        let spawn_y = player.pos.y as i32;
        let return_gate = (spawn_y - 10..spawn_y + 10).rev()
            .map(|y| IVec3::new(0, y, 0))
            .find(|&p| realm.get(p) == gate).context("Generated return gate missing")?;
        let harvest = if kind == RealmKind::Highfold {
            let cfg = realm.sample_config();
            let cell = (-25..=25).flat_map(|x| (-25..=25).map(move |z| (x, z)))
                .find_map(|(x, z)| (spawn_y..spawn_y + 15).map(|y| IVec3::new(x, y, z))
                    .find(|&p| engine_worldgen::sample_block(&cfg, p) == BlockId::Leaves))
                .context("No generated Highfold leaves near arrival")?;
            place(&mut realm, cell, BlockId::Air)?;
            lightless_life::tick(&mut realm, &player, &day, 1.0 / 60.0);
            Some(cell)
        } else {
            None
        };
        stand(&mut player, return_gate);
        let parent_name = surface.regions.view.session_name().to_owned();
        surface = cross_chart(&mut realm, &mut player, &day)?;
        ensure!(surface.regions.view.session_name() == parent_name, "Returned to the wrong parent checkpoint");
        ensure!(lightless_gate::realm_under(&surface, &player) == Some(kind), "Returned to the wrong gate");
        eprintln!("[realm-review] returned from {kind:?} at {:?}", player.pos);
        if let Some(cell) = harvest {
            day.set_day_index(day.day_index() + 5);
            let mut revisited = cross_chart(&mut surface, &mut player, &day)?;
            ensure!(revisited.admit_edit_chunk(world::chunk_coord_from_world_block(cell)), "Harvest chunk unavailable");
            ensure!(revisited.get(cell) == BlockId::Air, "Harvest was not persisted");
            lightless_life::tick(&mut revisited, &player, &day, 1.0 / 60.0);
            ensure!(revisited.get(cell) == BlockId::Leaves, "Highfold leaves did not regrow on return");
            eprintln!("[realm-review] Highfold leaves {cell:?} regrew after five away days");
            stand(&mut player, return_gate);
            surface = cross_chart(&mut revisited, &mut player, &day)?;
        }
    }
    Ok(())
}

pub(super) fn fixture() -> Result<()> {
    ensure!(std::env::var_os("VOXEL_SAVE_DIR").is_some(), "Set a disposable VOXEL_SAVE_DIR");
    ensure!(storage::load_named("RealmReview")?.is_none(), "RealmReview already exists");
    let mut source = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 74);
    let spawn = source.suggested_spawn().floor().as_ivec3();
    let mut min = spawn;
    min.y = pad_top(&source.sample_config(), min) + 13;
    let local = aperture(min);
    let mut cfg = source.sample_config();
    cfg.world_type = WorldType::Nether;
    cfg.generator_version = hell_gate::built::VERSION;
    cfg.planet = Some(source.sample_config().planet_settings().unwrap_or_default());
    let remote = aperture(IVec3::new(min.x, pad_top(&cfg, min) + 13, min.z));
    let gate = hell_gate::built::Threshold { local, remote, generator: cfg.generator_version };
    gate.validate(1.0)?;
    let mut cells = local.ring();
    for x in min.x - 2..=min.x + 5 {
        for z in min.z - 3..=min.z + 3 {
            cells.push(IVec3::new(x, min.y - 1, z));
        }
    }
    cells.sort_unstable_by_key(|p| p.to_array());
    cells.dedup();
    let mut edits = Vec::new();
    for cell in cells {
        ensure!(source.admit_edit_chunk(world::chunk_coord_from_world_block(cell)), "Fixture chunk unavailable");
        ensure!(source.get(cell) == BlockId::Air, "Fixture frame obstructed");
        edits.push((cell, BlockId::Basalt));
    }
    world::effects::dry_material_batch(&mut source, &edits, &mut HashSet::new(), &mut HashSet::new())
        .map_err(|failure| anyhow::anyhow!("Fixture frame: {failure:?}"))?;
    source.portals.hell.push(gate);
    let mut player = Player::new();
    player.pos = min.as_vec3() + Vec3::new(2.0, 0.0, 1.5);
    player.precise_pos = player.pos.as_dvec3();
    let source_feet = player.pos;
    source.adventure.begin_journey();
    source.ensure_starter_wand();
    source.sync_from_inventory();
    let day = DayNightCycle::default();
    let roundtrip = std::env::args().any(|arg| arg == "--realm-review-roundtrip");
    let mut destination = cross(&mut source, &mut player, &day, roundtrip.then_some(local))?;
    if roundtrip {
        ensure!(destination.world_type == WorldType::Nether, "Expected Hell after entering the surface gate");
        eprintln!("[portal-verify] Hell entered at {:?}", player.pos);
        let returned = cross(&mut destination, &mut player, &day, Some(remote))?;
        ensure!(returned.regions.view.session_name() == source.regions.view.session_name(), "Returned to the wrong surface save");
        eprintln!("[portal-verify] Hell returned at {:?}", player.pos);
        return Ok(());
    }
    storage::name_world(source.regions.view.session_name(), "RealmReview")?;
    eprintln!("[realm-review] surface={} hell={} gate={},{},{}",
        source.regions.view.session_name(), destination.regions.view.session_name(),
        source_feet.x, source_feet.y, source_feet.z);
    Ok(())
}

fn cross(source: &mut World, player: &mut Player, day: &DayNightCycle, threshold: Option<Aperture>) -> Result<World> {
    let mut saves = storage::SaveService::resident();
    let mut travel = hell_gate::Travel::default();
    if let Some(aperture) = threshold {
        let plane = aperture.frame(1.0, true);
        ensure!(!travel.crossing(source, player), "Travel fired before the sweep");
        let feet = player.pos.as_dvec3();
        player.pos = (feet - plane.normal() * (plane.local(feet).z * 2.0)).as_vec3();
        player.precise_pos = player.pos.as_dvec3();
        ensure!(travel.crossing(source, player), "Walking through the threshold did not start travel");
    }
    travel.request(&mut saves);
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if !travel.saving_destination() { saves.tick(Some(source), player, day); }
        if let Some(world) = travel.poll(source, player, day, &saves, |_, _, _| Ok(()))? {
            return Ok(world);
        }
        ensure!(Instant::now() < deadline, "Fixture checkpoint timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn aperture(min: IVec3) -> Aperture {
    Aperture { min, max: min + IVec3::new(3, 3, 0), axis: 2 }
}

fn pad_top(cfg: &engine_voxel::WorldSampleConfig, center: IVec3) -> i32 {
    (center.x - 2..=center.x + 5).flat_map(|x| {
        (center.z - 3..=center.z + 3).map(move |z| engine_worldgen::sample_column(cfg, x, z).height)
    }).max().unwrap_or(center.y)
}
