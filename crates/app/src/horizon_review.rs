//! Opt-in benchmark diagnostics. Never opens or modifies a player's save.
use crate::{
    player::Player,
    world::{self, World, WorldSampleConfig},
};
use glam::{IVec3, Vec3};
use std::{
    collections::HashSet,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::{Mutex, OnceLock},
};

#[cfg(test)]
#[test]
#[ignore = "bounded authoritative source diagnostic for the captured ridge approach"]
fn probe_approach_hill_materials() {
    let cfg = WorldSampleConfig {
        generator_version: 25, seed: 2_738_958_700,
        world_type: world::WorldType::Hills,
        voxel_scale: crate::fluid::VoxelScalePreset::OneMeter,
    };
    let eye = Vec3::new(2_581.863, 53.6200, -3893.5686);
    let (yaw, pitch) = (0.600041_f32, -0.2_f32);
    let forward = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    let right = forward.cross(Vec3::Y).normalize();
    let up = right.cross(forward);
    let tangent = (70.0_f32.to_radians() * 0.5).tan();
    for (px, py) in [(360, 280), (320, 292), (220, 325), (180, 340)] {
        let nx = (2.0 * (px as f32 + 0.5) / 1280.0 - 1.0) * (1280.0 / 720.0) * tangent;
        let ny = (1.0 - 2.0 * (py as f32 + 0.5) / 720.0) * tangent;
        let ray = (forward + right * nx + up * ny).normalize();
        let mut hit = None;
        // A source ray is a locator, not a claim of exact proxy raster coverage.
        for metres in 1..=2000 {
            let p = eye + ray * metres as f32;
            let x = p.x.floor() as i32;
            let z = p.z.floor() as i32;
            let (height, material) = world::distant_surface(&cfg, x, z);
            if p.y <= height {
                let surface = material.surface();
                println!("[approach-material-probe] pixel=({px},{py}) distance_m={metres} x={x} z={z} height={height} material={material:?} block_id={} roughness={} legacy_specular={} scope=generated_surface_ray_not_raster_identity", material as u16, surface.roughness, surface.specular);
                hit = Some((x, z));
                break;
            }
        }
        assert!(hit.is_some(), "bounded source ray missed at ({px},{py})");
    }
}

/// CPU data diagnostics only; no window, GPU, renderer or player save.
pub fn probe_only(cfg: WorldSampleConfig) -> std::io::Result<()> {
    let root = std::env::var("VOXEL_CAPTURE_DIR").unwrap_or_else(|_| "captures".into());
    probe(cfg, Path::new(&root))
}

