//! Distant ecological clusters, sampled on the existing terrain triangles.
//! Visual proxies only: collision and edits remain authoritative voxel data.
use super::{proxy_box, SIDE, HALF, LEVELS};
use crate::{mesh::{ChunkMesh, Vertex3}, world::{WorldSampleConfig, tree_visual, landscape::{visual_ecology, savanna_climate}}};
use glam::Vec3;

fn random(state: &mut u32) -> f32 {
    *state = state.wrapping_add(0x9e3779b9);
    let mut h = *state;
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
    ((h ^ (h >> 16)) >> 8) as f32 / 16777216.0
}

fn ground(mesh: &ChunkMesh, center: (i32,i32), x:f32, z:f32, scale:f32) -> (f32, f32) {
    let reach = (x-center.0 as f32).abs().max((z-center.1 as f32).abs());
    let level = (0..LEVELS).find(|l| reach < (8 << l) as f32 * HALF as f32).unwrap_or(LEVELS-1);
    let step = (8 << level) as f32;
    let gx = ((x-center.0 as f32)/step+HALF as f32).clamp(0.0,(SIDE-1) as f32-0.001);
    let gz = ((z-center.1 as f32)/step+HALF as f32).clamp(0.0,(SIDE-1) as f32-0.001);
    let a = level*SIDE*SIDE + gz.floor() as usize*SIDE + gx.floor() as usize;
    let (fx,fz) = (gx.fract(),gz.fract());
    let (ids,weights) = if fx+fz <= 1.0 { ([a,a+1,a+SIDE],[1.0-fx-fz,fx,fz]) }
        else { ([a+1,a+SIDE,a+SIDE+1],[1.0-fz,1.0-fx,fx+fz-1.0]) };
    let mut height=0.0; let mut normal=Vec3::ZERO;
    for i in 0..3 { height+=mesh.vertices[ids[i]].pos[1]*weights[i]; normal+=Vec3::from_array(mesh.vertices[ids[i]].normal)*weights[i]; }
    (height*scale,normal.normalize_or_zero().y)
}

