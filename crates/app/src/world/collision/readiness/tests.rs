use super::*;
use crate::{fluid::VoxelScalePreset, world::{ChunkData, WorldType, regions}};
use std::sync::Arc;
#[path = "reference.rs"]
mod reference;

fn fixture() -> World {
    let root = std::env::temp_dir().join(format!("axiom-collision-ready-{}-{}",
        std::process::id(),regions::checkpoint_id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut w=World::with_seed_in(WorldType::Superflat,VoxelScalePreset::OneMeter,7,&root);
    w.generator_version=287;
    for z in -2..=1 { for y in -1..=1 { for x in -2..=1 {
        w.chunks.insert(IVec3::new(x,y,z),Arc::new(ChunkData {
            blocks:Box::new([BlockId::Air;32*32*32]),light:Box::new([0;32*32*32]),
            torches:Default::default(),modified_bounds:None,structural_bounds:None,
        }));
    }}}
    w
}
fn put(w:&mut World,p:IVec3,b:BlockId) {
    let c=p.div_euclid(IVec3::splat(32));let q=p.rem_euclid(IVec3::splat(32));
    let mut edit=w.chunks.get_mut(&c).unwrap();
    let data=Arc::make_mut(&mut edit);
    data.blocks[(q.x+32*q.z+1024*q.y) as usize]=b;
}
fn bits(v:Vec3)->[u32;3] { v.to_array().map(f32::to_bits) }
fn compare(w:&World,min:Vec3,max:Vec3,origin:IVec3,picker:&mut ReadOnlyPicker) {
    let expected=reference::collides_world_local(w,min,max,origin);
    assert_eq!(collides_ready_local(w,min,max,origin,picker),
        Readiness::Ready(expected));
    assert_eq!(super::super::collides_world_local(w,min,max,origin),expected);
}

#[test]
fn ready_collisions_match_frozen_shapes_fittings_links_and_conduits() {
    let mut w=fixture();let mut picker=ReadOnlyPicker::default();
    for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::QuarterMeter] {
        w.voxel_scale=scale;
        for cell in [IVec3::new(0,10,0),IVec3::new(-32,10,-1),IVec3::new(31,10,31)] {
            for block in [BlockId::Air,BlockId::Stone,BlockId::PlankSlab,BlockId::PlankStairs,
                BlockId::PlankFence,BlockId::OakDoor,BlockId::OakTrapdoor] {
                put(&mut w,cell,block);
                for mask in [0_u8,1,5,15] {
                    for (i,d) in engine_content::shaped::FACINGS.iter().enumerate() {
                        put(&mut w,cell+*d,if mask&(1<<i)!=0 {BlockId::Stone}else{BlockId::Air});
                    }
                    for origin in [IVec3::ZERO,cell] {
                        let v=(cell-origin).as_vec3();
                        for (a,b) in [(Vec3::splat(0.01),Vec3::splat(0.1)),
                            (Vec3::splat(0.4),Vec3::splat(0.6)),
                            (Vec3::new(0.1,0.8,0.1),Vec3::new(0.9,0.95,0.9))] {
                            compare(&w,v+a,v+b,origin,&mut picker);
                        }
                    }
                }
            }
        }
    }
    w.voxel_scale=VoxelScalePreset::OneMeter;
    let p=IVec3::new(4,10,4);
    crate::world::factory::fixture_place(&mut w,p,crate::world::factory::Kind::Pipe).unwrap();
    for (a,b) in [(Vec3::splat(0.01),Vec3::splat(0.1)),(Vec3::splat(0.4),Vec3::splat(0.6))] {
        compare(&w,p.as_vec3()+a,p.as_vec3()+b,IVec3::ZERO,&mut picker);
    }
    crate::world::factory::fixture_place(&mut w,p+IVec3::X,crate::world::factory::Kind::Pipe).unwrap();
    compare(&w,p.as_vec3()+Vec3::new(0.8,0.4,0.4),p.as_vec3()+Vec3::new(0.95,0.6,0.6),IVec3::ZERO,&mut picker);
}

#[test]
fn movement_is_bit_exact_to_literal_original_across_scales_and_contacts() {
    let mut w=fixture();
    for z in -8..=8 {for x in -8..=8 {put(&mut w,IVec3::new(x,8,z),BlockId::Stone);}}
    for y in 9..=22 {for z in -8..=8 {
        put(&mut w,IVec3::new(3,y,z),BlockId::Stone);
        put(&mut w,IVec3::new(-3,y,z),BlockId::PlankSlab);
    }}
    let mut picker=ReadOnlyPicker::default();
    for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::QuarterMeter] {
        w.voxel_scale=scale;
        for crouch in [0.0,0.5,1.0] {for sneak in [false,true] {
            for delta in [Vec3::new(4.,0.,0.),Vec3::new(-4.,0.,0.),Vec3::new(0.,-2.,0.),
                Vec3::new(0.4,-0.5,0.7),Vec3::new(0.,0.,9.),Vec3::new(-0.0,0.,0.)] {
                let mut expected=Vec3::new(0.5,9.2,0.5);let mut actual=expected;
                let (mut eg,mut ag)=(false,false);
                reference::try_move_local(IVec3::ZERO,&w,1.,crouch,&mut expected,delta,&mut eg,sneak);
                let admitted = movement_bounds(&w,1.,crouch,actual,delta,sneak,IVec3::ZERO).is_ok();
                let before = bits(actual);
                let result = try_move_ready_local(IVec3::ZERO,&w,1.,crouch,&mut actual,delta,&mut ag,sneak,&mut picker);
                if admitted {
                    assert_eq!(result,Readiness::Ready(()));
                    assert_eq!(bits(actual),bits(expected));assert_eq!(ag,eg);
                } else {
                    assert!(matches!(result,Readiness::Failed(ref e) if e.starts_with("Movement sweep:")));
                    assert_eq!(bits(actual),before);assert!(!ag);
                }
                // The preserved synchronous wrapper must also stay bit exact.
                let mut sync=Vec3::new(0.5,9.2,0.5);let mut sg=false;
                crate::world::collision::try_move_local(IVec3::ZERO,&w,1.,crouch,&mut sync,delta,&mut sg,sneak);
                assert_eq!(bits(sync),bits(expected));assert_eq!(sg,eg);
            }
        }}
    }
}