pub fn frame(
    player: &mut Player,
    world: &mut World,
    elapsed: f32,
    dirty_blocks: &mut HashSet<IVec3>,
    dirty_chunks: &mut HashSet<IVec3>,
) {
    static PROBE: OnceLock<()> = OnceLock::new();
    if crate::terrain_review::option("VOXEL_HORIZON_PROBE") {
        PROBE.get_or_init(|| {
            let cfg = world.sample_config();
            let root = std::env::var("VOXEL_CAPTURE_DIR").unwrap_or_else(|_| "captures".into());
            // Diagnostic cost is intentionally excluded from ordinary timing runs.
            if let Err(e) = std::thread::Builder::new()
                .name("horizon-probe".into())
                .spawn(move || {
                    if let Err(e) = probe(cfg, Path::new(&root)) {
                        eprintln!("[horizon-probe] failed: {e}");
                    }
                })
            {
                eprintln!("[horizon-probe] spawn failed: {e}");
            }
        });
    }
    edit_frame(world, elapsed, dirty_blocks, dirty_chunks);
    if !crate::terrain_review::option("VOXEL_HORIZON_ROUTE") {
        return;
    }
    let t = elapsed.max(0.0);
    let scale = world.voxel_scale.meters();
    let origin = player.pos * scale;
    let shore = Vec3::new(1653.0-origin.x, 0.0, -1725.0-origin.z);
    let ridge = Vec3::new(2602.0-origin.x, 0.0, -3923.0-origin.z);
    let shore_back = shore - shore.normalize_or_zero()*120.0;
    let ridge_back = ridge - ridge.normalize_or_zero()*120.0;
    let facing = |p:Vec3| p.x.atan2(-p.z)-player.yaw;
    let smooth = |v:f32| { let v=v.clamp(0.0,1.0); v*v*(3.0-2.0*v) };
    let (offset,yaw,pitch,phase,ground_blend) = if t<20.0 {
        (Vec3::ZERO,0.0,0.0,0,0.0)
    } else if t<45.0 {
        let u=(t-20.0)/25.0; (shore_back*u,facing(shore)*smooth(u*5.0),0.0,1,smooth(u))
    } else if t<65.0 {
        (shore_back+shore.normalize_or_zero()*(t-45.0)*6.0,facing(shore),0.0,2,1.0)
    } else if t<85.0 {
        (shore-shore.normalize_or_zero()*(t-65.0)*6.0,facing(shore),0.0,3,1.0)
    } else if t<110.0 {
        let u=(110.0-t)/25.0; (shore_back*u,facing(shore)*smooth(u*5.0),0.0,4,smooth(u))
    } else if t<135.0 {
        let u=(t-110.0)/25.0; (ridge_back*u,facing(ridge)*smooth(u*5.0),0.0,5,smooth(u))
    } else if t<155.0 {
        (ridge_back+ridge.normalize_or_zero()*(t-135.0)*6.0,facing(ridge),0.0,6,1.0)
    } else if t<175.0 {
        (ridge-ridge.normalize_or_zero()*(t-155.0)*6.0,facing(ridge),0.0,7,1.0)
    } else if t<200.0 {
        let u=(200.0-t)/25.0; (ridge_back*u,facing(ridge)*smooth(u*5.0),0.0,8,smooth(u))
    } else if t<215.0 {
        let turn=((t-200.0)/3.0).floor() as i32%2;
        (Vec3::Y*120.0,turn as f32*std::f32::consts::PI,-0.12,10,0.0)
    } else if t<235.0 {
        let forest=Vec3::new(-origin.x,0.0,-3448.0-origin.z);
        (forest*((t-215.0)/20.0),facing(forest),0.0,11,0.0)
    } else if t<255.0 {
        (Vec3::new(65536.0+(t-235.0)*80.0,180.0,-65536.0),std::f32::consts::PI,-0.12,12,0.0)
    } else { (Vec3::ZERO,0.0,0.0,9,0.0) };
    let original_y=player.pos.y;
    player.pos += offset/scale;
    if ground_blend>0.0 {
        let ground=world.terrain_height_at(player.pos.x.floor() as i32,player.pos.z.floor() as i32) as f32+1.0;
        let water=world.sample_config().water_level() as f32+1.0-crate::mesh::WATER_SURFACE_INSET;
        player.pos.y=original_y+(ground.max(water)+3.0/scale-original_y)*ground_blend;
    }
    player.yaw += yaw;
    player.pitch += pitch;
    static PHASE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
    if PHASE.swap(phase, std::sync::atomic::Ordering::Relaxed) != phase {
        let speed_mps=match phase {
            1=>shore_back.length()/25.0, 2|6=>6.0, 3|7=>-6.0,
            4=>-shore_back.length()/25.0, 5=>ridge_back.length()/25.0,
            8=>-ridge_back.length()/25.0, 11=>Vec3::new(origin.x,0.0,origin.z+3448.0).length()/20.0,
            12=>80.0, _=>0.0,
        };
        eprintln!(
            "[horizon-route] phase={phase} elapsed={t:.2} signed_horizontal_speed_mps={speed_mps} feet_m={:?} eye_m={:?} yaw={} pitch={}",
            player.pos * scale,
            player.eye(world.voxel_scale) * scale,
            player.yaw,
            player.pitch
        );
    }
}

