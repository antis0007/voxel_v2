//! Surveyed, physical river crossings. Historical route layouts stay unchanged.
use super::{ground, height_at, Trail, WorldSampleConfig, Vec3, B, MAX_GRADE, SHOULDER};
use crate::Profiles;

#[derive(Clone)]
pub(crate) struct Span { pub start: f32, pub end: f32 }

pub(super) fn water(cfg: &WorldSampleConfig, x: f32, z: f32, ground: f32) -> Option<f32> {
    let level = crate::CachedProfiles.river_top(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z))
        .unwrap_or(engine_voxel::WATER_LEVEL_METERS);
    (ground <= level + 0.45).then_some(level)
}

fn reserved(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    if super::super::planet_roads::active(cfg) {
        return super::anchor_blocked(cfg, x, z);
    }
    crate::valley::built_up(cfg, x, z) || crate::amberroot::reserves(cfg, x, z)
        || crate::sites::trail_obstructed(cfg, x, z)
}

/// Metres at each end of a surveyed trail whose verge may touch the reserve it serves.
const DOORSTEP_M: f32 = 4.0;

/// A surveyed trail's verge may meet the building or site at either of its ends.
fn doorstep(trail: &Trail, t: f32) -> bool {
    let along = t * trail.length();
    !trail.path.is_empty() && (along < DOORSTEP_M || trail.length() - along < DOORSTEP_M)
}

pub(super) fn fit(cfg: &WorldSampleConfig, mut trail: Trail, start: Option<f32>, end: Option<f32>) -> Option<Trail> {
    let anchor = super::super::planet_roads::active(cfg);
    let max_grade = if anchor && trail.half_width > super::FOOTPATH_HALF_WIDTH { super::ANCHOR_ROAD_GRADE } else { MAX_GRADE };
    let width = bridge_width(cfg, &trail);
    let count = trail.length().ceil().max(2.0) as usize;
    let mut raw = Vec::with_capacity(count + 1);
    let mut wet = Vec::with_capacity(count + 1);
    let mut steps = Vec::with_capacity(count);
    let mut previous = trail.at(0.0);
    for i in 0..=count {
        let t = i as f32 / count as f32;
        let (x,z) = trail.at(t);
        if reserved(cfg,x,z) { return None; }
        let h = ground(cfg,x,z);
        let mut surface = water(cfg,x,z,h);
        let (tx,tz) = trail.tangent(t);
        for side in [-width, width] {
            let (sx,sz) = (x-tz*side,z+tx*side);
            let edge = ground(cfg,sx,sz);
            if reserved(cfg,sx,sz) && !doorstep(&trail,t) { return None; }
            if let Some(w) = water(cfg,sx,sz,edge) { surface = Some(surface.map_or(w, |a| a.max(w))); }
            else if surface.is_none() && (edge-h).abs()>3.0 { return None; }
        }
        raw.push(h); wet.push(surface);
        if i>0 { steps.push((x-previous.0).hypot(z-previous.1)*max_grade); }
        previous=(x,z);
    }
    if wet[0].is_some() || wet[count].is_some() { return None; }
    let mut grade=raw.clone();
    let mut fixed=vec![false;count+1];
    let mut i=1;
    while i<count {
        if wet[i].is_none() { i+=1; continue; }
        let first=i;
        while i<count && wet[i].is_some() { i+=1; }
        let a=first.saturating_sub(2); let b=(i+1).min(count);
        let max_span = if !anchor { 64 } else if trail.half_width > super::FOOTPATH_HALF_WIDTH { 32 } else { 14 };
        if b - a > max_span { return None; }
        let deck=wet[first..i].iter().flatten().copied().reduce(f32::max)?+1.75;
        let deck=deck.max(raw[a]).max(raw[b]);
        if raw[first..i].iter().any(|h| deck-h>12.0) { return None; }
        for j in a..=b { grade[j]=deck; fixed[j]=true; }
        trail.bridges.push(Span{start:a as f32/count as f32,end:b as f32/count as f32});
    }
    grade[0]=start.unwrap_or(raw[0]); grade[count]=end.unwrap_or(raw[count]);
    for _ in 0..16 {
        for j in 1..count {
            if !fixed[j] { grade[j]=grade[j].clamp(grade[j-1]-steps[j-1],grade[j-1]+steps[j-1]).clamp(raw[j]-3.0,raw[j]+3.0); }
        }
        for j in (1..count).rev() {
            if !fixed[j] { grade[j]=grade[j].clamp(grade[j+1]-steps[j],grade[j+1]+steps[j]).clamp(raw[j]-3.0,raw[j]+3.0); }
        }
    }
    if (0..count).any(|j| (grade[j+1]-grade[j]).abs()>steps[j]+0.01) { return None; }
    trail.grade=grade;
    trail.min_x-=SHOULDER; trail.max_x+=SHOULDER;
    trail.min_z-=SHOULDER; trail.max_z+=SHOULDER;
    Some(trail)
}

pub(super) fn supplement(cfg: &WorldSampleConfig, trails: &mut Vec<Trail>) {
    if trails.iter().any(|t| !t.bridges.is_empty()) { return; }
    for ring in 1..=4 {
        for direction in 0..16 {
            let angle=direction as f32*std::f32::consts::TAU/16.0;
            let (x,z)=(angle.cos()*ring as f32*40.0,angle.sin()*ring as f32*40.0);
            if water(cfg,x,z,ground(cfg,x,z)).is_none() { continue; }
            for (dx,dz) in [(48.0,0.0),(0.0,48.0)] {
                let mut trail=Trail::new(x-dx,z-dz,x+dx,z+dz,cfg.seed.wrapping_add(62000),1.6);
                trail.cx=x; trail.cz=z;
                if let Some(t)=fit(cfg,trail,None,None) && !t.bridges.is_empty() { trails.push(t); return; }
            }
        }
    }
}

fn bridge_width(cfg: &WorldSampleConfig, trail: &Trail) -> f32 {
    if super::super::planet_roads::active(cfg) { trail.half_width + 0.6 } else { 2.2 }
}

pub(super) fn block(cfg: &WorldSampleConfig, trail: &Trail, span: &Span, m: Vec3, natural: f32, distance: f32, t: f32) -> Option<B> {
    let v=cfg.voxel_meters();
    let deck=(height_at(trail,t)/v).round()*v;
    let width=bridge_width(cfg, trail);
    if distance>width { return None; }
    let along=(t-span.start)*trail.length();
    let length=(span.end-span.start)*trail.length();
    let side=distance>width-v.max(0.45);
    if m.y>=deck-v && m.y<deck { return Some(if side { B::StoneBricks } else { B::Planks }); }
    if side && m.y>=deck && m.y<deck+1.25 {
        let post=along.rem_euclid(4.0)<v.max(0.5) || along<0.75 || along>length-0.75;
        if post { return Some(B::Log); }
        if m.y>=deck+0.75 { return Some(B::Planks); }
    }
    if m.y>=deck && m.y<deck+3.0 { return Some(B::Air); }
    if m.y<deck-v && m.y>=natural-v {
        if along<1.25 || along>length-1.25 { return Some(B::StoneBricks); }
        if side && along.rem_euclid(12.0)<v.max(0.75) { return Some(B::Cobblestone); }
        if side && m.y>=deck-1.25 { return Some(B::Log); }
    }
    None
}
