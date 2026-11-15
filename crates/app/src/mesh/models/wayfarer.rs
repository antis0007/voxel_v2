//! Immutable Blender GLBs; authored clip sampling and bounded CPU skinning.
//! Topology, UVs, skin weights, inverse binds and clips are decoded only once.
use super::{Model, wand::WandView, wand_pose::{pose, RIG}, fingers::Fingers};
use crate::mesh::Vertex3;
use glam::{Mat3, Mat4, Quat, Vec3};
use std::{sync::LazyLock, cell::RefCell};

const LIMIT: usize = 128;
struct Node { parent: Option<usize>, t: Vec3, r: Quat, s: Vec3 }
struct Vertex { v: Vertex3, joints: [usize;4], weights: [f32;4] }
enum Values { Rotation(Vec<Quat>), Translation(Vec<Vec3>), Scale(Vec<Vec3>) }
struct Channel { node: usize, times: Vec<f32>, values: Values, step: bool }
struct Clip { name: String, duration: f32, channels: Vec<Channel> }
/// One finger joint's turn from the closed bind pose to the end of the authored `open` clip.
struct OpenJoint { node: usize, finger: usize, axis: Vec3, angle: f32 }
pub(super) struct Asset { vertices: Vec<Vertex>, indices: Vec<usize>, nodes: Vec<Node>, inverse: [Mat4;LIMIT], clips: Vec<Clip>, socket: usize, wrist:usize, forearm:usize, open: Vec<OpenJoint>, tips: [usize;5] }
thread_local! { static SKINNED: RefCell<Vec<Vertex3>> = const { RefCell::new(Vec::new()) }; }

