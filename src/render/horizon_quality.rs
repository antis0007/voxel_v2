//! Bounded exact-column recommendations from already evaluated canonical samples.
//! Selection is visual only; authoritative source/edits remain in the streamer.
use super::*;

const LIMIT:usize=16;
const CANDIDATE_LIMIT:usize=16384;

#[derive(Clone,Copy)]
pub(super) struct Candidate {
    column:(i32,i32),
    position:Vec3,
    feature_m:f32,
}

/// Worker-only: no additional generation, and at most three existing ring grids.
pub(super) fn summarize(cfg:WorldSampleConfig,center:(i32,i32),samples:&Samples,
    cancel:&AtomicBool)->Option<Vec<Candidate>> {
    let scale=cfg.voxel_meters();
    let sea=(cfg.water_level() as f32+1.0)*scale;
    let mut columns=HashMap::<(i32,i32),Candidate>::new();
    for level in 0..LEVELS.min(3) {
        let xs=ring_axis(center.0,level,scale)?;let zs=ring_axis(center.1,level,scale)?;
        for z in 0..SIDE-1 {
            if cancel.load(Ordering::Relaxed){return None;}
            for x in 0..SIDE-1 {
                if !cell_in_quality_domain(level,x,z){continue;}
                let keys=[(xs[x].1,zs[z].1),(xs[x+1].1,zs[z].1),
                    (xs[x].1,zs[z+1].1),(xs[x+1].1,zs[z+1].1)];
                let values=keys.map(|key|samples.get(&key));
                let [Some(a),Some(b),Some(c),Some(d)]=values else {continue;};
                if [a,b,c,d].iter().any(|s|s.2!=0 || !s.0.is_finite()){continue;}
                let low=a.0.min(b.0).min(c.0).min(d.0)*scale;
                let high=a.0.max(b.0).max(c.0).max(d.0)*scale;
                let feature=high-low;
                if low<=sea || feature<ring_step(level)*0.5 || feature<2.0*scale {continue;}
                let bx=(i64::from(keys[0].0)+i64::from(keys[3].0)).div_euclid(2);
                let bz=(i64::from(keys[0].1)+i64::from(keys[3].1)).div_euclid(2);
                let column=((bx.div_euclid(32)) as i32,(bz.div_euclid(32)) as i32);
                let item=Candidate{column,position:Vec3::new((column.0 as f32*32.0+16.0)*scale,
                    (low+high)*0.5,(column.1 as f32*32.0+16.0)*scale),feature_m:feature};
                columns.entry(column).and_modify(|c|{if feature>c.feature_m {*c=item;}}).or_insert(item);
            }
        }
    }
    let mut result:Vec<_>=columns.into_values().collect();
    // A bounded worker result, prioritizing sharp source variation near its real
    // built centre. Camera-specific ranking below never changes terrain answers.
    let origin=Vec3::new(center.0 as f32,0.0,center.1 as f32);
    result.sort_unstable_by(|a,b|{
        let score=|c:&Candidate|{let d=c.position-origin;c.feature_m/(1.0+(d.x*d.x+d.z*d.z)/147456.0)};
        score(b).total_cmp(&score(a)).then(a.column.cmp(&b.column))
    });
    result.truncate(CANDIDATE_LIMIT);Some(result)
}

fn cell_in_quality_domain(level:usize,x:usize,z:usize)->bool {
    level==0 || !(x>=HALF/2 && x<HALF+HALF/2 && z>=HALF/2 && z<HALF+HALF/2)
}

pub(super) struct View {
    pub eye:Vec3,pub forward:Vec3,pub focal:f32,pub viewport:[u32;2],pub ordinary_lod0_m:f32,
}

#[derive(Default)]
pub(super) struct Selector {
    cfg:Option<WorldSampleConfig>,last:Option<Instant>,held:Vec<Candidate>,wanted:Vec<(i32,i32)>,
}