#[test]
fn required_read_order_floor_reservations_and_unused_errors_are_exact() {
    let mut w=fixture();let min=Vec3::new(0.,10.,0.);let max=Vec3::new(2.,11.,1.);
    let mut seen=Vec::new();
    let result=collides_exact(&w,min,max,IVec3::ZERO,|p| {
        seen.push(p);if p.x==0 {Ok(BlockId::Stone)}else{Err(Readiness::Failed("unused".into()))}
    });
    assert_eq!(result,Ok(true));assert_eq!(seen,vec![IVec3::new(0,10,0)]);
    w.ca_reservations.insert(IVec3::new(0,10,0));
    assert_eq!(collides_exact(&w,min,max,IVec3::ZERO,|_|panic!("reservation must precede get")),Ok(true));
    // Snapshot callers omit World metadata. Both adapters still traverse the
    // same ordered cells and must not consult the reservation in that mode.
    let mut sync_reads=Vec::new();
    let sync=super::super::collides_with(w.sample_config(),min,max,IVec3::ZERO,|p| {
        sync_reads.push(p);BlockId::Air
    },None);
    let mut fallible_reads=Vec::new();
    let fallible:Result<bool,Unavailable>=super::super::collides_try_with(
        w.sample_config(),min,max,IVec3::ZERO,|p| {
            fallible_reads.push(p);Ok(BlockId::Air)
        },None);
    assert!(!sync);assert_eq!(fallible,Ok(false));
    assert_eq!(sync_reads,vec![IVec3::new(0,10,0),IVec3::new(1,10,0)]);
    assert_eq!(fallible_reads,sync_reads);
    w.ca_reservations.clear();
    let p=IVec3::new(0,10,0);let min=p.as_vec3()+Vec3::splat(0.4);let max=p.as_vec3()+Vec3::splat(0.6);
    let mut seen=Vec::new();
    assert_eq!(collides_exact(&w,min,max,IVec3::ZERO,|q| {
        seen.push(q);if q==p {Ok(BlockId::PlankFence)}else if q==p+IVec3::NEG_Z {Err(Readiness::Pending)}else{Ok(BlockId::Air)}
    }),Err(Readiness::Pending));
    assert_eq!(seen,vec![p,p+IVec3::Z,p+IVec3::NEG_Z]);
    let floor=w.sample_config().min_world_y();let origin=IVec3::new(0,floor,0);
    let min=Vec3::splat(0.1);let max=Vec3::splat(0.9);
    assert_eq!(collides_exact(&w,min,max,origin,|_|Err(Readiness::Pending)),Err(Readiness::Pending));
    assert_eq!(collides_exact(&w,min,max,origin,|_|Ok(BlockId::Air)),Ok(true));
}

