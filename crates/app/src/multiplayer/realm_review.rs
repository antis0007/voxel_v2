//! Disposable saves for the native cross-realm review.
use super::*;
use world::{BlockId, hell_gate, portals::gates::Aperture, storage};

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
