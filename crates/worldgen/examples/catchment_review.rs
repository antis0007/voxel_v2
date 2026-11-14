//! Offline comparison of two regional erosion solves on the spherical planet.
use anyhow::{Context, Result, ensure};
use engine_core::VoxelScalePreset;
use engine_worldgen::planetary::review::catchment::{self, Catchment, SPAN_M};
use engine_worldgen::{WorldSampleConfig, WorldType};
use glam::{DVec2, DVec3};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args.next().context(
        "usage: catchment_review OUTPUT_DIRECTORY [SEED] [--finer] [--extreme | --rock-package [--weathering | --jointed [--sediment-history]]] [--half-step] [--parent VERSION] | measure SPACING HEIGHT [SEDIMENT OUTLET_MASK] | braid OUTPUT_DIRECTORY [SEED]",
    )?;
    if output == "measure" {
        return measure(args);
    }
    if output == "replay-valley" {
        let sills = match args.next().as_deref() {
            Some("257") => false,
            Some("265") => true,
            _ => anyhow::bail!("expected preview 257 or 265"),
        };
        let spacing = args.next().context("missing spacing")?.parse()?;
        let datum = args.next().context("missing rock datum")?.parse()?;
        let seed = args.next().context("missing seed")?.parse()?;
        let rock = raster(args.next().context("missing bedrock raster")?)?;
        let soil = raster(args.next().context("missing sediment raster")?)?;
        let path = args.next().context("missing output raster")?;
        ensure!(args.next().is_none(), "too many replay arguments");
        let height = catchment::replay_valley(rock, soil, spacing, datum, seed, sills)
            .context("expected finite bedrock and soil rasters for a 4 km window")?;
        let mut out = BufWriter::new(File::create_new(path)?);
        for h in height { out.write_all(&h.to_le_bytes())?; }
        return Ok(out.flush()?);
    }
    if output == "whole" {
        let spacing = args.next().context("missing spacing")?.parse()?;
        let target = args.next().context("missing target area in km²")?.parse()?;
        let mut height = raster(args.next().context("missing height raster")?)?;
        if let Some(path) = args.next().filter(|p| p != "-") {
            let soil = raster(path)?;
            ensure!(soil.len() == height.len(), "raster sizes differ");
            for (h, s) in height.iter_mut().zip(soil) { *h += s; }
        }
        let outlet = args.next().map(|v| v.parse()).transpose()?;
        let (m, outlet) = catchment::measure_whole(&height, spacing, target, outlet)
            .context("no enclosed upstream catchment for that outlet or area")?;
        println!("outlet_cell={outlet}");
        print_measurement(&m);
        return Ok(());
    }
    if output == "braid" {
        return braid(args);
    }
    if output == "live-trails" {
        let seed = args.next().context("missing seed")?.parse()?;
        let generator_version = args.next().map_or(
            Ok(engine_worldgen::planetary::CATCHMENT_EXPLORATION_GENERATOR), |value| value.parse(),
        )?;
        let cfg = WorldSampleConfig {
            seed, generator_version,
            planet: None, world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter,
        };
        let paths = catchment::playable_trails(&cfg).context("no surveyed alternate trails")?;
        for (i, path) in paths.iter().enumerate() {
            let gravel = path.iter().filter(|p| {
                engine_worldgen::sample_column(&cfg, p.x.floor() as i32, p.y.floor() as i32).surface
                    == engine_content::BlockId::Gravel
            }).count();
            let mut clearance = 0;
            let mut blocked = 0;
            let mut largest_step = 0;
            let mut last = None::<i32>;
            for pair in path.windows(2) {
                let n = pair[0].distance(pair[1]).ceil() as u32;
                for k in 0..n {
                    let p = pair[0].lerp(pair[1], f64::from(k) / f64::from(n));
                    let (x, z) = (p.x.floor() as i32, p.y.floor() as i32);
                    let h = engine_worldgen::sample_column(&cfg, x, z).height;
                    clearance += 1;
                    let blocks: [_; 7] = std::array::from_fn(|i| {
                        engine_worldgen::sample_block(&cfg, glam::IVec3::new(x, h + i as i32 - 2, z))
                    });
                    let open = |b: engine_content::BlockId| !b.is_solid()
                        && !matches!(b, engine_content::BlockId::Water | engine_content::BlockId::Lava);
                    let floor = blocks.windows(3).enumerate().rev().find_map(|(i, b)| {
                        let y = h + i as i32 - 2;
                        (b[0].is_solid() && open(b[1]) && open(b[2])).then_some(y)
                    });
                    if let Some(y) = floor {
                        if let Some(prev) = last { largest_step = largest_step.max((y - prev).abs()); }
                        last = Some(y);
                    } else {
                        if blocked < 8 { println!("blocked at {x},{z} column={blocks:?}"); }
                        blocked += 1;
                    }
                }
            }
            println!("live trail={} points={} marked={} length_m={:.0} start={:?} end={:?} clearance_samples={clearance} blocked={blocked} max_step_voxels={largest_step}",
                i + 1, path.len(), gravel, length(path), path.first(), path.last());
            let waypoints = path.iter().map(|p| format!("{:.3},{:.3}", p.x, p.y)).collect::<Vec<_>>();
            println!("route_{}={}", i + 1, waypoints.join(";"));
        }
        return Ok(());
    }
    let output = Path::new(&output);
    let seed = args.next().map_or(Ok(74), |v| v.parse::<u32>())?;
    let mut finer = false;
    let mut extreme = false;
    let mut rock_package = false;
    let mut weathering = false;
    let mut jointed = false;
    let mut explore = false;
    let mut valley_form = false;
    let mut sill_erosion = false;
    let mut sediment_history = false;
    let mut half_step = false;
    let mut generator_version = engine_voxel::PLANET_GENERATOR_VERSION;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--finer" => finer = true,
            "--extreme" => extreme = true,
            "--rock-package" => rock_package = true,
            "--weathering" => weathering = true,
            "--jointed" => jointed = true,
            "--explore" => explore = true,
            "--valley-form" => valley_form = true,
            "--sill-erosion" => sill_erosion = true,
            "--sediment-history" => sediment_history = true,
            "--half-step" => half_step = true,
            "--parent" => {
                generator_version = args.next().context("missing parent version")?.parse()?
            }
            _ => anyhow::bail!("unknown option {arg}"),
        }
    }
    ensure!(!(extreme && rock_package), "choose one canyon profile");
    ensure!(!weathering || rock_package, "weathering requires --rock-package");
    ensure!(!jointed || (rock_package && !weathering), "jointed requires --rock-package and includes weathering");
    ensure!(!explore || jointed, "exploration requires --jointed");
    ensure!(!sediment_history || (jointed && !explore), "sediment history requires --jointed without --explore");
    ensure!(!(valley_form || sill_erosion) || (jointed && !sediment_history), "valley erosion requires --jointed");
    ensure!(!(valley_form && sill_erosion), "choose one valley profile");
    fs::create_dir(output).with_context(|| format!("create {}", output.display()))?;
    let cfg = WorldSampleConfig {
        seed,
        generator_version,
        planet: None,
        world_type: WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let started = std::time::Instant::now();
    let basins = if sill_erosion {
        vec![catchment::solve_sill_erosion(&cfg, finer, half_step).context("no inland canyon window for this seed")?]
    } else if valley_form {
        vec![catchment::solve_valley_form(&cfg, finer, half_step).context("no inland canyon window for this seed")?]
    } else if sediment_history {
        vec![catchment::solve_sediment_weathering_canyon(&cfg, finer, half_step)
            .context("no inland canyon window for this seed")?]
    } else if jointed {
        vec![catchment::solve_jointed_canyon_with_routes(&cfg, finer, half_step, explore)
            .context("no inland canyon window for this seed")?]
    } else if rock_package {
        vec![
            catchment::solve_rock_canyon_with_weathering(&cfg, finer, half_step, weathering)
                .context("no inland canyon window for this seed")?,
        ]
    } else {
        Vec::from(
            catchment::solve_pair_with(&cfg, finer, extreme, half_step)
                .context("no pair of inland, contrasting catchment windows for this seed")?,
        )
    };
    let mut report = BufWriter::new(File::create(output.join("report.txt"))?);
    writeln!(
        report,
        "seed={seed} generator={} extreme={extreme} rock_package={rock_package} weathering={} jointed={jointed} sediment_history={sediment_history} valley_form={valley_form} sill_erosion={sill_erosion} half_step={half_step} solve_seconds={:.2}",
        cfg.generator_version,
        weathering || jointed,
        started.elapsed().as_secs_f64()
    )?;
    for (name, basin) in ["canyon", "valley"].into_iter().zip(&basins) {
        if let Some([abraded, entrained, exported]) = basin.history_wind_m3 {
            writeln!(report, "wind_abraded_m3={abraded:.3} wind_entrained_m3={entrained:.3} wind_exported_m3={exported:.3}")?;
        }
        if let Some([abraded, regolith, solute]) = basin.history_process_m3 {
            writeln!(report, "fluvial_abrasion_m3={abraded:.3} weathered_regolith_m3={regolith:.3} dissolved_rock_export_m3={solute:.3}")?;
        }
        export(output, name, basin, &mut report)?;
    }
    if rock_package {
        sections(output, &basins[0])?;
    }
    if let Some(paths) = basins[0].trail_points() {
        let mut trails = BufWriter::new(File::create(output.join("trails.csv"))?);
        writeln!(trails, "trail,local_x_m,local_z_m,elevation_m")?;
        for (i, path) in paths.iter().enumerate() {
            let mut low = f64::INFINITY;
            let mut high = f64::NEG_INFINITY;
            for &point in path {
                let local = basins[0].frame.local(point);
                let h = basins[0].sample(point).context("trail outside solved region")?.height_m;
                low = low.min(h);
                high = high.max(h);
                writeln!(trails, "{},{:.2},{:.2},{h:.2}", i + 1, local.x, local.y)?;
            }
            writeln!(report, "trail={} length_m={:.0} elevation_min_m={low:.1} elevation_max_m={high:.1}",
                i + 1, length(path))?;
        }
    }
    report.flush()?;
    overview(&output.join("overview.bmp"), &basins)?;
    println!("{}", output.join("report.txt").display());
    Ok(())
}

