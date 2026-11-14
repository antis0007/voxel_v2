#[path="../src/viewmodel/asset.rs"]
mod asset;

fn main() -> anyhow::Result<()> {
    let path=std::env::args().nth(1).ok_or_else(||anyhow::anyhow!("provide Blender GLB path"))?;
    let model=asset::Asset::load(&std::fs::read(&path)?)?;
    let mut lo=glam::Vec3::splat(f32::INFINITY);
    let mut hi=glam::Vec3::splat(f32::NEG_INFINITY);
    for v in &model.vertices {lo=lo.min(glam::Vec3::from(v.position));hi=hi.max(glam::Vec3::from(v.position));}
    println!("GLB_VALID path={path} vertices={} triangles={} joints={} materials={} images={} bounds={lo:?}..{hi:?}",
        model.vertices.len(),model.indices.len()/3,model.joints.len(),model.materials.len(),model.images.len());
    for clip in &model.clips {println!("CLIP {} duration={} channels={}",clip.name,clip.duration,clip.tracks.len());}
    Ok(())
}
