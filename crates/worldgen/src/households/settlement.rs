//! Four neighbouring household parcels share a surveyed village lane.
//! Parcel ownership and resident IDs remain stable at negative coordinates too.
use super::*;

pub(super) fn hub(x: i32, z: i32) -> (f32, f32) {
    ((x.div_euclid(2) * 2 + 1) as f32 * SPAN,
     (z.div_euclid(2) * 2 + 1) as f32 * SPAN)
}

fn ground(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let col = crate::sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    cfg.world_y(col.height)
}

// Walk from each inward-facing door to a common cross street. Survey the
// actual route and its shoulders: a flat house plot alone is not enough.
pub(super) fn connected(cfg: &WorldSampleConfig, x: i32, z: i32, wx: f32, wz: f32, floor: f32) -> bool {
    let (hx, hz) = hub(x, z);
    let start = Vec3::new(wx, 0.0, wz + (hz - wz).signum() * 6.0);
    let bend = Vec3::new(wx, 0.0, hz);
    let end = Vec3::new(hx, 0.0, hz);
    let mut previous = floor - cfg.voxel_meters();
    for (a, b) in [(start, bend), (bend, end)] {
        let length = a.distance(b);
        let steps = (length / 3.0).ceil().max(1.0) as usize;
        let stride = length / steps as f32;
        for i in 0..=steps {
            let p = a.lerp(b, i as f32 / steps as f32);
            if crate::valley::reserves(cfg,p.x,p.z) || crate::amberroot::reserves(cfg,p.x,p.z)
                || crate::sites::surface_reserved(cfg,p.x,p.z) { return false; }
            let h = ground(cfg,p.x,p.z);
            if h <= crate::WATER_LEVEL_METERS + 1.0 || (h - previous).abs() > (stride * 0.28).max(cfg.voxel_meters()) { return false; }
            for (dx,dz) in [(-1.5,0.0),(1.5,0.0),(0.0,-1.5),(0.0,1.5)] {
                if (ground(cfg,p.x+dx,p.z+dz) - h).abs() > 1.0 { return false; }
            }
            previous = h;
        }
    }
    true
}

pub(super) fn lane(cfg: &WorldSampleConfig, m: Vec3, x: i32, z: i32) -> Option<B> {
    let (hx,hz) = hub(x,z);
    if (m.x-hx).abs()>43.0 || (m.z-hz).abs()>43.0 { return None; }
    let (gx,gz) = (x.div_euclid(2)*2,z.div_euclid(2)*2);
    for dz in 0..2 { for dx in 0..2 {
        let (cx,cz) = (gx+dx,gz+dz);
        let (_,wx,wz) = horizontal(cfg,cx,cz);
        let door = wz + (hz-wz).signum()*6.0;
        let on_approach = (m.x-wx).abs()<1.3 && m.z>=door.min(hz) && m.z<=door.max(hz);
        let on_street = (m.z-hz).abs()<1.5 && m.x>=wx.min(hx) && m.x<=wx.max(hx);
        if !(on_approach || on_street) || site(cfg,cx,cz).is_none() { continue; }
        let h = ground(cfg,m.x,m.z);
        let y = m.y-h;
        if (0.0..cfg.voxel_meters()).contains(&y) { return Some(B::Dirt); }
        if (cfg.voxel_meters()..2.5).contains(&y) { return Some(B::Air); }
    }}
    None
}