fn length(path: &[DVec2]) -> f64 {
    path.windows(2).map(|p| p[0].distance(p[1])).sum()
}

fn braid(mut args: impl Iterator<Item = String>) -> Result<()> {
    let output = args.next().context("missing braid output directory")?;
    let output = Path::new(&output);
    let seed = args.next().map_or(Ok(74), |v| v.parse::<u32>())?;
    ensure!(args.next().is_none(), "too many braid arguments");
    fs::create_dir(output).with_context(|| format!("create {}", output.display()))?;
    let reach = catchment::braid::evolve(seed, 180.0);
    for (name, field) in [
        ("start", &reach.start_m),
        ("bed", &reach.bed_m),
        ("flow", &reach.flow_m3_s),
    ] {
        let mut out = BufWriter::new(File::create(output.join(format!("{name}.f32")))?);
        for &v in field {
            out.write_all(&v.to_le_bytes())?;
        }
        out.flush()?;
    }
    let report = format!(
        "seed={seed} grid={}x{} spacing_m={} outlet_flow_m3_s={:.6} sediment_residual_m3={:.9}\n",
        catchment::braid::ACROSS,
        catchment::braid::DOWNSTREAM,
        catchment::braid::SPACING_M,
        reach.outlet_flow_m3_s,
        reach.sediment_residual_m3,
    );
    fs::write(output.join("report.txt"), &report)?;
    print!("{report}");
    Ok(())
}

