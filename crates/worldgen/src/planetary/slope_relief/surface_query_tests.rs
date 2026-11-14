//! V13 Stage oracle follows the original per-call path, not SurfaceQuery.
use super::*;
#[path = "surface_query_reference.rs"] mod reference;

fn same(a:&Stage,b:&Stage) {
    let Stage { grid,rock_delta,soil,base }=a;
    assert_eq!(grid.n,b.grid.n);assert_eq!(grid.cell_m.to_bits(),b.grid.cell_m.to_bits());
    assert_eq!(grid.min_m.map(f64::to_bits),b.grid.min_m.map(f64::to_bits));
    let bits=|v:&[f32]|v.iter().map(|v|v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(rock_delta),bits(&b.rock_delta));assert_eq!(bits(soil),bits(&b.soil));
    assert_eq!(base.as_ref().map(|v|bits(v)),b.base.as_ref().map(|v|bits(v)));
}
fn key(world:&World,cfg:&WorldSampleConfig)->Key {
    let dir=world.mesh.directions.iter().copied().find(|&dir| {
        let v=world.volcano_shape(dir);let g=world.ground_query_reference(cfg,dir,8.0,v);
        let (_,s,t)=Projection::Tangent.from_sphere(dir);
        s.abs().max(t.abs())<0.85 && !v.active_caldera && world.edifice_lift(dir)<=12.0
            && exposure(&g,crate::planetary::watershed_erosion(cfg.generator_version),
                crate::planetary::rifted_margins(cfg.generator_version)||crate::planetary::connected_relief(cfg.generator_version),
                cfg.generator_version==crate::planetary::LAYERED_CANYON_GENERATOR,
                crate::planetary::evolved_watersheds(cfg.generator_version))>0.1
    }).expect("exposed tile");
    let (face,s,t)=Projection::Tangent.from_sphere(dir);
    Key { face:face as u8,x:(s*atlas::HALF_M/CORE_M).floor() as i32,z:(t*atlas::HALF_M/CORE_M).floor() as i32 }
}
fn chain(world:&World,cfg:&WorldSampleConfig,key:Key) {
    let (mut expected,mut actual)=(None,None);
    for (spacing,passes) in [(32.0,8),(16.0,7),(8.0,8),(4.0,5)] {
        crate::planetary::HISTORY_ROCK.with(|m|*m.borrow_mut()=None);
        let old=Stage::solve_surface_query_reference(world,cfg,key,spacing,passes,expected.as_ref(),false);
        crate::planetary::HISTORY_ROCK.with(|m|*m.borrow_mut()=None);
        let new=Stage::solve(world,cfg,key,spacing,passes,actual.as_ref(),false);
        same(&old,&new); expected=Some(old);actual=Some(new);
    }
}
#[test]
#[ignore="actual full Stage chain; isolated AXIOM_SURFACE_QUERY_VERSION/SEED"]
fn surface_query_matches_frozen_stages_with_parents_and_missing_history() {
    let cfg=crate::planetary::surface_query::tests::config();
    let mut world=crate::planetary::build(&cfg,&geo::Progress::default(),true);
    let key=key(&world,&cfg);chain(&world,&cfg,key);
    if crate::planetary::geological_history(cfg.generator_version) {
        world.history_top.clear();world.slope_tiles=Tiles::default();
        chain(&world,&cfg,key);
    }
    eprintln!("[surface-query-stage-exact] version={} seed={} full_fields=true parents=true",
        cfg.generator_version,cfg.seed);
}