impl Asset {
    fn load(bytes: &[u8], label: &str, kind: f32) -> Self {
        let glb=gltf::Gltf::from_slice(bytes).expect("valid embedded Blender GLB");
        let data=glb.blob.as_deref().expect("embedded GLB buffer");
        let buffer=|_: gltf::Buffer<'_>| Some(data);
        assert!(glb.nodes().count()<=LIMIT);
        let mut nodes: Vec<_>=glb.nodes().map(|n| {
            let (t,r,s)=n.transform().decomposed();
            Node { parent:None,t:Vec3::from_array(t),r:Quat::from_array(r),s:Vec3::from_array(s) }
        }).collect();
        for n in glb.nodes() { for child in n.children() { nodes[child.index()].parent=Some(n.index()); } }
        let mut inverse=[Mat4::IDENTITY;LIMIT];
        let mut vertices=Vec::new();
        let mut indices=Vec::new();
        for n in glb.nodes() {
            let Some(mesh)=n.mesh() else {continue};
            let skin=n.skin().expect("viewmodel meshes must be rigged");
            let joints:Vec<_>=skin.joints().map(|j|j.index()).collect();
            let binds:Vec<_>=skin.reader(buffer).read_inverse_bind_matrices().expect("inverse binds").collect();
            assert_eq!(joints.len(),binds.len());
            for (&j,b) in joints.iter().zip(binds) { inverse[j]=Mat4::from_cols_array_2d(&b); assert!(inverse[j].is_finite()); }
            for p in mesh.primitives() {
                assert_eq!(p.mode(),gltf::mesh::Mode::Triangles);
                let reader=p.reader(buffer);
                let pos:Vec<_>=reader.read_positions().expect("positions").collect();
                let normals:Vec<_>=reader.read_normals().expect("normals").collect();
                let uv:Vec<_>=reader.read_tex_coords(0).expect("UV").into_f32().collect();
                let joint:Vec<_>=reader.read_joints(0).expect("joints").into_u16().collect();
                let weights:Vec<_>=reader.read_weights(0).expect("weights").into_f32().collect();
                let mat=p.material();
                let finish=match mat.name().unwrap_or("") { "brass"=>0.4,"crystal"=>0.6,"cloth"=>0.2,_=>0.0 };
                let base=vertices.len();
                indices.extend(reader.read_indices().expect("indices").into_u32().map(|i|base+i as usize));
                for i in 0..pos.len() {
                    let u=uv[i][0]*0.5+if kind==55.0 {0.5} else {0.0};
                    assert!(Vec3::from_array(pos[i]).is_finite() && Vec3::from_array(normals[i]).is_finite());
                    assert!((Vec3::from_array(normals[i]).length()-1.0).abs()<0.01);
                    assert!(uv[i].iter().all(|x|x.is_finite() && (0.0..=1.0).contains(x)));
                    assert!(weights[i].iter().all(|w|w.is_finite() && *w>=0.0));
                    assert!((weights[i].iter().sum::<f32>()-1.0).abs()<0.002);
                    vertices.push(Vertex { v:Vertex3 {pos:pos[i],normal:normals[i],color:[1.0;3],
                        surface:[u+2.0,uv[i][1],kind,finish],ao_strength:1.0},
                        joints:joint[i].map(|j|joints[j as usize]),weights:weights[i] });
                }
            }
        }
        let clips=glb.animations().map(|a| {
            let channels:Vec<_>=a.channels().map(|c| {
                let interpolation=c.sampler().interpolation();
                assert_ne!(interpolation,gltf::animation::Interpolation::CubicSpline,"export clips with force sampling");
                let r=c.reader(buffer);
                let times:Vec<_>=r.read_inputs().expect("clip times").collect();
                assert!(times.windows(2).all(|w|w[1]>w[0]) && times.iter().all(|t|t.is_finite()));
                use gltf::animation::util::ReadOutputs;
                let values=match r.read_outputs().expect("clip values") {
                    ReadOutputs::Rotations(v)=>Values::Rotation(v.into_f32().map(Quat::from_array).collect()),
                    ReadOutputs::Translations(v)=>Values::Translation(v.map(Vec3::from_array).collect()),
                    ReadOutputs::Scales(v)=>Values::Scale(v.map(Vec3::from_array).collect()),
                    _=>panic!("unsupported viewmodel channel"),
                };
                let count=match &values {
                    Values::Rotation(v)=>{assert!(v.iter().all(|q|q.is_finite() && (q.length()-1.0).abs()<0.01));v.len()},
                    Values::Translation(v)|Values::Scale(v)=>{assert!(v.iter().all(|q|q.is_finite()));v.len()},
                };
                assert!(!times.is_empty() && times.len()==count);
                Channel {node:c.target().node().index(),times,values,step:interpolation==gltf::animation::Interpolation::Step}
            }).collect();
            let duration=channels.iter().filter_map(|c|c.times.last()).copied().fold(0.0,f32::max);
            Clip {name:a.name().unwrap_or("unnamed").to_owned(),duration,channels}
        }).collect::<Vec<_>>();
        assert!(!vertices.is_empty() && clips.iter().any(|c|c.name=="idle"));
        assert!(indices.iter().all(|&i|i<vertices.len()));
        eprintln!("[wayfarer] {label} triangles={} vertices={} nodes={} clips={}",indices.len()/3,vertices.len(),nodes.len(),clips.len());
        let socket=glb.nodes().find(|n|n.name()==Some("GripSocket")).expect("grip socket").index();
        let wrist=glb.nodes().find(|n|n.name()==Some("Wrist")).expect("wrist joint").index();
        let forearm=glb.nodes().find(|n|n.name()==Some("Forearm")).expect("forearm joint").index();
        let open=clips.iter().find(|c|c.name=="open").map(|clip| clip.channels.iter().filter_map(|c| {
            let (Values::Rotation(q),Some(n))=(&c.values,nodes.get(c.node)) else {return None};
            let name=glb.nodes().nth(c.node)?.name()?;
            let finger=Fingers::of_joint(name)?;
            let (axis,angle)=(*q.last()?*n.r.inverse()).to_axis_angle();
            (axis.is_finite() && angle.is_finite()).then_some(OpenJoint {node:c.node,finger,axis,angle})
        }).collect()).unwrap_or_default();
        let tip=|finger:&str| glb.nodes().find(|n|n.name()==Some(finger)).expect("fingertip joint").index();
        let tips=[tip("Thumb2"),tip("Index2"),tip("Middle2"),tip("Ring2"),tip("Little2")];
        Self {vertices,indices,nodes,inverse,clips,socket,wrist,forearm,open,tips}
    }

    fn matrices(&self,v:&WandView) -> [Mat4;LIMIT] {
        self.relaxed(v,Fingers::CLOSED)
    }

