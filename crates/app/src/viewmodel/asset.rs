//! Validated Blender GLB meshes, materials, skins and authored animation.
use anyhow::{Context, Result, ensure, bail};
use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Transform { pub translation: Vec3, pub rotation: Quat, pub scale: Vec3 }
impl Transform {
    pub fn matrix(self) -> Mat4 { Mat4::from_scale_rotation_translation(self.scale,self.rotation,self.translation) }
}
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32;3], pub normal: [f32;3], pub uv: [f32;2],
    pub tangent: [f32;4], pub joints: [u32;4], pub weights: [f32;4], pub material: u32,
}
pub struct Material {
    pub name: String, pub color: [f32;4], pub roughness: f32, pub metallic: f32,
    pub emission: [f32;3], pub image: Option<usize>,
}
pub struct Joint { pub node: usize, pub inverse_bind: Mat4 }
pub enum Values { Translation(Vec<Vec3>), Rotation(Vec<Quat>), Scale(Vec<Vec3>) }
pub struct Track { pub node: usize, pub times: Vec<f32>, pub values: Values, pub step: bool }
pub struct Clip { pub name: String, pub duration: f32, pub tracks: Vec<Track> }
pub struct Asset {
    pub vertices: Vec<Vertex>, pub indices: Vec<u32>, pub materials: Vec<Material>,
    pub images: Vec<Vec<u8>>, pub names: Vec<String>, pub rest: Vec<Transform>,
    pub parents: Vec<Option<usize>>, pub joints: Vec<Joint>, pub clips: Vec<Clip>,
}

