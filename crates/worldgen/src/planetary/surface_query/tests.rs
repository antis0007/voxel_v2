//! The expected paths never call SurfaceQuery or the prepared World methods.
use super::*;
use super::super::*;
#[path = "ground_reference.rs"] mod ground_reference;
#[path = "history_reference.rs"] mod history_reference;

fn clear_memo() { HISTORY_ROCK.with(|m| *m.borrow_mut()=None); }
fn same_column(a:&Option<drift::history::RockColumn>,b:&Option<drift::history::RockColumn>) {
    match (a,b) {
        (Some(a),Some(b)) => {
            let drift::history::RockColumn { beds,top,lava_m,carbonate,weathered_myr }=a;
            beds.assert_history_stencil_bits_eq(&b.beds);
            assert_eq!([*top,*lava_m,*carbonate,*weathered_myr].map(f64::to_bits),
                [b.top,b.lava_m,b.carbonate,b.weathered_myr].map(f64::to_bits));
        }
        (None,None)=>{}, _=>panic!("column presence")
    }
}
fn ground_bits(g:geo::Ground)->Vec<u32> {
    let geo::Ground { top_m,slope,temperature,moisture,sediment,placer,river_gravel,
        river_bedrock,maat_c,permafrost,water_m,riparian,delta,delta_mud,
        suspended,intertidal,scree_m, deposit_m, deposit_bed_slope, deposit_bed_top, outlook_clearing, .. }=g;
    let mut out=[top_m,slope,temperature,moisture,sediment,placer,maat_c,
        riparian,delta,delta_mud,suspended,intertidal,scree_m, deposit_m, deposit_bed_slope, deposit_bed_top, outlook_clearing].map(f32::to_bits).to_vec();
    out.extend([u32::from(river_gravel),u32::from(river_bedrock),u32::from(permafrost),
        u32::from(water_m.is_some()),water_m.unwrap_or(0.0).to_bits()]); out
}
fn same_shaped(a:crate::volcano::Shaped,b:crate::volcano::Shaped) {
    let crate::volcano::Shaped { metres,lake,slope,heat,crust,active_caldera }=a;
    assert_eq!([metres,slope,heat].map(f64::to_bits),[b.metres,b.slope,b.heat].map(f64::to_bits));
    assert_eq!(lake.map(|(h,f)|(h.to_bits(),f)),b.lake.map(|(h,f)|(h.to_bits(),f)));
    assert_eq!(crust,b.crust); assert_eq!(active_caldera,b.active_caldera);
}
pub(in crate::planetary) fn config()->WorldSampleConfig {
    if let Some(path)=std::env::var_os("AXIOM_SURFACE_QUERY_GEO_CACHE") {
        assert!(configure_geology_cache(path.into()));
    }
    let version=std::env::var("AXIOM_SURFACE_QUERY_VERSION").unwrap_or_else(|_|"287".into()).parse().unwrap();
    assert!([275,277,278,279,280,283,286,287].contains(&version));
    WorldSampleConfig { generator_version:version,
        seed:std::env::var("AXIOM_SURFACE_QUERY_SEED").unwrap_or_else(|_|"74".into()).parse().unwrap(),
        world_type:WorldType::Planet,voxel_scale:engine_core::VoxelScalePreset::OneMeter,planet:None }
}
fn directions(world:&World)->Vec<DVec3> {
    let mut dirs:Vec<_>=world.mesh.directions.iter().step_by((world.mesh.directions.len()/96).max(1)).copied().collect();
    for face in Face::ALL { for s in [-1.0,-0.999999,0.0,0.999999,1.0] {
        for t in [-1.0,-0.999999,0.0,0.999999,1.0] { dirs.push(Projection::Tangent.to_sphere(face,s,t)); }
    }}
    for vent in world.volcanoes.vents.iter().step_by((world.volcanoes.vents.len()/24).max(1)) {
        dirs.push(vent.dir);
        let tangent=DVec3::Y.cross(vent.dir).try_normalize().unwrap_or(DVec3::X);
        dirs.push((vent.dir+tangent*(vent.crater_radius_m()*0.2/atlas::RADIUS_M)).normalize());
    }
    dirs
}
fn history(world:&World,cfg:&WorldSampleConfig,dir:DVec3,prewarm:bool) {
    clear_memo(); let expected=world.history_query_reference(cfg,dir);
    clear_memo(); let point=world.surface_query(dir);
    assert!(point.stencil.get().is_none());
    if prewarm { same_shaped(point.volcano_shape(),world.volcano_shape(dir)); }
    let actual=point.history_strata(cfg); same_column(&actual,&expected);
    let fresh=world.surface_query(dir);
    same_column(&fresh.history_strata(cfg),&expected);
    // A memo hit (or an empty/missing history path) must not sample the World.
    assert!(fresh.stencil.get().is_none()); assert!(fresh.edifice.get().is_none());
    assert!(fresh.volcanic.get().is_none());
    HISTORY_ROCK.with(|memo| {
        if let Some((key,column))=&*memo.borrow() {
            assert_eq!(key.1,dir.to_array().map(f64::to_bits));
            same_column(&Some(*column),&expected);
        } else { assert!(expected.is_none()); }
    });
}
fn check(world:&World,cfg:&WorldSampleConfig,dir:DVec3) {
    let point=world.surface_query(dir);
    assert!(point.stencil.get().is_none());
    let volcanic=point.volcano_shape();
    same_shaped(volcanic,world.volcano_shape(dir));
    assert_eq!(point.volcanic_base().to_bits(),world.volcanic_base(dir).to_bits());
    assert_eq!(point.edifice_lift().to_bits(),world.edifice_lift(dir).to_bits());
    for footprint in [0.25,1.0,8.0,32.0,128.0] {
        let expected=world.ground_query_reference(cfg,dir,footprint,volcanic);
        assert_eq!(ground_bits(point.ground_base(cfg,footprint,volcanic)),ground_bits(expected));
        assert_eq!(ground_bits(world.ground_base(cfg,dir,footprint,volcanic)),ground_bits(expected));
    }
    assert_eq!(point.collision().to_bits(),world.mesh.sample(&world.collision,dir).to_bits());
    if !world.sediment_cover.is_empty() {
        assert_eq!(point.sediment_cover().to_bits(),world.mesh.sample(&world.sediment_cover,dir).to_bits());
    }
    if !world.fault_damage.is_empty() {
        assert_eq!(point.fault_damage().to_bits(),world.mesh.sample(&world.fault_damage,dir).to_bits());
    }
    let before=point.stencil.get().unwrap();
    let independently=world.mesh.stencil(dir);
    assert_eq!(before.0,independently.0);
    assert_eq!(before.1.map(f64::to_bits),independently.1.map(f64::to_bits));
    history(world,cfg,dir,true);
}
#[test]
#[ignore="actual World/full Ground/history; isolated AXIOM_SURFACE_QUERY_VERSION/SEED"]
fn surface_query_matches_frozen_ground_history_and_lazy_returns() {
    let cfg=config(); let mut world=build(&cfg,&geo::Progress::default(),true);
    let dirs=directions(&world);
    for &dir in &dirs { check(&world,&cfg,dir); }
    // The empty optional-edifice fast path is distinct from a zero sample.
    let old=std::mem::take(&mut world.edifice_lift);
    check(&world,&cfg,dirs[0]); world.edifice_lift=old;
    if geological_history(cfg.generator_version) {
        let n=world.height.len();
        let old_history=std::mem::replace(&mut world.history_top,vec![200.0;n]);
        let old_height=std::mem::replace(&mut world.height,vec![50.0;n]);
        let old_edifice=std::mem::replace(&mut world.edifice_lift,vec![0.0;n]);
        for (top,lift,branch) in [(50.0,0.0,"trim"),(250.0,0.0,"deposit"),(600.0,20.0,"lava")] {
            world.height.fill(top); world.edifice_lift.fill(lift);
            clear_memo(); let reference=world.history_query_reference(&cfg,dirs[0]).unwrap();
            let historical=fit_mountain_height(world.mesh.sample(&world.history_top,dirs[0]));
            match branch {
                "trim"=>assert!(reference.top-reference.lava_m<historical),
                "deposit"=>assert!(reference.top-reference.lava_m>=historical),
                _=>assert!(reference.lava_m>0.0),
            }
            history(&world,&cfg,dirs[0],true);
            history(&world,&cfg,dirs[0],false);
        }
        world.history_top=old_history; world.height=old_height; world.edifice_lift=old_edifice;
    }
    clear_memo(); let old=world.history_query_reference(&cfg,dirs[0]);
    let history_top=std::mem::take(&mut world.history_top);
    let point=world.surface_query(dirs[0]);
    assert!(point.history_strata(&cfg).is_none()); assert!(point.stencil.get().is_none());
    assert!(point.edifice.get().is_none()); assert!(point.volcanic.get().is_none());
    HISTORY_ROCK.with(|m|same_column(&m.borrow().as_ref().map(|(_,c)|*c),&old));
    world.history_top=history_top; clear_memo();
    if geological_history(cfg.generator_version) {
        world.plates.clear_drift_for_history_stencil_test();
        let point=world.surface_query(dirs[0]);
        assert!(point.history_strata(&cfg).is_none()); assert!(point.stencil.get().is_none());
        assert!(point.edifice.get().is_none()); assert!(point.volcanic.get().is_none());
        HISTORY_ROCK.with(|m|assert!(m.borrow().is_none()));
    }
    eprintln!("[surface-query-exact] version={} seed={} directions={} full_fields=true lazy_returns=true",
        cfg.generator_version,cfg.seed,dirs.len());
}
