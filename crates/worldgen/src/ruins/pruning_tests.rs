use super::*;
fn frozen_near(cfg: &WorldSampleConfig, m: Vec3, radius: f32) -> Vec<Rc<District>> {
    if !enabled(cfg) {
        return Vec::new();
    }
    let (x0, x1) = (((m.x - radius) / CELL).round() as i32, ((m.x + radius) / CELL).round() as i32);
    let (z0, z1) = (((m.z - radius) / CELL).round() as i32, ((m.z + radius) / CELL).round() as i32);
    let mut found = Vec::new();
    for iz in z0..=z1 {
        for ix in x0..=x1 {
            if let Some(d) = district_at(cfg, ix as f32 * CELL, iz as f32 * CELL)
                && d.centre.distance(Vec2::new(m.x, m.z)) < radius
            {
                found.push(d);
            }
        }
    }
    found
}
fn frozen_route(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    if !line::active(cfg) {
        return 0.0;
    }
    let m = glam::Vec3::new(x, 0.0, z);
    frozen_near(cfg, m, 2.0 * CELL)
        .iter()
        .map(|d| {
            let q = d.local(glam::Vec3::new(x, d.datum, z));
            d.approaches.iter()
                .filter_map(|a| a.offset(Vec2::new(q.x, q.z), 0.0))
                .map(|(_, across)| 1.0 - across / urban::AVENUE)
                .fold(0.0, f32::max)
        })
        .fold(0.0, f32::max)
}
#[test]
#[ignore = "actual planet and full district survey reference; run alone"]
fn pruned_queries_match_frozen_actual_districts() {
    let version = std::env::var("AXIOM_RUINS_VERSION").unwrap_or_else(|_| "287".into()).parse().unwrap();
    let scale = match std::env::var("AXIOM_RUINS_SCALE").as_deref() {
        Ok("quarter") => engine_core::VoxelScalePreset::QuarterMeter,
        Ok("1m") | Err(_) => engine_core::VoxelScalePreset::OneMeter,
        _ => panic!("scale must be1m orquarter"),
    };
    let cfg = WorldSampleConfig { seed:74,generator_version:version,world_type:crate::WorldType::Planet,planet:None,voxel_scale:scale };
    if let Some(p)=std::env::var_os("AXIOM_RUINS_CACHE") {
        let p=std::path::PathBuf::from(p); assert!(p.is_absolute());
        assert!(crate::planetary::configure_geology_cache(p));
    }
    let _world=crate::planetary::regional_view(&cfg);
    for p in [Vec2::new(5338.0,1101.0),Vec2::new(70520.0,10997.0)] {
        for radius in [0.0,512.0,2048.0] {
            let m=Vec3::new(p.x,0.0,p.y);
            let old=frozen_near(&cfg,m,radius);
            let new=near(&cfg,m,radius);
            assert_eq!(old.len(),new.len());
            for (a,b) in old.iter().zip(&new) {
                // Same cached Rc proves the entire returned district, not only centre.
                assert!(Rc::ptr_eq(a,b));
            }
        }
        assert_eq!(frozen_route(&cfg,p.x,p.y).to_bits(),line::route_proximity(&cfg,p.x,p.y).to_bits());
    }
}
#[test]
fn prefilter_preserves_district_coordinate_roundtrip_aliases() {
    let cfg=WorldSampleConfig { seed:74,generator_version:287,world_type:crate::WorldType::Planet,
        planet:None,voxel_scale:engine_core::VoxelScalePreset::OneMeter };
    let indices=[i32::MIN,i32::MIN+1,-33_554_433,-16_777_217,-16_777_216,-1,0,1,
        16_777_215,16_777_216,16_777_217,33_554_433,i32::MAX-1,i32::MAX];
    let mut aliases=0;
    for ix in indices { for iz in indices {
        let sample_x=ix as f32*CELL;
        let sample_z=iz as f32*CELL;
        // Frozen district_at key expression, with the exact passed arguments.
        let original_key=((sample_x/CELL).round() as i32,(sample_z/CELL).round() as i32);
        let sampled_key=((sample_x/CELL).round() as i32,(sample_z/CELL).round() as i32);
        assert_eq!(sampled_key,original_key);
        aliases+=usize::from(sampled_key!=(ix,iz));
        let original=candidate(&cfg,original_key.0,original_key.1);
        let prefilter=candidate(&cfg,sampled_key.0,sampled_key.1);
        assert_eq!(original.map(|p|p.to_array().map(f32::to_bits)),
            prefilter.map(|p|p.to_array().map(f32::to_bits)));
    }}
    assert!(aliases>0,"fixture must exercise real f32 cell aliases");
}