pub fn append(mesh: &mut ChunkMesh, cfg: &WorldSampleConfig, center:(i32,i32)) {
    let scale=cfg.voxel_meters();
    // Reserve the near anchor layer first. Even a fully wooded flat world
    // stays below 90 MiB of proxy vertices; terrain/water/edit geometry has
    // a separate budget. Keep a whole tree or omit it, never cut a crown.
    let vertex_limit = mesh.vertices.len() + 2_200_000;
    // Real spawn anchors bridge the old 128--512 m empty belt at fine scales.
    // Profiles are cached by the same generator used by editable chunks; this
    // happens only on the horizon worker, never on the render thread.
    let cell_m = cfg.blocks_for_span(12.0) as f32 * scale;
    let cells = (1100.0 / cell_m).ceil() as i32 + 1;
    let cx = (center.0 as f32 / cell_m).floor() as i32;
    let cz = (center.1 as f32 / cell_m).floor() as i32;
    for iz in cz-cells..=cz+cells { for ix in cx-cells..=cx+cells {
        let dx = (ix as f32 + 0.5)*cell_m-center.0 as f32;
        let dz = (iz as f32 + 0.5)*cell_m-center.1 as f32;
        if dx*dx+dz*dz > 1100.0*1100.0 { continue; }
        let Some(tree) = tree_visual(cfg,ix,iz) else { continue; };
        let distance = (tree.base.x-center.0 as f32).hypot(tree.base.z-center.1 as f32);
        if distance >= 1100.0 { continue; }
        let (y,_) = ground(mesh,center,tree.base.x,tree.base.z,scale);
        let base = Vec3::new(tree.base.x,y-0.15,tree.base.z);
        let mut rng=tree.seed;
        let tone=random(&mut rng);
        let color=if tree.conifer {[0.24+tone*0.05,0.40+tone*0.07,0.22+tone*0.05]}
            else {[0.34+tone*0.08,0.48+tone*0.08,0.22+tone*0.05]};
        if mesh.vertices.len()+160 <= vertex_limit {
            woodland_tree(mesh,base,tree.height,tree.radius,tree.crown_height,tree.conifer,
                scale,color,&mut rng,if distance<240.0 {0}else{2});
        }
    }}
    // Independent world-aligned patch grids avoid the old grid thinning with
    // terrain tessellation. Larger far clusters preserve canopy mass cheaply.
    for (tier, spacing, inner, outer) in [(0,48,1100.0,2200.0),(1,96,2200.0,4200.0),(2,192,4200.0,8000.0)] {
        let radius=(outer/spacing as f32).ceil() as i32+1;
        let cx=center.0.div_euclid(spacing); let cz=center.1.div_euclid(spacing);
        let mut cells=Vec::with_capacity(((radius*2+1)*(radius*2+1)) as usize);
        for iz in cz-radius..=cz+radius { for ix in cx-radius..=cx+radius {
            let mut state=(ix as u32).wrapping_mul(73856093) ^ (iz as u32).wrapping_mul(19349663) ^ cfg.seed;
            cells.push(((random(&mut state)*u32::MAX as f32) as u32,ix,iz));
        }}
        // At the hard cap, keep spatially distributed groves instead of
        // filling scan rows and leaving a visibly barren half of the horizon.
        cells.sort_unstable();
        for (_,ix,iz) in cells {
            let mut rng=(ix as u32).wrapping_mul(73856093) ^ (iz as u32).wrapping_mul(19349663) ^ cfg.seed ^ (spacing as u32*3137);
            let x=(ix as f32+random(&mut rng))*spacing as f32;
            let z=(iz as f32+random(&mut rng))*spacing as f32;
            let distance=(x-center.0 as f32).hypot(z-center.1 as f32);
            if distance<inner || distance>=outer { continue; }
            let (height,slope)=ground(mesh,center,x,z,scale);
            let elev_m = height / scale.max(0.05);
            if slope<0.79 || height<(cfg.water_level() as f32+1.0)*scale+2.0 || elev_m > 122.0 { continue; }
            let timberline = ((122.0 - elev_m) / 36.0).clamp(0.0, 1.0);
            let (heat,wet,grove)=visual_ecology(cfg,x,z,height);
            let savanna = cfg.generator_version >= 10 && savanna_climate(heat, wet);
            let rainforest=cfg.generator_version>=10 && heat>0.56 && wet>0.68;
            let heath=cfg.generator_version>=10 && heat>=0.24 && heat<0.44 && wet<0.58 && elev_m>32.0;
            let density=(if savanna {0.12} else if rainforest {0.90} else if heath {0.045}
                else {((wet-0.24)*1.3+(grove-0.40)*1.7).clamp(0.03,0.88)}) * timberline;
            if random(&mut rng)>density || heat<0.08 { continue; }
            let conifer=(heat<0.38 || elev_m>68.0 || random(&mut rng)<0.18) && !savanna && !rainforest;
            let dry=wet<0.30;
            let count=if tier==2 { 6 } else { 3+(random(&mut rng)*4.0) as usize };
            for _ in 0..count {
                let px=x+(random(&mut rng)-0.5)*spacing as f32*0.80;
                let pz=z+(random(&mut rng)-0.5)*spacing as f32*0.80;
                let (y,slope)=ground(mesh,center,px,pz,scale);
                let y_m = y / scale.max(0.05);
                if slope<0.77 || y<(cfg.water_level() as f32+1.0)*scale+1.0 || y_m > 120.0 {continue;}
                let krummholz = ((124.0 - y_m) / 42.0).clamp(0.45, 1.0);
                let h=(if savanna {5.0+random(&mut rng)*3.0} else if rainforest {11.0+random(&mut rng)*7.0}
                    else if heath {2.5+random(&mut rng)*2.0} else {(6.0+random(&mut rng)*12.0)*if dry {0.48}else{1.0}}) * krummholz;
                let r=h*if savanna {0.72} else if conifer {0.26}else{0.36+random(&mut rng)*0.18};
                let p=Vec3::new(px,y-0.25,pz);
                let tone=random(&mut rng);
                let color=if conifer {[0.22+tone*0.06,0.36+tone*0.08,0.20+tone*0.06]}
                    else if dry {[0.38+tone*0.07,0.37+tone*0.05,0.18]}
                    else {[0.28+tone*0.10,0.42+tone*0.10,0.18+tone*0.06]};
                if mesh.vertices.len()+84 <= vertex_limit {
                    woodland_tree(mesh,p,h,r,if savanna {h*0.25}else{h*0.55},conifer,
                        scale,color,&mut rng,(tier+1).max(2));
                }
            }
        }
    }
}

fn woodland_tree(mesh:&mut ChunkMesh, p:Vec3, h:f32, r:f32, crown_h:f32,
    conifer:bool, scale:f32, color:[f32;3], rng:&mut u32, tier:usize) {
    if tier<3 {
        let trunk_h = if conifer { h * 0.24 } else { h * 0.56 };
        let trunk_w = (r * 0.16).clamp(0.32, 0.58);
        proxy_box(mesh,p+Vec3::Y*(trunk_h*0.5),Vec3::new(trunk_w,trunk_h,trunk_w),scale,[0.25,0.20,0.15],5.0);
    }
    if conifer {
        conifer_crown(mesh,p,h,r,scale,color,rng,tier>=2);
        return;
    }
    let turn=random(rng)*std::f32::consts::TAU;
    let lobes=if tier<2 {4}else{2};
    for i in 0..lobes {
        let variation=random(rng);
        let angle=turn+i as f32*2.399963;
        let (offset,extent)={
            let reach=r*if tier<2 {0.48}else{0.28};
            (Vec3::new(angle.cos()*reach,h+(variation-0.5)*crown_h*0.70,angle.sin()*reach),
             Vec3::new(r*(0.50+variation*0.16),crown_h*(0.37+variation*0.18),r*(0.46+variation*0.14)))
        };
        let shade=0.90+variation*0.17;
        crown(mesh,p+offset,extent,scale,color.map(|c|c*shade),rng,tier>=2);
    }
}