/// Bounded synthetic structure, shore excavation, actual tree removal, restore.
/// Generated benchmark only; cold-save reload still needs separate evidence.
fn edit_frame(
    world: &mut World,
    elapsed: f32,
    dirty_blocks: &mut HashSet<IVec3>,
    dirty_chunks: &mut HashSet<IVec3>,
) {
    if !crate::terrain_review::option("VOXEL_HORIZON_EDITS") || elapsed < 30.0 {
        return;
    }
    struct Edit {
        original: Vec<(IVec3, world::BlockId, u8)>,
        ground: i32,
        stage: i32,
    }
    static STATE: OnceLock<Mutex<Option<Edit>>> = OnceLock::new();
    let mut state = STATE.get_or_init(|| Mutex::new(None)).lock().unwrap();
    let cfg = world.sample_config();
    if state.is_none() {
        let scale = cfg.voxel_meters();
        let x = (1653.0 / scale).round() as i32;
        let z = (-1725.0 / scale).round() as i32;
        let ground = world::terrain_height(&cfg, x, z).max(cfg.water_level() + 1);
        let mut original = Vec::new();
        for dz in -2..2 {
            for dx in -2..2 {
                for dy in -4..20 {
                    let p = IVec3::new(x + dx, ground + dy, z + dz);
                    original.push((p, world.get(p), 0));
                }
            }
        }
        // Pick an actual shore-height candidate, then open a bounded cut into
        // its bed. Geometry and water must continue to agree while in flight.
        let (sx, sz, sh) = (3..=18)
            .map(|i| {
                let d = i as f32 * 128.0;
                let sx = (d * 0.4_f32.sin() / scale).round() as i32;
                let sz = ((-120.0 - d * 0.4_f32.cos()) / scale).round() as i32;
                (sx, sz, world::terrain_height(&cfg, sx, sz))
            })
            .min_by_key(|&(_, _, h)| (h - cfg.water_level()).abs())
            .unwrap();
        for dz in -3..3 {
            for dx in -3..3 {
                for dy in -6..2 {
                    let p = IVec3::new(sx + dx, sh + dy, sz + dz);
                    original.push((p, world.get(p), 1));
                }
            }
        }
        let cell = cfg.blocks_for_span(12.0);
        let mut tree = None;
        'search: for distance in [384.0, 1024.0, 2304.0, 4608.0] {
            for bearing in -4..=4 {
                let angle = 0.4 + bearing as f32 * 0.10;
                let cx = ((angle.sin() * distance / scale).floor() as i32).div_euclid(cell);
                let cz =
                    (((-120.0 - angle.cos() * distance) / scale).floor() as i32).div_euclid(cell);
                for dz in -2..=2 {
                    for dx in -2..=2 {
                        if let Some(candidate) = world::tree_visual(&cfg, cx + dx, cz + dz) {
                            tree = Some(candidate);
                            break 'search;
                        }
                    }
                }
            }
        }
        if let Some(tree) = tree {
            let base = (tree.base / scale).floor().as_ivec3();
            let radius = (tree.radius / scale).ceil() as i32 + 1;
            let height = ((tree.height + tree.crown_height) / scale).ceil() as i32 + 2;
            let cells = (radius * 2 + 1).pow(2) as usize * height as usize;
            if cells <= 65536 {
                let mut removed = 0;
                for dz in -radius..=radius {
                    for dx in -radius..=radius {
                        for dy in 0..height {
                            let p = base + IVec3::new(dx, dy, dz);
                            let b = world.get(p);
                            if matches!(
                                b,
                                world::BlockId::Log
                                    | world::BlockId::Leaves
                                    | world::BlockId::PineLeaves
                            ) {
                                original.push((p, b, 2));
                                removed += 1;
                            }
                        }
                    }
                }
                eprintln!(
                    "[horizon-edit] tree_base={base:?} removed_voxels={removed} source=canonical_placement"
                );
            } else {
                eprintln!("[horizon-edit] tree removal skipped cells={cells} limit=65536");
            }
        } else {
            eprintln!(
                "[horizon-edit] no authoritative tree in bounded search; tree acceptance missing"
            );
        }
        // A target can overlap the small shore cut. Keep one original and one
        // owner per voxel so restore is deterministic even in that case.
        original.sort_unstable_by_key(|(p, _, _)| (p.x, p.y, p.z));
        original.dedup_by_key(|(p, _, _)| *p);
        *state = Some(Edit {
            original,
            ground,
            stage: -1,
        });
    }
    let edit = state.as_mut().unwrap();
    let stage = if elapsed < 55.0 { 0 } else if elapsed < 260.0 { 1 } else { 2 };
    if edit.stage == stage {
        return;
    }
    let changes: Vec<_> = edit
        .original
        .iter()
        .map(|&(p, block, kind)| {
            (
                p,
                match stage {
                    0 if kind == 0 && p.y > edit.ground => world::BlockId::StoneBricks,
                    0 if kind > 0 => world::BlockId::Air,
                    0 => block,
                    1 => world::BlockId::Air,
                    _ => block,
                },
            )
        })
        .collect();
    world.set_many(&changes);
    for &(p, _) in &changes {
        dirty_blocks.insert(p);
        world::mark_chunk_and_neighbors_dirty(dirty_chunks, p);
    }
    edit.stage = stage;
    eprintln!(
        "[horizon-edit] stage={stage} blocks={} revision={} target={:?}",
        changes.len(),
        world.revision(),
        changes[0].0
    );
}