impl Asset {
    pub fn load(bytes: &[u8]) -> Result<Self> {
        let glb=gltf::Gltf::from_slice(bytes).context("validate Blender GLB")?;
        let blob=glb.blob.as_deref().context("GLB has no embedded binary buffer")?;
        ensure!(glb.buffers().count()==1,"only self-contained GLB buffers are supported");
        let get=|b:gltf::Buffer<'_>| (b.index()==0).then_some(blob);
        let mut a=Self {vertices:vec![],indices:vec![],materials:vec![],images:vec![],
            names:vec![],rest:vec![],parents:vec![None;glb.nodes().count()],joints:vec![],clips:vec![]};
        for node in glb.nodes() {
            let (t,r,s)=node.transform().decomposed();
            let t=Transform{translation:Vec3::from(t),rotation:Quat::from_array(r),scale:Vec3::from(s)};
            ensure!(t.matrix().is_finite() && t.scale.min_element()>0.,"invalid node transform");
            a.names.push(node.name().unwrap_or("unnamed").to_owned());a.rest.push(t);
            for child in node.children() {
                ensure!(a.parents[child.index()].is_none(),"node has multiple parents");
                a.parents[child.index()]=Some(node.index());
            }
        }
        for i in 0..a.parents.len() {
            let mut at=Some(i);
            for _ in 0..=a.parents.len() { at=at.and_then(|n|a.parents[n]); }
            ensure!(at.is_none(),"cyclic node hierarchy");
        }
        ensure!(glb.skins().count()==1,"viewmodel requires exactly one shared skin");
        for skin in glb.skins() {
            let matrices:Vec<_>=skin.reader(get).read_inverse_bind_matrices()
                .context("missing inverse bind matrices")?.collect();
            ensure!(matrices.len()==skin.joints().count(),"inverse bind count mismatch");
            for (node,m) in skin.joints().zip(matrices) {
                let inverse_bind=Mat4::from_cols_array_2d(&m);
                ensure!(inverse_bind.is_finite(),"nonfinite inverse bind");
                a.joints.push(Joint{node:node.index(),inverse_bind});
            }
        }
        ensure!(a.joints.len()<=64,"viewmodel exceeds 64-joint palette");
        for img in glb.images() {
            let gltf::image::Source::View{view,..}=img.source() else {bail!("external image URI")};
            a.images.push(blob.get(view.offset()..view.offset()+view.length())
                .context("image outside GLB buffer")?.to_vec());
        }
        for mat in glb.materials() {
            let p=mat.pbr_metallic_roughness();
            ensure!(mat.normal_texture().is_none(),"normal maps require a tangent-space material path");
            a.materials.push(Material{name:mat.name().unwrap_or("material").to_owned(),
                color:p.base_color_factor(),roughness:p.roughness_factor(),metallic:p.metallic_factor(),
                emission:mat.emissive_factor(),image:p.base_color_texture().map(|t|t.texture().source().index())});
        }
        for node in glb.nodes().filter(|n|n.mesh().is_some()) {
            ensure!(node.skin().is_some(),"every viewmodel mesh must use the shared rig");
            let mesh=node.mesh().context("mesh node missing mesh")?;
            for p in mesh.primitives() {
                ensure!(p.mode()==gltf::mesh::Mode::Triangles,"non-triangle primitive");
                let read=p.reader(get);
                let pos:Vec<_>=read.read_positions().context("missing positions")?.collect();
                let normals:Vec<_>=read.read_normals().context("missing normals")?.collect();
                let uvs:Vec<_>=read.read_tex_coords(0).context("missing UVs")?.into_f32().collect();
                let joints:Vec<_>=read.read_joints(0).context("missing skin joints")?.into_u16().collect();
                let weights:Vec<_>=read.read_weights(0).context("missing weights")?.into_f32().collect();
                let tangents:Vec<_>=read.read_tangents().map(|x|x.collect()).unwrap_or(vec![[1.,0.,0.,1.];pos.len()]);
                ensure!([normals.len(),uvs.len(),joints.len(),weights.len(),tangents.len()].iter().all(|n|*n==pos.len()),"attribute count mismatch");
                let material=p.material().index().context("missing material")? as u32;
                let base=a.vertices.len() as u32;
                for i in 0..pos.len() {
                    ensure!(pos[i].iter().chain(&normals[i]).chain(&uvs[i]).chain(&weights[i]).all(|x|x.is_finite()),"nonfinite vertex");
                    ensure!((Vec3::from(normals[i]).length()-1.).abs()<0.01,"invalid normal");
                    ensure!(joints[i].iter().all(|j|usize::from(*j)<a.joints.len()),"invalid joint index");
                    ensure!((weights[i].iter().sum::<f32>()-1.).abs()<0.01 && weights[i].iter().all(|w|*w>=0.),"invalid weights");
                    a.vertices.push(Vertex{position:pos[i],normal:normals[i],uv:uvs[i],tangent:tangents[i],
                        joints:joints[i].map(u32::from),weights:weights[i],material});
                }
                for index in read.read_indices().context("missing indices")?.into_u32() {
                    ensure!((index as usize)<pos.len(),"index outside primitive");a.indices.push(base+index);
                }
            }
        }
        for animation in glb.animations() {
            let mut clip=Clip{name:animation.name().unwrap_or("unnamed").to_owned(),duration:0.,tracks:vec![]};
            for channel in animation.channels() {
                let sampler=channel.sampler();
                ensure!(sampler.interpolation()!=gltf::animation::Interpolation::CubicSpline,"export linearly sampled animation");
                let read=channel.reader(get);
                let times:Vec<_>=read.read_inputs().context("missing animation times")?.collect();
                ensure!(!times.is_empty() && times.iter().all(|t|t.is_finite()) && times.windows(2).all(|t|t[1]>t[0]),"invalid animation time sequence");
                clip.duration=clip.duration.max(times[times.len()-1]);
                use gltf::animation::util::ReadOutputs;
                let values=match read.read_outputs().context("missing animation values")? {
                    ReadOutputs::Translations(v)=>Values::Translation(v.map(Vec3::from).collect()),
                    ReadOutputs::Rotations(v)=>Values::Rotation(v.into_f32().map(Quat::from_array).collect()),
                    ReadOutputs::Scales(v)=>Values::Scale(v.map(Vec3::from).collect()),
                    _=>bail!("morph animation is not a skeletal clip"),
                };
                let (count,finite)=match &values {
                    Values::Translation(v)|Values::Scale(v)=>(v.len(),v.iter().all(|x|x.is_finite())),
                    Values::Rotation(v)=>(v.len(),v.iter().all(|x|x.is_finite() && x.length_squared()>0.99)),
                };
                ensure!(count==times.len() && finite,"invalid animation values");
                clip.tracks.push(Track{node:channel.target().node().index(),times,values,
                    step:sampler.interpolation()==gltf::animation::Interpolation::Step});
            }
            a.clips.push(clip);
        }
        ensure!(!a.vertices.is_empty() && a.indices.len()%3==0,"empty or invalid mesh");
        Ok(a)
    }
}
