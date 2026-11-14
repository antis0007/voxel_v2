//! Concept-derived cutouts. The grip origin is shared by held meshes and hands.
use super::{Model, Vertex3};
use glam::{Mat4, Vec3};
use std::sync::LazyLock;

struct Sprite { width: usize, rgba: Vec<u8>, vertices: Vec<Vertex3>, grip: f32 }
const HEIGHT: usize = 144;
const STEP: f32 = 960.0 / HEIGHT as f32;
static SPRITES: LazyLock<Vec<Sprite>> = LazyLock::new(|| {
    // The source sheet is kept under assets/source/wand_ecosystem. Only the
    // three sampled, alpha-thresholded sprites are needed by the executable.
    let image = image::load_from_memory(include_bytes!("../../../../../assets/runtime/viewmodels/reference-cutouts.png"))
        .expect("embedded reference cutouts").into_rgba8();
    let stride = image.width() as usize * 4;
    [(0,780.0,48),(48,744.0,88),(136,710.0,72)].into_iter().enumerate().map(|(index,(offset,anchor,width))| {
        let grip=(anchor-20.0)/STEP;
        let mut rgba=Vec::with_capacity(width*HEIGHT*4);
        for y in 0..HEIGHT {
            let start = y * stride + offset * 4;
            rgba.extend_from_slice(&image.as_raw()[start..start + width * 4]);
        }
        let occupied=|x:i32,y:i32| x>=0 && y>=0 && x<width as i32 && y<HEIGHT as i32 && rgba[(y as usize*width+x as usize)*4+3]!=0;
        let point=|x:f32,y:f32,z:f32| {
            // Broader blade, while retaining the hand-sized binding at the socket.
            let blade=if index==0{((grip-y-16.0)/4.0).clamp(0.0,1.0)}else{0.0};
            Vec3::new((x-width as f32*0.5)*(0.0024+blade*0.0008),(grip-y)*0.004,z*(1.0-blade*0.65))
        };
        let mut vertices=Vec::new();
        for y in 0..HEIGHT {for x in 0..width {
            if !occupied(x as i32,y as i32){continue;}
            // The world vertex shader decodes sRGB; do not decode it twice.
            let color=std::array::from_fn(|c| rgba[(y*width+x)*4+c] as f32/255.0);
            let (l,r,t,b)=(x as f32,x as f32+1.0,y as f32,y as f32+1.0);
            let mut quad=|p:[Vec3;4],n:Vec3| {
                let order=if (p[1]-p[0]).cross(p[2]-p[0]).dot(n)>0.0{[0,1,2,0,2,3]}else{[0,2,1,0,3,2]};
                let finish = if index==0 && (y as f32)<grip-17.0 {0.4} else {0.2};
                for i in order {vertices.push(Vertex3{pos:p[i].to_array(),normal:n.to_array(),color,surface:[0.48,0.42,55.0,finish],ao_strength:1.0});}
            };
            for s in [-1.0,1.0] {quad([point(l,b,s*0.012),point(r,b,s*0.012),point(r,t,s*0.012),point(l,t,s*0.012)],Vec3::Z*s);}
            for (dx,dy,n,a,b) in [(-1,0,Vec3::NEG_X,(l,t),(l,b)),(1,0,Vec3::X,(r,t),(r,b)),(0,-1,Vec3::Y,(l,t),(r,t)),(0,1,Vec3::NEG_Y,(l,b),(r,b))] {
                if !occupied(x as i32+dx,y as i32+dy){quad([point(a.0,a.1,-0.012),point(b.0,b.1,-0.012),point(b.0,b.1,0.012),point(a.0,a.1,0.012)],n);}
            }
        }}
        Sprite{width,rgba,vertices,grip}
    }).collect()
});

pub(crate) fn icon(index:usize)->([usize;2],Vec<u8>){let s=&SPRITES[index];([s.width,HEIGHT],s.rgba.clone())}
pub(crate) fn midpoint(index:usize)->f32 {(SPRITES[index].grip-HEIGHT as f32*0.5)*0.004}
pub(crate) fn muzzle(index:usize)->Vec3 {if index==1{Vec3::new(0.085,0.266,0.0)}else{Vec3::new(0.0,0.36,0.0)}}
pub(crate) fn draw(m:&mut Model,frame:Mat4,index:usize,kind:f32){
    let transform=Mat4::from_scale(Vec3::splat(1.0/m.scale))*frame;
    let normal=frame.inverse().transpose();
    m.vertices.extend(SPRITES[index].vertices.iter().map(|v|{let mut out=*v;
        out.pos=transform.transform_point3(Vec3::from_array(v.pos)).to_array();
        out.normal=normal.transform_vector3(Vec3::from_array(v.normal)).normalize_or_zero().to_array();out.surface[2]=kind;out
    }));
}