fn probe(cfg: WorldSampleConfig, root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let mut candidates = BufWriter::new(File::create(root.join("region-candidates.csv"))?);
    let mut visibility = BufWriter::new(File::create(root.join("visibility.csv"))?);
    let mut coasts = BufWriter::new(File::create(root.join("coast-candidates.csv"))?);
    writeln!(
        coasts,
        "x_m,z_m,min_height_m,max_height_m,wet_samples,dry_samples,terrain_ray_clear,probe_span_m"
    )?;
    writeln!(
        visibility,
        "target_kind,x_m,y_m,z_m,eye_x_m,eye_y_m,eye_z_m,source_terrain_occluded,min_ray_clearance_m,ray_steps,step_bound_m,target_submerged,scope"
    )?;
    writeln!(
        candidates,
        "seed,generator,scale_m,x_m,z_m,height_m,material,trees_of_25,canopy_top_m,shore_distance_m"
    )?;
    let scale = cfg.voxel_meters();
    let sea = (cfg.water_level() as f32 + 1.0 - crate::mesh::WATER_SURFACE_INSET) * scale;
    let mut ridge: Option<(i32, i32, f32)> = None;
    let mut shore: Option<(i32, i32, f32)> = None;
    let mut forest: Option<(i32, i32, i32)> = None;
    let mut config = BufWriter::new(File::create(root.join("probe-config.txt"))?);
    writeln!(
        config,
        "config={cfg:?}\nwater_surface_m={sea}\nsource=generated_columns\nvisibility=see_visibility_csv_generated_terrain_only\ncache_check=evaluator_order_only_not_retained_horizon_cache\ncurrent_horizon_sampler=exact_through_64m_ring_step_else_fixed_8m_preview\ncurrent_horizon_cache_policy=exact_0_fixed8_1\nsource_proxy_csv=varying_footprint_diagnostic_not_actual_per_region_ring_policy\nsource_excludes=edits,overhangs,caves,structures\ncandidate_camera=0,-120,100,0.4,-0.2\nfootprint_units=metres"
    )?;
    config.flush()?;
    // Bearings stay inside the fixed camera's horizontal FOV. These are
    // candidates, not proof of visibility: intervening terrain can occlude them.
    for distance in [384.0, 640.0, 1024.0, 1536.0, 2304.0, 3328.0, 4608.0] {
        for bearing in -4..=4 {
            let angle = 0.4 + bearing as f32 * 0.10;
            let x = (angle.sin() * distance / scale).round() as i32;
            let z = ((-120.0 - angle.cos() * distance) / scale).round() as i32;
            let (height, block) = world::distant_surface(&cfg, x, z);
            let h = height * scale;
            let cell = cfg.blocks_for_span(12.0);
            let mut trees = 0;
            let mut top = h;
            let mut crown = None;
            for dz in -2..=2 {
                for dx in -2..=2 {
                    if let Some(tree) =
                        world::tree_visual(&cfg, x.div_euclid(cell) + dx, z.div_euclid(cell) + dz)
                    {
                        trees += 1;
                        let crown_top = tree.base.y + tree.height + tree.crown_height;
                        if crown_top > top {
                            top = crown_top;
                            crown = Some(Vec3::new(tree.base.x, crown_top, tree.base.z));
                        }
                    }
                }
            }
            let coastal = (h - sea).abs();
            let terrain_visible = visibility_row(
                &mut visibility,
                cfg,
                "terrain",
                Vec3::new(x as f32 * scale, h, z as f32 * scale),
                sea,
            )?;
            let crown_visible = if let Some(crown) = crown {
                visibility_row(&mut visibility, cfg, "canonical_tree_top", crown, sea)?
            } else {
                false
            };
            let (mut min_h, mut max_h, mut wet, mut dry) = (f32::INFINITY, f32::NEG_INFINITY, 0, 0);
            for dz in -2..=2 {
                for dx in -2..=2 {
                    let ph = world::distant_surface(
                        &cfg,
                        x + dx * cfg.blocks_for_span(32.0),
                        z + dz * cfg.blocks_for_span(32.0),
                    )
                    .0 * scale;
                    min_h = min_h.min(ph);
                    max_h = max_h.max(ph);
                    if ph < sea {
                        wet += 1;
                    } else {
                        dry += 1;
                    }
                }
            }
            writeln!(
                coasts,
                "{},{},{min_h},{max_h},{wet},{dry},{terrain_visible},128",
                x as f32 * scale,
                z as f32 * scale
            )?;
            writeln!(
                candidates,
                "{},{},{scale},{},{},{h},{block:?},{trees},{top},{coastal}",
                cfg.seed,
                cfg.generator_version,
                x as f32 * scale,
                z as f32 * scale
            )?;
            if terrain_visible && h >= sea && ridge.is_none_or(|(_, _, score)| h > score) {
                ridge = Some((x, z, h));
            }
            let coast_score =
                if (cfg.world_x(x) - 1653.0).abs() < 2.0 && (cfg.world_z(z) + 1725.0).abs() < 2.0 {
                    -1.0
                } else {
                    coastal
                };
            if terrain_visible
                && wet > 0
                && dry > 0
                && shore.is_none_or(|(_, _, score)| coast_score < score)
            {
                shore = Some((x, z, coast_score));
            }
            if crown_visible && trees > 0 && forest.is_none_or(|(_, _, score)| trees > score) {
                forest = Some((x, z, trees));
            }
        }
    }
    candidates.flush()?;
    visibility.flush()?;
    coasts.flush()?;
    let mut out = BufWriter::new(File::create(root.join("source-proxy.csv"))?);
    let mut canonical = BufWriter::new(File::create(root.join("canonical-sampler.csv"))?);
    writeln!(
        canonical,
        "region,x_m,z_m,current_cache_policy,source_height_m,current_height_m,source_material,current_material,order_stable,scope"
    )?;
    writeln!(
        out,
        "region,x_m,z_m,footprint_m,source_height_m,proxy_height_m,height_error_m,source_material,proxy_material,order_stable,source_wet,proxy_wet"
    )?;
    for (name, target) in [
        ("ridge", ridge.map(|(x, z, _)| (x, z))),
        ("shore", shore.map(|(x, z, _)| (x, z))),
        ("forest", forest.map(|(x, z, _)| (x, z))),
    ] {
        let Some((cx, cz)) = target else {
            eprintln!("[horizon-probe] no {name} candidate in bounded search");
            continue;
        };
        eprintln!(
            "[horizon-probe] candidate={name} x_m={} z_m={} span_m=256 visibility=unverified",
            cx as f32 * scale,
            cz as f32 * scale
        );
        for dz in -2..=2 {
            for dx in -2..=2 {
                let x = cx + dx * cfg.blocks_for_span(32.0);
                let z = cz + dz * cfg.blocks_for_span(32.0);
                let (h, block) = world::distant_surface(&cfg, x, z);
                for footprint in [8.0, 16.0, 32.0, 64.0, 128.0, 256.0] {
                    let (ph, pb) = world::landscape::preview_surface_lod(&cfg, x, z, footprint);
                    // A/B/A checks order independence without averaging material IDs.
                    let _ = world::landscape::preview_surface_lod(
                        &cfg,
                        x,
                        z,
                        if footprint == 256.0 { 64.0 } else { 256.0 },
                    );
                    let again = world::landscape::preview_surface_lod(&cfg, x, z, footprint);
                    let stable = ph.to_bits() == again.0.to_bits() && pb == again.1;
                    writeln!(
                        out,
                        "{name},{},{},{footprint},{},{},{},{block:?},{pb:?},{stable},{},{}",
                        x as f32 * scale,
                        z as f32 * scale,
                        h * scale,
                        ph * scale,
                        (ph - h) * scale,
                        h * scale < sea,
                        ph * scale < sea
                    )?;
                }
                let policy = if cfg.world_x(x).abs().max(cfg.world_z(z).abs()) <= 96.0 * 64.0 {
                    0
                } else {
                    1
                };
                let evaluate = || {
                    if policy == 0 {
                        world::distant_surface(&cfg, x, z)
                    } else {
                        world::landscape::preview_surface_lod(&cfg, x, z, 8.0)
                    }
                };
                let current = evaluate();
                let again = evaluate();
                writeln!(
                    canonical,
                    "{name},{},{},{policy},{},{},{block:?},{:?},{},hybrid_point_evaluator_not_mesh_interpolation_or_retained_cache",
                    x as f32 * scale,
                    z as f32 * scale,
                    h * scale,
                    current.0 * scale,
                    current.1,
                    current.0.to_bits() == again.0.to_bits() && current.1 == again.1
                )?;
            }
        }
    }
    out.flush()?;
    canonical.flush()?;
    eprintln!(
        "[horizon-probe] complete source=generated_columns edits=excluded visibility=unverified"
    );
    Ok(())
}

