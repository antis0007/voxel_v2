//! Authored district joined to the cultivation descent. Metres, stable identities,
//! explicit solids and voids; older generators keep their original terrain.
use crate::{BlockId as B, WorldSampleConfig, WorldType};
use glam::Vec3;
pub mod waterworks;
pub mod freight;
mod spillway;

pub const VERSION: u32 = 78;
pub const RESIDENTS: u32 = 4;

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION && matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged)
}

pub fn resident_home(cfg: &WorldSampleConfig, id: u32) -> Option<Vec3> {
    if !active(cfg) { return None; }
    if id==7 && freight::active(cfg) {return Some(freight::point(cfg,freight::CLERK));}
    let p = match id {
        3 => Vec3::new(408.5, 0.02, 40.5),
        4 => Vec3::new(429.5, 0.02, 57.5),
        5 => Vec3::new(459.5, 0.02, 56.5),
        6 => Vec3::new(443.5, 0.02, 59.5),
        _ => return None,
    };
    Some(p + Vec3::Y * crate::habitats_region::floor(cfg))
}

/// Refuge boundaries are spatial, not an immunity flag on its inhabitants.
pub fn refuge(cfg: &WorldSampleConfig, p: Vec3) -> bool {
    if freight::refuge(cfg,p) {return true;}
    if crate::households::refuge(cfg,p){return true;}
    if !active(cfg) || !(389.0..483.0).contains(&p.x) || !(29.0..68.0).contains(&p.z) { return false; }
    (p.y - crate::habitats_region::floor(cfg)).abs() < 12.0
}

/// A workshop closes when its actual water supply or work surface is lost.
pub fn workshop_cells(cfg: &WorldSampleConfig, id: u32) -> Option<(Vec3, Vec3)> {
    if !active(cfg) || !(3..6).contains(&id) { return None; }
    let (water, bench) = match id {
        3 => (Vec3::new(402.5, -0.5, 34.5), Vec3::new(405.5, 0.5, 34.5)),
        4 => (Vec3::new(428.5, -0.5, 34.5), Vec3::new(428.5, 0.5, 61.5)),
        _ => (Vec3::new(458.5, -0.5, 35.5), Vec3::new(458.5, 0.5, 61.5)),
    };
    let offset = Vec3::Y * crate::habitats_region::floor(cfg);
    Some((water + offset, bench + offset))
}

pub fn nursery(cfg: &WorldSampleConfig) -> Option<Vec3> {
    active(cfg).then(|| Vec3::new(465.5, crate::habitats_region::floor(cfg) + 0.02, 55.5))
}

fn room(p: Vec3, x: f32, z: f32, half_x: f32, half_z: f32, height: f32, wall: B) -> Option<B> {
    let dx = (p.x - x).abs();
    let dz = (p.z - z).abs();
    if dx >= half_x + 1.0 || dz >= half_z + 1.0 || !(-2.0..height + 1.0).contains(&p.y) { return None; }
    Some(if dx >= half_x || dz >= half_z || p.y < 0.0 || p.y >= height { wall } else { B::Air })
}

fn passage(p: Vec3, a: Vec3, b: Vec3, half_width: f32) -> Option<B> {
    let axis = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
    let len = axis.length_squared();
    let t = ((p - a).dot(axis) / len).clamp(0.0, 1.0);
    let center = a + (b - a) * t;
    let flat = Vec3::new(p.x - center.x, 0.0, p.z - center.z);
    if flat.length_squared() >= (half_width + 1.0).powi(2) { return None; }
    let floor = center.y.floor();
    let y = p.y - floor;
    if !(-1.0..5.0).contains(&y) { return None; }
    Some(if flat.length_squared() >= half_width.powi(2) || y < 0.0 || y >= 4.0 { B::StoneBricks } else { B::Air })
}