fn sections(output: &Path, basin: &Catchment) -> Result<()> {
    let mut out = BufWriter::new(File::create(output.join("sections.csv"))?);
    writeln!(
        out,
        "z_m,x_m,surface_m,bedrock_m,unit,contact_1_m,contact_2_m,contact_3_m,contact_4_m,contact_5_m"
    )?;
    for z in [960.0, 1984.0, 3104.0] {
        for x in (2..4000).step_by(4) {
            let x = f64::from(x);
            let column = basin
                .sample(basin.frame.chart(x, z))
                .context("section left canyon window")?;
            let rock = column.height_m - column.sediment_m;
            let unit = basin
                .rock_unit_at(x, rock, z)
                .context("missing rock package")?;
            let [a, b, c, d, e] = basin.rock_contacts(x, z).context("missing rock contacts")?;
            writeln!(
                out,
                "{z},{x},{:.3},{rock:.3},{unit},{a:.3},{b:.3},{c:.3},{d:.3},{e:.3}",
                column.height_m
            )?;
        }
    }
    out.flush()?;
    Ok(())
}

fn raster(path: String) -> Result<Vec<f32>> {
        let bytes = fs::read(&path).with_context(|| format!("read {path}"))?;
        let chunks = bytes.chunks_exact(4);
        ensure!(
            chunks.remainder().is_empty(),
            "{path} is not a float32 raster"
        );
        Ok(chunks
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect())
}

