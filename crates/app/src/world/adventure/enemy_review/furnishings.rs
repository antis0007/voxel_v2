//! Opt-in material swatches in the existing disposable review arena.
//! Ordinary editable blocks, production meshing and lighting; no fake textures.
use super::{BlockId, IVec3, Player, Vec3, World};
use std::sync::OnceLock;

pub(super) fn install(world: &mut World, player: &mut Player) {
    let v = world.voxel_scale.meters();
    let n = v.recip().round() as i32;
    let fittings=std::env::var("VOXEL_FURNISHING_VIEW").is_ok_and(|v|v=="fittings");
    let mut edits = Vec::new();
    for x in -7..=7 { for z in -4..=10 { for y in 79..=85 {
        let block = if y == 79 { BlockId::Limestone }
            else if !fittings && (-4..=-2).contains(&x) && z == 0 && y < 83 { BlockId::Bookcase }
            else if !fittings && x == 0 && z == 0 && y == 80 { BlockId::SalvageCrate }
            else if !fittings && (3..=5).contains(&x) && (0..=2).contains(&z) && y < 82 { BlockId::Stone }
            else { BlockId::Air };
        let p = IVec3::new(x*n,y*n,z*n);
        for dx in 0..n { for dy in 0..n { for dz in 0..n {
            edits.push((p+IVec3::new(dx,dy,dz),block));
        }}}
    }}}
    world.set_many(&edits);
    world.adventure.enemies.clear();
    if fittings {
        player.pos=Vec3::new(0.0,80.0,3.0)/v;
        player.precise_pos=player.pos.as_dvec3();
        player.yaw=0.0;
        let mut blocks=std::collections::HashSet::new();
        let mut chunks=std::collections::HashSet::new();
        for (x,z,b,open) in [(-3,0,BlockId::OakDoor,false),(-1,0,BlockId::OakDoor,true),
            (2,0,BlockId::OakTrapdoor,false),(3,0,BlockId::OakTrapdoor,true),(1,2,BlockId::Lever,false)] {
            let p=IVec3::new(x*n,80*n,z*n);
            crate::world::fittings::place(world,p,b,player,None,&mut blocks,&mut chunks);
            if open { crate::world::fittings::interact(world,p,player,&mut blocks,&mut chunks); }
            println!("[fittings-review] cell={p:?} state={:?} voxel_m={v}",world.get(p));
        }
    }
    camera(player, v);
    println!("[furnishings-review] editable Bookcase/SalvageCrate/Stone swatches voxel_m={v}; camera only, not gameplay acceptance");
}

pub(super) fn camera(player: &mut Player, v: f32) -> bool {
    static VIEW: OnceLock<String> = OnceLock::new();
    let view=VIEW.get_or_init(||std::env::var("VOXEL_FURNISHING_VIEW").unwrap_or_default());
    let (eye,target)=match view.as_str() {
        "fittings"=>(Vec3::new(0.0,82.2,6.0),Vec3::new(0.0,80.85,0.5)),
        "books"=>(Vec3::new(-2.5,81.65,3.6),Vec3::new(-2.5,81.5,0.0)),
        "chest"=>(Vec3::new(2.2,81.65,2.8),Vec3::new(0.5,80.5,0.5)),
        "lichen"=>(Vec3::new(4.5,83.5,4.0),Vec3::new(4.5,81.8,0.8)),
        "far"=>(Vec3::new(0.0,83.4,18.0),Vec3::new(0.0,81.2,0.0)),
        _=>(Vec3::new(0.0,83.4,9.0),Vec3::new(0.0,81.2,0.0)),
    };
    player.pos=(eye-Vec3::Y*1.62)/v;
    player.precise_pos=player.pos.as_dvec3();
    let d=(target-eye).normalize();
    player.yaw=d.x.atan2(-d.z);player.pitch=d.y.asin();
    true
}