    /// Clip pose with each finger uncurled by its `fingers` multiple of the authored "open" turn.
    fn relaxed(&self,v:&WandView,fingers:Fingers) -> [Mat4;LIMIT] {
        let p=pose(v);
        let (name,phase)=if v.motion.interact>0.1 || p.fingers>0.4 {("open",v.motion.interact.max(p.fingers/0.60))}
            else if v.motion.inspect>0.1 || p.fingers>0.12 {("inspect",0.5)}
            else if p.cast_left>0.76 {("windup",(1.0-p.cast_left)/0.24)}
            else if p.cast_left>0.60 {("release",(0.76-p.cast_left)/0.16)}
            else if p.cast_left>0.0 {("recovery",(0.60-p.cast_left)/0.60)}
            else if v.motion.equip<0.99 && v.motion.tier>0 {("equip",v.motion.equip)}
            else {("idle",p.time.rem_euclid(3.0)/3.0)};
        let clip=self.clips.iter().find(|c|c.name==name).unwrap_or(&self.clips[0]);
        let time=phase.clamp(0.0,1.0)*clip.duration;
        let mut t=[Vec3::ZERO;LIMIT]; let mut r=[Quat::IDENTITY;LIMIT]; let mut s=[Vec3::ONE;LIMIT];
        for (i,n) in self.nodes.iter().enumerate() {t[i]=n.t;r[i]=n.r;s[i]=n.s;}
        for c in &clip.channels {
            let b=c.times.partition_point(|x|*x<time).min(c.times.len()-1); let a=b.saturating_sub(1);
            let mix=if a==b {0.0} else if c.step {if time>=c.times[b] {1.0} else {0.0}}
                else {((time-c.times[a])/(c.times[b]-c.times[a])).clamp(0.0,1.0)};
            match &c.values {
                Values::Rotation(q)=>r[c.node]=q[a].slerp(q[b],mix),
                Values::Translation(q)=>t[c.node]=q[a].lerp(q[b],mix),
                Values::Scale(q)=>s[c.node]=q[a].lerp(q[b],mix),
            }
        }
        if name=="inspect" {
            let weight=(p.fingers/0.16).clamp(0.0,1.0);
            for (i,n) in self.nodes.iter().enumerate() {r[i]=n.r.slerp(r[i],weight);}
        }
        // The bind pose is a closed grip and "open" only eases it, so a grip scales that turn per finger.
        for j in &self.open {
            let amount=fingers.0[j.finger];
            if amount.abs()>1e-4 {r[j.node]=Quat::from_axis_angle(j.axis,j.angle*amount)*r[j.node];}
        }
        let mut world=[Mat4::IDENTITY;LIMIT];
        for i in 0..self.nodes.len() {
            let mut matrix=Mat4::from_scale_rotation_translation(s[i],r[i],t[i]);
            let mut parent=self.nodes[i].parent;
            for _ in 0..LIMIT {
                let Some(j)=parent else {break};
                matrix=Mat4::from_scale_rotation_translation(s[j],r[j],t[j])*matrix;
                parent=self.nodes[j].parent;
            }
            world[i]=matrix*self.inverse[i];
        }
        world
    }

    fn draw(&self,m:&mut Model,v:&WandView) {
        self.draw_placed(m,v,None,Fingers::CLOSED);
    }