fn visibility_row(
    out: &mut impl Write,
    cfg: WorldSampleConfig,
    kind: &str,
    target: Vec3,
    sea: f32,
) -> std::io::Result<bool> {
    // Actual bookmarked capture eye, supplied by the native lighting review.
    // These bounded rays test generated terrain occlusion, not raster visibility
    // or obstruction by other trees, structures, atmosphere or persistent edits.
    let eye = Vec3::new(0.0, 132.0, -120.0);
    let distance = (target - eye).length();
    let steps = (distance / 16.0).ceil().clamp(2.0, 512.0) as usize;
    let scale = cfg.voxel_meters();
    let mut clearance = f32::INFINITY;
    for i in 1..steps {
        let p = eye.lerp(target + Vec3::Y * 0.05, i as f32 / steps as f32);
        let height = world::distant_surface(
            &cfg,
            (p.x / scale).floor() as i32,
            (p.z / scale).floor() as i32,
        )
        .0 * scale;
        clearance = clearance.min(p.y - height);
    }
    writeln!(
        out,
        "{kind},{},{},{},0,132,-120,{},{},{},{},{},generated_terrain_only_16m_sampling_not_raster_acceptance",
        target.x,
        target.y,
        target.z,
        clearance < 0.0,
        clearance,
        steps,
        distance / steps as f32,
        target.y < sea
    )?;
    Ok(clearance >= 0.0)
}