fn measure(mut args: impl Iterator<Item = String>) -> Result<()> {
    let spacing = args.next().context("missing spacing in metres")?.parse()?;
    let mut height = raster(args.next().context("missing height raster")?)?;
    let mask = if let Some(soil) = args.next() {
        let soil = raster(soil)?;
        let mask = raster(args.next().context("missing outlet mask")?)?;
        ensure!(
            height.len() == soil.len() && height.len() == mask.len(),
            "raster sizes differ"
        );
        for (h, s) in height.iter_mut().zip(soil) {
            *h += s;
        }
        Some(mask.into_iter().map(|v| v == 1.0).collect::<Vec<_>>())
    } else {
        None
    };
    ensure!(args.next().is_none(), "too many measurement arguments");
    let m = catchment::measure(&height, spacing, mask.as_deref()).context(
        "expected a finite 4 km elevation raster sampled at 2 m or 4 m and a valid outlet",
    )?;
    print_measurement(&m);
    Ok(())
}

fn print_measurement(m: &catchment::Measurements) {
    let display = |v: Option<f64>| v.map_or_else(|| "NA".into(), |v| format!("{v:.2}"));
    println!(
        "basin_area_km2={:.3} channel_heads_per_km2={:.2} ridge_spacing_m={}",
        m.basin_area_km2,
        m.head_density_km2,
        display(m.ridge_spacing_m)
    );
    println!(
        "width_m_0.03_to_0.10_km2={} width_m_0.10_to_0.40_km2={} width_m_over_0.40_km2={} sample_counts={:?}",
        display(m.width_m[0]),
        display(m.width_m[1]),
        display(m.width_m[2]),
        m.width_samples
    );
    println!(
        "main_stem_m={:.0} profile_breaks={} median_prominence_m={} peaks_over_10m={}",
        m.main_stem_m,
        m.profile_breaks,
        display(m.prominence_m),
        m.prominent_peaks
    );
}