    fn draw_placed(&self,m:&mut Model,v:&WandView,placement:Option<(Vec3,Quat)>,fingers:Fingers) {
        let mut p=pose(v);
        if let Some((grip,rotation))=placement {p.grip=grip;p.rotation=rotation;}
        if !p.grip.is_finite() || !p.rotation.is_finite() {return;}
        let world=self.relaxed(v,fingers);
        // Solve in the animated wrist frame: a fixed cuff anchor separates from
        // the hand during inspection/recoil even when the rest mesh is continuous.
        let wrist=Vec3::new(0.105,-0.070,0.0);
        let rest=Vec3::new(0.235,-0.207,-0.015)-wrist;
        let elbow=super::wand_pose::elbow(v);
        let wrist_frame=world[self.wrist];
        let wrist_inverse=wrist_frame.inverse();
        let wrist_normal=wrist_inverse.transpose();
        let target=wrist_inverse.transform_point3(p.rotation.inverse()*(elbow-p.grip)/RIG.scale)-wrist;
        let axis=rest.normalize();
        let stretch=target.length()/rest.length();
        let rotation=Quat::from_rotation_arc(axis,target.normalize());
        let (bend_axis,bend_angle)=rotation.to_axis_angle();
        let outer=Mat4::from_scale_rotation_translation(Vec3::splat(RIG.scale/m.scale),p.rotation,p.grip/m.scale);
        m.vertices.reserve(self.indices.len());
        SKINNED.with(|scratch| {
        let mut skinned=scratch.borrow_mut();
        skinned.resize(self.vertices.len(),self.vertices[0].v);
        for (vertex,out) in self.vertices.iter().zip(skinned.iter_mut()) {
            let mut position=Vec3::ZERO; let mut normal=Vec3::ZERO;
            for (&j,&weight) in vertex.joints.iter().zip(&vertex.weights) {
                if weight==0.0 {continue;}
                if j==self.forearm {
                    // Preserve the cuff tangent and distribute the elbow turn
                    // along the sleeve. The endpoint still reaches the elbow;
                    // clamping the whole bone left its cut end on screen.
                    let source=Vec3::from_array(vertex.v.pos);
                    let u=((source-wrist).dot(axis)/rest.length()).clamp(0.0,1.0);
                    let bend=u*u*(3.0-2.0*u);
                    let local_stretch=1.0+(stretch-1.0)*bend;
                    let axial=Mat3::IDENTITY+Mat3::from_cols(axis*axis.x,axis*axis.y,axis*axis.z)*(local_stretch-1.0);
                    let turn=Mat3::from_quat(Quat::IDENTITY.slerp(rotation,bend));
                    let local=turn*axial;
                    position+=wrist_frame.transform_point3(wrist+local*(source-wrist))*weight;
                    // The bend varies along the sleeve. Include that derivative
                    // in the normal transform so folds follow the curved surface.
                    let d=source-wrist;
                    let derivative=turn*(bend_axis.cross(axial*d)*bend_angle
                        + axis*d.dot(axis)*(stretch-1.0));
                    let gradient=axis*(6.0*u*(1.0-u)/rest.length());
                    let jacobian=local+Mat3::from_cols(derivative*gradient.x,
                        derivative*gradient.y,derivative*gradient.z);
                    normal+=wrist_normal.transform_vector3(jacobian.inverse().transpose()
                        *Vec3::from_array(vertex.v.normal))*weight;
                    continue;
                }
                position+=world[j].transform_point3(Vec3::from_array(vertex.v.pos))*weight;
                normal+=world[j].transform_vector3(Vec3::from_array(vertex.v.normal))*weight;
            }
            *out=vertex.v;
            out.pos=outer.transform_point3(position).to_array();
            out.normal=(p.rotation*normal).normalize_or_zero().to_array();
            if out.surface[3]>0.5 {
                out.surface[3]=0.60+p.cast*0.35;
                out.color=v.spell;
            }
        }
        m.vertices.extend(self.indices.iter().map(|&i|skinned[i]));
        });
    }
}
static WAND:LazyLock<Asset>=LazyLock::new(|| {
    let item=crate::viewmodel_review::current();
    Asset::load(item.geometry,item.name,55.0)
});
static ARM:LazyLock<Asset>=LazyLock::new(|| {
    Asset::load(crate::viewmodel_review::DEFAULT_HAND,"fitted Artificer hand",54.0)
});
pub(super) fn draw_hand(m:&mut Model,v:&WandView,placement:Option<(Vec3,Quat)>,fingers:Fingers) {
    ARM.draw_placed(m,v,placement,fingers);
}pub(super) fn draw(m:&mut Model,v:&WandView,style:crate::wand_style::WandStyle) {
    if super::wand_sprite::enabled() {super::wand_sprite::draw(m,v,style);}
    else {WAND.draw(m,v);}
    ARM.draw(m,v);
}
pub(super) fn attachment(v:&WandView) -> Mat4 {
    let p=pose(v);
    Mat4::from_scale_rotation_translation(Vec3::splat(RIG.scale),p.rotation,p.grip)
        * WAND.matrices(v)[WAND.socket]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn blender_glbs_have_registered_skin_and_authored_clips() {
        for a in [&*WAND,&*ARM] {
            assert!(a.nodes.len()>=19);
            for name in ["idle","equip","windup","release","recovery","inspect","open"] {
                assert!(a.clips.iter().any(|c|c.name==name),"missing {name}");
            }
            assert!(a.vertices.len()<30000);
            assert!(a.indices.len()%3==0);
        }
    }
    #[test] fn viewmodel_clips_keep_weapon_and_hand_on_one_socket() {
        let mut v=WandView { eye:Vec3::ZERO,forward:Vec3::NEG_Z,right:Vec3::X,up:Vec3::Y,
            time:0.7,cast:0.0,spell:[0.3,0.8,0.7],
            boost:0.0,motion:Default::default() };
        let rest=ARM.matrices(&v);
        for cast in [0.0,0.95,0.82,0.72,0.61,0.4,0.1] {
            v.cast=cast;
            let hand=ARM.matrices(&v); let wand=WAND.matrices(&v);
            for point in [Vec3::ZERO,Vec3::Y*0.3] {
                assert!(hand[ARM.socket].transform_point3(point).distance(wand[WAND.socket].transform_point3(point))<1e-5);
            }
        }
        v.cast=0.0;v.motion.interact=1.0;
        let open=ARM.matrices(&v);
        assert!(open.iter().zip(rest).any(|(a,b)|a.transform_point3(Vec3::X).distance(b.transform_point3(Vec3::X))>0.05));
    }

    fn probe_view() -> WandView {
        WandView { eye:Vec3::ZERO,forward:Vec3::NEG_Z,right:Vec3::X,up:Vec3::Y,
            time:0.7,cast:0.0,spell:[0.3,0.8,0.7],boost:0.0,motion:Default::default() }
    }

    /// Fingertip positions in hand-rig metres, relative to the grip socket.
    fn fingertips(f:Fingers) -> [Vec3;5] {
        let w=ARM.relaxed(&probe_view(),f);
        let origin=|n:usize| (w[n]*ARM.inverse[n].inverse()).transform_point3(Vec3::ZERO);
        let socket=origin(ARM.socket);
        ARM.tips.map(|n| {
            // The distal bone continues the line from its parent joint.
            let parent=ARM.nodes[n].parent.expect("finger joints have parents");
            let tip=origin(n);
            tip+(tip-origin(parent)).normalize_or_zero()*0.028-socket
        })
    }

    /// Dev table: `cargo test -p minecraft_style_voxel_prototype fingertip_table -- --ignored --nocapture`.
    #[test] #[ignore] fn fingertip_table() {
        for open in [-0.5,0.0,0.5,1.0,2.0,3.0,4.0] {
            let t=fingertips(Fingers::uniform(open)).map(|t|(t.length()*1000.0).round());
            eprintln!("open {open:>4}: tip reach from grip socket (mm) thumb/index/middle/ring/little = {t:?}");
        }
    }

    #[test] fn viewmodel_item_stays_fixed_in_the_hand() {
        // The held item sits on the placement; the hand's grip socket must not drift from it
        // while the hand's own clips run.
        let socket=|v:&WandView| { let w=ARM.relaxed(v,Fingers::CLOSED); (w[ARM.socket]*ARM.inverse[ARM.socket].inverse()).transform_point3(Vec3::ZERO) };
        let mut v=WandView { eye:Vec3::ZERO,forward:Vec3::NEG_Z,right:Vec3::X,up:Vec3::Y,
            time:0.7,cast:0.0,spell:[0.3,0.8,0.7],boost:0.0,motion:Default::default() };
        let rest=socket(&v);
        for (interact,equip,time) in [(0.0,1.0,0.1),(0.0,1.0,2.4),(0.0,0.4,0.7),(0.3,1.0,0.7),(1.0,1.0,0.7)] {
            v.motion.interact=interact; v.motion.equip=equip; v.time=time;
            let at=socket(&v);
            assert!(at.distance(rest)*RIG.scale<2e-3,"socket moved {} m at interact={interact} equip={equip} time={time}",at.distance(rest)*RIG.scale);
        }
    }
}