// A continuous, uneven branch envelope instead of three detached geometric
// cones. It also uses fewer vertices than the previous multi-lobe proxy.
fn conifer_crown(mesh:&mut ChunkMesh,p:Vec3,h:f32,r:f32,scale:f32,
    color:[f32;3],rng:&mut u32,coarse:bool) {
    let sides=if coarse {6}else{8};
    let rings=if coarse {7}else{8};
    let turn=random(rng)*std::f32::consts::TAU;
    let lean=Vec3::new(turn.cos(),0.0,turn.sin())*r*0.22;
    let base=mesh.vertices.len() as u32;
    for j in 0..rings {
        let t=j as f32/(rings-1) as f32;
        let width=(1.0-t*0.96).powf(0.78)*(0.87+random(rng)*0.16);
        for i in 0..sides {
            let a=turn+i as f32*std::f32::consts::TAU/sides as f32+j as f32*0.12;
            let fan=0.77+random(rng)*0.33;
            let radial=Vec3::new(a.cos(),0.0,a.sin());
            let q=p+Vec3::Y*(h*(0.18+t*0.81)+(random(rng)-0.5)*h*0.035)
                +radial*(r*width*fan)+lean*t*t;
            let n=(radial+Vec3::Y*0.28).normalize();
            mesh.vertices.push(Vertex3 {pos:(q/scale).to_array(),normal:n.to_array(),
                color,surface:[0.95,0.02,6.0,0.0],ao_strength:-1.0});
        }
    }
    for j in 0..rings-1 {for i in 0..sides {
        let a=base+(j*sides+i) as u32;let b=base+(j*sides+(i+1)%sides) as u32;
        let c=a+sides as u32;let d=b+sides as u32;
        mesh.indices.extend_from_slice(&[a,c,b,b,c,d]);
    }}
    for (j,y,sign) in [(0,h*0.17,-1.0),(rings-1,h*1.015,1.0)] {
        let cap=mesh.vertices.len() as u32;
        let q=p+Vec3::Y*y+lean*if sign>0.0 {1.0}else{0.0};
        mesh.vertices.push(Vertex3 {pos:(q/scale).to_array(),normal:(Vec3::Y*sign).to_array(),
            color,surface:[0.95,0.02,6.0,0.0],ao_strength:-1.0});
        for i in 0..sides {
            let a=base+(j*sides+i) as u32;let b=base+(j*sides+(i+1)%sides) as u32;
            if sign>0.0 {mesh.indices.extend_from_slice(&[cap,b,a]);}
            else {mesh.indices.extend_from_slice(&[cap,a,b]);}
        }
    }
}

// Irregular contour rings give rounded branch masses without boxes/diamonds.
// Smooth leaf normals avoid jewel-like facets. Distant rings use fewer vertices;
// no alpha testing, per-frame mesh generation, or individual tree draw calls.
fn crown(mesh:&mut ChunkMesh, center:Vec3, extent:Vec3, scale:f32,
    color:[f32;3], rng:&mut u32, coarse:bool) {
    let sides=if coarse {6}else{8};
    let rings=if coarse {3}else{4};
    let phase=random(rng)*std::f32::consts::TAU;
    let base=mesh.vertices.len() as u32;
    for j in 0..rings {
        let t=j as f32/(rings-1) as f32;
        let y=-0.8+t*1.6;
        let width=(1.0-y*y).sqrt();
        for i in 0..sides {
            let angle=phase+i as f32*std::f32::consts::TAU/sides as f32;
            let variation=0.86+random(rng)*0.24;
            let local=Vec3::new(angle.cos()*width*variation,y+(random(rng)-0.5)*0.13,angle.sin()*width*variation);
            let normal=(local/extent.max(Vec3::splat(0.1))).normalize();
            mesh.vertices.push(Vertex3 {pos:((center+local*extent)/scale).to_array(),
                normal:normal.to_array(),color,surface:[0.95,0.02,6.0,0.0],ao_strength:-1.0});
        }
    }
    for j in 0..rings-1 { for i in 0..sides {
        let a=base+(j*sides+i) as u32;
        let b=base+(j*sides+(i+1)%sides) as u32;
        let c=a+sides as u32; let d=b+sides as u32;
        mesh.indices.extend_from_slice(&[a,c,b,b,c,d]);
    }}
    for (j,sign) in [(0,-1.0),(rings-1,1.0)] {
        let cap=mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex3 {pos:((center+Vec3::Y*extent.y*sign)/scale).to_array(),
            normal:(Vec3::Y*sign).to_array(),color,surface:[0.95,0.02,6.0,0.0],ao_strength:-1.0});
        for i in 0..sides {
            let a=base+(j*sides+i) as u32;let b=base+(j*sides+(i+1)%sides) as u32;
            if sign>0.0 {mesh.indices.extend_from_slice(&[cap,b,a]);}
            else {mesh.indices.extend_from_slice(&[cap,a,b]);}
        }
    }
}