fn export(output: &Path, name: &str, basin: &Catchment, out: &mut impl Write) -> Result<()> {
    if let Some([rain, recharge, runoff]) = basin.history_water_m3_year {
        let residual = rain - recharge - runoff;
        writeln!(
            out,
            "geologic_domain_rain_m3_year={rain:.6} recharge_m3_year={recharge:.6} runoff_export_m3_year={runoff:.6} residual_m3_year={residual:.6}"
        )?;
    }
    if let Some([initial, remap, eroded, exported, stored]) = basin.history_sediment_m3 {
        let residual = initial + remap + eroded - exported - stored;
        writeln!(
            out,
            "geologic_domain_initial_soil_m3={initial:.3} remap_m3={remap:.3} eroded_rock_m3={eroded:.3} sediment_export_m3={exported:.3} stored_m3={stored:.3} residual_m3={residual:.6}"
        )?;
    }
    let b = &basin.budget;
    let rock_to_sediment = b.initial_soil_m3 + b.refinement_soil_m3 + b.eroded_rock_m3;
    let sediment_residual = rock_to_sediment - b.exported_sediment_m3 - b.final_soil_m3;
    let water_residual = b.rainfall_m3_year - b.exported_runoff_m3_year;
    let height = |i: usize| basin.bedrock_m[i] + basin.sediment_m[i];
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    let mut gained = 0.0;
    let mut lost = 0.0;
    let mut cells = 0;
    let mut exposed = 0;
    let mut thickened = 0;
    let area = basin.spacing_m.powi(2);
    for (i, selected) in basin.outlet_catchment.iter().copied().enumerate() {
        if !selected {
            continue;
        }
        cells += 1;
        low = low.min(height(i));
        high = high.max(height(i));
        let change = height(i) - basin.initial_m[i];
        gained += f64::from(change.max(0.0)) * area;
        lost += f64::from(-change.min(0.0)) * area;
        exposed += usize::from(basin.sediment_m[i] < 0.2);
        thickened += usize::from(change > 0.3);
    }
    ensure!(
        cells > 0 && low.is_finite() && high.is_finite(),
        "{name} has an invalid outlet catchment"
    );
    writeln!(
        out,
        "\n{name} spacing_m={} grid={}x{} outlet_target_m={:.3} parent_weighted_area_km2={:.3}",
        basin.spacing_m,
        basin.side,
        basin.side,
        basin.outlet_m,
        basin.parent_runoff_area_m2 / 1e6
    )?;
    writeln!(
        out,
        "chart_origin_xz={:?} chart_across_xz={:?} chart_downstream_xz={:?}",
        basin.frame.origin.to_array(),
        basin.frame.across.to_array(),
        basin.frame.downstream.to_array()
    )?;
    writeln!(
        out,
        "outlet_area_km2={:.3} relief_m={:.2} exposed_rock_pct={:.2} raised_over_0.3m_pct={:.2}",
        cells as f64 * area / 1e6,
        high - low,
        100.0 * exposed as f64 / cells as f64,
        100.0 * thickened as f64 / cells as f64
    )?;
    writeln!(
        out,
        "outlet_surface_gain_m3={gained:.3} outlet_surface_loss_m3={lost:.3}"
    )?;
    writeln!(
        out,
        "window_rainfall_m3_year={:.6} window_runoff_export_m3_year={:.6} runoff_residual_m3_year={water_residual:.6}",
        b.rainfall_m3_year, b.exported_runoff_m3_year
    )?;
    writeln!(
        out,
        "window_soil_initial_m3={:.3} soil_refinement_change_m3={:.3} bedrock_eroded_m3={:.3} sediment_export_m3={:.3} soil_final_m3={:.3} sediment_residual_m3={sediment_residual:.3}",
        b.initial_soil_m3,
        b.refinement_soil_m3,
        b.eroded_rock_m3,
        b.exported_sediment_m3,
        b.final_soil_m3
    )?;
    let chart = basin.frame.chart(2000.0, 3500.0);
    let column = |voxel_m: f64| {
        let at = (chart / voxel_m).round() * voxel_m;
        basin
            .sample(at)
            .map(|s| (s.height_m / voxel_m).round() * voxel_m)
    };
    writeln!(
        out,
        "sample_chart_xz={:?} height_m={:.3} quantized_1m={:?} quantized_0.25m={:?}",
        chart.to_array(),
        basin
            .sample(chart)
            .context("sample lies outside basin")?
            .height_m,
        column(1.0),
        column(0.25)
    )?;

    // Row-major float32 little-endian cache. The report records the
    // transform from cell centres to the planet's chart.
    let fields = ["initial", "bedrock", "sediment", "runoff", "outlet"];
    for field in fields {
        let mut file = BufWriter::new(File::create(output.join(format!("{name}-{field}.f32")))?);
        for i in 0..basin.bedrock_m.len() {
            let value = match field {
                "initial" => basin.initial_m[i],
                "bedrock" => basin.bedrock_m[i],
                "sediment" => basin.sediment_m[i],
                "runoff" => basin.runoff_m3_year[i] as f32,
                _ => f32::from(u8::from(basin.outlet_catchment[i])),
            };
            file.write_all(&value.to_le_bytes())?;
        }
        file.flush()?;
    }
    Ok(())
}

