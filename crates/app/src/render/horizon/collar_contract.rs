//! Geometric invariants for the real dry collar and its original-parent variant.
use super::*;

fn plane(x:[f32;2],z:[f32;2],y:f32,resident:bool)->ChunkMesh {
    let mut mesh=ChunkMesh::empty();
    for (x,z) in [(x[0],z[0]),(x[1],z[0]),(x[0],z[1]),(x[1],z[1])] {
        mesh.vertices.push(Vertex3{pos:[x,y,z],normal:[0.0,1.0,0.0],color:[0.4,0.5,0.3],
            surface:if resident {[0.9,0.04,1.0,0.0]}else{[0.9,0.04,0.0,1.0]},
            ao_strength:if resident {1.0}else{-1.0}});
    }
    mesh.indices.extend([0,2,3,0,3,1]);mesh
}

fn input(height:f32)->Arc<collar::Input> {
    let prepared=collar::Prepared::from_mesh(IVec3::ZERO,&plane([0.0,32.0],[0.0,32.0],height,true),&AtomicBool::new(false));
    let installed=collar::Installed::new(collar::Scope{coord:IVec3::ZERO,lod:0,
        world_epoch:7,cache_epoch:3,job_id:1,revision:0},prepared).expect("dry installed profile");
    collar::Input::new((0,0),1,vec![installed]).expect("complete dry +X edge")
}

fn projected_area(mesh:&ChunkMesh,triangles:impl Iterator<Item=u32>)->f64 {
    let indices:Vec<_>=triangles.collect();
    indices.chunks_exact(3).map(|tri|{
        let [a,b,c]=[tri[0],tri[1],tri[2]].map(|i|Vec3::from_array(mesh.vertices[i as usize].pos));
        ((b.x-a.x) as f64*(c.z-a.z) as f64-(b.z-a.z) as f64*(c.x-a.x) as f64).abs()*0.5
    }).sum()
}

#[test]
fn dry_join_and_fallback_each_cover_exactly_the_original_parent() {
    let original=plane([16.0,48.0],[-16.0,48.0],8.0,false);
    let original_corners:Vec<_>=original.indices.iter().map(|&i|original.vertices[i as usize]).collect();
    let mut mesh=original;
    let outcome=collar::apply(&mut mesh,&input(10.0),&AtomicBool::new(false),MAX_MESH_BYTES).unwrap();
    let collar::Outcome::Applied(ranges)=outcome else {panic!("bounded dry strip was not exercised")};
    assert_eq!(ranges.uncollared.len(),6);
    let fallback_corners:Vec<_>=mesh.indices[ranges.uncollared.start as usize..ranges.uncollared.end as usize]
        .iter().map(|&i|mesh.vertices[i as usize]).collect();
    assert_eq!(bytemuck::cast_slice::<_,u8>(&fallback_corners),bytemuck::cast_slice::<_,u8>(&original_corners),
        "fallback must preserve full original affected triangles exactly");
    let batches=batches::partition_variants(&mut mesh,1.0,(0,0),Some(&ranges));
    for collared in [false,true] {
        let selected=batches.iter().filter(|b|b.selected(collared))
            .flat_map(|b|mesh.indices[b.indices.start as usize..b.indices.end as usize].iter().copied());
        assert!((projected_area(&mesh,selected)-2048.0).abs()<0.001,
            "one selected landscape must cover original XZ area, without overlapping variants");
    }
    assert!(batches.iter().any(|b|b.variant==batches::Variant::Collared));
    assert!(batches.iter().any(|b|b.variant==batches::Variant::Uncollared));
    for tri in mesh.indices.chunks_exact(3) {
        let [a,b,c]=[tri[0],tri[1],tri[2]].map(|i|Vec3::from_array(mesh.vertices[i as usize].pos));
        assert!((b-a).cross(c-a).length_squared()>0.0,"no degenerate stored triangle");
    }
}

#[test]
fn water_parent_is_rejected_without_suppressing_any_geometry() {
    let mut mesh=plane([16.0,48.0],[-16.0,48.0],8.0,false);
    for v in &mut mesh.vertices {v.surface[2]=8.0;v.surface[3]=2.0;}
    let vertices=bytemuck::cast_slice::<_,u8>(&mesh.vertices).to_vec();let indices=mesh.indices.clone();
    assert!(matches!(collar::apply(&mut mesh,&input(10.0),&AtomicBool::new(false),MAX_MESH_BYTES),
        Some(collar::Outcome::Ineligible)));
    assert_eq!(indices,mesh.indices);
    assert_eq!(vertices,bytemuck::cast_slice::<_,u8>(&mesh.vertices));
}
