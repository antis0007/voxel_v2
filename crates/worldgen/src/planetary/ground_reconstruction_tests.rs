use super::*;
#[path = "ground_reconstruction_reference.rs"] mod reference;

fn directions() -> Vec<DVec3> {
    let mut result=vec![DVec3::X,-DVec3::X,DVec3::Y,-DVec3::Y,DVec3::Z,-DVec3::Z];
    for a in [-1.0,1.0] { for b in [-1.0,1.0] {
        for c in [-1.0,-0.5,0.0,0.5,1.0] { for epsilon in [-1e-12,0.0,1e-12] {
            result.push(DVec3::new(a,b+epsilon,c).normalize());
            result.push(DVec3::new(c,a,b+epsilon).normalize());
            result.push(DVec3::new(b+epsilon,c,a).normalize());
        }}
    }}
    result
}

#[test]
fn ground_reconstruction_stencil_matches_literal_original() {
    for side in [4,17,84] { for irregular in [false,true] {
        let mesh=if irregular { topology::Mesh::irregular(side) } else { topology::Mesh::with_side(side) };
        let fields:Vec<Vec<f64>>=(0..4).map(|k|(0..mesh.directions.len()).map(|i|match k {
            0=>(i%131) as f64*0.03125-23.0,
            1=>if i%2==0 {-0.0} else {0.0},
            2=>((i*37%1009) as f64-504.0)*1e12,
            _=>(i%71) as f64*1e-200,
        }).collect()).collect();
        let gradients:Vec<_>=mesh.directions.iter().enumerate()
            .map(|(i,d)|*d*((i%13) as f64-6.0)).collect();
        for dir in mesh.directions.iter().copied().chain(directions()) {
            let stencil=mesh.stencil(dir);
            for field in &fields {
                let expected=reference::reconstruct(&mesh,field,&gradients,dir);
                let actual=mesh.reconstruct_height_from_stencil(field,&gradients,dir,stencil);
                let wrapper=mesh.reconstruct_height(field,&gradients,dir);
                assert_eq!(expected.to_bits(),actual.to_bits(),"side={side} irregular={irregular} dir={dir:?}");
                assert_eq!(expected.to_bits(),wrapper.to_bits());
            }
        }
    }}
}

fn bits(g:geo::Ground)->Vec<u32> {
    let geo::Ground { top_m,slope,temperature,moisture,sediment,placer,river_gravel,
        river_bedrock,maat_c,permafrost,water_m,riparian,delta,delta_mud,
        suspended,intertidal,scree_m, deposit_m, deposit_bed_slope, deposit_bed_top, outlook_clearing, .. }=g;
    let mut bits=[top_m,slope,temperature,moisture,sediment,placer,maat_c,
        riparian,delta,delta_mud,suspended,intertidal,scree_m, deposit_m, deposit_bed_slope, deposit_bed_top, outlook_clearing]
        .map(f32::to_bits).to_vec();
    bits.extend([u32::from(river_gravel),u32::from(river_bedrock),u32::from(permafrost),
        u32::from(water_m.is_some()),water_m.unwrap_or(0.0).to_bits()]);
    bits
}
fn compare(world:&World,cfg:&WorldSampleConfig,dirs:&[DVec3]) {
    for &dir in dirs { for footprint in [0.25,1.0,8.0,32.0,128.0] {
        let volcanic=world.volcano_shape(dir);
        let expected=world.ground_reconstruction_reference(cfg,dir,footprint,volcanic);
        let actual=world.ground_base(cfg,dir,footprint,volcanic);
        assert_eq!(bits(expected),bits(actual),
            "version={} seed={} dir={dir:?} footprint={footprint}",cfg.generator_version,cfg.seed);
    }}
}

#[test]
#[ignore="actual full Ground reference; separate process per version/seed; optional AXIOM_RECONSTRUCTION_GEO_CACHE"]
fn ground_reconstruction_matches_frozen_ground_and_edifice_branches() {
    if let Some(path)=std::env::var_os("AXIOM_RECONSTRUCTION_GEO_CACHE") {
        assert!(configure_geology_cache(path.into()));
    }
    let version=std::env::var("AXIOM_RECONSTRUCTION_VERSION").unwrap_or_else(|_|"287".into()).parse().unwrap();
    assert!([275,277,278,279,280,283,286,287].contains(&version));
    let cfg=WorldSampleConfig { generator_version:version,
        seed:std::env::var("AXIOM_RECONSTRUCTION_SEED").unwrap_or_else(|_|"74".into()).parse().unwrap(),
        world_type:WorldType::Planet,voxel_scale:engine_core::VoxelScalePreset::OneMeter,planet:None };
    let mut world=build(&cfg,&geo::Progress::default(),true);
    let mut dirs=directions();
    dirs.extend(world.mesh.directions.iter().step_by((world.mesh.directions.len()/256).max(1)).copied());
    for field in [&world.height,&world.edifice_lift] {
        if let Some((i,_))=field.iter().enumerate().max_by(|a,b|a.1.total_cmp(b.1)) {
            dirs.push(world.mesh.directions[i]);
        }
    }
    compare(&world,&cfg,&dirs);
    // Explicitly hit empty, zero, and positive edifice paths on reconstructed
    // highland instead of relying on a sparse volcanic probe being selected.
    let (i,_)=world.height.iter().enumerate().max_by(|a,b|a.1.total_cmp(b.1)).unwrap();
    let land=world.mesh.directions[i];
    let coarse=world.mesh.sample(&world.height,land);
    assert!(coarse>SEA+24.0,"fixture requires reconstructed highland");
    let old_lift=std::mem::take(&mut world.edifice_lift);
    let old_gradient=std::mem::take(&mut world.edifice_gradient);
    compare(&world,&cfg,&[land]); // empty check prevents all optional indexing
    world.edifice_lift=vec![0.0;world.mesh.directions.len()];
    world.edifice_gradient=vec![DVec3::ZERO;world.mesh.directions.len()];
    compare(&world,&cfg,&[land]); // nonempty but zero skips reconstruction
    world.edifice_lift.fill(12.5);
    assert!(world.edifice_lift(land)>0.0);
    compare(&world,&cfg,&[land]); // positive forces the second reconstruction
    world.edifice_lift=old_lift; world.edifice_gradient=old_gradient;
    eprintln!("[ground-reconstruction-exact] version={version} seed={} probes={} full_fields=true forced_edifice=true",
        cfg.seed,dirs.len());
}
