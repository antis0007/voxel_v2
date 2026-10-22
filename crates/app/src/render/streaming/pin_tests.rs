use super::*;

#[test]
#[ignore = "native Vulkan buffers for atomic vertical membership; run serially"]
fn gpu_published_column_survives_pending_vertical_expansion() {
    let adapter = engine_gpu::test_adapter();
    let (_device,_)=pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mesh=||ChunkGpuMesh::test_mesh(3, 0, false, 32);
    let a=IVec3::ZERO;let b=IVec3::Y;
    let mut cache=ChunkCache::new(a);cache.visible=vec![ChunkKey {coord:a,lod:0}];cache.rebuild_visible_sets();
    assert!(!cache.resident_coverage(Vec3::ZERO,4,1.0).contains_column(0,0));
    cache.install_mesh(a,mesh());
    assert!(cache.resident_coverage(Vec3::ZERO,4,1.0).contains_column(0,0));
    cache.visible.push(ChunkKey {coord:b,lod:0});cache.rebuild_visible_sets();
    assert!(cache.published_coords.contains(&a));assert!(!cache.published_coords.contains(&b));
    let before=cache.published_coords.clone();
    let revision=cache.gpu_mesh_revision;
    cache.dirty_meshes.insert(a);
    cache.install_mesh(a,mesh());
    assert_eq!(cache.published_coords,before,"geometry-only replacement preserves the old complete bundle while another member is pending");
    assert_eq!(cache.gpu_mesh_revision,revision.wrapping_add(1),"changed geometry still invalidates draw/shadow data");
    assert!(!cache.dirty_meshes.contains(&a));
    assert!(!cache.desired_resident_coverage(Vec3::ZERO,4,1.0).contains_column(0,0));
    assert!(cache.resident_coverage(Vec3::ZERO,4,1.0).contains_column(0,0),"pending vertical expansion cannot revoke published coverage");
    assert!(!cache.desired_resident_coverage(Vec3::ZERO,4,1.0).contains_column(0,0));
    cache.set_horizon_pins(vec![(0,0)]);
    assert!(cache.horizon_pin_coords.contains(&a));assert!(!cache.horizon_pin_coords.contains(&b));
    cache.install_mesh(b,mesh());
    assert_eq!(cache.published_coords.len(),2);
    assert!(cache.desired_resident_coverage(Vec3::ZERO,4,1.0).contains_column(0,0));
    cache.reset(IVec3::new(100,0,100));
    assert!(cache.published_coords.is_empty() && cache.published_columns.is_empty());
    println!("[column-publication-contract] old membership retained through pending expansion; replacement membership admitted only with GPU buffers; world reset cleared both on {:?}",adapter.get_info());
}