#[test]
fn malformed_and_excessive_requests_fail_without_mutation() {
    let mut w=fixture();let mut picker=ReadOnlyPicker::default();
    for (min,max,origin) in [(Vec3::NAN,Vec3::ONE,IVec3::ZERO),
        (Vec3::ZERO,Vec3::splat(30.),IVec3::ZERO),
        (Vec3::ZERO,Vec3::ONE,IVec3::splat(i32::MAX))] {
        assert!(matches!(collides_ready_local(&w,min,max,origin,&mut picker),Readiness::Failed(_)));
    }
    for delta in [Vec3::NAN,Vec3::X*200.,Vec3::splat(60.)] {
        let mut p=Vec3::new(0.5,10.,0.5);let before=bits(p);let mut ground=true;
        assert!(matches!(try_move_ready_local(IVec3::ZERO,&w,1.,0.,&mut p,delta,&mut ground,false,&mut picker),Readiness::Failed(_)));
        assert_eq!(bits(p),before);assert!(ground);
    }
    let b=bounds(Vec3::new(-1.2,0.,-1.2),Vec3::new(1.2,7.2,1.2),IVec3::ZERO).unwrap();
    let pts=points(b,IVec3::ZERO);assert_eq!(pts.len(),256);
    assert_eq!(pts.iter().copied().collect::<HashSet<_>>().len(),pts.len());
    assert!(pts.len()<=ReadOnlyPicker::MAX_GROUP_POINTS);
    w.generator_version=286;
    assert!(matches!(collides_ready_local(&w,Vec3::ZERO,Vec3::ONE,IVec3::ZERO,&mut picker),Readiness::Failed(_)));
}

#[test]
fn pending_after_prior_axis_progress_keeps_both_outputs_unchanged() {
    let mut w=fixture();
    let removed:Vec<_>=w.chunks.keys().copied().filter(|c|c.y==1).collect();
    for c in removed { w.chunks.remove(&c); }
    let mut picker=ReadOnlyPicker::default();
    let _held=picker.hold_group_for_test(&w,IVec3::new(2,32,2));
    let delta=Vec3::new(0.2,0.6,0.);
    for initially_grounded in [false,true] {
        let mut p=Vec3::new(2.5,29.8,2.5);let before=bits(p);let mut ground=initially_grounded;
        assert_eq!(try_move_ready_local(IVec3::ZERO,&w,1.,0.,&mut p,delta,&mut ground,false,&mut picker),Readiness::Pending);
        assert_eq!(bits(p),before);assert_eq!(ground,initially_grounded);
    }
    // CA reservations short-circuit before an otherwise missing exact source.
    let q=IVec3::new(2,32,2);w.ca_reservations.insert(q);
    assert_eq!(collides_ready_local(&w,q.as_vec3(),(q+IVec3::ONE).as_vec3(),IVec3::ZERO,&mut picker),Readiness::Ready(true));
    w.ca_reservations.clear();
    assert_eq!(collides_ready_local(&w,q.as_vec3(),(q+IVec3::ONE).as_vec3(),IVec3::ZERO,&mut picker),Readiness::Pending);
    // A current resident edit supersedes the in-flight source without installation.
    w.chunks.insert(IVec3::Y,Arc::new(ChunkData {
        blocks:Box::new([BlockId::Air;32*32*32]),light:Box::new([0;32*32*32]),
        torches:Default::default(),modified_bounds:None,structural_bounds:None,
    }));
    let mut p=Vec3::new(2.5,29.8,2.5);let mut expected=p;let (mut g,mut eg)=(false,false);
    reference::try_move_local(IVec3::ZERO,&w,1.,0.,&mut expected,delta,&mut eg,false);
    assert_eq!(try_move_ready_local(IVec3::ZERO,&w,1.,0.,&mut p,delta,&mut g,false,&mut picker),Readiness::Ready(()));
    assert_eq!(bits(p),bits(expected));assert_eq!(g,eg);
    w.generator_version=286;let before=bits(p);let gb=g;
    assert!(matches!(try_move_ready_local(IVec3::ZERO,&w,1.,0.,&mut p,delta,&mut g,false,&mut picker),Readiness::Failed(_)));
    assert_eq!(bits(p),before);assert_eq!(g,gb);
}