impl Selector {
    pub(super) fn choose(&mut self,cfg:WorldSampleConfig,candidates:&[Candidate],view:View,now:Instant)->Vec<(i32,i32)> {
        if self.cfg!=Some(cfg){self.cfg=Some(cfg);self.held.clear();self.wanted.clear();self.last=None;}
        if !view.eye.is_finite() || !view.forward.is_finite() || !view.focal.is_finite() || view.focal<=0.0 {
            return self.wanted.clone();
        }
        if self.last.is_some_and(|last|now.saturating_duration_since(last)<CHECK_INTERVAL){return self.wanted.clone();}
        self.last=Some(now);
        let scale=cfg.voxel_meters();let forward=view.forward.try_normalize().unwrap_or(Vec3::NEG_Z);
        let right=forward.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);let up=right.cross(forward);
        let mut ranked=Vec::new();
        for &c in candidates {
            let delta=c.position-view.eye;let distance=delta.length().max(scale);
            if distance<view.ordinary_lod0_m || scale*view.focal/distance<1.25 {continue;}
            let radius=23.0*scale+c.feature_m*0.5;let depth=delta.dot(forward);
            // Conservative plane/sphere tests also admit bounds crossing the
            // camera plane. Requiring the whole bound in front misses cliffs.
            if depth+radius<=0.05 {continue;}
            let horizontal=view.viewport[0] as f32/(2.0*view.focal);
            let vertical=view.viewport[1] as f32/(2.0*view.focal);
            if delta.dot(right).abs()-depth*horizontal>radius*(1.0+horizontal*horizontal).sqrt()
                || delta.dot(up).abs()-depth*vertical>radius*(1.0+vertical*vertical).sqrt() {continue;}
            ranked.push((c,c.feature_m*view.focal/distance));
        }
        ranked.sort_unstable_by(|a,b|b.1.total_cmp(&a.1).then(a.0.column.cmp(&b.0.column)));
        // Retain one existing compact neighbourhood through turns and small
        // movement. Coarsen only after the unit feature falls below0.7pixel.
        self.held.retain(|c|scale*view.focal/(c.position-view.eye).length().max(scale)>=0.7);
        if self.held.is_empty(){if let Some(&(c,_))=ranked.first(){self.held.push(c);}}
        self.held.truncate(1);
        let mut wanted=Vec::new();
        for c in &self.held {
            for dz in -1..=1 {for dx in -1..=1 {
                if let (Some(x),Some(z))=(c.column.0.checked_add(dx),c.column.1.checked_add(dz)) {wanted.push((x,z));}
            }}
        }
        wanted.sort_unstable();wanted.dedup();wanted.truncate(LIMIT);
        if wanted!=self.wanted {
            println!("[quality-columns] desired={} candidates={} limit={LIMIT} unit_pixels_new=1.25 unit_pixels_retire=0.7 held={:?} columns={wanted:?}",wanted.len(),candidates.len(),self.held.iter().map(|c|c.column).collect::<Vec<_>>());
        }
        self.wanted=wanted;self.wanted.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset,world::WorldType};
    fn cfg(seed:u32)->WorldSampleConfig {WorldSampleConfig{seed,generator_version:25,world_type:WorldType::Hills,voxel_scale:VoxelScalePreset::OneMeter}}
    fn view(eye:Vec3,forward:Vec3)->View {View{eye,forward,focal:512.0,viewport:[1280,720],ordinary_lod0_m:128.0}}
    #[test]
    fn projected_quality_is_bounded_and_retained_through_turns_then_world_reset() {
        let mut s=Selector::default();let now=Instant::now();
        let c=Candidate{column:(4,-8),position:Vec3::new(144.0,10.0,-240.0),feature_m:20.0};
        assert_eq!(s.choose(cfg(1),&[c],view(Vec3::ZERO,c.position.normalize()),now).len(),9);
        assert_eq!(s.choose(cfg(1),&[c],view(Vec3::ZERO,-c.position.normalize()),now+Duration::from_secs(1)).len(),9);
        assert!(s.choose(cfg(1),&[c],view(Vec3::new(2000.0,0.0,0.0),Vec3::X),now+Duration::from_secs(2)).is_empty());
        assert!(s.choose(cfg(2),&[],view(Vec3::ZERO,Vec3::NEG_Z),now+Duration::from_secs(3)).is_empty());
    }
    #[test]
    fn unresolvable_or_behind_camera_features_do_not_request_full_voxels() {
        let mut s=Selector::default();let now=Instant::now();
        let far=Candidate{column:(0,-100),position:Vec3::new(0.0,0.0,-3200.0),feature_m:100.0};
        let back=Candidate{column:(0,8),position:Vec3::new(0.0,0.0,256.0),feature_m:30.0};
        assert!(s.choose(cfg(1),&[far,back],view(Vec3::ZERO,Vec3::NEG_Z),now).is_empty());
    }
    #[test]
    fn visible_cliff_bounds_crossing_camera_plane_remain_eligible() {
        let mut s=Selector::default();
        let c=Candidate{column:(2,-2),position:Vec3::new(80.0,100.0,-48.0),feature_m:120.0};
        assert_eq!(s.choose(cfg(1),&[c],view(Vec3::ZERO,Vec3::NEG_Z),Instant::now()).len(),9);
    }
}