#[test]
#[ignore = "native Vulkan buffers for retained-column ownership; run serially"]
fn gpu_retained_columns_keep_draw_mask_dirty_and_budget_ownership() {
    let adapter = engine_gpu::test_adapter();
    let (_device,_)=pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mesh=|| ChunkGpuMesh::test_mesh(3, 0, false, 300*1024*1024);
    // Allocation sizes above are tiny; bytes is synthetic admission metadata,
    // not a claim that the test allocated or measured 1.2 GiB of native memory.
    let a=IVec3::new(50,0,0); let b=IVec3::new(49,0,0);
    let mut cache=ChunkCache::new(a);
    cache.visible=vec![ChunkKey{coord:a,lod:0},ChunkKey{coord:a+IVec3::Y,lod:0}];
    cache.visible_coords=cache.visible.iter().map(|k|k.coord).collect();
    cache.gpu_meshes.insert(a,mesh()); cache.gpu_meshes.insert(a+IVec3::Y,mesh());
    cache.set_horizon_pins(vec![(50,0)]);
    cache.center_chunk=IVec3::ZERO;
    cache.dirty_meshes.insert(a);
    cache.retain_visible();
    assert!(cache.gpu_meshes.contains_key(&a));
    assert!(cache.dirty_meshes.contains(&a));
    let draw=cache.resident_coverage(Vec3::ZERO,4,1.0);
    let desired=cache.desired_resident_coverage(Vec3::ZERO,4,1.0);
    assert!(draw.contains_column(50,0)); assert!(!desired.contains_column(50,0));
    let vp=glam::Mat4::orthographic_rh(-5000.0,5000.0,-5000.0,5000.0,0.1,10000.0)
        *glam::Mat4::look_at_rh(Vec3::new(0.0,500.0,2000.0),Vec3::ZERO,Vec3::Y);
    assert!(build_draw_list(&cache,vp).iter().any(|k|k.coord==a));
    cache.gpu_meshes.insert(b,mesh()); cache.gpu_meshes.insert(b+IVec3::Y,mesh());
    cache.visible.push(ChunkKey{coord:b,lod:0}); cache.visible.push(ChunkKey{coord:b+IVec3::Y,lod:0});
    cache.visible_coords.extend([b,b+IVec3::Y]);
    cache.set_horizon_pins(vec![(49,0),(50,0)]);
    assert!(cache.horizon_pins.contains(&(50,0)),"new requests cannot displace the existing owner under pressure");
    assert!(!cache.horizon_pins.contains(&(49,0)));
    cache.set_horizon_pins(Vec::new());
    assert!(!cache.resident_coverage(Vec3::ZERO,4,1.0).contains_column(50,0));
    assert!(!build_draw_list(&cache,vp).iter().any(|k|k.coord==a));
    cache.reset(IVec3::ZERO);
    assert!(cache.horizon_pins.is_empty() && cache.horizon_pin_coords.is_empty() && cache.gpu_meshes.is_empty());
    println!("[horizon-pins-contract] retained mask/draw/dirty, old-first budget, release and world reset passed on {:?}",adapter.get_info());
}

#[test]
#[ignore = "native Vulkan buffers for edited-source LOD retirement; run serially"]
fn gpu_edited_source_cannot_resurrect_previous_or_inactive_lods() {
    let adapter = engine_gpu::test_adapter();
    let (device, _) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    // index_count identifies distinct fixture sources; these buffers are not drawn.
    let mesh = |lod, index_count| ChunkGpuMesh::test_mesh(index_count, lod, false, 80);
    let coord = IVec3::ZERO;
    let key = |lod| ChunkKey { coord, lod };
    let mut cache = ChunkCache::new(coord);
    cache.visible = vec![key(0)];
    cache.rebuild_visible_sets();
    cache.install_mesh(coord, mesh(2, 3));
    cache.install_mesh(coord, mesh(1, 6));
    assert_eq!(cache.lod_meshes.bytes(), 80);
    assert!(cache.activate_lod(key(2)), "unchanged source must retain normal LOD reuse");
    assert_eq!(cache.gpu_meshes[&coord].index_count, 3);
    assert_eq!(cache.lod_meshes.bytes(), 80, "LOD1 is now an inactive sibling");

    let mut world = crate::world::World::new(
        crate::world::WorldType::Superflat, crate::fluid::VoxelScalePreset::OneMeter,
    );
    let snapshot = world.snapshot();
    let mut streamer = ChunkStreamer::new(&device, 1).unwrap();
    // Exercise the actual dirty invalidation entrypoint, not a clone of its rules.
    streamer.rebuild_dirty(&snapshot, &mut cache, &HashSet::new(), &HashSet::from([coord]), 1);
    assert!(cache.dirty_meshes.contains(&coord));
    assert_eq!(cache.lod_meshes.bytes(), 0, "all inactive siblings must be invalidated");
    let revision = cache.gpu_mesh_revision;
    cache.install_mesh(coord, mesh(0, 9));
    assert!(!cache.dirty_meshes.contains(&coord));
    assert_eq!(cache.gpu_bytes, 80);
    assert_eq!(cache.gpu_mesh_revision, revision.wrapping_add(1));
    assert!(cache.published_coords.contains(&coord));
    assert!(cache.resident_coverage(Vec3::ZERO, 4, 1.0).contains_column(0, 0));
    assert_eq!(cache.lod_meshes.bytes(), 0, "replacing dirty source must not re-cache its old active LOD");
    assert!(!cache.activate_lod(key(2)), "edited old active LOD must never resurrect");
    assert!(!cache.activate_lod(key(1)), "edited inactive sibling must never resurrect");
    assert_eq!(cache.gpu_meshes[&coord].index_count, 9);
    println!("[edited-lod-contract] unchanged-source reuse preserved; dirty active/inactive LODs retired; bytes/revision/coverage preserved on {:?}", adapter.get_info());
}