#[test]
fn motion_callback_aborts_at_first_unavailable_not_speculative_full_trajectory() {
    for unavailable in [Readiness::<()>::Pending,Readiness::Failed("required source".into())] {
        let mut p=Vec3::new(2.5,10.,2.5);let original=p;let mut ground=false;let mut calls=0;
        let result=super::super::try_move_with(VoxelScalePreset::OneMeter,1.,0.,&mut p,
            Vec3::new(0.2,0.2,0.2),&mut ground,false,|_,_| {
                calls+=1;if calls==2 {Err(unavailable.clone())}else{Ok(false)}
            });
        assert_eq!(result,Err(unavailable));assert_eq!(calls,2);
        assert_eq!(bits(p),bits(original+Vec3::new(0.2,0.,0.)));
        // This is precisely the private candidate that the public transaction discards.
        assert!(!ground);
    }
}

#[test]
fn swept_budget_rejects_long_legal_axis_move_before_source_work() {
    let mut w=fixture();w.voxel_scale=VoxelScalePreset::QuarterMeter;
    let resident:Vec<_>=w.chunks.keys().copied().collect();
    for coord in resident { w.chunks.remove(&coord); }
    let mut picker=ReadOnlyPicker::default();
    let delta=Vec3::X*100.;
    assert!((delta.x/0.45).ceil()<=256.);
    for ground in [false,true] {
        let mut p=Vec3::new(0.5,10.,0.5);let before=bits(p);let mut g=ground;
        let result=try_move_ready_local(IVec3::ZERO,&w,1.,0.,&mut p,delta,&mut g,false,&mut picker);
        assert!(matches!(result,Readiness::Failed(ref e) if e.starts_with("Movement sweep:")));
        assert_eq!(bits(p),before);assert_eq!(g,ground);
    }
    // No resident geometry can short-circuit this cold trajectory. Rejection is
    // the sweep admission error, before the source-dependent movement callback.
}

#[test]
fn swept_budget_accepts_typical_scales_and_contains_every_query() {
    let mut w=fixture();
    for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::QuarterMeter] {
        w.voxel_scale=scale;
        for delta in [Vec3::new(0.2,-0.1,0.3),Vec3::new(-0.3,0.1,-0.2),
            Vec3::new(8.0*scale.meters(),-0.1,0.3)] {
            for sneak in [false,true] {
                let p=Vec3::new(0.5,10.,0.5);
                let sweep=movement_bounds(&w,1.,0.,p,delta,sneak,IVec3::ZERO).unwrap();
                let allowed:HashSet<_>=points(sweep,IVec3::ZERO).into_iter().collect();
                assert!(allowed.len()<=ReadOnlyPicker::MAX_GROUP_POINTS);
                // Exercise unobstructed paths and contact bisection/support paths.
                for hit in [false,true] {
                    let mut actual=p;let mut g=false;
                    let result:Result<(),()>=super::super::try_move_with(scale,1.,0.,&mut actual,delta,&mut g,sneak,|min,max| {
                        let query=bounds(min,max,IVec3::ZERO).unwrap();
                        assert!(points(query,IVec3::ZERO).iter().all(|q|allowed.contains(q)));
                        Ok(hit)
                    });
                    assert_eq!(result,Ok(()));
                }
                let mut picker=ReadOnlyPicker::default();let mut actual=p;let mut g=false;
                assert_eq!(try_move_ready_local(IVec3::ZERO,&w,1.,0.,&mut actual,delta,&mut g,sneak,&mut picker),Readiness::Ready(()));
            }
        }
    }
}