/// Three columns show starting relief, solved relief and soil/drainage.
/// Both rows use one light direction, one distance scale and unit exaggeration.
fn overview(path: &Path, basins: &[Catchment]) -> Result<()> {
    const PANEL: usize = 500;
    const WIDTH: usize = 3 * PANEL;
    let height = PANEL * basins.len();
    let mut out = BufWriter::new(File::create(path)?);
    // 24-bit BMP, with top-down rows. WIDTH * 3 is already four-byte aligned.
    out.write_all(b"BM")?;
    out.write_all(&(54 + (WIDTH * height * 3) as u32).to_le_bytes())?;
    out.write_all(&[0; 4])?;
    for word in [54, 40, WIDTH as i32, -(height as i32)] {
        out.write_all(&word.to_le_bytes())?;
    }
    out.write_all(&[1, 0, 24, 0])?;
    out.write_all(&[0; 24])?;
    let light = DVec3::new(-0.62, 0.69, -0.37).normalize();
    let pixel_m = SPAN_M / PANEL as f64;
    for basin in basins {
        let at = |p: DVec2| {
            let p = (p / basin.spacing_m).clamp(DVec2::ZERO, DVec2::splat((basin.side - 1) as f64));
            p.y as usize * basin.side + p.x as usize
        };
        for z in 0..PANEL {
            for panel in 0..3 {
                for x in 0..PANEL {
                    let p = DVec2::new(x as f64 + 0.5, z as f64 + 0.5) * pixel_m;
                    let height = |p| {
                        let i = at(p);
                        f64::from(if panel == 0 {
                            basin.initial_m[i]
                        } else {
                            basin.bedrock_m[i] + basin.sediment_m[i]
                        })
                    };
                    let dx = (height(p + DVec2::X * pixel_m) - height(p - DVec2::X * pixel_m))
                        / (2.0 * pixel_m);
                    let dz = (height(p + DVec2::Y * pixel_m) - height(p - DVec2::Y * pixel_m))
                        / (2.0 * pixel_m);
                    let normal = DVec3::new(-dx, 1.0, -dz).normalize();
                    let shade = 0.26 + 0.74 * light.dot(normal).max(0.0);
                    let i = at(p);
                    let mut rgb = [228.0, 226.0, 218.0];
                    if panel == 2 {
                        let soil = (f64::from(basin.sediment_m[i]) / 4.0).clamp(0.0, 1.0);
                        rgb = [
                            217.0 - soil * 35.0,
                            171.0 + soil * 35.0,
                            127.0 + soil * 12.0,
                        ];
                        if !basin.outlet_catchment[i] {
                            rgb = [153.0, 153.0, 153.0];
                        } else if basin.runoff_m3_year[i] >= 24_000.0 {
                            rgb = [62.0, 157.0, 224.0];
                        }
                    }
                    let [r, g, b] = rgb.map(|v| (v * shade) as u8);
                    out.write_all(&[b, g, r])?;
                }
            }
        }
    }
    out.flush()?;
    Ok(())
}