#[test]
#[ignore = "native Vulkan retained pin ownership beyond dense window; run serially"]
fn gpu_sparse_pin_survives_world_origin_shift_and_matches_actual_child_draws() {
    let adapter = engine_gpu::test_adapter();
    let (_device,_)=pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let coord=IVec3::new(-600,0,-620);
    let mut cache=ChunkCache::new(coord);
    cache.visible=vec![ChunkKey {coord,lod:0}];
    cache.rebuild_visible_sets();
    cache.install_mesh(coord,ChunkGpuMesh::test_mesh(3, 0, false, 32));
    cache.set_horizon_pins(vec![(coord.x,coord.z)]);
    let vp=glam::Mat4::orthographic_rh(-100_000.0,100_000.0,-100_000.0,100_000.0,0.1,1_000_000.0)
        *glam::Mat4::look_at_rh(Vec3::new(0.0,5000.0,200_000.0),Vec3::ZERO,Vec3::Y);
    for center in [IVec3::ZERO,IVec3::new(1_000_000,0,-1_000_000),coord] {
        cache.center_chunk=center;
        cache.rebuild_visible_sets();
        for scale in [1.0,0.5,0.25] {
            let map=cache.resident_coverage(Vec3::ZERO,4,scale);
            assert!(map.contains_column(coord.x,coord.z));
            let origin=coord.as_vec3()*32.0*scale;
            assert!(map.contains_bounds(origin,origin+Vec3::splat(32.0*scale),scale));
            if center!=coord {
                assert_eq!(map.sparse_header[0],1);
                assert!(!cache.desired_resident_coverage(Vec3::ZERO,4,scale).contains_column(coord.x,coord.z));
            }
            let draw=build_draw_list(&cache,vp);
            assert!(draw.iter().any(|k|k.coord==coord),"valid sparse pin must remain drawable");
            assert!(draw.iter().all(|k|map.contains_column(k.coord.x,k.coord.z)),"no child may draw without its parent ownership mask");
            assert!(map==cache.resident_coverage(Vec3::ZERO,4,scale),"producer output must be byte-deterministic");
        }
    }
    cache.center_chunk=IVec3::ZERO;
    cache.rebuild_visible_sets();
    // Shared tiny buffers exercise membership/admission, not VRAM measurement
    // or rendered geometry. The original valid pin must win over new requests.
    let mut wanted=vec![(coord.x,coord.z)];
    for x in 100..612 {
        let q=IVec3::new(x,0,100);
        cache.gpu_meshes.insert(q,ChunkGpuMesh::test_mesh(3, 0, false, 32));
        cache.published_coords.insert(q);
        wanted.push((q.x,q.z));
    }
    cache.gpu_bytes=cache.gpu_meshes.values().map(|m|m.bytes).sum();
    cache.publication_dirty=false;
    cache.set_horizon_pins(wanted);
    assert_eq!(cache.horizon_pins.len(),512);
    assert!(cache.horizon_pins.contains(&(coord.x,coord.z)));
    for scale in [1.0,0.5,0.25] {
        let map=cache.resident_coverage(Vec3::ZERO,4,scale);
        assert_eq!(map.sparse_header[0],512);
        assert!(cache.horizon_pins.iter().all(|&(x,z)|map.contains_column(x,z)));
        assert!(map==cache.resident_coverage(Vec3::ZERO,4,scale),"collision-table bytes must be stable despite producer HashMap iteration");
        let draw=build_draw_list(&cache,vp);
        assert_eq!(draw.len(),512);
        assert!(draw.iter().all(|k|map.contains_column(k.coord.x,k.coord.z)));
    }
    println!("[sparse-pin-contract] actual cache/pin/draw membership survives3origins/3scales; all512admittedpins represented; old valid pin beats speculative513th on {:?}",adapter.get_info());
}
