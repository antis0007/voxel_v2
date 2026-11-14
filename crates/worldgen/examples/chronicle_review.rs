//! Native generation audit; CSVs contain actual survey and transition records.
use anyhow::{Context, Result, ensure};
use engine_content::BlockId;
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, civilization as civ};
use glam::Vec3;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::Path,
    time::Instant,
};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args
        .next()
        .context("usage: chronicle_review OUTPUT_DIRECTORY [SEED]")?;
    let seed = args.next().map_or(Ok(74), |v| v.parse::<u32>())?;
    let dir = Path::new(&output);
    fs::create_dir_all(dir)?;
    let mut cfg = WorldSampleConfig {
        seed,
        generator_version: civ::CHRONICLE_GENERATOR,
        world_type: WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter,
        planet: None,
    };
    let x = args.next().map_or(Ok(0.0), |v| v.parse::<f32>())?;
    let z = args.next().map_or(Ok(0.0), |v| v.parse::<f32>())?;
    let t = Instant::now();
    let landscape_hash = engine_worldgen::planetary::regional_view(&cfg).checksum;
    let geology_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let sites = civ::settlements_near(&cfg, Vec3::new(x, 0.0, z), 1600.0);
    let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
    ensure!(
        !sites.is_empty(),
        "no admitted sites in the survey; enlarge or move the review window"
    );
    let mut records = BTreeMap::new();
    let mut survey = File::create(dir.join("sites.csv"))?;
    let mut events = File::create(dir.join("events.csv"))?;
    let mut lore = File::create(dir.join("archives.txt"))?;
    writeln!(
        survey,
        "id,x_m,y_m,z_m,rock,flood_risk_ppm,sediment_ppm,scree_mm,founded,peak_households,households,abandoned,record_hash,cache_x,cache_y,cache_z"
    )?;
    writeln!(
        events,
        "event_id,site,year,cause,peer,founded,households_before,households_after,stores_before,stores_after,harvest,demand,discarded,unmet,aid_received,aid_sent,migrants_in,migrants_out,shortage_departures,growth"
    )?;
    let mut scale_checks = 0;
    let mut roof_checks = 0;
    let mut voxel_hash = 0xcbf29ce484222325_u64;
    for s in &sites {
        let c = civ::generation_record(&cfg, s.id).context("missing chronicle")?;
        ensure!(
            c.replay().map_err(anyhow::Error::msg)? == *c,
            "replay disagrees with baked history"
        );
        let r = c
            .sites
            .iter()
            .flatten()
            .find(|r| r.id == s.id)
            .context("missing site provenance")?;
        let p = civ::archive_position(&cfg, s.id).context("missing archive position")?;
        let g = r.landscape;
        let h = s.history;
        writeln!(
            survey,
            "{},{},{},{},{:?},{},{},{},{},{},{},{:?},{:016x},{},{},{}",
            s.id,
            s.center.x,
            s.center.y,
            s.center.z,
            g.bedrock,
            g.flood_risk_ppm,
            g.sediment_ppm,
            g.scree_mm,
            h.founded,
            h.peak_households,
            h.households,
            h.abandoned,
            c.fingerprint(),
            p.x,
            p.y,
            p.z
        )?;
        let text =
            civ::archive_text(&cfg, p, BlockId::SurveyCache).context("missing found record")?;
        ensure!(
            text == c.archive_text(s.id).context("missing record text")?,
            "archive disagrees with history"
        );
        writeln!(lore, "{} @ {},{},{}\n{}\n", s.id, p.x, p.y, p.z, text)?;
        // This exercises the production voxel sampler, archive selector and loot path.
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            cfg.voxel_scale = scale;
            let same = civ::generation_record(&cfg, s.id).context("scale lost site")?;
            ensure!(*same == *c, "physical history changed with voxel scale");
            let mut roofs = Vec::new();
            civ::visit_skylines(
                &cfg,
                glam::Vec2::new(s.center.x, s.center.z),
                25.0,
                &std::sync::atomic::AtomicBool::new(false),
                |ring, _, _, _| roofs.push(ring[8]),
            );
            ensure!(
                roofs.len() == h.households as usize,
                "skyline count disagrees with surviving households"
            );
            for roof in roofs {
                let cell = ((roof - Vec3::Y * (scale.meters() * 0.5)) / scale.meters())
                    .floor()
                    .as_ivec3();
                ensure!(
                    engine_worldgen::sample_block(&cfg, cell) == BlockId::Planks,
                    "inhabited roof disagrees with skyline: seed={seed}, site={}, scale={scale:?}",
                    s.id
                );
                roof_checks += 1;
            }
            let p = civ::archive_position(&cfg, s.id).context("scale lost archive")?;
            let block = engine_worldgen::sample_block(&cfg, p);
            ensure!(
                block == BlockId::SurveyCache,
                "archive not physically generated: seed={seed} site={} scale={scale:?} p={p:?} block={block:?}",
                s.id
            );
            ensure!(
                engine_worldgen::ruins::cache_loot(&cfg, p, block).is_some(),
                "archive has no loot route"
            );
            ensure!(
                civ::archive_text(&cfg, p, block).as_deref() == Some(text.as_str()),
                "scale changed found record"
            );
            voxel_hash = (voxel_hash
                ^ block as u64
                ^ p.x as u64
                ^ (p.y as u64).rotate_left(13)
                ^ (p.z as u64).rotate_left(27))
            .wrapping_mul(0x100000001b3);
            scale_checks += 1;
        }
        cfg.voxel_scale = VoxelScalePreset::OneMeter;
        records.insert(c.provenance.region, c);
    }
    let mut digest = 0;
    let mut event_count = 0;
    for c in records.values() {
        ensure!(
            c.events.len() <= civ::chronicle::MAX_EVENTS,
            "unbounded history"
        );
        digest ^= c.fingerprint();
        event_count += c.events.len();
        for e in &c.events {
            writeln!(
                events,
                "{},{},{},{},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                e.id,
                e.site,
                e.year,
                e.cause.label(),
                e.peer,
                e.founded,
                e.households_before,
                e.households_after,
                e.stores_before,
                e.stores_after,
                e.harvest,
                e.demand,
                e.surplus_discarded,
                e.unmet_demand,
                e.aid_received,
                e.aid_sent,
                e.migrants_in,
                e.migrants_out,
                e.shortage_departures,
                e.growth
            )?;
        }
    }
    let t = Instant::now();
    for _ in 0..1000 {
        for s in &sites {
            std::hint::black_box(civ::generation_record(&cfg, s.id));
        }
    }
    let warm_us = t.elapsed().as_secs_f64() * 1e6 / (1000 * sites.len()) as f64;
    let manifest = format!(
        "{{\n  \"seed\": {seed}, \"generator\": {}, \"model_revision\": {},\n  \"deep_time_myr\": 240, \"history_years\": 1000, \"step_years\": 20,\n  \"landscape_checksum\": \"{landscape_hash:016x}\", \"record_digest\": \"{digest:016x}\", \"voxel_digest\": \"{voxel_hash:016x}\",\n  \"sites\": {}, \"regions\": {}, \"events\": {event_count}, \"scale_checks\": {scale_checks},\n  \"geology_ms\": {geology_ms:.3}, \"cold_survey_ms\": {cold_ms:.3}, \"warm_record_us\": {warm_us:.3}\n}}\n",
        civ::CHRONICLE_GENERATOR,
        civ::chronicle::MODEL_REVISION,
        sites.len(),
        records.len()
    );
    fs::write(dir.join("manifest.json"), &manifest)?;
    fs::write(
        dir.join("roof-checks.txt"),
        format!("{roof_checks} inhabited roof samples matched native voxels and skyline plans\n"),
    )?;
    print!("{manifest}");
    Ok(())
}