pub fn sample(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    if let Some(block)=freight::sample(cfg,m) {return Some(block);}
    if let Some(block) = entrance(cfg, m) { return Some(block); }
    if !active(cfg) || !(289.0..538.0).contains(&m.x) || !(28.0..88.0).contains(&m.z) { return None; }
    let p = m - Vec3::Y * crate::habitats_region::floor(cfg);
    if !(-14.0..22.0).contains(&p.y) { return None; }
    if let Some(block) = spillway::sample(cfg, p) { return Some(block); }
    if let Some(block) = waterworks::sample(cfg, p) { return Some(block); }
    if let Some(block) = cistern(cfg, p) { return Some(block); }
    if p.x < 296.0 && p.z >= 62.0 { return None; } // Existing daylight stair owns this junction.

    // The high return gallery joins the existing daylight stair at z=80.
    // Its elevation follows that stair's actual datum rather than assuming flat terrain.
    let rise = (crate::geo::site_floor(cfg, 292.5, 177.0) - crate::habitats_region::floor(cfg)) / 115.0;
    let landing = (18.0 * rise).floor();
    for (a, b) in [
        (Vec3::new(384.0,0.0,48.0), Vec3::new(491.0,0.0,48.0)),
        (Vec3::new(470.0,0.0,58.0), Vec3::new(470.0,landing,80.0)),
        (Vec3::new(470.0,landing,80.0), Vec3::new(292.5,landing,80.0)),
    ] {
        if let Some(block) = passage(p, a, b, 2.0) {
            let inside_room = (((390.0..438.0).contains(&p.x) || (440.0..482.0).contains(&p.x))
                && (29.0..67.0).contains(&p.z) && (0.0..10.0).contains(&p.y))
                || ((483.0..535.0).contains(&p.x) && (31.0..65.0).contains(&p.z) && (0.0..20.0).contains(&p.y))
                || ((382.0..386.0).contains(&p.x) && (40.0..56.0).contains(&p.z) && (0.0..7.0).contains(&p.y));
            if block == B::Air || !inside_room { return Some(block); }
        }
    }

    // Deep crossing: a continuous bridge, a visible lower river, and a monumental
    // far doorway. The far chamber has an end; the return route is always usable.
    if let Some(shell) = room(p + Vec3::Y * 12.0, 509.0, 48.0, 26.0, 17.0, 32.0, B::Basalt) {
        if p.y < -11.0 { return Some(B::Basalt); }
        if p.y < -9.0 && (p.z - 48.0).abs() < 7.0 { return Some(B::Water); }
        if (p.z - 48.0).abs() < 2.0 && (-1.0..0.0).contains(&p.y) { return Some(B::StoneBricks); }
        if (2.0..3.0).contains(&(p.z - 48.0).abs()) && (-1.0..1.0).contains(&p.y) { return Some(B::StoneBricks); }
        if p.x > 527.0 && (-1.0..0.0).contains(&p.y) { return Some(B::StoneBricks); }
        if (529.0..531.0).contains(&p.x) && ((p.z - 48.0).abs() > 3.0 || p.y > 9.0) && p.y >= 0.0 {
            return Some(B::StoneBricks);
        }
        return Some(shell);
    }

    let borough = (389.0..439.0).contains(&p.x);
    let orchard = (439.0..483.0).contains(&p.x);
    let shell = if borough { room(p,414.0,48.0,24.0,19.0,10.0,B::StoneBricks) }
        else if orchard { room(p,461.0,48.0,21.0,19.0,17.0,B::Limestone) }
        else { None }?;

    // Open channels beside dry walking routes. Removing their banks can flood
    // workspaces; repaired channels use the ordinary water simulation.
    if (-1.0..0.0).contains(&p.y) && (34.0..37.0).contains(&p.z) && p.x > 397.0 && p.x < 478.0 {
        return Some(B::Water);
    }
    if orchard && (-1.0..0.0).contains(&p.y) {
        if (p.x-467.5).abs()<0.5 && (p.z-55.5).abs()<0.5 { return Some(B::Water); }
        if (p.x-465.5).abs()<1.0 && (p.z-55.5).abs()<1.0 { return Some(B::FungalSoil); }
    }
    if shell != B::Air { return Some(shell); }

    // Small household on the orchard's west wall, with a full-height visit route
    // outside and a low doorway for its resident. Clay roofs read at coarse scale.
    if orchard && (441.0..446.0).contains(&p.x) && (60.0..65.0).contains(&p.z) && p.y < 3.0 {
        let wall = p.x < 442.0 || p.x >= 445.0 || p.z >= 64.0 || p.z < 61.0;
        let door = (442.0..444.0).contains(&p.x) && p.z < 61.0 && p.y < 1.0;
        if p.y >= 2.0 { return Some(B::Planks); }
        if wall && !door { return Some(B::Limestone); }
    }

    for (x,z) in [(405.5,34.5),(428.5,61.5),(458.5,61.5)] {
        if (p.x-x).abs() < 1.5 && (p.z-z).abs() < 0.5 && p.y < 1.0 {
            // The boatwright's broken worktop is an ordinary missing block.
            // Repairing it restores trade through the same block inspection.
            if x == 428.5 && (p.x-x).abs() < 0.5 { return Some(B::Air); }
            return Some(B::Planks);
        }
    }
    // Occupied bays: beds, shelving and supplies with broad doorways.
    if borough {
        for x in [401.0,415.0,429.0] {
            let dx = (p.x-x).abs();
            if dx < 5.0 && p.z > 54.0 {
                if ((dx >= 4.0 || p.z < 55.0) && !(dx < 1.5 && p.z < 56.0)) && p.y < 5.0 { return Some(B::Planks); }
                if dx < 4.0 && p.z > 60.0 && p.y < 1.0 { return Some(B::GlazedTiles); }
                if dx > 2.0 && p.z > 63.0 && p.y < 3.0 { return Some(B::Bookcase); }
            }
        }
    }
    // Large, grouped crowns and visible irrigation rather than scattered noise.
    if orchard {
        for (x,z,h) in [(447.5,40.5,7.0),(461.5,40.5,9.0),(475.5,40.5,6.0),(448.5,58.5,6.0),(475.5,59.5,8.0)] {
            let dx = (p.x-x).abs(); let dz = (p.z-z).abs();
            if dx < 0.5 && dz < 0.5 && p.y < h { return Some(B::Log); }
            if dx < 3.0 && dz < 3.0 && dx+dz < 4.5 && (h-1.0..h+2.0).contains(&p.y) { return Some(B::Leaves); }
            if dx < 2.0 && dz < 2.0 && p.y < 1.0 { return Some(B::FungalSoil); }
        }
    }
    for (x,z) in [(397.5,40.5),(432.5,40.5),(442.5,54.5),(479.5,54.5)] {
        if (p.x-x).abs()<0.5 && (p.z-z).abs()<0.5 {
            if p.y < 2.0 { return Some(B::StoneBricks); }
            if p.y < 3.0 { return Some(B::Torch); }
        }
    }
    Some(B::Air)
}

fn entrance(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    if !active(cfg) || !(286.0..300.0).contains(&m.x) || !(171.0..185.0).contains(&m.z) { return None; }
    let p = m - Vec3::new(292.5,crate::geo::site_floor(cfg,292.5,177.0),177.0);
    if !(-1.0..7.0).contains(&p.y) { return None; }
    // The landing remains open to the stair and the surface approach. Only its
    // outer corners carry posts; the roof makes the entrance readable from afar.
    if p.x.abs()<5.5 && (-4.0..5.0).contains(&p.z) {
        if (p.x.abs()-4.5).abs()<0.5 && ((p.z+2.5).abs()<0.5 || (p.z-3.5).abs()<0.5) && p.y < 5.0 {
            return Some(B::Log);
        }
        let roof = 6.0-(p.x.abs()*0.3).floor();
        if (roof..roof+1.0).contains(&p.y) { return Some(B::Planks); }
        if p.y >= 0.0 && p.y < roof { return Some(B::Air); }
        if p.z >= 1.0 && p.y < 0.0 { return Some(B::StoneBricks); }
    }
    None
}

/// Retains the teaching route and source coordinates while replacing the old
/// eight-metre box with a water court, a balcony and a repeated pier rhythm.
pub(crate) fn cistern(cfg: &WorldSampleConfig, p: Vec3) -> Option<B> {
    if !active(cfg) || !(310.0..333.0).contains(&p.x) || !(34.0..63.0).contains(&p.z)
        || !(-2.0..15.0).contains(&p.y) { return None; }
    if p.y < -1.0 || p.y >= 14.0 { return Some(B::Limestone); }
    if p.y < 0.0 {
        return Some(if p.z < 44.0 && !(321.0..324.0).contains(&p.x) { B::Water } else { B::Limestone });
    }
    // Central crossing and all entrances keep generous headroom.
    if (46.0..51.0).contains(&p.z) && p.y < 5.0 { return Some(B::Air); }
    for x in [312.5,329.5] {
        for z in [36.5,59.5] {
            if (p.x-x).abs()<0.75 && (p.z-z).abs()<0.75 { return Some(B::StoneBricks); }
            if (p.x-x).abs()<1.5 && (p.z-z).abs()<1.5 && (11.0..13.0).contains(&p.y) { return Some(B::StoneBricks); }
        }
    }
    if p.z > 57.0 {
        let top = ((p.x-311.0)*0.5).floor().clamp(0.0,4.0);
        if p.y < top { return Some(B::Limestone); }
        if p.z < 58.0 && p.x > 321.0 && p.y < top+1.0 { return Some(B::StoneBricks); }
    }
    for x in [316.5,326.5] {
        if (p.x-x).abs()<0.5 && (p.z-54.5).abs()<0.5 {
            if p.y < 2.0 { return Some(B::StoneBricks); }
            if p.y < 3.0 { return Some(B::Torch); }
        }
    }
    Some(B::Air)
}
